// Copyright (c) 2019 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    change::{self, Change},
    error::ImlDevicesError,
};
use futures::TryStreamExt;
use iml_postgres::{select_all, Client, Transaction};
use iml_wire_types::{
    db::{Device, DeviceHost, DeviceId, DeviceIds, DeviceType, MountPath, Name, Paths, Size},
    Fqdn,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    iter,
    path::PathBuf,
};

pub type FlatDevices = BTreeMap<DeviceId, FlatDevice>;

pub type Devices = BTreeMap<DeviceId, Device>;

pub type DevicesRef<'a> = BTreeMap<&'a DeviceId, &'a Device>;

pub type DeviceHostKey = (DeviceId, Fqdn);

pub type DeviceHosts = BTreeMap<DeviceHostKey, DeviceHost>;

pub type DeviceHostsRef<'a> = BTreeMap<&'a DeviceHostKey, &'a DeviceHost>;

/// A device (Block or Virtual).
/// These should be unique per cluster
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FlatDevice {
    pub id: DeviceId,
    pub size: u64,
    pub device_type: DeviceType,
    pub parents: BTreeSet<DeviceId>,
    // pub usable_for_lustre: bool,
    pub children: BTreeSet<DeviceId>,
    pub paths: BTreeSet<PathBuf>,
    pub mount_path: Option<PathBuf>,
    pub fs_type: Option<String>,
    pub fs_label: Option<String>,
    pub fs_uuid: Option<String>,
}

fn create_dev(
    flat_dev: &FlatDevice,
    fqdn: Fqdn,
) -> ((DeviceId, Device), (DeviceHostKey, DeviceHost)) {
    let d = Device {
        id: flat_dev.id.clone(),
        size: Size(flat_dev.size),
        device_type: flat_dev.device_type.clone(),
        parents: DeviceIds(flat_dev.parents.clone()),
        children: DeviceIds(flat_dev.children.clone()),
        // usable_for_lustre: flat_dev.usable_for_lustre,
        usable_for_lustre: false,
    };

    let dh = DeviceHost {
        device_id: flat_dev.id.clone(),
        fqdn,
        local: true,
        paths: Paths(flat_dev.paths.clone()),
        mount_path: MountPath(flat_dev.mount_path.clone()),
        fs_type: flat_dev.fs_type.clone(),
        fs_label: flat_dev.fs_label.clone(),
        fs_uuid: flat_dev.fs_uuid.clone(),
    };

    (
        (flat_dev.id.clone(), d),
        ((flat_dev.id.clone(), dh.fqdn.clone()), dh),
    )
}

pub fn convert_flat_devices(flat_devices: &FlatDevices, fqdn: Fqdn) -> (Devices, DeviceHosts) {
    flat_devices
        .iter()
        .map(|x| create_dev(x.1, fqdn.clone()))
        .unzip()
}

/// Given a device id and some `DeviceHosts`,
/// filter to the cooresponding hosts.
fn filter_device_hosts<'a>(
    id: &'a DeviceId,
    device_hosts: &'a DeviceHosts,
) -> impl Iterator<Item = (&'a DeviceHostKey, &'a DeviceHost)> {
    device_hosts.iter().filter(move |(_, v)| &v.device_id == id)
}

/// Given a device id and some `DeviceHosts`,
/// try to find the first cooresponding host.
fn find_device_host<'a>(
    id: &'a DeviceId,
    device_hosts: &'a DeviceHostsRef<'a>,
) -> Option<(&'a &'a DeviceHostKey, &'a &'a DeviceHost)> {
    device_hosts
        .into_iter()
        .find(move |(_, v)| &v.device_id == id)
}

pub fn get_local_device_hosts<'a>(
    device_hosts: &'a DeviceHosts,
    fqdn: &Fqdn,
) -> DeviceHostsRef<'a> {
    device_hosts
        .into_iter()
        .filter(|(_k, v)| &v.fqdn == fqdn && v.local)
        .collect()
}

pub fn get_local_devices<'a>(
    local_device_hosts: &DeviceHostsRef<'_>,
    devices: &'a Devices,
) -> DevicesRef<'a> {
    devices
        .into_iter()
        .filter(|(k, _)| {
            local_device_hosts
                .into_iter()
                .find(|(_, v)| &&v.device_id == k)
                .is_some()
        })
        .collect()
}

pub fn get_other_device_hosts<'a>(
    db_device_hosts: &'a DeviceHosts,
    fqdn: &'a Fqdn,
) -> DeviceHostsRef<'a> {
    db_device_hosts
        .iter()
        .filter(move |(_, v)| &v.fqdn != fqdn)
        .collect()
}

pub fn get_devices_by_device_host<'a>(
    device_hosts: &'a DeviceHostsRef<'a>,
    devices: &'a Devices,
) -> DevicesRef<'a> {
    devices
        .iter()
        .filter(move |(k, _)| find_device_host(&k, device_hosts).is_some())
        .collect()
}

