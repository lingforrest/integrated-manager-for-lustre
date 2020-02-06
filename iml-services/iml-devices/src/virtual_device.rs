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

fn make_other_device_host(
    device_id: DeviceId,
    fqdn: Fqdn,
    virtual_device_host: Option<&DeviceHost>,
) -> DeviceHost {
    DeviceHost {
        device_id,
        fqdn,
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
    }
}

fn make_other_device_host_for_removal(device_id: DeviceId, fqdn: Fqdn) -> DeviceHost {
    DeviceHost {
        device_id,
        fqdn,
        local: true,
        paths: Paths(BTreeSet::new()),
        mount_path: MountPath(None),
        fs_type: None,
        fs_label: None,
        fs_uuid: None,
    }
}

fn add_device_host(
    device_id: DeviceId,
    fqdn: Fqdn,
    virtual_device_host: Option<&DeviceHost>,
    results: &mut BTreeMap<(DeviceId, Fqdn), Change<DeviceHost>>,
) {
    let other_device_host =
        make_other_device_host(device_id.clone(), fqdn.clone(), virtual_device_host);

    tracing::info!(
        "Adding new device host with id {:?} to host {:?}",
        device_id,
        fqdn
    );

    results.insert(
        (device_id.clone(), fqdn.clone()),
        Change::Add(other_device_host),
    );
}

fn update_device_host(
    device_id: DeviceId,
    fqdn: Fqdn,
    virtual_device_host: Option<&DeviceHost>,
    results: &mut BTreeMap<(DeviceId, Fqdn), Change<DeviceHost>>,
) {
    let other_device_host =
        make_other_device_host(device_id.clone(), fqdn.clone(), virtual_device_host);

    tracing::info!(
        "Updating device host with id {:?} on host {:?}",
        device_id,
        fqdn
    );

    results.insert(
        (device_id.clone(), fqdn.clone()),
        Change::Update(other_device_host),
    );
}

fn remove_device_host(
    device_id: DeviceId,
    fqdn: Fqdn,
    results: &mut BTreeMap<(DeviceId, Fqdn), Change<DeviceHost>>,
) {
    let other_device_host = make_other_device_host_for_removal(device_id.clone(), fqdn.clone());

    tracing::info!(
        "Removing device host with id {:?} on host {:?}",
        device_id,
        fqdn,
    );
    results.insert(
        (device_id.clone(), fqdn.clone()),
        Change::Remove(other_device_host),
    );
}

fn are_all_parents_available(
    devices: &Devices,
    device_hosts: &DeviceHosts,
    host: Fqdn,
    child_id: &DeviceId,
) -> bool {
    let mut i = BreadthFirstParentIterator::new(devices, child_id);
    let all_available = i.all(|p| {
        let result = device_hosts.get(&(p.clone(), host.clone())).is_some();
        tracing::info!(
            "Checking device {:?} on host {:?} in incoming: {:?}",
            p,
            host,
            result
        );
        result
    });
    tracing::info!(
        "Host: {:?}, device: {:?}, all_available: {:?}",
        host,
        child_id,
        all_available
    );
    all_available
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
        // We can have incoming devices where there are two levels of parents of virtual devices
        {
            let all_available = are_all_parents_available(
                incoming_devices,
                incoming_device_hosts,
                fqdn.clone(),
                &virtual_device.id,
            );
            if !all_available {
                // remove from db if present and not in flight
                // remove from in-flight if in flight - is it necessary though?

                // TODO: Factor out removal to be a function
                // Same for addition and update
                if db_device_hosts
                    .get(&(virtual_device.id.clone(), fqdn.clone()))
                    .is_some()
                {
                    remove_device_host(virtual_device.id.clone(), fqdn.clone(), &mut results);
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
                let result_db = db_device_hosts.get(&(p.clone(), host.clone())).is_some();
                tracing::info!(
                    "Checking device {:?} on host {:?} in DB: {:?}",
                    p,
                    host,
                    result_db
                );
                let result_results = results.get(&(p.clone(), host.clone())).is_some();
                tracing::info!(
                    "Checking device {:?} on host {:?} in results: {:?}",
                    p,
                    host,
                    result_db
                );
                result_db || result_results
            });
            tracing::info!(
                "Host: {:?}, device: {:?}, all_available: {:?}",
                host,
                virtual_device.id,
                all_available
            );
            if all_available {
                // add to database if missing and not in flight
                // update in database if present and not in flight
                // update in flight if in flight - is it necessary though?

                if db_device_hosts
                    .get(&(virtual_device.id.clone(), host.clone()))
                    .is_none()
                    && incoming_device_hosts
                        .get(&(virtual_device.id.clone(), host.clone()))
                        .is_none()
                    && results
                        .get(&(virtual_device.id.clone(), host.clone()))
                        .is_none()
                {
                    add_device_host(
                        virtual_device.id.clone(),
                        host.clone(),
                        virtual_device_host,
                        &mut results,
                    );
                } else if db_device_hosts
                    .get(&(virtual_device.id.clone(), host.clone()))
                    .is_some()
                    && incoming_device_hosts
                        .get(&(virtual_device.id.clone(), host.clone()))
                        .is_none()
                    && results
                        .get(&(virtual_device.id.clone(), host.clone()))
                        .is_none()
                {
                    update_device_host(
                        virtual_device.id.clone(),
                        host.clone(),
                        virtual_device_host,
                        &mut results,
                    );
                } else if results
                    .get(&(virtual_device.id.clone(), host.clone()))
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

                if db_device_hosts
                    .get(&(virtual_device.id.clone(), host.clone()))
                    .is_some()
                {
                    remove_device_host(virtual_device.id.clone(), host.clone(), &mut results);
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
    // An intermediary non-virtual parent disappears from the host
    // Its children are getting removed
    // Virtual devices from the other host receive updates that aren't necessary but aren't harmful
    #[test_case("vd_with_two_levels_of_shared_parents_removed_from_oss2_when_parent_disappears")]
    // A leaf virtual device is replaced with another virtual device on one host
    // Previous one stays in the DB as available
    // We're not removing it since parents are still available
    // Virtual device that is parent of the replaced receives update that isn't necessary but isn't harmful
    #[test_case("vd_with_two_levels_of_shared_parents_replaced_on_oss2")]
    // A leaf device has changed data on one host
    // Virtual device that is parent of the replaced receives update that isn't necessary but isn't harmful
    // It has to have updated data on the other one
    #[test_case("vd_with_two_levels_of_shared_parents_updated_on_oss2")]
    fn compute_virtual_device_changes(test_name: &str) {
        // crate::db::test::_init_subscriber();
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
