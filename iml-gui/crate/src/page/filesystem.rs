// Copyright (c) 2020 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    components::{
        action_dropdown, alert_indicator, lock_indicator, paging, progress_circle, resource_links, stratagem,
        table as t, Placement,
    },
    extensions::MergeAttrs,
    generated::css_classes::C,
    route::RouteId,
    sleep_with_handle, GMsg, Route,
};
use futures::channel::oneshot;
use iml_wire_types::{
    warp_drive::{ArcCache, Locks},
    Filesystem, Session, Target, TargetConfParam, TargetKind, ToCompositeId,
};
use number_formatter as nf;
use seed::{prelude::*, *};
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};

pub struct Row {
    dropdown: action_dropdown::Model,
}

#[derive(Default, serde::Deserialize, Clone, Debug)]
pub(crate) struct Stats {
    pub(crate) bytes_total: Option<u64>,
    pub(crate) bytes_free: Option<u64>,
    pub(crate) bytes_avail: Option<u64>,
    pub(crate) files_total: Option<u64>,
    pub(crate) files_free: Option<u64>,
    pub(crate) clients: Option<u64>,
}

pub struct Model {
    pub fs: Arc<Filesystem>,
    mdts: Vec<Arc<Target<TargetConfParam>>>,
    mdt_paging: paging::Model,
    mgt: Vec<Arc<Target<TargetConfParam>>>,
    osts: Vec<Arc<Target<TargetConfParam>>>,
    ost_paging: paging::Model,
    rows: HashMap<u32, Row>,
    stratagem: stratagem::Model,
    stats: Stats,
    stats_cancel: Option<oneshot::Sender<()>>,
    stats_url: String,
}

