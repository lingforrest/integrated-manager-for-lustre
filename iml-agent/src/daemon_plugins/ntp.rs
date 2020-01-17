// Copyright (c) 2019 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    agent_error::ImlAgentError,
    daemon_plugins::{DaemonPlugin, Output},
    systemd,
};
use futures::{future, Future, StreamExt, TryFutureExt, TryStreamExt};
use iml_cmd::cmd_output;
use iml_wire_types::{time, RunState};
use lazy_static::lazy_static;
use regex::Regex;
use std::pin::Pin;

static NTP_CONFIG_FILE: &'static str = "/etc/ntp.conf";

async fn get_time_sync_services() -> Result<(RunState, RunState), ImlAgentError> {
    let ntpd = systemd::get_run_state("ntpd.service".into());
    let chronyd = systemd::get_run_state("chronyd.service".into());

    future::try_join(ntpd, chronyd).await
}

fn parse_ntp_synced(output: impl ToString) -> Option<time::Synced> {
    let output = output.to_string();

    let x = match output.as_ref() {
        "b true" => Some(time::Synced::Synced),
        "b false" => Some(time::Synced::Unsynced),
        _ => None,
    };

    tracing::debug!("is_ntp_synced: {:?}; output {}", x, output);

    x
}

fn parse_ntp_time_offset(output: impl ToString) -> Option<time::Offset> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"time correct to within (.+)\n").unwrap();
    }

    RE.captures(&output.to_string())
        .and_then(|caps| caps.get(1))
        .map(|x| x.as_str().into())
}

fn parse_chrony_time_offset(output: impl ToString) -> Option<time::Offset> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"Offset          : [+-](.+)\n").unwrap();
    }

    RE.captures(&output.to_string())
        .and_then(|caps| caps.get(1))
        .map(|x| x.as_str().into())
}

#[derive(Debug)]
pub struct Ntp;

pub fn create() -> impl DaemonPlugin {
    Ntp
}

async fn is_ntp_configured_by_iml() -> Result<bool, ImlAgentError> {
    let x = iml_fs::stream_file_lines(NTP_CONFIG_FILE)
        .boxed()
        .try_filter(|l| future::ready(l.find("# IML_EDIT").is_some()))
        .try_next()
        .await?
        .is_some();

    Ok(x)
}

async fn get_ntpstat_command() -> Result<String, ImlAgentError> {
    let p = cmd_output("ntpstat", vec![]).await?;

    Ok(std::str::from_utf8(&p.stdout).unwrap_or("").to_string())
}

async fn get_chronyc_ntpdata_command() -> Result<String, ImlAgentError> {
    let p = cmd_output("chronyc", vec!["ntpdata"]).await?;

    Ok(std::str::from_utf8(&p.stdout).unwrap_or("").to_string())
}

async fn is_ntp_synced_command() -> Result<String, ImlAgentError> {
    let x = cmd_output(
        "busctl",
        vec![
            "get-property",
            "org.freedesktop.timedate1",
            "/org/freedesktop/timedate1",
            "org.freedesktop.timedate1",
            "NTPSynchronized",
        ],
    )
    .await?;

    Ok(std::str::from_utf8(&x.stdout)
        .unwrap_or_default()
        .trim()
        .to_string())
}

/// Returns Ntp sync and offset.
async fn ntpd_synced() -> Result<(Option<time::Synced>, Option<time::Offset>), ImlAgentError> {
    let ntp_synced = is_ntp_synced_command().map_ok(parse_ntp_synced);
    let time_offset = get_ntpstat_command().map_ok(parse_ntp_time_offset);

    future::try_join(ntp_synced, time_offset).await
}

/// Returns Chrony sync and offset
async fn chronyd_synced() -> Result<(Option<time::Synced>, Option<time::Offset>), ImlAgentError> {
    let ntp_synced = is_ntp_synced_command().map_ok(parse_ntp_synced);
    let time_offset = get_chronyc_ntpdata_command().map_ok(parse_chrony_time_offset);

    future::try_join(ntp_synced, time_offset).await
}

async fn time_synced<
    F1: Future<Output = Result<bool, ImlAgentError>>,
    F2: Future<Output = Result<(RunState, RunState), ImlAgentError>>,
    F3: Future<Output = Result<(Option<time::Synced>, Option<time::Offset>), ImlAgentError>>,
    F4: Future<Output = Result<(Option<time::Synced>, Option<time::Offset>), ImlAgentError>>,
