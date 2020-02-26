use chrono::offset::{TimeZone, Utc};
use futures::Future;
use number_formatter::format_bytes;
use seed::{
    fetch::{FetchObject, RequestController},
    prelude::*,
    *,
};

pub static MAX_INODE_ENTRIES: u32 = 20;

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Clone)]
pub struct INodeCount {
    timestamp: i64,
    uid: String,
    count: u32,
    size: i64,
}

#[derive(Default, Debug)]
pub struct Model {
    inodes: Vec<INodeCount>,
    destroyed: bool,
    cancel: Option<futures::channel::oneshot::Sender<()>>,
    request_controller: Option<RequestController>,
    pub last_known_scan: Option<String>,
    pub fs_name: String,
}

#[derive(Clone, Debug)]
pub enum Msg {
    FetchInodes,
    InodesFetched(FetchObject<InfluxResults>),
    OnFetchError(InodeError),
    Destroy,
    Noop,
}

#[derive(serde::Deserialize, Clone, Debug)]
struct InfluxSeries {
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    columns: Vec<String>,
    values: Vec<(i64, String, u32, i64)>,
}

#[derive(serde::Deserialize, Clone, Debug)]
struct InfluxResult {
    #[serde(skip)]
    statement_id: u16,
    series: Option<Vec<InfluxSeries>>,
}

#[derive(serde::Deserialize, Clone, Debug)]
struct InfluxResults {
    results: Vec<InfluxResult>,
}

#[derive(Debug, Clone)]
enum InodeError {
    Cancelled(futures::channel::oneshot::Canceled),
    FailedFetch(seed::fetch::FailReason<InfluxResults>),
}

impl std::fmt::Display for InodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            InodeError::Cancelled(ref err) => write!(f, "{}", err),
            InodeError::FailedFetch(ref err) => write!(f, "{:?}", err),
        }
    }
}

impl std::error::Error for InodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            InodeError::Cancelled(ref err) => Some(err),
            InodeError::FailedFetch(_) => None,
        }
    }
}

impl From<futures::channel::oneshot::Canceled> for InodeError {
    fn from(err: futures::channel::oneshot::Canceled) -> Self {
        InodeError::Cancelled(err)
    }
}

impl From<seed::fetch::FailReason<InfluxResults>> for InodeError {
    fn from(err: seed::fetch::FailReason<InfluxResults>) -> Self {
        InodeError::FailedFetch(err)
    }
}

pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    if model.destroyed {
        return;
    }

    match msg {
        Msg::FetchInodes => {
            model.cancel = None;

            let (fut, request_controller) = fetch_inodes(&model.fs_name);
            model.request_controller = request_controller;
            orders.skip().perform_cmd(fut);
        }
        Msg::InodesFetched(fetch_object) => {
            model.request_controller = None;

            match fetch_object.response() {
                Ok(response) => {
                    let mut data: InfluxResults = response.data;
                    model.inodes = data
                        .results
                        .drain(..)
                        .take(1)
                        .filter_map(|result| result.series)
                        .flatten()
                        .take(1)
                        .map(|v| v.values)
                        .flatten()
                        .map(|(timestamp, uid, count, size)| INodeCount {
                            timestamp,
                            uid,
                            count,
                            size,
                        })
                        .collect();

                    model.last_known_scan = model.inodes.first().map(|x| get_date_time(x.timestamp));
                }
                Err(fail_reason) => {
                    orders.send_msg(Msg::OnFetchError(fail_reason.into()));
                }
            }

            let sleep = iml_sleep::Sleep::new(60000)
                .map(move |_| Msg::FetchInodes)
                .map_err(|_| unreachable!());

            let (p, c) = futures::channel::oneshot::channel::<()>();

            model.cancel = Some(p);

            let c = c.map(move |_| Msg::Noop).map_err(move |_| {
                log!("Inodes poll timeout dropped");

                Msg::Noop
            });

            // Fetch the inodes after 60 seconds unless the producer is dropped before
            // it has an opportunity to fetch.
            let fut = sleep
                .select2(c)
                .map(futures::future::Either::split)
                .map(|(x, _)| x)
                .map_err(futures::future::Either::split)
                .map_err(|(x, _)| x);
            orders.perform_cmd(fut);
        }
        Msg::OnFetchError(e) => {
            error!("Fetch Error: {}", e);
            orders.skip();
        }
        Msg::Noop => {
            orders.skip();
        }
        Msg::Destroy => {
            model.cancel = None;

            if let Some(c) = model.request_controller.take() {
                c.abort()
            }

            model.destroyed = true;
            model.inodes = vec![];
        }
    }
}

pub fn fetch_inodes(fs_name: &str) -> (impl Future, Option<seed::fetch::RequestController>) {
    let mut request_controller = None;
    let url:String = format!("influx/db=iml_stratagem_scans&epoch=ns&q=SELECT%20counter_name,%20count,%20size%20FROM%20stratagem_scan%20WHERE%20group_name=%27user_distribution%27%20and%20fs_name=%27{}%27%20limit%2020", fs_name);
    let fut = seed::fetch::Request::new(url)
        .controller(|controller| request_controller = Some(controller))
        .fetch_json(Msg::InodesFetched);

    (fut, request_controller)
}

fn get_inode_elements<T>(inodes: &Vec<INodeCount>) -> Vec<Node<T>> {
    inodes
        .into_iter()
        .map(|x| {
            tr![
                td![x.uid],
                td![x.count.to_string()],
                td![format_bytes(x.size as f64, None)]
            ]
        })
        .collect()
}

fn detail_panel<T>(children: Vec<Node<T>>) -> Node<T> {
    let mut div = div!(children);
    div.add_style("display", "grid")
        .add_style("grid-template-columns", "50% 50%")
        .add_style("grid-row-gap", px(20));
    div
}

fn get_date_time(timestamp: i64) -> String {
    let dt = Utc.timestamp_nanos(timestamp);

    format!("{}", dt.format("%A, %B %d, %Y %H:%M:%S %Z"))
}

/// View
pub fn view(model: &Model) -> Node<Msg> {
    if model.destroyed {
        seed::empty()
    } else {
        let entries = get_inode_elements(&model.inodes);

        div![
            h4![class!["section-header"], "Top inode Users"],
            if !entries.is_empty() {
                let inode_table = bs_table::table(
                    Attrs::empty(),
                    vec![
                        thead![tr![th!["Name"], th!["Count"], th!["Space Used"]]],
                        tbody![entries],
                    ],
                );

                if let Some(timestamp) = &model.last_known_scan {
                    div![
                        p![class!["text-muted"], format!("Last Scanned: {}", timestamp)],
                        inode_table
                    ]
                } else {
                    div![p![class!["text-muted"], format!("No recorded scans yet.")], inode_table]
                }
            } else {
                div![detail_panel(vec![p!["No Data"]])]
            }
        ]
    }
}