impl Model {
    pub(crate) fn new(fs: &Arc<Filesystem>) -> Self {
        let query = format!(
            r#"SELECT SUM(b_total), SUM(b_free), SUM(b_avail),
                    SUM(f_total), SUM(f_free),
                    SUM(clients)
               FROM (SELECT LAST(bytes_total) AS b_total
                          , LAST(bytes_free) AS b_free
                          , LAST(bytes_avail) AS b_avail
                          , LAST(files_total) AS f_total
                          , LAST(files_free) AS f_free
                      FROM target
                      WHERE "kind" = 'OST' AND "fs" = '{fs_name}'
                      GROUP BY target)
                   , (SELECT LAST(connected_clients) AS clients
                      FROM target
                      WHERE "fs"='{fs_name}' AND "kind"='MDT'
                      GROUP BY target)"#,
            fs_name = &fs.name
        );

        Self {
            fs: Arc::clone(fs),
            mdts: Default::default(),
            mdt_paging: Default::default(),
            mgt: Default::default(),
            osts: Default::default(),
            ost_paging: Default::default(),
            rows: Default::default(),
            stratagem: stratagem::Model::new(Arc::clone(fs)),
            stats: Stats::default(),
            stats_cancel: None,
            stats_url: format!(r#"/influx?db=iml_stats&q={}"#, query),
        }
    }
}

type StatsTuple = (
    String,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
);

#[derive(serde::Deserialize, Clone, Debug)]
pub struct InfluxSeries {
    values: Vec<StatsTuple>,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct InfluxResult {
    series: Option<Vec<InfluxSeries>>,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct InfluxResponse {
    results: Vec<InfluxResult>,
}

#[derive(Clone, Debug)]
pub enum Msg {
    FetchStats,
    StatsFetched(Box<fetch::ResponseDataResult<InfluxResponse>>),
    ActionDropdown(Box<action_dropdown::IdMsg>),
    AddTarget(Arc<Target<TargetConfParam>>),
    RemoveTarget(u32),
    SetTargets(Vec<Arc<Target<TargetConfParam>>>),
    OstPaging(paging::Msg),
    MdtPaging(paging::Msg),
    UpdatePaging,
    Stratagem(stratagem::Msg),
    Noop,
}

pub fn init(cache: &ArcCache, orders: &mut impl Orders<Msg, GMsg>) {
    orders.send_msg(Msg::SetTargets(cache.target.values().cloned().collect()));
    orders
        .proxy(Msg::Stratagem)
        .send_msg(stratagem::Msg::CheckStratagem)
        .send_msg(stratagem::Msg::SetStratagemConfig(
            cache.stratagem_config.values().cloned().collect(),
        ));
    orders.send_msg(Msg::FetchStats);
}

pub fn update(msg: Msg, cache: &ArcCache, model: &mut Model, orders: &mut impl Orders<Msg, GMsg>) {
    match msg {
        Msg::FetchStats => {
            model.stats_cancel = None;
            let request = seed::fetch::Request::new(model.stats_url.clone());
            orders
                .skip()
                .perform_cmd(request.fetch_json_data(|x| Msg::StatsFetched(Box::new(x))));
        }
        Msg::StatsFetched(res) => {
            match *res {
                Ok(response) => {
                    let r = response
                        .results
                        .into_iter()
                        .take(1)
                        .filter_map(|result| result.series)
                        .flatten()
                        .take(1)
                        .map(|v| v.values)
                        .flatten()
                        .next();

                    if let Some((_, bt, bf, ba, ft, ff, cc)) = r {
                        model.stats.bytes_total = bt;
                        model.stats.bytes_free = bf;
                        model.stats.bytes_avail = ba;
                        model.stats.files_total = ft;
                        model.stats.files_free = ff;
                        model.stats.clients = cc;
                    }
                }
                Err(e) => {
                    error!(e);
                    orders.skip();
                }
            }
            let (cancel, fut) = sleep_with_handle(Duration::from_secs(30), Msg::FetchStats, Msg::Noop);
            model.stats_cancel = Some(cancel);
            orders.perform_cmd(fut);
        }
        Msg::ActionDropdown(x) => {
            let action_dropdown::IdMsg(id, msg) = *x;

            if let Some(x) = model.rows.get_mut(&id) {
                action_dropdown::update(
                    action_dropdown::IdMsg(id, msg),
                    cache,
                    &mut x.dropdown,
                    &mut orders.proxy(|x| Msg::ActionDropdown(Box::new(x))),
                );
            }
        }
        Msg::RemoveTarget(id) => {
            model.mgt.retain(|x| x.id != id);
            model.mdts.retain(|x| x.id != id);
            model.osts.retain(|x| x.id != id);

            model.rows.remove(&id);

            orders.send_msg(Msg::UpdatePaging);
        }
        Msg::AddTarget(x) => {
            if !is_fs_target(model.fs.id, &x) {
                return;
            }

            let xs = match x.kind {
                TargetKind::Mgt => &mut model.mgt,
                TargetKind::Mdt => &mut model.mdts,
                TargetKind::Ost => &mut model.osts,
            };

            match xs.iter().position(|y| y.id == x.id) {
                Some(p) => {
                    xs.remove(p);
                    xs.insert(p, x);
                }
                None => {
                    model.rows.insert(
                        x.id,
                        Row {
                            dropdown: action_dropdown::Model::new(vec![x.composite_id()]),
                        },
                    );
                }
            }

            orders.send_msg(Msg::UpdatePaging);
        }
        Msg::SetTargets(xs) => {
            model.rows = xs
                .iter()
                .map(|x| {
                    (
                        x.id,
                        Row {
                            dropdown: action_dropdown::Model::new(vec![x.composite_id()]),
                        },
                    )
                })
                .collect();

            let (mgt, mut mdts, mut osts) = xs.into_iter().filter(|t| is_fs_target(model.fs.id, t)).fold(
                (vec![], vec![], vec![]),
                |(mut mgt, mut mdts, mut osts), x| {
                    match x.kind {
                        TargetKind::Mgt => mgt.push(x),
                        TargetKind::Mdt => mdts.push(x),
                        TargetKind::Ost => osts.push(x),
                    }
                    (mgt, mdts, osts)
                },
            );

            mdts.sort_by(|a, b| natord::compare(&a.name, &b.name));
            osts.sort_by(|a, b| natord::compare(&a.name, &b.name));

            model.mgt = mgt;
            model.mdts = mdts;
            model.osts = osts;

            orders.send_msg(Msg::UpdatePaging);
        }
        Msg::MdtPaging(msg) => {
            paging::update(msg, &mut model.mdt_paging, &mut orders.proxy(Msg::MdtPaging));
        }
        Msg::OstPaging(msg) => {
            paging::update(msg, &mut model.ost_paging, &mut orders.proxy(Msg::OstPaging));
        }
        Msg::UpdatePaging => {
            orders
                .proxy(Msg::MdtPaging)
                .send_msg(paging::Msg::SetTotal(model.mdts.len()));
            orders
                .proxy(Msg::OstPaging)
                .send_msg(paging::Msg::SetTotal(model.osts.len()));
        }
        Msg::Stratagem(msg) => stratagem::update(msg, cache, &mut model.stratagem, &mut orders.proxy(Msg::Stratagem)),
        Msg::Noop => {}
    }
}

fn paging_view(pager: &paging::Model) -> Node<paging::Msg> {
    div![
        class![C.flex, C.justify_end, C.py_1, C.pr_3],
        paging::limit_selection_view(pager),
        paging::page_count_view(pager),
        paging::next_prev_view(pager)
    ]
}

pub(crate) fn view(
    cache: &ArcCache,
    model: &Model,
    all_locks: &Locks,
    session: Option<&Session>,
    use_stratagem: bool,
) -> Node<Msg> {
    let stratagem_content = if use_stratagem {
        stratagem::view(&model.stratagem, all_locks).map_msg(Msg::Stratagem)
    } else {
        empty![]
    };

    div![
        details_table(cache, all_locks, model),
        stratagem_content,
        targets(
            "Management Target",
            cache,
            all_locks,
            session,
            &model.rows,
            &model.mgt[..],
            None
        ),
        targets(
            "Metadata Targets",
            cache,
            all_locks,
            session,
            &model.rows,
            &model.mdts[model.mdt_paging.range()],
            paging_view(&model.mdt_paging).map_msg(Msg::MdtPaging)
        ),
        targets(
            "Object Storage Targets",
            cache,
            all_locks,
            session,
            &model.rows,
            &model.osts[model.ost_paging.range()],
            paging_view(&model.ost_paging).map_msg(Msg::OstPaging)
        ),
    ]
}

fn details_table(cache: &ArcCache, all_locks: &Locks, model: &Model) -> Node<Msg> {
    div![
        class![C.bg_white, C.border_t, C.border_b, C.border, C.rounded_lg, C.shadow],
        div![
            class![C.flex, C.justify_between, C.px_6, C._mb_px, C.bg_gray_200],
            h3![
                class![C.py_4, C.font_normal, C.text_lg],
                format!("Filesystem {}", &model.fs.label)
            ]
        ],
        t::wrapper_view(vec![
            tr![
                t::th_left(plain!("Space Used / Total")),
                t::td_view(space_used_view(
                    model.stats.bytes_free,
                    model.stats.bytes_total,
                    model.stats.bytes_avail
                ))
            ],
            tr![
                t::th_left(plain!("Files Created / Maximum")),
                t::td_view(files_created_view(model.stats.files_free, model.stats.files_total))
            ],
            tr![
                t::th_left(plain!("State")),
                t::td_view(plain![model.fs.state.to_string()])
            ],
            tr![t::th_left(plain!("MGS")), t::td_view(mgs(&model.mgt, &model.fs)),],
            tr![
                t::th_left(plain!("Number of MGTs")),
                t::td_view(plain!(model.mdts.len().to_string()))
            ],
            tr![
                t::th_left(plain!("Number of OSTs")),
                t::td_view(plain!(model.osts.len().to_string()))
            ],
            tr![
                t::th_left(plain!["Number of Connected Clients"]),
                t::td_view(clients_view(model.stats.clients))
            ],
            tr![
                t::th_left(plain!["Status"]),
                t::td_view(status_view(cache, all_locks, &model.fs))
            ],
            tr![
                t::th_left(plain!["Client mount command"]),
                t::td_view(plain![model.fs.mount_command.to_string()])
            ],
        ]),
    ]
}

fn targets(
    title: &str,
    cache: &ArcCache,
    all_locks: &Locks,
    session: Option<&Session>,
    rows: &HashMap<u32, Row>,
    tgts: &[Arc<Target<TargetConfParam>>],
    pager: impl Into<Option<Node<Msg>>>,
) -> Node<Msg> {
    div![
        class![
            C.bg_white,
            C.border,
            C.border_b,
            C.border_t,
            C.mt_24,
            C.rounded_lg,
            C.shadow,
        ],
        div![
            class![C.flex, C.justify_between, C.px_6, C._mb_px, C.bg_gray_200],
            h3![class![C.py_4, C.font_normal, C.text_lg], title]
        ],
        table![
            class![C.table_fixed, C.w_full],
            style! {
                St::BorderSpacing => px(10),
                St::BorderCollapse => "initial"
            },
            vec![
                t::thead_view(vec![
                    t::th_left(plain!["Name"]).merge_attrs(class![C.w_32]),
                    t::th_left(plain!["Volume"]),
                    t::th_left(plain!["Primary Server"]).merge_attrs(class![C.w_48]),
                    t::th_left(plain!["Failover Server"]).merge_attrs(class![C.w_48]),
                    t::th_left(plain!["Started on"]).merge_attrs(class![C.w_48]),
                    th![class![C.w_48]]
                ]),
                tbody![tgts.iter().map(|x| match rows.get(&x.id) {
                    None => empty![],
                    Some(row) => tr![
                        t::td_view(vec![
                            a![
                                class![C.text_blue_500, C.hover__underline],
                                attrs! {At::Href => Route::Target(RouteId::from(x.id)).to_href()},
                                &x.name
                            ],
                            lock_indicator::view(all_locks, &x).merge_attrs(class![C.ml_2]),
                            alert_indicator(&cache.active_alert, &x, true, Placement::Right)
                                .merge_attrs(class![C.ml_2]),
                        ]),
                        t::td_view(resource_links::volume_link(x)),
                        t::td_view(resource_links::server_link(
                            Some(&x.primary_server),
                            &x.primary_server_name
                        )),
                        t::td_view(resource_links::server_link(
                            x.failover_servers.first(),
                            &x.failover_server_name
                        )),
                        t::td_view(resource_links::server_link(x.active_host.as_ref(), &x.active_host_name)),
                        td![
                            class![C.p_3, C.text_center],
                            action_dropdown::view(x.id, &row.dropdown, all_locks, session)
                                .map_msg(|x| Msg::ActionDropdown(Box::new(x)))
                        ]
                    ],
                })]
            ]
        ]
        .merge_attrs(class![C.p_6]),
        match pager.into() {
            Some(x) => x,
            None => empty![],
        }
    ]
}

pub(crate) fn status_view<T>(cache: &ArcCache, all_locks: &Locks, x: &Filesystem) -> Node<T> {
    span![
        class![C.whitespace_no_wrap],
        span![class![C.mx_1], lock_indicator::view(all_locks, x)],
        alert_indicator(&cache.active_alert, x, false, Placement::Top)
    ]
}

pub(crate) fn mgs<T>(xs: &[Arc<Target<TargetConfParam>>], f: &Filesystem) -> Node<T> {
    if let Some(t) = xs.iter().find(|t| t.kind == TargetKind::Mgt && f.mgt == t.resource_uri) {
        resource_links::server_link(Some(&t.primary_server), &t.primary_server_name)
    } else {
        plain!("---")
    }
}

pub(crate) fn clients_view<T>(cc: impl Into<Option<u64>>) -> Node<T> {
    plain![cc.into().map(|c| c.to_string()).unwrap_or_else(|| "---".to_string())]
}

fn is_fs_target(fs_id: u32, t: &Target<TargetConfParam>) -> bool {
    t.filesystem_id == Some(fs_id)
        || t.filesystems
            .as_ref()
            .and_then(|f| f.iter().find(|x| x.id == fs_id))
            .is_some()
}

pub(crate) fn space_used_view<T>(
    free: impl Into<Option<u64>>,
    total: impl Into<Option<u64>>,
    avail: impl Into<Option<u64>>,
) -> Node<T> {
    total
        .into()
        .and_then(|total| {
            let free = free.into()?;
            let avail = avail.into()?;
            let used = total.saturating_sub(free);
            let pct = used as f64 / total as f64;

            Some(span![
                class![C.whitespace_no_wrap],
                progress_circle::view((pct, progress_circle::used_to_color(pct)))
                    .merge_attrs(class![C.h_16, C.inline, C.mx_2]),
                nf::format_bytes(used, None),
                " / ",
                nf::format_bytes(avail, None)
            ])
        })
        .unwrap_or_else(|| plain!["---"])
}

fn files_created_view<T>(free: impl Into<Option<u64>>, total: impl Into<Option<u64>>) -> Node<T> {
    total
        .into()
        .and_then(|total| {
            let free = free.into()?;
            let used = total.saturating_sub(free);
            let pct = used as f64 / total as f64;

            Some(span![
                class![C.whitespace_no_wrap],
                progress_circle::view((pct, progress_circle::used_to_color(pct)))
                    .merge_attrs(class![C.h_16, C.inline, C.mx_2]),
                nf::format_number(used, None),
                " / ",
                nf::format_number(total, None)
            ])
        })
        .unwrap_or_else(|| plain!["---"])
}
