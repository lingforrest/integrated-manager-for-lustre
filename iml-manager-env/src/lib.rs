// Copyright (c) 2020 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{
    env,
    net::{SocketAddr, ToSocketAddrs},
    path::PathBuf,
};

/// Get the environment variable or panic
fn get_var(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{} environment variable is required.", name))
}

/// Convert a given host and port to a `SocketAddr` or panic
fn to_socket_addr(host: &str, port: &str) -> SocketAddr {
    let raw_addr = format!("{}:{}", host, port);

    let mut addrs_iter = raw_addr.to_socket_addrs().unwrap_or_else(|_| {
        panic!(
            "Address not parsable to SocketAddr. host: {}, port: {}",
            host, port
        )
    });

    addrs_iter
        .next()
        .expect("Could not convert to a SocketAddr")
}

fn empty_str_to_none(x: String) -> Option<String> {
    match x.as_ref() {
        "" => None,
        _ => Some(x),
    }
}

fn string_to_bool(x: String) -> bool {
    match x.as_ref() {
        "true" => true,
        _ => false,
    }
}

/// Get anonymous read permission from the env or panic
pub fn get_allow_anonymous_read() -> bool {
    string_to_bool(get_var("ALLOW_ANONYMOUS_READ"))
}

/// Get build num from the env or panic
pub fn get_build() -> String {
    get_var("BUILD")
}

/// Get is release from the env or panic
pub fn get_is_release() -> bool {
    string_to_bool(get_var("IS_RELEASE"))
}

/// Get version from the env or panic
pub fn get_version() -> String {
    get_var("VERSION")
}

/// Get the broker URL from the env or panic
pub fn get_amqp_broker_url() -> String {
    get_var("AMQP_BROKER_URL")
}

/// Get the broker user from the env or panic
pub fn get_user() -> String {
    get_var("AMQP_BROKER_USER")
}

/// Get the broker password from the env or panic
pub fn get_password() -> String {
    get_var("AMQP_BROKER_PASSWORD")
}

/// Get the broker vhost from the env or panic
pub fn get_vhost() -> String {
    get_var("AMQP_BROKER_VHOST")
}

/// Get the broker host from the env or panic
pub fn get_host() -> String {
    get_var("AMQP_BROKER_HOST")
}

/// Get the broker port from the env or panic
pub fn get_port() -> String {
    get_var("AMQP_BROKER_PORT")
}

/// Get the IML API port from the env or panic
pub fn get_iml_api_port() -> String {
    get_var("IML_API_PORT")
}

/// Get the IML API address from the env or panic
pub fn get_iml_api_addr() -> SocketAddr {
    to_socket_addr(&get_server_host(), &get_iml_api_port())
}

/// Get the `http_agent2` port from the env or panic
pub fn get_http_agent2_port() -> String {
    get_var("HTTP_AGENT2_PORT")
}

pub fn get_http_agent2_addr() -> SocketAddr {
    to_socket_addr(&get_server_host(), &get_http_agent2_port())
}

/// Get the server host from the env or panic
pub fn get_server_host() -> String {
    get_var("PROXY_HOST")
}

/// Get the AMQP server address or panic
pub fn get_addr() -> SocketAddr {
    to_socket_addr(&get_host(), &get_port())
}

/// Get the warp drive port from the env or panic
pub fn get_warp_drive_port() -> String {
    get_var("WARP_DRIVE_PORT")
}

/// Get the warp drive address from the env or panic
pub fn get_warp_drive_addr() -> SocketAddr {
    to_socket_addr(&get_server_host(), &get_warp_drive_port())
}

/// Get the mailbox port from the env or panic
pub fn get_mailbox_port() -> String {
    get_var("MAILBOX_PORT")
}
/// Get the mailbox address from the env or panic
pub fn get_mailbox_addr() -> SocketAddr {
    to_socket_addr(&get_server_host(), &get_mailbox_port())
}

/// Get the timer port
pub fn get_timer_port() -> String {
    get_var("TIMER_PORT")
}

/// Get the timer address from the env or panic
pub fn get_timer_addr() -> SocketAddr {
    to_socket_addr(&get_server_host(), &get_timer_port())
}

/// Get the influxdb port from the env or panic
pub fn get_influxdb_port() -> String {
    get_var("INFLUXDB_PORT")
}

/// Get the influxdb address from the env or panic
pub fn get_influxdb_addr() -> SocketAddr {
    to_socket_addr(&get_server_host(), &get_influxdb_port())
}

/// Get the metrics influxdb database name
pub fn get_influxdb_metrics_db() -> String {
    get_var("INFLUXDB_IML_STATS_DB")
}

/// Get the path to the mailbox from the env or panic
pub fn get_mailbox_path() -> PathBuf {
    get_var("MAILBOX_PATH").into()
}

/// Get the api key from the env or panic
pub fn get_api_key() -> String {
    get_var("API_KEY")
}

/// Get the api user from the env or panic
pub fn get_api_user() -> String {
    get_var("API_USER")
}

pub fn get_manager_url() -> String {
    get_var("SERVER_HTTP_URL")
}

pub fn get_db_user() -> String {
    get_var("DB_USER")
}

pub fn get_db_host() -> Option<String> {
    empty_str_to_none(get_var("DB_HOST"))
}

pub fn get_db_name() -> Option<String> {
    empty_str_to_none(get_var("DB_NAME"))
}

pub fn get_db_password() -> Option<String> {
    empty_str_to_none(get_var("DB_PASSWORD"))
}

pub fn get_branding() -> String {
    get_var("BRANDING")
}

pub fn get_use_stratagem() -> bool {
    string_to_bool(get_var("USE_STRATAGEM"))
}

/// Gets a connection string from the IML env
pub fn get_db_conn_string() -> String {
    let mut xs = vec![format!("user={}", get_db_user())];

    let host = match get_db_host() {
        Some(x) => x,
        None => "/var/run/postgresql".into(),
    };

    xs.push(format!("host={}", host));

    if let Some(x) = get_db_name() {
        xs.push(format!("dbname={}", x));
    }

    if let Some(x) = get_db_password() {
        xs.push(format!("password={}", x));
    }

    xs.join(" ")
}
