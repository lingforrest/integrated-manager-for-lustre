use crate::{
    breadth_first_parent_iterator::BreadthFirstParentIterator,
    change::Change,
    db::{DeviceHosts, Devices},
    error::ImlDevicesError,
};
use iml_wire_types::db::{Device, DeviceHost, DeviceId, DeviceType, MountPath, Paths};
use iml_wire_types::Fqdn;
use std::collections::{BTreeMap, BTreeSet};

fn is_virtual_device(device: &Device) -> bool {
    device.device_type == DeviceType::MdRaid
        || device.device_type == DeviceType::VolumeGroup
        || device.device_type == DeviceType::Dataset
        || device.device_type == DeviceType::Zpool
}

pub fn compute_virtual_device_changes<'a>(
    fqdn: &Fqdn,
    incoming_devices: &Devices,
    incoming_device_hosts: &DeviceHosts,
    db_devices: &Devices,
    db_device_hosts: &DeviceHosts,
) -> Result<BTreeMap<(DeviceId, Fqdn), Change<DeviceHost>>, ImlDevicesError> {
    tracing::info!(
        "Incoming: devices: {}, device hosts: {}, Database: devices: {}, device hosts: {}",
        incoming_devices.len(),
        incoming_device_hosts.len(),
        db_devices.len(),
        db_device_hosts.len()
    );
    let mut results = BTreeMap::new();

    let virtual_devices = incoming_devices
        .iter()
        .filter(|(_, d)| is_virtual_device(d))
        .map(|(_, d)| d);

    for virtual_device in virtual_devices {
        tracing::info!("virtual_device: {:#?}", virtual_device);
        let virtual_device_host =
            incoming_device_hosts.get(&(virtual_device.id.clone(), fqdn.clone()));
        tracing::info!("virtual_device_host: {:#?}", virtual_device_host);

        // For this host itself, run parents check for this virtual device ON THE INCOMING DEVICE HOSTS
        // If it fails, remove the device host of this virtual device FROM THE DB
        // We don't add virtual devices here, because either: - TURNS OUT THIS IS INCORRECT
        // We can have incoming devices where there are two levels of parents of virtual devices
        // 1. Device scanner sends us the virtual device, if it's physically present on the host
        // 2. We add it as part of processing of other hosts in the loop below
        {
            let mut i = BreadthFirstParentIterator::new(incoming_devices, &virtual_device.id);
            let all_available = i.all(|p| {
                let result = incoming_device_hosts
                    .get(&(p.clone(), fqdn.clone()))
                    .is_some();
                tracing::info!(
                    "Checking device {:?} on host {:?} in incoming: {:?}",
                    p,
                    fqdn,
                    result
                );
                result
            });
            tracing::info!(
                "Host: {:?}, device: {:?}, all_available: {:?}",
                fqdn,
                virtual_device.id,
                all_available
            );
            if all_available {
                let other_device_host = DeviceHost {
                    device_id: virtual_device.id.clone(),
                    fqdn: fqdn.clone(),
                    local: true,
                    // Does it make sense to use paths from other hosts?
                    paths: Paths(
                        virtual_device_host
                            .map(|x| x.paths.clone())
                            .unwrap_or(BTreeSet::new()),
                    ),
                    // It can't be mounted on other hosts at the time this is processed?
                    mount_path: MountPath(None),
                    fs_type: virtual_device_host
                        .map(|x| x.fs_type.clone())
                        .unwrap_or(None),
                    fs_label: virtual_device_host
                        .map(|x| x.fs_label.clone())
                        .unwrap_or(None),
                    fs_uuid: virtual_device_host
                        .map(|x| x.fs_uuid.clone())
                        .unwrap_or(None),
                };

                // add to database if missing and not in flight
                // update in database if present and not in flight
                // update in flight if in flight - is it necessary though?

                if db_device_hosts
                    .get(&(virtual_device.id.clone(), fqdn.clone()))
                    .is_none()
                    && incoming_device_hosts
                        .get(&(virtual_device.id.clone(), fqdn.clone()))
                        .is_none()
                    && results
                        .get(&(virtual_device.id.clone(), fqdn.clone()))
                        .is_none()
                {
                    tracing::info!(
                        "Adding new device host with id {:?} to host {:?}",
                        virtual_device.id,
                        fqdn
                    );

                    results.insert(
                        (virtual_device.id.clone(), fqdn.clone()),
                        Change::Add(other_device_host),
                    );
                } else if db_device_hosts
                    .get(&(virtual_device.id.clone(), fqdn.clone()))
                    .is_some()
                    && incoming_device_hosts
                        .get(&(virtual_device.id.clone(), fqdn.clone()))
                        .is_none()
                    && results
                        .get(&(virtual_device.id.clone(), fqdn.clone()))
                        .is_none()
                {
                    tracing::info!(
                        "Updating device host with id {:?} on host {:?}",
                        virtual_device.id,
                        fqdn
                    );
                    results.insert(
                        (virtual_device.id.clone(), fqdn.clone()),
                        Change::Update(other_device_host),
                    );
                } else if results
                    .get(&(virtual_device.id.clone(), fqdn.clone()))
                    .is_some()
                {
                    unreachable!();
                } else {
                    tracing::warn!(
                        "DB: {:?}, incoming: {:?}, results: {:?}",
                        db_device_hosts
                            .get(&(virtual_device.id.clone(), fqdn.clone()))
                            .is_some(),
                        incoming_device_hosts
                            .get(&(virtual_device.id.clone(), fqdn.clone()))
                            .is_some(),
                        results
                            .get(&(virtual_device.id.clone(), fqdn.clone()))
                            .is_some()
                    );
                }
            } else {
                // remove from db if present and not in flight
                // remove from in-flight if in flight - is it necessary though?

                // TODO: Factor out removal to be a function
                // Same for addition and update
                if db_device_hosts
                    .get(&(virtual_device.id.clone(), fqdn.clone()))
                    .is_some()
                {
                    let other_device_host = DeviceHost {
                        device_id: virtual_device.id.clone(),
                        fqdn: fqdn.clone(),
                        local: true,
                        paths: Paths(BTreeSet::new()),
                        mount_path: MountPath(None),
                        fs_type: None,
                        fs_label: None,
                        fs_uuid: None,
                    };

                    tracing::info!(
                        "Removing device host with id {:?} on host {:?}",
                        virtual_device.id,
                        fqdn,
                    );
                    results.insert(
                        (virtual_device.id.clone(), fqdn.clone()),
                        Change::Remove(other_device_host),
                    );
                } else {
                    // It wasn't in the DB in the first place, nothing to do
                }
            }
        }

        // For all other hosts, run parents check for this virtual device ON THE DB.
        // This is because main loop processes updates from single host at a time.
        // That means current state of other hosts is in DB at this point.

        // TODO: Consider just using db_device_hosts. Incoming are only for current fqdn
        let all_other_host_fqdns: BTreeSet<_> = incoming_device_hosts
            .iter()
            .chain(db_device_hosts.iter())
            .filter_map(|((_, f), _)| if f != fqdn { Some(f) } else { None })
            .collect();

        for host in all_other_host_fqdns {
            let mut i = BreadthFirstParentIterator::new(incoming_devices, &virtual_device.id);
            let all_available = i.all(|p| {
                let result = db_device_hosts.get(&(p.clone(), host.clone())).is_some();
                tracing::info!(
                    "Checking device {:?} on host {:?} in DB: {:?}",
                    p,
                    host,
                    result
                );
                result
            });
            tracing::info!(
                "Host: {:?}, device: {:?}, all_available: {:?}",
                host,
                virtual_device.id,
                all_available
            );
            if all_available {
                let other_device_host = DeviceHost {
                    device_id: virtual_device.id.clone(),
                    fqdn: host.clone(),
                    local: true,
                    // Does it make sense to use paths from other hosts?
                    paths: Paths(
                        virtual_device_host
                            .map(|x| x.paths.clone())
                            .unwrap_or(BTreeSet::new()),
                    ),
                    // It can't be mounted on other hosts at the time this is processed?
                    mount_path: MountPath(None),
                    fs_type: virtual_device_host
                        .map(|x| x.fs_type.clone())
                        .unwrap_or(None),
                    fs_label: virtual_device_host
                        .map(|x| x.fs_label.clone())
                        .unwrap_or(None),
                    fs_uuid: virtual_device_host
                        .map(|x| x.fs_uuid.clone())
                        .unwrap_or(None),
                };

                // add to database if missing and not in flight
                // update in database if present and not in flight
                // update in flight if in flight - is it necessary though?

                if db_device_hosts
                    .get(&(virtual_device.id.clone(), host.clone()))
                    .is_none()
                    && results
                        .get(&(virtual_device.id.clone(), host.clone()))
                        .is_none()
                {
                    tracing::info!(
                        "Adding new device host with id {:?} to host {:?}",
                        virtual_device.id,
                        host
                    );

                    results.insert(
                        (virtual_device.id.clone(), host.clone()),
                        Change::Add(other_device_host),
                    );
                } else if db_device_hosts
                    .get(&(virtual_device.id.clone(), host.clone()))
                    .is_some()
                    && results
                        .get(&(virtual_device.id.clone(), host.clone()))
                        .is_none()
                {
                    tracing::info!(
                        "Updating device host with id {:?} on host {:?}",
                        virtual_device.id,
                        host
                    );
                    results.insert(
                        (virtual_device.id.clone(), host.clone()),
                        Change::Update(other_device_host),
                    );
                } else if results
                    .get(&(virtual_device.id.clone(), host.clone()))
                    .is_some()
                {
                    unreachable!();
                } else {
                    unreachable!();
                }
            } else {
                // remove from db if present and not in flight
                // remove from in-flight if in flight - is it necessary though?

                if db_device_hosts
                    .get(&(virtual_device.id.clone(), host.clone()))
                    .is_some()
                {
                    let other_device_host = DeviceHost {
                        device_id: virtual_device.id.clone(),
                        fqdn: host.clone(),
                        local: true,
                        paths: Paths(BTreeSet::new()),
                        mount_path: MountPath(None),
                        fs_type: None,
                        fs_label: None,
                        fs_uuid: None,
                    };

                    tracing::info!(
                        "Removing device host with id {:?} on host {:?}",
                        virtual_device.id,
                        host,
                    );
                    results.insert(
                        (virtual_device.id.clone(), host.clone()),
                        Change::Remove(other_device_host),
                    );
                } else {
                    // It wasn't in the DB in the first place, nothing to do
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::db::test::{deser_devices, deser_fixture};
    use ::test_case::test_case;
    use insta::assert_debug_snapshot;

    #[test_case("vd_with_shared_parents_added_to_oss2")]
    #[test_case("vd_with_no_shared_parents_not_added_to_oss2")]
    // A leaf device has changed data on one host
    // It has to have updated data on the other one
    #[test_case("vd_with_shared_parents_updated_on_oss2")]
    // A leaf device is replaced with another leaf device
    // Previous one stays in the DB as available
    // We're not removing it since parents are still available
    #[test_case("vd_with_shared_parents_replaced_on_oss2")]
    #[test_case("vd_with_shared_parents_removed_from_oss2_when_parent_disappears")]
    #[test_case("vd_with_two_levels_of_shared_parents_added_to_oss2")]
    fn compute_virtual_device_changes(test_name: &str) {
        let (fqdn, incoming_devices, incoming_device_hosts, db_devices, db_device_hosts) =
            deser_fixture(test_name);

        let updates = super::compute_virtual_device_changes(
            &fqdn,
            &incoming_devices,
            &incoming_device_hosts,
            &db_devices,
            &db_device_hosts,
        )
        .unwrap();

        assert_debug_snapshot!(test_name, updates);
    }

    #[test_case("single_device_doesnt_iterate", "a")]
    #[test_case("single_parent_produces_one_item", "b")]
    #[test_case("two_parents_produce_two_items", "c")]
    #[test_case("parent_and_double_parent_produce_three_items", "c1")]
    #[test_case("triple_parent_and_double_parent_produce_five_items", "c1")]
    fn breadth_first_iterator(test_case: &str, child: &str) {
        let prefix = String::from("fixtures/") + test_case + "/";
        let devices = deser_devices(prefix.clone() + "devices.json");

        let id = DeviceId(child.into());
        let i = BreadthFirstParentIterator::new(&devices, &id);
        let result: Vec<_> = i.collect();
        assert_debug_snapshot!(test_case, result);
    }
}
