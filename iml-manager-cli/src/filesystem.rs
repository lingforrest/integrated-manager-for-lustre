// Copyright (c) 2020 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    api_utils::{
        create_command, get, get_all, get_hosts, get_one, wait_for_cmds, SendCmd, SendJob,
    },
    display_utils::{generate_table, wrap_fut},
    error::ImlManagerCliError,
    ostpool::{ostpool_cli, OstPoolCommand},
};
use futures::future::{try_join, try_join_all};
use iml_wire_types::{ApiList, Filesystem, FlatQuery, Mgt, Ost};
use number_formatter::{format_bytes, format_number};
use prettytable::{Row, Table};
use std::collections::{BTreeMap, HashMap};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum FilesystemCommand {
    /// List all configured filesystems
    #[structopt(name = "list")]
    List,
    /// Show filesystem
    #[structopt(name = "show")]
    Show {
        #[structopt(name = "FSNAME")]
        fsname: String,
    },
    /// Ost Pools
    #[structopt(name = "pool")]
    Pool {
        #[structopt(subcommand)]
        command: OstPoolCommand,
    },
    /// Detect existing filesystem
    #[structopt(name = "detect")]
    Detect {
        #[structopt(short, long)]
        hosts: Option<String>,
    },
}

fn usage(
    free: Option<f64>,
    total: Option<f64>,
    formatter: fn(u64, Option<usize>) -> String,
) -> String {
    match (free, total) {
        (Some(free), Some(total)) if total >= free => format!(
            "{} / {}",
            formatter((total - free) as u64, Some(0)),
            formatter(total as u64, Some(0))
        ),
        (None, Some(total)) => format!("Calculating ... / {}", formatter(total as u64, Some(0))),
        _ => "Calculating ...".to_string(),
    }
}

async fn detect_filesystem(hosts: Option<String>) -> Result<(), ImlManagerCliError> {
    let hosts = if let Some(hl) = hosts {
        let hostlist = hostlist_parser::parse(&hl)?;
        tracing::debug!("Host Names: {:?}", hostlist);
        let all_hosts = get_hosts().await?;

        let hostmap: BTreeMap<&str, &str> = all_hosts
            .objects
            .iter()
            .map(|h| {
                vec![
                    (h.nodename.as_str(), h.resource_uri.as_str()),
                    (h.fqdn.as_str(), h.resource_uri.as_str()),
                ]
            })
            .flatten()
            .collect();

        hostlist
            .iter()
            .filter_map(|h| hostmap.get(h.as_str()))
            .map(|x| (*x).to_string())
            .collect()
    } else {
        vec![]
    };

    tracing::debug!("Host APIs: {:?}", hosts);

    let args = if hosts.is_empty() {
        vec![]
    } else {
        vec![("hosts".to_string(), hosts)]
    };

    let cmd = SendCmd {
        message: "Detecting filesystems".into(),
        jobs: vec![SendJob::<HashMap<String, Vec<String>>> {
            class_name: "DetectTargetsJob".into(),
            args: args.into_iter().collect(),
        }],
    };
    let cmd = wrap_fut("Detecting filesystems...", create_command(cmd)).await?;

    wait_for_cmds(vec![cmd]).await?;
    Ok(())
}

pub async fn filesystem_cli(command: FilesystemCommand) -> Result<(), ImlManagerCliError> {
    match command {
        FilesystemCommand::List => {
            let filesystems: ApiList<Filesystem> =
                wrap_fut("Fetching filesystems...", get_all()).await?;

            tracing::debug!("FSs: {:?}", filesystems);

            let table = generate_table(
                &[
                    "Name", "State", "Space", "Inodes", "Clients", "MDTs", "OSTs",
                ],
                filesystems.objects.into_iter().map(|f| {
                    vec![
                        f.label,
                        f.state,
                        usage(f.bytes_free, f.bytes_total, format_bytes),
                        usage(f.files_free, f.files_total, format_number),
                        format_number(f.client_count.unwrap_or(0.0) as u64, Some(0)),
                        f.mdts.len().to_string(),
                        f.osts.len().to_string(),
                    ]
                }),
            );

            table.printstd();
        }
        FilesystemCommand::Show { fsname } => {
            let fs: Filesystem =
                wrap_fut("Fetching filesystem...", get_one(vec![("name", &fsname)])).await?;

            tracing::debug!("FS: {:?}", fs);

            let (mgt, osts): (Mgt, Vec<Ost>) = try_join(
                wrap_fut("Fetching MGT...", get(&fs.mgt, Mgt::query())),
                try_join_all(fs.osts.into_iter().map(|o| async move {
                    wrap_fut("Fetching OST...", get(&o, Ost::query())).await
                })),
            )
            .await?;

            let mut table = Table::new();
            table.add_row(Row::from(&["Name".to_string(), fs.label]));
            table.add_row(Row::from(&[
                "Space".to_string(),
                usage(fs.bytes_free, fs.bytes_total, format_bytes),
            ]));
            table.add_row(Row::from(&[
                "Inodes".to_string(),
                usage(fs.files_free, fs.files_total, format_number),
            ]));
            table.add_row(Row::from(&["State".to_string(), fs.state]));
            table.add_row(Row::from(&[
                "Management Server".to_string(),
                mgt.active_host_name,
            ]));

            let mdtnames: Vec<String> = fs.mdts.into_iter().map(|m| m.name).collect();
            table.add_row(Row::from(&["MDTs".to_string(), mdtnames.join("\n")]));

            let ostnames: Vec<String> = osts.into_iter().map(|m| m.name).collect();
            table.add_row(Row::from(&["OSTs".to_string(), ostnames.join("\n")]));

            table.add_row(Row::from(&[
                "Clients".to_string(),
                format!("{:0}", fs.client_count.unwrap_or(0.0)),
            ]));
            table.add_row(Row::from(&["Mount Path".to_string(), fs.mount_path]));
            table.printstd();
        }
        FilesystemCommand::Pool { command } => ostpool_cli(command).await?,
        FilesystemCommand::Detect { hosts } => detect_filesystem(hosts).await?,
    };

    Ok(())
}