pub async fn get_db_devices(mut client: &mut Client) -> Result<Devices, iml_postgres::Error> {
    select_all(
        &mut client,
        &format!("SELECT * FROM {}", Device::table_name()),
        iter::empty(),
    )
    .await?
    .map_ok(Device::from)
    .map_ok(|x| (x.id.clone(), x))
    .try_collect()
    .await
}

pub async fn get_db_device_hosts(
    mut client: &mut Client,
) -> Result<Vec<DeviceHost>, iml_postgres::Error> {
    select_all(
        &mut client,
        &format!("SELECT * FROM {}", DeviceHost::table_name()),
        iter::empty(),
    )
    .await?
    .map_ok(DeviceHost::from)
    .try_collect()
    .await
}

async fn insert_device_host(
    transaction: &mut Transaction<'_>,
    fqdn: &Fqdn,
    x: &DeviceHost,
) -> Result<(), ImlDevicesError> {
    let s = transaction.prepare(
        &format!("INSERT INTO {} (device_id, fqdn, local, paths, mount_path, fs_type, fs_label, fs_uuid) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)", DeviceHost::table_name())
    ).await?;

    transaction
        .execute(
            &s,
            &[
                &x.device_id,
                &fqdn.0,
                &x.local,
                &x.paths,
                &x.mount_path,
                &x.fs_type,
                &x.fs_label,
                &x.fs_uuid,
            ],
        )
        .await?;

    Ok(())
}

async fn update_device_host(
    transaction: &mut Transaction<'_>,
    fqdn: &Fqdn,
    x: &DeviceHost,
) -> Result<(), ImlDevicesError> {
    let s = transaction.prepare(&format!("UPDATE {} SET local = $3, paths = $4, mount_path = $5, fs_type = $6, fs_label= $7, fs_uuid=$8 WHERE device_id = $1 AND fqdn = $2", DeviceHost::table_name())).await?;

    transaction
        .execute(
            &s,
            &[
                &x.device_id,
                &fqdn.0,
                &x.local,
                &x.paths,
                &x.mount_path,
                &x.fs_type,
                &x.fs_label,
                &x.fs_uuid,
            ],
        )
        .await?;

    Ok(())
}

async fn remove_device_host(
    transaction: &mut Transaction<'_>,
    fqdn: &Fqdn,
    id: &DeviceId,
) -> Result<(), ImlDevicesError> {
    let s = transaction
        .prepare(&format!(
            "DELETE FROM {} WHERE device_id = $1 AND fqdn = $2",
            DeviceHost::table_name()
        ))
        .await?;

    transaction.execute(&s, &[id, &fqdn.0]).await?;

    Ok(())
}

pub async fn persist_local_device_hosts<'a>(
    mut transaction: &mut Transaction<'a>,
    incoming_devices: &DeviceHosts,
    local_db_device_hosts: &DeviceHostsRef<'_>,
) -> Result<(), ImlDevicesError> {
    for c in change::get_changes_values(local_db_device_hosts, &incoming_devices.iter().collect()) {
        match c {
            Change::Add(d) => {
                tracing::debug!(
                    "Going to insert new devicehost {:?}, {:?}",
                    d.fqdn,
                    d.device_id
                );

                insert_device_host(&mut transaction, &d.fqdn, d).await?;
            }
            Change::Update(d) => {
                tracing::debug!("Going to update devicehost {:?}, {:?}", d.fqdn, d.device_id);

                update_device_host(&mut transaction, &d.fqdn, d).await?;
            }
            Change::Remove(d) => {
                tracing::debug!("Going to remove devicehost {:?}, {:?}", d.fqdn, d.device_id);

                remove_device_host(&mut transaction, &d.fqdn, &d.device_id).await?;
            }
        };
    }

    Ok(())
}

