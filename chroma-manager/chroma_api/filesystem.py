#
# ========================================================
# Copyright (c) 2012 Whamcloud, Inc.  All rights reserved.
# ========================================================


from collections import defaultdict

from django.db.models import Q
from chroma_core.models import ManagedFilesystem, ManagedTarget
from chroma_core.models import ManagedOst, ManagedMdt, ManagedMgs
from chroma_core.models import Volume, VolumeNode
from chroma_core.models import Command
import chroma_core.lib.conf_param
import chroma_core.lib.util

import tastypie.http as http
from tastypie import fields
from tastypie.validation import Validation
from tastypie.authorization import DjangoAuthorization
from chroma_api.authentication import AnonymousAuthentication
from chroma_api.utils import custom_response, ConfParamResource, MetricResource, dehydrate_command
from chroma_api import api_log
from chroma_core.lib import conf_param


class FilesystemValidation(Validation):

    def _validate_put(self, bundle, request):
        errors = defaultdict(list)

        if 'conf_params' in bundle.data and bundle.data['conf_params'] != None:
            try:
                fs = ManagedFilesystem.objects.get(pk = bundle.data['id'])
            except ManagedFilesystem.DoesNotExist:
                errors['id'] = "Filesystem with id %s not found" % bundle.data['id']
            except KeyError:
                errors['id'] = "Field is mandatory"
            else:
                if fs.immutable_state:
                    if not conf_param.compare(bundle.data['conf_params'], conf_param.get_conf_params(fs)):
                        errors['conf_params'].append("Cannot modify conf_params on immutable_state objects")
                else:
                    conf_param_errors = conf_param.validate_conf_params(ManagedFilesystem, bundle.data['conf_params'])
                    if conf_param_errors:
                        errors['conf_params'] = conf_param_errors

        return errors

    def _validate_post(self, bundle, request):
        errors = defaultdict(list)

        targets = defaultdict(list)
        # Check 'mgt', 'mdt', 'osts' are present and compose
        # a record of targets which will be formatted
        try:
            # Check that client hasn't specified an existing MGT
            # *and* a volume to format.
            if 'id' in bundle.data['mgt'] and 'volume_id' in bundle.data['mgt']:
                errors['mgt'].append("id and volume_id are mutually exclusive")

            mgt = bundle.data['mgt']
            if 'volume_id' in mgt:
                targets['mgt'].append(mgt)
        except KeyError:
            errors['mgt'].append("This field is mandatory")
        try:
            targets['mdt'].append(bundle.data['mdt'])
        except KeyError:
            errors['mdt'].append("This field is mandatory")
        try:
            targets['osts'].extend(bundle.data['osts'])
        except KeyError:
            errors['osts'].append("This field is mandatory")

        if 'conf_params' not in bundle.data:
            errors['conf_params'].append("This field is mandatory")

        if 'name' not in bundle.data:
            errors['name'].append("This field is mandatory")

        # Return if some of the things we're going to validate in detail are absent
        if len(errors):
            return errors

        errors['mgt'] = defaultdict(list)
        errors['mdt'] = defaultdict(list)
        errors['osts'] = []

        # Validate filesystem name
        if len(bundle.data['name']) > 8:
            errors['name'].append("Name '%s' too long (max 8 characters)" % bundle.data['name'])
        if len(bundle.data['name']) < 1:
            errors['name'].append("Name '%s' too short (min 1 character)" % bundle.data['name'])
        if bundle.data['name'].find(" ") != -1:
            errors['name'].append("Name may not contain spaces")

        # Check volume IDs are present and correct
        used_volume_ids = set()

        def check_volume(field, volume_id):
            # Check we haven't tried to use the same volume twice
            if volume_id in used_volume_ids:
                return "Volume ID %s specified for multiple targets!" % volume_id

            try:
                # Check the volume exists
                volume = Volume.objects.get(id = volume_id)
                try:
                    # Check the volume isn't in use
                    target = ManagedTarget.objects.get(volume = volume)
                    return "Volume with ID %s is already in use by target %s" % (volume_id, target)
                except ManagedTarget.DoesNotExist:
                    pass
            except Volume.DoesNotExist:
                return "Volume with ID %s not found" % volume_id

            used_volume_ids.add(volume_id)

        try:
            mgt_volume_id = bundle.data['mgt']['volume_id']
            error = check_volume('mgt', mgt_volume_id)
            if error:
                errors['mgt']['volume_id'].append(error)
        except KeyError:
            mgt_volume_id = None

            try:
                mgt = ManagedMgs.objects.get(id = bundle.data['mgt']['id'])
                if mgt.immutable_state:
                    errors['mgt']['id'].append("MGT is unmanaged")

                try:
                    ManagedFilesystem.objects.get(name = bundle.data['name'], mgs = mgt)
                    errors['mgt']['name'].append("A file system with name '%s' already exists for this MGT" % bundle.data['name'])
                except ManagedFilesystem.DoesNotExist:
                    pass
            except KeyError:
                errors['mgt']['id'].append("One of id or volume_id must be set")
        except ManagedMgs.DoesNotExist:
            errors['mgt']['id'].append("MGT with ID %s not found" % (bundle.data['mgt']['id']))

        try:
            mdt_volume_id = bundle.data['mdt']['volume_id']
            check_volume('mdt', mdt_volume_id)
        except KeyError:
            errors['mdt']['volume_id'].append("volume_id attribute is mandatory")

        for ost in bundle.data['osts']:
            try:
                volume_id = ost['volume_id']
                check_volume('osts', volume_id)
            except KeyError:
                errors['osts'].append("volume_id attribute is mandatory for all osts")

        # If formatting an MGS, check its not on a host already used as an MGS
        # If this is an MGS, there may not be another MGS on
        # this host
        if mgt_volume_id:
            mgt_volume = Volume.objects.get(id = mgt_volume_id)
            hosts = [vn.host for vn in VolumeNode.objects.filter(volume = mgt_volume, use = True)]
            conflicting_mgs_count = ManagedTarget.objects.filter(~Q(managedmgs = None), managedtargetmount__host__in = hosts).count()
            if conflicting_mgs_count > 0:
                errors['mgt']['volume_id'].append("Volume %s cannot be used for MGS (only one MGS is allowed per server)" % mgt_volume.label)

        def validate_target(klass, target):
            target_errors = defaultdict(list)

            volume = Volume.objects.get(id = target['volume_id'])
            if 'inode_count' in target and 'bytes_per_inode' in target:
                target_errors['inode_count'].append("inode_count and bytes_per_inode are mutually exclusive")

            if 'conf_params' in target:
                conf_param_errors = conf_param.validate_conf_params(klass, target['conf_params'])
                if conf_param_errors:
                    # FIXME: not really representing target-specific validations cleanly,
                    # will sort out while fixing HYD-1077.
                    target_errors['conf_params'] = conf_param_errors

            for setting in ['inode_count', 'inode_size', 'bytes_per_inode']:
                if setting in target:
                    if target[setting] is not None and not isinstance(target[setting], int):
                        target_errors[setting].append("Must be an integer")

            # If they specify and inode size and a bytes_per_inode, check the inode fits
            # within the ratio
            try:
                inode_size = target['inode_size']
                bytes_per_inode = target['bytes_per_inode']
                if inode_size >= bytes_per_inode:
                    target_errors['inode_size'].append("inode_size must be less than bytes_per_inode")
            except KeyError:
                pass

            # If they specify an inode count, check it will fit on the device
            try:
                inode_count = target['inode_count']
            except KeyError:
                # If no inode_count is specified, no need to check it against inode_size
                pass
            else:
                try:
                    inode_size = target['inode_size']
                except KeyError:
                    inode_size = {ManagedMgs: 128, ManagedMdt: 512, ManagedOst: 256}[klass]

                if inode_size is not None and inode_count is not None:
                    if inode_count * inode_size > volume.size:
                        target_errors['inode_count'].append("%d %d-byte inodes too large for %s-byte device" % (
                            inode_count, inode_size, volume.size))

            return target_errors

        # Validate generic target settings
        for attr, targets in targets.items():
            for target in targets:
                if attr == 'mdt':
                    klass = ManagedMdt
                elif attr == 'osts':
                    klass = ManagedOst
                elif attr == 'mgt':
                    klass = ManagedMgs
                else:
                    raise NotImplementedError(attr)

                target_errors = validate_target(klass, target)

                if attr == 'osts':
                    errors[attr].append(target_errors)
                else:
                    errors[attr].update(target_errors)

        conf_param_errors = conf_param.validate_conf_params(ManagedFilesystem, bundle.data['conf_params'])
        if conf_param_errors:
            errors['conf_params'] = conf_param_errors

        def recursive_count(o):
            """Count the number of non-empty dicts/lists or other objects"""
            if isinstance(o, dict):
                c = 0
                for v in o.values():
                    c += recursive_count(v)
                return c
            elif isinstance(o, list):
                c = 0
                for v in o:
                    c += recursive_count(v)
                return c
            else:
                return 1

        if not recursive_count(errors):
            errors = {}

        return errors

    def is_valid(self, bundle, request=None):
        if request.method == "POST":
            return self._validate_post(bundle, request)
        elif request.method == "PUT":
            return self._validate_put(bundle, request)
        else:
            return {}


