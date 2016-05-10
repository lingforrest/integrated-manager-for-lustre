#
# INTEL CONFIDENTIAL
#
# Copyright 2013-2016 Intel Corporation All Rights Reserved.
#
# The source code contained or described herein and all documents related
# to the source code ("Material") are owned by Intel Corporation or its
# suppliers or licensors. Title to the Material remains with Intel Corporation
# or its suppliers and licensors. The Material contains trade secrets and
# proprietary and confidential information of Intel or its suppliers and
# licensors. The Material is protected by worldwide copyright and trade secret
# laws and treaty provisions. No part of the Material may be used, copied,
# reproduced, modified, published, uploaded, posted, transmitted, distributed,
# or disclosed in any way without Intel's prior express written permission.
#
# No license under any patent, copyright, trade secret or other intellectual
# property right is granted to or conferred upon you by disclosure or delivery
# of the Materials, either expressly, by implication, inducement, estoppel or
# otherwise. Any license under such intellectual property rights must be
# express and approved by Intel in writing.


from chroma_core.models.alert import AlertStateBase, AlertState
from chroma_core.models.sparse_model import VariantGenericForeignKey, VariantDescriptor


class LearnEvent(AlertStateBase):
    variant_fields = [VariantDescriptor('learned_item_id', int, None, None, 0),
                      VariantDescriptor('learned_item_type',
                                        int,
                                        None,
                                        lambda self_, value: self_.set_variant('learned_item_type', int, value.id),
                                        None)]

    learned_item = VariantGenericForeignKey('learned_item_type', 'learned_item_id')

    class Meta:
        app_label = 'chroma_core'
        db_table = AlertStateBase.table_name

    @staticmethod
    def type_name():
        return "Autodetection"

    def alert_message(self):
        from chroma_core.models import ManagedTarget, ManagedFilesystem, ManagedTargetMount
        if isinstance(self.learned_item, ManagedTargetMount):
            return "Discovered mount point of %s on %s" % (self.learned_item, self.learned_item.host)
        elif isinstance(self.learned_item, ManagedTarget):
            return "Discovered formatted target %s" % self.learned_item
        elif isinstance(self.learned_item, ManagedFilesystem):
            return "Discovered filesystem %s on MGS %s" % (self.learned_item, self.learned_item.mgs.primary_host)
        else:
            return "Discovered %s" % self.learned_item


class AlertEvent(AlertStateBase):
    class Meta:
        app_label = 'chroma_core'
        db_table = AlertStateBase.table_name

    variant_fields = [VariantDescriptor('message_str', str, None, None, ''),
                      VariantDescriptor('alert',
                                        AlertState,
                                        lambda self_: AlertState.objects.get(id=self_.get_variant('alert_id', None, int)),
                                        lambda self_, value: self_.set_variant('alert_id', int, value.id),
                                        None)]

    @staticmethod
    def type_name():
        return "Alert"

    def alert_message(self):
        return self.message_str


class SyslogEvent(AlertStateBase):
    variant_fields = [VariantDescriptor('message_str', str, None, None, '')]

    class Meta:
        app_label = 'chroma_core'
        db_table = AlertStateBase.table_name

    @staticmethod
    def type_name():
        return "Syslog"

    def alert_message(self):
        return self.message_str


class ClientConnectEvent(AlertStateBase):
    class Meta:
        app_label = 'chroma_core'
        db_table = AlertStateBase.table_name

    variant_fields = [VariantDescriptor('message_str', str, None, None, '')]

    def alert_message(self):
        return self.message_str

    @staticmethod
    def type_name():
        return "ClientConnect"