pub async fn persist_local_devices<'a>(
    transaction: &mut Transaction<'a>,
    incoming_devices: &Devices,
    other_devices: &DevicesRef<'a>,
    local_db_devices: &DevicesRef<'a>,
) -> Result<(), ImlDevicesError> {
    for c in change::get_changes_values(&local_db_devices, &incoming_devices.iter().collect()) {
        match c {
            Change::Add(d) => {
                if other_devices.get(&d.id).is_some() {
                    tracing::info!("Device {:?} already added by another host.", d.id);
                    continue;
                }

                tracing::debug!("Going to add device {:?}", d.id);

                let s = transaction.prepare("INSERT INTO chroma_core_device (id, size, usable_for_lustre, device_type, parents, children) VALUES ($1, $2, $3, $4, $5, $6)").await?;

                transaction
                    .execute(
                        &s,
                        &[
                            &d.id,
                            &d.size,
                            &d.usable_for_lustre,
                            &d.device_type,
                            &d.parents,
                            &d.children,
                        ],
                    )
                    .await?;
            }
            Change::Update(d) => {
                tracing::debug!("Going to update device {:?}", d.id);

                let s = transaction.prepare("UPDATE chroma_core_device SET size = $2, usable_for_lustre = $3, device_type = $4, parents=$5, children=$6 WHERE id = $1").await?;

                transaction
                    .execute(
                        &s,
                        &[
                            &d.id,
                            &d.size,
                            &d.usable_for_lustre,
                            &d.device_type,
                            &d.parents,
                            &d.children,
                        ],
                    )
                    .await?;
            }
            Change::Remove(d) => {
                // @TODO: I think devices should probably not be deleted.
                // Orphan devices should probably be surfaced as alerts.
                tracing::debug!("Going to remove device {:?}", d.id);

                let s = transaction
                    .prepare(&format!(
                        "DELETE FROM {} WHERE id = $1",
                        Device::table_name()
                    ))
                    .await?;

                transaction.execute(&s, &[&d.id]).await?;
            }
        }
    }

    Ok(())
}

fn compute_virtual_device_changes<'a>(
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

    for virtual_device in incoming_devices.values() {
        tracing::info!("virtual_device: {:#?}", virtual_device);
        let virtual_device_host =
            incoming_device_hosts.get(&(virtual_device.id.clone(), fqdn.clone()));
        tracing::info!("virtual_device_host: {:#?}", virtual_device_host);

        let mut parents = virtual_device.parents.clone();

        let mut depth = 1;
        let max_depth = 8;

        while depth < max_depth {
            tracing::info!("depth = {}, parents = {:#?}", depth, parents);
            let mut new_parents = BTreeSet::new();

            for parent in parents.iter() {
                let other_hosts = filter_device_hosts(&parent, &incoming_device_hosts)
                    .filter(|(_, v)| &v.fqdn != fqdn)
                    .map(|(_, v)| v);

                for other_host in other_hosts {
                    let other_device_host = DeviceHost {
                        device_id: virtual_device.id.clone(),
                        fqdn: other_host.fqdn.clone(),
                        local: true,
                        // Does it make sense to import paths from other hosts?
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

                    if db_device_hosts
                        .get(&(virtual_device.id.clone(), other_host.fqdn.clone()))
                        .is_none()
                    {
                        tracing::info!(
                            "Adding new device host with id {:?} to host {:?}",
                            virtual_device.id,
                            other_host.fqdn
                        );

                        results.insert(
                            (virtual_device.id.clone(), other_host.fqdn.clone()),
                            Change::Add(other_device_host),
                        );
                    } else {
                        tracing::info!(
                            "Updating device host with id {:?} on host {:?}",
                            virtual_device.id,
                            other_host.fqdn
                        );
                        results.insert(
                            (virtual_device.id.clone(), other_host.fqdn.clone()),
                            Change::Update(other_device_host),
                        );
                    }
                }

                for (id, db_host) in db_device_hosts {
                    let &(ref device_id, _) = id;
                    let device = incoming_devices.get(device_id);
                    if let Some(d) = device {
                        let parents = &d.parents;
                        for parent in parents.iter() {
                            if incoming_device_hosts
                                .get(&(parent.clone(), db_host.fqdn.clone()))
                                .is_none()
                                && results
                                    .get(&(parent.clone(), db_host.fqdn.clone()))
                                    .is_none()
                            {
                                let other_device_host = DeviceHost {
                                    device_id: virtual_device.id.clone(),
                                    fqdn: db_host.fqdn.clone(),
                                    local: true,
                                    // Does it make sense to import paths from other hosts?
                                    paths: Paths(BTreeSet::new()),
                                    // It can't be mounted on other hosts at the time this is processed?
                                    mount_path: MountPath(None),
                                    fs_type: None,
                                    fs_label: None,
                                    fs_uuid: None,
                                };

                                tracing::info!(
                                    "Removing device host with id {:?} on host {:?}",
                                    virtual_device.id,
                                    other_device_host.fqdn
                                );
                                results.insert(
                                    (virtual_device.id.clone(), db_host.fqdn.clone()),
                                    Change::Remove(other_device_host),
                                );
                            }
                        }
                    }
                }

                incoming_devices
                    .get(parent)
                    .or_else(|| db_devices.get(parent))
                    .map(|x| {
                        for p in x.parents.iter() {
                            new_parents.insert(p.clone());
                        }
                    });
            }

            if new_parents.is_empty() {
                break;
            }
            parents = new_parents;
            depth += 1;
        }

        if depth == max_depth {
            tracing::error!("Hit upper limit {} on recursion", max_depth);
        }
    }

    Ok(results)
}

