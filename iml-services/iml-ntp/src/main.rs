// Copyright (c) 2019 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use futures::TryStreamExt;
use iml_ntp::db::{add_alert, get_active_alert_for_fqdn, get_managed_host_items, lower_alert};
use iml_service_queue::service_queue::consume_data;
use iml_wire_types::time::{State, Synced};
use tracing_subscriber::{fmt::Subscriber, EnvFilter};

pub fn valid_state(mh_state: String) -> bool {
    ["monitored", "managed", "working"]
        .iter()
        .find(|&&x| x == mh_state)
        .is_some()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let pool = iml_orm::pool()?;

    let mut s = consume_data::<State>("rust_agent_ntp_rx");

    while let Some((fqdn, state)) = s.try_next().await? {
        tracing::debug!("fqdn: {:?} state: {:?}", fqdn, state);

        let alert_id = get_active_alert_for_fqdn(&fqdn.0, "NtpOutOfSyncAlert", &pool).await?;

        // FIXME: Handle Synced is None
        if let (Some(alert_id), Some(Synced::Synced)) = (alert_id, state.ntp.1) {
            tracing::info!(
                "Setting alert {} to inactive for host {}.",
                alert_id,
                fqdn.0
            );

            lower_alert(alert_id, &pool).await?;
        } else if state.ntp.1 == Some(Synced::Unsynced) {
            let host_info = get_managed_host_items(&fqdn.0, &pool).await?;

            tracing::debug!(
                "host info used to create new NtpOutOfSyncAlert: {:?}",
                host_info
            );

            if let Some((managed_host_id, content_type_id, managed_host_state)) = host_info {
                let is_valid_state = valid_state(managed_host_state);

                tracing::debug!("is valid state: {:?}", is_valid_state);

                if is_valid_state && alert_id.is_none() {
                    tracing::info!("Creating new NtpOutOfSync entry for {}", fqdn.0);
                    add_alert(&fqdn.0, content_type_id, managed_host_id, &pool).await?;
                }
            }
        }
    }

    Ok(())
}
