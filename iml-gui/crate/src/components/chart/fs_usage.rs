// Copyright (c) 2020 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{sleep::sleep_with_handle, GMsg};
use futures::channel::oneshot;
use seed::{prelude::*, Request, *};
use std::time::Duration;

static DB_NAME: &str = "iml_stats";

pub struct Model {
    fs_name: Option<String>,
    cancel: Option<oneshot::Sender<()>>,
    pub metric_data: Option<FsUsage>,
    pub ratio_used: f64,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            fs_name: None,
            cancel: None,
            metric_data: None,
            ratio_used: 0.0,
        }
    }
}

impl Model {
    pub fn new(fs_name: impl Into<Option<String>>) -> Self {
        Self {
            fs_name: fs_name.into(),
            ..Default::default()
        }
    }
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct InfluxSeries {
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    columns: Vec<String>,
    values: Vec<(String, u64, u64, u64)>,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct InfluxResult {
    #[serde(skip)]
    statement_id: u16,
    series: Option<Vec<InfluxSeries>>,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct InfluxResults {
    results: Vec<InfluxResult>,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct FsUsage {
    pub bytes_used: u64,
    pub bytes_avail: u64,
    pub bytes_total: u64,
}

#[derive(Clone, Debug)]
pub enum Msg {
    DataFetched(Box<seed::fetch::ResponseDataResult<InfluxResults>>),
    FetchData,
    Noop,
}

async fn fetch_metrics(db: &str, query: String) -> Result<Msg, Msg> {
    let url = format!("/influx?db={}&q={}", db, query);

    Request::new(url)
        .fetch_json_data(|x| Msg::DataFetched(Box::new(x)))
        .await
}

pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg, GMsg>) {
    match msg {
        Msg::FetchData => {
            let part = if let Some(fs_name) = &model.fs_name {
                format!(r#"AND "fs" = '{}'"#, fs_name)
            } else {
                "".into()
            };

            let query = format!(
                r#"SELECT SUM(bytes_total) as bytes_total,
 SUM(bytes_free) as bytes_free,
 SUM("bytes_avail") as bytes_avail
 FROM (
    SELECT LAST("bytes_total") AS bytes_total,
    LAST("bytes_free") as bytes_free,
    LAST("bytes_avail") as bytes_avail
    FROM "target" WHERE "kind" = 'OST' {} GROUP BY target
    )
"#,
                part
            );

            orders.skip().perform_cmd(fetch_metrics(DB_NAME, query));
        }
        Msg::DataFetched(influx_data) => {
            match *influx_data {
                Ok(influx_data) => {
                    let result: &InfluxResult = &influx_data.results[0];

                    if let Some(series) = &(*result).series {
                        let bytes_total = series[0].values[0].1;
                        let bytes_free = series[0].values[0].2;
                        let bytes_avail = series[0].values[0].3;
                        let bytes_used = bytes_total - bytes_free;

                        model.metric_data = Some(FsUsage {
                            bytes_used,
                            bytes_avail,
                            bytes_total,
                        });

                        model.ratio_used = bytes_used as f64 / bytes_total as f64;
                    }
                }
                Err(e) => {
                    error!("Failed to fetch filesystem usage metrics - {:#?}", e);
                    orders.skip();
                }
            }

            let (cancel, fut) = sleep_with_handle(Duration::from_secs(10), Msg::FetchData, Msg::Noop);

            model.cancel = Some(cancel);

            orders.perform_cmd(fut);
        }
        Msg::Noop => {}
    }
}