>(
    is_ntp_configured_by_iml: fn() -> F1,
    get_time_sync_services: fn() -> F2,
    ntpd_synced: fn() -> F3,
    chronyd_synced: fn() -> F4,
) -> Result<Output, ImlAgentError> {
    let configured = is_ntp_configured_by_iml().await?;

    tracing::debug!("Configured: {:?}", configured);

    let (ntp_runstate, chrony_runstate) = get_time_sync_services().await?;

    let (ntp_synced, ntp_offset) = ntpd_synced().await?;

    let (chrony_synced, chrony_offset) = chronyd_synced().await?;

    tracing::debug!(
        "ntp runstate: {:?}. chrony runstate: {:?}",
        ntp_runstate,
        chrony_runstate
    );

    let x = time::State {
        iml_configured: configured,
        ntp: (ntp_runstate, ntp_synced, ntp_offset),
        chrony: (chrony_runstate, chrony_synced, chrony_offset),
    };

    let x = serde_json::to_value(x).map(Some)?;

    Ok(x)
}

impl DaemonPlugin for Ntp {
    fn start_session(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Output, ImlAgentError>> + Send>> {
        self.update_session()
    }

    fn update_session(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Output, ImlAgentError>> + Send>> {
        Box::pin(time_synced(
            is_ntp_configured_by_iml,
            get_time_sync_services,
            ntpd_synced,
            chronyd_synced,
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::FutureExt;

    #[test]
    fn test_is_ntp_synced() {
        let s = r#"b true"#;

        assert_eq!(parse_ntp_synced(s), Some(time::Synced::Synced));
    }

    #[test]
    fn test_is_ntp_not_synced() {
        let s = r#"b false"#;

        assert_eq!(parse_ntp_synced(s), Some(time::Synced::Unsynced));
    }

    #[test]
    fn test_bad_output() {
        let s = r#"b tr"#;

        assert_eq!(parse_ntp_synced(s), None);
    }

    #[test]
    fn test_get_ntp_time_offset() {
        let s = r#"synchronised to NTP server (10.73.10.10) at stratum 12
time correct to within 949 ms
polling server every 64 s"#;

        assert_eq!(parse_ntp_time_offset(s), Some("949 ms".to_string().into()));
    }

    #[test]
    fn test_get_chrony_time_offset() {
        let s = r#"

Remote address  : 10.73.10.10 (0A490A0A)
Remote port     : 123
Local address   : 10.73.10.12 (0A490A0C)
Leap status     : Normal
Version         : 4
Mode            : Server
Stratum         : 11
Poll interval   : 4 (16 seconds)
Precision       : -25 (0.000000030 seconds)
Root delay      : 0.000000 seconds
Root dispersion : 0.011124 seconds
Reference ID    : 7F7F0100 ()
Reference time  : Fri Sep 13 08:03:04 2019
Offset          : +0.000026767 seconds
Peer delay      : 0.000403893 seconds
Peer dispersion : 0.000000056 seconds
Response time   : 0.000079196 seconds
Jitter asymmetry: +0.00
NTP tests       : 111 111 1111
Interleaved     : No
Authenticated   : No
TX timestamping : Kernel
RX timestamping : Kernel
Total TX        : 1129
Total RX        : 1129
Total valid RX  : 1129"#;

        assert_eq!(
            parse_chrony_time_offset(s),
            Some("0.000026767 seconds".to_string().into())
        );
    }

    #[tokio::test]
    async fn test_session_with_ntp_configured_by_iml() -> Result<(), ImlAgentError> {
        fn is_ntp_configured_by_iml(
        ) -> Pin<Box<dyn Future<Output = Result<bool, ImlAgentError>> + Send>> {
            println!("TestSideEffect: is ntp configured by iml");
            Box::pin(future::ok::<bool, ImlAgentError>(true))
        }

        fn get_time_sync_services(
        ) -> Pin<Box<dyn Future<Output = Result<(RunState, RunState), ImlAgentError>> + Send>>
        {
            future::ok((RunState::Setup, RunState::Stopped)).boxed()
        }

        fn ntpd_synced() -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            (Option<time::Synced>, Option<time::Offset>),
                            ImlAgentError,
                        >,
                    > + Send,
            >,
        > {
            future::ok((Some(time::Synced::Synced), Some("949 ms".into()))).boxed()
        }

        fn chronyd_synced() -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            (Option<time::Synced>, Option<time::Offset>),
                            ImlAgentError,
                        >,
                    > + Send,
            >,
        > {
            future::ok((Some(time::Synced::Unsynced), None)).boxed()
        }

        let r = time_synced(
            is_ntp_configured_by_iml,
            get_time_sync_services,
            ntpd_synced,
            chronyd_synced,
        )
        .await?;

        assert_eq!(
            r,
            serde_json::to_value(time::State {
                iml_configured: true,
                ntp: (
                    RunState::Setup,
                    Some(time::Synced::Synced),
                    Some("949 ms".into())
                ),
                chrony: (RunState::Stopped, Some(time::Synced::Unsynced), None)
            })
            .ok()
        );

        Ok(())
    }
}