class FilesystemResource(MetricResource, ConfParamResource):
    """
    A Lustre filesystem, consisting of one or mode MDTs, and one or more OSTs.

    When POSTing to create a filesystem, specify volumes to use like this:
    ::

        {osts: [{volume_id: 22}],
        mdt: {volume_id: 23},
        mgt: {volume_id: 24}}

    To create a filesystem using an existing MGT instead of creating a new
    MGT, set the `id` attribute instead of the `volume_id` attribute for
    that target (i.e. `mgt: {id: 123}`).

    Note: Lustre filesystems are owned by an MGT, and the ``name`` of a filesystem
    is unique within that MGT.  Do not use ``name`` as a globally unique identifier
    for filesystems in your application.
    """
    bytes_free = fields.IntegerField()
    bytes_total = fields.IntegerField()
    files_free = fields.IntegerField()
    files_total = fields.IntegerField()
    client_count = fields.IntegerField()

    mount_command = fields.CharField(null = True, help_text = "Example command for\
            mounting this file system on a Lustre client, e.g. \"mount -t lustre 192.168.0.1:/testfs /mnt/testfs\"")

    mount_path = fields.CharField(null = True, help_text = "Path for mounting the file system\
            on a Lustre client, e.g. \"192.168.0.1:/testfs\"")

    osts = fields.ToManyField('chroma_api.target.TargetResource', null = True,
            attribute = lambda bundle: ManagedOst.objects.filter(filesystem = bundle.obj),
            help_text = "List of OSTs which belong to this file system")
    # NB a filesystem must always report an MDT, although it may be deleted just before
    # the filesystem is deleted, so use _base_manager
    mdts = fields.ToManyField('chroma_api.target.TargetResource',
            attribute = lambda bundle: ManagedMdt._base_manager.filter(filesystem = bundle.obj), full = True,
            help_text = "List of MDTs in this file system, should be at least 1 unless the file system\
            is in the process of being deleted")
    mgt = fields.ToOneField('chroma_api.target.TargetResource', attribute = 'mgs', full = True,
            help_text = "The MGT on which this file system is registered")

    def _get_stat_simple(self, bundle, klass, stat_name, factor = 1):
        try:
            return bundle.obj.metrics.fetch_last(klass, fetch_metrics=[stat_name])[1][stat_name] * factor
        except (KeyError, IndexError, TypeError):
            return None

    def dehydrate_mount_path(self, bundle):
        return bundle.obj.mount_path()

    def dehydrate_mount_command(self, bundle):
        path = self.dehydrate_mount_path(bundle)
        if path:
            return "mount -t lustre %s /mnt/%s" % (path, bundle.obj.name)
        else:
            return None

    def dehydrate_bytes_free(self, bundle):
        return self._get_stat_simple(bundle, ManagedOst, 'kbytesfree', 1024)

    def dehydrate_bytes_total(self, bundle):
        return self._get_stat_simple(bundle, ManagedOst, 'kbytestotal', 1024)

    def dehydrate_files_free(self, bundle):
        return self._get_stat_simple(bundle, ManagedMdt, 'filesfree')

    def dehydrate_files_total(self, bundle):
        return self._get_stat_simple(bundle, ManagedMdt, 'filestotal')

    def dehydrate_client_count(self, bundle):
        return self._get_stat_simple(bundle, ManagedMdt, 'client_count')

    class Meta:
        queryset = ManagedFilesystem.objects.all()
        resource_name = 'filesystem'
        authorization = DjangoAuthorization()
        authentication = AnonymousAuthentication()
        excludes = ['not_deleted', 'ost_next_index', 'mdt_next_index']
        ordering = ['name']
        filtering = {'id': ['exact', 'in'], 'name': ['exact']}
        list_allowed_methods = ['get', 'post']
        detail_allowed_methods = ['get', 'delete', 'put']
        readonly = ['bytes_free', 'bytes_total', 'files_free', 'files_total', 'client_count', 'mount_command', 'mount_path']
        validation = FilesystemValidation()
        always_return_data = True

    def _format_attrs(self, target_data):
        """Map API target attributes to kwargs suitable for model construction"""
        # Exploit that these attributes happen to have the same name in
        # the API and in the ManagedTarget model
        result = {}
        for attr in ['inode_count', 'inode_size', 'bytes_per_inode']:
            try:
                result[attr] = target_data[attr]
            except KeyError:
                pass
        return result

    def obj_create(self, bundle, request = None, **kwargs):
        # Set up an errors dict in the bundle to allow us to carry
        # hydration errors through to validation.
        mgt_data = bundle.data['mgt']
        if 'volume_id' in mgt_data:
            mgt = ManagedMgs.create_for_volume(mgt_data['volume_id'], **self._format_attrs(mgt_data))
            mgt_id = mgt.pk
        else:
            mgt_id = mgt_data['id']

        api_log.debug("fsname: %s" % bundle.data['name'])
        api_log.debug("MGS: %s" % mgt_id)
        api_log.debug("MDT: %s" % bundle.data['mdt'])
        api_log.debug("OSTs: %s" % bundle.data['osts'])
        api_log.debug("conf_params: %s" % bundle.data['conf_params'])

        from django.db import transaction
        with transaction.commit_on_success():
            mgs = ManagedMgs.objects.get(id = mgt_id)
            fs = ManagedFilesystem(mgs=mgs, name = bundle.data['name'])
            fs.save()

            chroma_core.lib.conf_param.set_conf_params(fs, bundle.data['conf_params'])

            mdt_data = bundle.data['mdt']
            mdt = ManagedMdt.create_for_volume(mdt_data['volume_id'], filesystem = fs, **self._format_attrs(mdt_data))
            chroma_core.lib.conf_param.set_conf_params(mdt, mdt_data['conf_params'])
            osts = []
            for ost_data in bundle.data['osts']:
                ost = ManagedOst.create_for_volume(ost_data['volume_id'], filesystem = fs, **self._format_attrs(ost_data))
                osts.append(ost)
                chroma_core.lib.conf_param.set_conf_params(ost, ost_data['conf_params'])
        # Important that a commit happens here so that the targets
        # land in DB before the set_state jobs act upon them.

        command = Command.set_state([(fs, 'available')], "Creating filesystem %s" % bundle.data['name'])
        filesystem_data = self.full_dehydrate(self.build_bundle(obj = fs)).data

        raise custom_response(self, request, http.HttpAccepted,
                {'command': dehydrate_command(command),
                 'filesystem': filesystem_data})