/// Some devices should appear on multiple hosts even if they are physically existent on one host.
///
/// Examples are Zpools / Datasets, LVs / VGs and MdRaid.
pub async fn update_virtual_devices<'a>(
    mut transaction: &mut Transaction<'a>,
    fqdn: &Fqdn,
    incoming_devices: &Devices,
    incoming_device_hosts: &DeviceHosts,
    db_devices: &Devices,
    db_device_hosts: &DeviceHosts,
) -> Result<(), ImlDevicesError> {
    let changes = compute_virtual_device_changes(
        fqdn,
        incoming_devices,
        incoming_device_hosts,
        db_devices,
        db_device_hosts,
    )?;

    for c in changes {
        match c {
            (_, Change::Add(d)) => {
                tracing::debug!(
                    "Going to insert new devicehost {:?}, {:?}",
                    d.fqdn,
                    d.device_id
                );

                insert_device_host(&mut transaction, &d.fqdn, &d).await?;
            }
            (_, Change::Update(d)) => {
                tracing::debug!("Going to update devicehost {:?}, {:?}", d.fqdn, d.device_id);

                update_device_host(&mut transaction, &d.fqdn, &d).await?;
            }
            (_, Change::Remove(d)) => {
                tracing::debug!("Going to remove devicehost {:?}, {:?}", d.fqdn, d.device_id);

                remove_device_host(&mut transaction, &d.fqdn, &d.device_id).await?;
            }
        };
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use ::test_case::test_case;
    use insta::assert_debug_snapshot;
    use std::{fs, path::Path};
    use tracing_subscriber::FmtSubscriber;

    fn deser_devices<P>(path: P) -> BTreeMap<DeviceId, Device>
    where
        P: AsRef<Path>,
    {
        let devices_from_json = fs::read_to_string(path).unwrap();
        serde_json::from_str(&devices_from_json).unwrap()
    }

    fn deser_device_hosts<P>(path: P) -> BTreeMap<(DeviceId, Fqdn), DeviceHost>
    where
        P: AsRef<Path>,
    {
        let device_hosts_from_json = fs::read_to_string(path).unwrap();
        let vec: Vec<DeviceHost> = serde_json::from_str(&device_hosts_from_json).unwrap();
        vec.into_iter()
            .map(|x| ((x.device_id.clone(), x.fqdn.clone()), x))
            .collect()
    }

    fn deser_fixture(
        test_name: &str,
    ) -> (
        BTreeMap<DeviceId, Device>,
        BTreeMap<(DeviceId, Fqdn), DeviceHost>,
        BTreeMap<DeviceId, Device>,
        BTreeMap<(DeviceId, Fqdn), DeviceHost>,
    ) {
        let prefix = String::from("fixtures/") + test_name + "/";

        let incoming_devices = deser_devices(prefix.clone() + "incoming_devices.json");
        let db_devices = deser_devices(prefix.clone() + "db_devices.json");

        let incoming_device_hosts =
            deser_device_hosts(prefix.clone() + "incoming_device_hosts.json");
        let db_device_hosts = deser_device_hosts(prefix + "db_device_hosts.json");

        (
            incoming_devices,
            incoming_device_hosts,
            db_devices,
            db_device_hosts,
        )
    }

    fn _init_subscriber() {
        let subscriber = FmtSubscriber::new();

        tracing::subscriber::set_global_default(subscriber)
            .map_err(|_err| eprintln!("Unable to set global default subscriber"))
            .unwrap();
    }

    #[test_case("vd_with_shared_parents_added_to_oss2")]
    #[test_case("vd_with_no_shared_parents_not_added_to_oss2")]
    #[test_case("vd_with_shared_parents_updated_on_oss2")]
    #[test_case("vd_with_shared_parents_removed_from_oss2_when_parent_disappears")]
    fn compute_virtual_device_changes(test_name: &str) {
        let (incoming_devices, incoming_device_hosts, db_devices, db_device_hosts) =
            deser_fixture(test_name);

        let updates = super::compute_virtual_device_changes(
            &Fqdn("oss1".into()),
            &incoming_devices,
            &incoming_device_hosts,
            &db_devices,
            &db_device_hosts,
        )
        .unwrap();

        assert_debug_snapshot!(test_name, updates);
    }

    #[test_case("local_device_hosts_persisted_on_clean_db")]
    fn get_changes_values(test_case: &str) {
        let (_, incoming_device_hosts, _, db_device_hosts) = deser_fixture(test_case);
        let fqdn = Fqdn("oss1".into());
        let local_db_device_hosts = get_local_device_hosts(&db_device_hosts, &fqdn);

        let t = incoming_device_hosts.iter().collect();
        let changes = change::get_changes_values(&local_db_device_hosts, &t);
        assert_debug_snapshot!(test_case, changes);
    }
}
