// Copyright (c) 2019 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use serde_json;
use std::{
    cmp::{Ord, Ordering},
    collections::{BTreeMap, BTreeSet, HashMap},
    convert::TryFrom,
    fmt,
};

#[derive(Eq, PartialEq, Hash, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct PluginName(pub String);

impl From<PluginName> for String {
    fn from(PluginName(s): PluginName) -> Self {
        s
    }
}

impl From<&str> for PluginName {
    fn from(name: &str) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for PluginName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(
    Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Clone, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Fqdn(pub String);

impl From<Fqdn> for String {
    fn from(Fqdn(s): Fqdn) -> Self {
        s
    }
}

impl fmt::Display for Fqdn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Id(pub String);

impl From<&str> for Id {
    fn from(name: &str) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Seq(pub u64);

impl From<u64> for Seq {
    fn from(name: u64) -> Self {
        Self(name)
    }
}

impl Default for Seq {
    fn default() -> Self {
        Self(0)
    }
}

impl Seq {
    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

/// The payload from the agent.
/// One or many can be packed into an `Envelope`
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Message {
    Data {
        fqdn: Fqdn,
        plugin: PluginName,
        session_id: Id,
        session_seq: Seq,
        body: serde_json::Value,
    },
    SessionCreateRequest {
        fqdn: Fqdn,
        plugin: PluginName,
    },
}

/// `Envelope` of `Messages` sent to the manager.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Envelope {
    pub messages: Vec<Message>,
    pub server_boot_time: String,
    pub client_start_time: String,
}

impl Envelope {
    pub fn new(
        messages: Vec<Message>,
        client_start_time: impl Into<String>,
        server_boot_time: impl Into<String>,
    ) -> Self {
        Self {
            messages,
            server_boot_time: server_boot_time.into(),
            client_start_time: client_start_time.into(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagerMessage {
    SessionCreateResponse {
        fqdn: Fqdn,
        plugin: PluginName,
        session_id: Id,
    },
    Data {
        fqdn: Fqdn,
        plugin: PluginName,
        session_id: Id,
        body: serde_json::Value,
    },
    SessionTerminate {
        fqdn: Fqdn,
        plugin: PluginName,
        session_id: Id,
    },
    SessionTerminateAll {
        fqdn: Fqdn,
    },
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct ManagerMessages {
    pub messages: Vec<ManagerMessage>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PluginMessage {
    SessionTerminate {
        fqdn: Fqdn,
        plugin: PluginName,
        session_id: Id,
    },
    SessionCreate {
        fqdn: Fqdn,
        plugin: PluginName,
        session_id: Id,
    },
    Data {
        fqdn: Fqdn,
        plugin: PluginName,
        session_id: Id,
        session_seq: Seq,
        body: serde_json::Value,
    },
}

pub trait FlatQuery {
    fn query() -> Vec<(&'static str, &'static str)> {
        vec![("limit", "0")]
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub struct ActionName(pub String);

impl From<&str> for ActionName {
    fn from(name: &str) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for ActionName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ActionId(pub String);

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Things we can do with actions
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    ActionStart {
        action: ActionName,
        args: serde_json::value::Value,
        id: ActionId,
    },
    ActionCancel {
        id: ActionId,
    },
}

impl Action {
    pub fn get_id(&self) -> &ActionId {
        match self {
            Action::ActionStart { id, .. } | Action::ActionCancel { id, .. } => id,
        }
    }
}

impl TryFrom<Action> for serde_json::Value {
    type Error = serde_json::Error;

    fn try_from(action: Action) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(action)
    }
}

/// The result of running the action on an agent.
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct ActionResult {
    pub id: ActionId,
    pub result: Result<serde_json::value::Value, String>,
}

pub type AgentResult = std::result::Result<serde_json::Value, String>;

pub trait ToJsonValue {
    fn to_json_value(&self) -> Result<serde_json::Value, String>;
}

impl<T: serde::Serialize> ToJsonValue for T {
    fn to_json_value(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(self).map_err(|e| format!("{:?}", e))
    }
}

pub trait ToBytes {
    fn to_bytes(&self) -> Result<Vec<u8>, serde_json::error::Error>;
}

impl<T: serde::Serialize> ToBytes for T {
    fn to_bytes(&self) -> Result<Vec<u8>, serde_json::error::Error> {
        serde_json::to_vec(&self)
    }
}

pub struct CompositeId(pub u32, pub u32);

impl fmt::Display for CompositeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
    }
}

pub trait ToCompositeId {
    fn composite_id(&self) -> CompositeId;
}

pub trait Label {
    fn label(&self) -> &str;
}

pub trait EndpointName {
    fn endpoint_name() -> &'static str;
}

/// The type of lock
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LockType {
    Read,
    Write,
}

/// The Action associated with a `LockChange`
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LockAction {
    Add,
    Remove,
}

/// A change to be applied to `Locks`
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LockChange {
    pub uuid: String,
    pub job_id: u64,
    pub content_type_id: u32,
    pub item_id: u32,
    pub description: String,
    pub lock_type: LockType,
    pub action: LockAction,
}

impl ToCompositeId for LockChange {
    fn composite_id(&self) -> CompositeId {
        CompositeId(self.content_type_id, self.item_id)
    }
}

/// Meta is the metadata object returned by a fetch call
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct Meta {
    pub limit: u32,
    pub next: Option<u32>,
    pub offset: u32,
    pub previous: Option<u32>,
    pub total_count: u32,
}

/// ApiList contains the metadata and the `Vec` of objects returned by a fetch call
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct ApiList<T> {
    pub meta: Meta,
    pub objects: Vec<T>,
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone, Debug)]
pub struct ActionArgs {
    pub host_id: Option<u64>,
    pub target_id: Option<u64>,
}

// An available action from `/api/action/`
#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone, Debug)]
pub struct AvailableAction {
    pub args: Option<ActionArgs>,
    pub composite_id: String,
    pub class_name: Option<String>,
    pub confirmation: Option<String>,
    pub display_group: u64,
    pub display_order: u64,
    pub long_description: String,
    pub state: Option<String>,
    pub verb: String,
}

impl EndpointName for AvailableAction {
    fn endpoint_name() -> &'static str {
        "action"
    }
}

/// A `NtpConfiguration` record from `/api/ntp_configuration/`
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct NtpConfiguration {
    pub content_type_id: u32,
    pub id: u32,
    pub immutable_state: bool,
    pub label: String,
    pub not_deleted: Option<bool>,
    pub resource_uri: String,
    pub state: String,
    pub state_modified_at: String,
}

impl EndpointName for NtpConfiguration {
    fn endpoint_name() -> &'static str {
        "ntp_configuration"
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ClientMount {
    pub filesystem_name: String,
    pub mountpoint: Option<String>,
    pub state: String,
}

/// A Host record from `/api/host/`
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Host {
    pub address: String,
    pub boot_time: Option<String>,
    pub client_mounts: Option<Vec<ClientMount>>,
    pub content_type_id: u32,
    pub corosync_configuration: Option<String>,
    pub corosync_ring0: String,
    pub fqdn: String,
    pub id: u32,
    pub immutable_state: bool,
    pub install_method: String,
    pub label: String,
    pub lnet_configuration: String,
    pub member_of_active_filesystem: bool,
    pub needs_update: bool,
    pub nids: Option<Vec<String>>,
    pub nodename: String,
    pub pacemaker_configuration: Option<String>,
    pub private_key: Option<String>,
    pub private_key_passphrase: Option<String>,
    pub properties: String,
    pub resource_uri: String,
    pub root_pw: Option<String>,
    pub server_profile: ServerProfile,
    pub state: String,
    pub state_modified_at: String,
}

impl FlatQuery for Host {}

impl ToCompositeId for Host {
    fn composite_id(&self) -> CompositeId {
        CompositeId(self.content_type_id, self.id)
    }
}

impl Label for Host {
    fn label(&self) -> &str {
        &self.label
    }
}

impl Label for &Host {
    fn label(&self) -> &str {
        &self.label
    }
}

impl db::Id for Host {
    fn id(&self) -> u32 {
        self.id
    }
}

impl db::Id for &Host {
    fn id(&self) -> u32 {
        self.id
    }
}

impl EndpointName for Host {
    fn endpoint_name() -> &'static str {
        "host"
    }
}

/// A server profile record from api/server_profile/
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ServerProfile {
    pub corosync: bool,
    pub corosync2: bool,
    pub default: bool,
    pub initial_state: String,
    pub managed: bool,
    pub name: String,
    pub ntp: bool,
    pub pacemaker: bool,
    pub repolist: Vec<String>,
    pub resource_uri: String,
    pub ui_description: String,
    pub ui_name: String,
    pub user_selectable: bool,
    pub worker: bool,
}

impl FlatQuery for ServerProfile {}

impl EndpointName for ServerProfile {
    fn endpoint_name() -> &'static str {
        "server_profile"
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HostProfileWrapper {
    pub host_profiles: Option<HostProfile>,
    pub error: Option<String>,
    pub traceback: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostProfile {
    pub address: String,
    pub host: u32,
    pub profiles: HashMap<String, Vec<ProfileTest>>,
    pub profiles_valid: bool,
    pub resource_uri: String,
}

impl EndpointName for HostProfile {
    fn endpoint_name() -> &'static str {
        "host_profile"
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProfileTest {
    pub description: String,
    pub error: String,
    pub pass: bool,
    pub test: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Command {
    pub cancelled: bool,
    pub complete: bool,
    pub created_at: String,
    pub errored: bool,
    pub id: u32,
    pub jobs: Vec<String>,
    pub logs: String,
    pub message: String,
    pub resource_uri: String,
}

impl EndpointName for Command {
    fn endpoint_name() -> &'static str {
        "command"
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobLock {
    pub locked_item_content_type_id: u32,
    pub locked_item_id: u32,
    pub locked_item_uri: String,
    pub resource_uri: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AvailableTransition {
    pub label: String,
    pub state: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Job<T> {
    pub available_transitions: Vec<AvailableTransition>,
    pub cancelled: bool,
    pub class_name: String,
    pub commands: Vec<String>,
    pub created_at: String,
    pub description: String,
    pub errored: bool,
    pub id: u32,
    pub modified_at: String,
    pub read_locks: Vec<JobLock>,
    pub resource_uri: String,
    pub state: String,
    pub step_results: HashMap<String, T>,
    pub steps: Vec<String>,
    pub wait_for: Vec<String>,
    pub write_locks: Vec<JobLock>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Check {
    pub name: String,
    pub value: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostValididity {
    pub address: String,
    pub status: Vec<Check>,
    pub valid: bool,
}

pub type TestHostJob = Job<HostValididity>;

impl<T> EndpointName for Job<T> {
    fn endpoint_name() -> &'static str {
        "job"
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FilesystemConfParams {
    #[serde(rename = "llite.max_cached_mb")]
    pub llite_max_cached_mb: Option<String>,
    #[serde(rename = "llite.max_read_ahead_mb")]
    pub llite_max_read_ahead_mb: Option<String>,
    #[serde(rename = "llite.max_read_ahead_whole_mb")]
    pub llite_max_read_ahead_whole_mb: Option<String>,
    #[serde(rename = "llite.statahead_max")]
    pub llite_statahead_max: Option<String>,
    #[serde(rename = "sys.at_early_margin")]
    pub sys_at_early_margin: Option<String>,
    #[serde(rename = "sys.at_extra")]
    pub sys_at_extra: Option<String>,
    #[serde(rename = "sys.at_history")]
    pub sys_at_history: Option<String>,
    #[serde(rename = "sys.at_max")]
    pub sys_at_max: Option<String>,
    #[serde(rename = "sys.at_min")]
    pub sys_at_min: Option<String>,
    #[serde(rename = "sys.ldlm_timeout")]
    pub sys_ldlm_timeout: Option<String>,
    #[serde(rename = "sys.timeout")]
    pub sys_timeout: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Clone)]
pub struct MdtConfParams {
    #[serde(rename = "lov.qos_prio_free")]
    lov_qos_prio_free: Option<String>,
    #[serde(rename = "lov.qos_threshold_rr")]
    lov_qos_threshold_rr: Option<String>,
    #[serde(rename = "lov.stripecount")]
    lov_stripecount: Option<String>,
    #[serde(rename = "lov.stripesize")]
    lov_stripesize: Option<String>,
    #[serde(rename = "mdt.MDS.mds.threads_max")]
    mdt_mds_mds_threads_max: Option<String>,
    #[serde(rename = "mdt.MDS.mds.threads_min")]
    mdt_mds_mds_threads_min: Option<String>,
    #[serde(rename = "mdt.MDS.mds_readpage.threads_max")]
    mdt_mds_mds_readpage_threads_max: Option<String>,
    #[serde(rename = "mdt.MDS.mds_readpage.threads_min")]
    mdt_mds_mds_readpage_threads_min: Option<String>,
    #[serde(rename = "mdt.MDS.mds_setattr.threads_max")]
    mdt_mds_mds_setattr_threads_max: Option<String>,
    #[serde(rename = "mdt.MDS.mds_setattr.threads_min")]
    mdt_mds_mds_setattr_threads_min: Option<String>,
    #[serde(rename = "mdt.hsm_control")]
    mdt_hsm_control: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Clone)]
pub struct OstConfParams {
    #[serde(rename = "osc.active")]
    osc_active: Option<String>,
    #[serde(rename = "osc.max_dirty_mb")]
    osc_max_dirty_mb: Option<String>,
    #[serde(rename = "osc.max_pages_per_rpc")]
    osc_max_pages_per_rpc: Option<String>,
    #[serde(rename = "osc.max_rpcs_in_flight")]
    osc_max_rpcs_in_flight: Option<String>,
    #[serde(rename = "ost.OSS.ost.threads_max")]
    ost_oss_ost_threads_max: Option<String>,
    #[serde(rename = "ost.OSS.ost.threads_min")]
    ost_oss_ost_threads_min: Option<String>,
    #[serde(rename = "ost.OSS.ost_create.threads_max")]
    ost_oss_ost_create_threads_max: Option<String>,
    #[serde(rename = "ost.OSS.ost_create.threads_min")]
    ost_oss_ost_create_threads_min: Option<String>,
    #[serde(rename = "ost.OSS.ost_io.threads_max")]
    ost_oss_ost_io_threads_max: Option<String>,
    #[serde(rename = "ost.OSS.ost_io.threads_min")]
    ost_oss_ost_io_threads_min: Option<String>,
    #[serde(rename = "ost.read_cache_enable")]
    ost_read_cache_enable: Option<String>,
    #[serde(rename = "ost.readcache_max_filesize")]
    ost_readcache_max_filesize: Option<String>,
    #[serde(rename = "ost.sync_journal")]
    ost_sync_journal: Option<String>,
    #[serde(rename = "ost.sync_on_lock_cancel")]
    ost_sync_on_lock_cancel: Option<String>,
    #[serde(rename = "ost.writethrough_cache_enable")]
    ost_writethrough_cache_enable: Option<String>,
}

/// A Volume record from api/volume/
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Volume {
    pub filesystem_type: Option<String>,
    pub id: u32,
    pub kind: String,
    pub label: String,
    pub resource_uri: String,
    pub size: Option<i64>,
    pub status: Option<String>,
    pub storage_resource: Option<String>,
    pub usable_for_lustre: bool,
    pub volume_nodes: Vec<VolumeNode>,
}

impl FlatQuery for Volume {}

impl Label for Volume {
    fn label(&self) -> &str {
        &self.label
    }
}

impl Label for &Volume {
    fn label(&self) -> &str {
        &self.label
    }
}

impl db::Id for Volume {
    fn id(&self) -> u32 {
        self.id
    }
}

impl db::Id for &Volume {
    fn id(&self) -> u32 {
        self.id
    }
}

impl EndpointName for Volume {
    fn endpoint_name() -> &'static str {
        "volume"
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct VolumeNode {
    pub host: String,
    pub host_id: u32,
    pub host_label: String,
    pub id: u32,
    pub path: String,
    pub primary: bool,
    pub resource_uri: String,
    #[serde(rename = "use")]
    pub _use: bool,
    pub volume_id: u32,
}

impl FlatQuery for VolumeNode {}

impl EndpointName for VolumeNode {
    fn endpoint_name() -> &'static str {
        "volume_node"
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum TargetConfParam {
    MdtConfParam(MdtConfParams),
    OstConfParam(OstConfParams),
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum VolumeOrResourceUri {
    ResourceUri(String),
    Volume(Volume),
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TargetKind {
    Mgt,
    Mdt,
    Ost,
}

/// A Target record from /api/target/
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct Target<T> {
    pub active_host: Option<String>,
    pub active_host_name: String,
    pub conf_params: Option<T>,
    pub content_type_id: u32,
    pub failover_server_name: String,
    pub failover_servers: Vec<String>,
    pub filesystem: Option<String>,
    pub filesystem_id: Option<u32>,
    pub filesystem_name: Option<String>,
    pub filesystems: Option<Vec<FilesystemShort>>,
    pub ha_label: Option<String>,
    pub id: u32,
    pub immutable_state: bool,
    pub index: Option<u32>,
    pub inode_count: Option<u64>,
    pub inode_size: Option<u32>,
    pub kind: TargetKind,
    pub label: String,
    pub name: String,
    pub primary_server: String,
    pub primary_server_name: String,
    pub resource_uri: String,
    pub state: String,
    pub state_modified_at: String,
    pub uuid: Option<String>,
    pub volume: VolumeOrResourceUri,
    pub volume_name: String,
}

impl<T> FlatQuery for Target<T> {
    fn query() -> Vec<(&'static str, &'static str)> {
        vec![("limit", "0"), ("dehydrate__volume", "false")]
    }
}

impl<T> ToCompositeId for Target<T> {
    fn composite_id(&self) -> CompositeId {
        CompositeId(self.content_type_id, self.id)
    }
}

impl<T> Label for Target<T> {
    fn label(&self) -> &str {
        &self.label
    }
}

impl<T> Label for &Target<T> {
    fn label(&self) -> &str {
        &self.label
    }
}

impl<T> EndpointName for Target<T> {
    fn endpoint_name() -> &'static str {
        "target"
    }
}

pub type Mdt = Target<MdtConfParams>;
pub type Mgt = Target<Option<TargetConfParam>>;
pub type Ost = Target<OstConfParams>;

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone, Debug)]
pub struct HsmControlParamMdt {
    pub id: String,
    pub kind: String,
    pub resource: String,
    pub conf_params: MdtConfParams,
}

/// HsmControlParams used for hsm actions
#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone, Debug)]
pub struct HsmControlParam {
    pub long_description: String,
    pub param_key: String,
    pub param_value: String,
    pub verb: String,
    pub mdt: HsmControlParamMdt,
}

/// A Filesystem record from /api/filesystem/
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Filesystem {
    pub bytes_free: Option<f64>,
    pub bytes_total: Option<f64>,
    pub cdt_mdt: Option<String>,
    pub cdt_status: Option<String>,
    pub client_count: Option<f64>,
    pub conf_params: FilesystemConfParams,
    pub content_type_id: u32,
    pub files_free: Option<f64>,
    pub files_total: Option<f64>,
    pub hsm_control_params: Option<Vec<HsmControlParam>>,
    pub id: u32,
    pub immutable_state: bool,
    pub label: String,
    pub mdts: Vec<Mdt>,
    pub mgt: String,
    pub mount_command: String,
    pub mount_path: String,
    pub name: String,
    pub osts: Vec<String>,
    pub resource_uri: String,
    pub state: String,
    pub state_modified_at: String,
}

impl FlatQuery for Filesystem {
    fn query() -> Vec<(&'static str, &'static str)> {
        vec![("limit", "0"), ("dehydrate__mgt", "false")]
    }
}

impl ToCompositeId for Filesystem {
    fn composite_id(&self) -> CompositeId {
        CompositeId(self.content_type_id, self.id)
    }
}

impl Label for Filesystem {
    fn label(&self) -> &str {
        &self.label
    }
}

impl Label for &Filesystem {
    fn label(&self) -> &str {
        &self.label
    }
}

impl db::Id for Filesystem {
    fn id(&self) -> u32 {
        self.id
    }
}

impl db::Id for &Filesystem {
    fn id(&self) -> u32 {
        self.id
    }
}

impl EndpointName for Filesystem {
    fn endpoint_name() -> &'static str {
        "filesystem"
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FilesystemShort {
    pub id: u32,
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum AlertType {
    AlertState,
    LearnEvent,
    AlertEvent,
    SyslogEvent,
    ClientConnectEvent,
    CommandRunningAlert,
    CommandSuccessfulAlert,
    CommandCancelledAlert,
    CommandErroredAlert,
    CorosyncUnknownPeersAlert,
    CorosyncToManyPeersAlert,
    CorosyncNoPeersAlert,
    CorosyncStoppedAlert,
    StonithNotEnabledAlert,
    PacemakerStoppedAlert,
    HostContactAlert,
    HostOfflineAlert,
    HostRebootEvent,
    UpdatesAvailableAlert,
    TargetOfflineAlert,
    TargetFailoverAlert,
    TargetRecoveryAlert,
    StorageResourceOffline,
    StorageResourceAlert,
    StorageResourceLearnEvent,
    PowerControlDeviceUnavailableAlert,
    IpmiBmcUnavailableAlert,
    LNetOfflineAlert,
    LNetNidsChangedAlert,
    StratagemUnconfiguredAlert,
    NtpOutOfSyncAlert,
}

#[derive(
    serde::Serialize, serde::Deserialize, Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq,
)]
pub enum AlertSeverity {
    DEBUG,
    INFO,
    WARNING,
    ERROR,
    CRITICAL,
}

/// An Alert record from /api/alert/
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Alert {
    pub _message: Option<String>,
    pub active: Option<bool>,
    pub affected: Option<Vec<String>>,
    pub alert_item: String,
    pub alert_item_id: Option<i32>,
    pub alert_item_str: String,
    pub alert_type: String,
    pub begin: String,
    pub dismissed: bool,
    pub end: Option<String>,
    pub id: u32,
    pub lustre_pid: Option<i32>,
    pub message: String,
    pub record_type: AlertType,
    pub resource_uri: String,
    pub severity: AlertSeverity,
    pub variant: String,
}

impl FlatQuery for Alert {
    fn query() -> Vec<(&'static str, &'static str)> {
        vec![("limit", "0"), ("active", "true")]
    }
}

impl EndpointName for Alert {
    fn endpoint_name() -> &'static str {
        "alert"
    }
}

/// A `StratagemConfiguration` record from `api/stratagem_configuration`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StratagemConfiguration {
    pub content_type_id: u32,
    pub filesystem: String,
    pub id: u32,
    pub immutable_state: bool,
    pub interval: u64,
    pub label: String,
    pub not_deleted: Option<bool>,
    pub purge_duration: Option<u64>,
    pub report_duration: Option<u64>,
    pub resource_uri: String,
    pub state: String,
    pub state_modified_at: String,
}

impl EndpointName for StratagemConfiguration {
    fn endpoint_name() -> &'static str {
        "stratagem_configuration"
    }
}

pub mod db {
    use crate::{Fqdn, Label};
    use std::{collections::BTreeSet, fmt, ops::Deref, path::PathBuf};

    #[cfg(feature = "postgres-interop")]
    use bytes::BytesMut;
    #[cfg(feature = "postgres-interop")]
    use postgres_types::{to_sql_checked, FromSql, IsNull, ToSql, Type};
    #[cfg(feature = "postgres-interop")]
    use std::io;
    #[cfg(feature = "postgres-interop")]
    use tokio_postgres::Row;

    pub trait Id {
        /// Returns the `Id` (`u32`).
        fn id(&self) -> u32;
    }

    pub trait NotDeleted {
        /// Returns if the record is not deleted.
        fn not_deleted(&self) -> bool;
        /// Returns if the record is deleted.
        fn deleted(&self) -> bool {
            !self.not_deleted()
        }
    }

    fn not_deleted(x: Option<bool>) -> bool {
        x.filter(|&x| x).is_some()
    }

    /// The name of a `chroma` table
    #[derive(serde::Deserialize, Debug, PartialEq, Eq)]
    #[serde(transparent)]
    pub struct TableName<'a>(pub &'a str);

    impl fmt::Display for TableName<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    pub trait Name {
        /// Get the name of a `chroma` table
        fn table_name() -> TableName<'static>;
    }

    /// Record from the `chroma_core_managedfilesystem` table
    #[derive(serde::Deserialize, Debug)]
    pub struct FsRecord {
        id: u32,
        state_modified_at: String,
        state: String,
        immutable_state: bool,
        name: String,
        mgs_id: u32,
        mdt_next_index: u32,
        ost_next_index: u32,
        not_deleted: Option<bool>,
        content_type_id: Option<u32>,
    }

    impl Id for FsRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl NotDeleted for FsRecord {
        fn not_deleted(&self) -> bool {
            not_deleted(self.not_deleted)
        }
    }

    pub const MANAGED_FILESYSTEM_TABLE_NAME: TableName = TableName("chroma_core_managedfilesystem");

    impl Name for FsRecord {
        fn table_name() -> TableName<'static> {
            MANAGED_FILESYSTEM_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_volume` table
    #[derive(serde::Deserialize, Debug)]
    pub struct VolumeRecord {
        id: u32,
        storage_resource_id: Option<u32>,
        size: Option<u64>,
        label: String,
        filesystem_type: Option<String>,
        not_deleted: Option<bool>,
        usable_for_lustre: bool,
    }

    impl Id for VolumeRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl NotDeleted for VolumeRecord {
        fn not_deleted(&self) -> bool {
            not_deleted(self.not_deleted)
        }
    }

    pub const VOLUME_TABLE_NAME: TableName = TableName("chroma_core_volume");

    impl Name for VolumeRecord {
        fn table_name() -> TableName<'static> {
            VOLUME_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_volumenode` table
    #[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
    pub struct VolumeNodeRecord {
        pub id: u32,
        pub volume_id: u32,
        pub host_id: u32,
        pub path: String,
        pub storage_resource_id: Option<u32>,
        pub primary: bool,
        #[serde(rename = "use")]
        pub _use: bool,
        pub not_deleted: Option<bool>,
    }

    impl Id for VolumeNodeRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl Id for &VolumeNodeRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl NotDeleted for VolumeNodeRecord {
        fn not_deleted(&self) -> bool {
            not_deleted(self.not_deleted)
        }
    }

    impl Label for VolumeNodeRecord {
        fn label(&self) -> &str {
            &self.path
        }
    }

    impl Label for &VolumeNodeRecord {
        fn label(&self) -> &str {
            &self.path
        }
    }

    pub const VOLUME_NODE_TABLE_NAME: TableName = TableName("chroma_core_volumenode");

    impl Name for VolumeNodeRecord {
        fn table_name() -> TableName<'static> {
            VOLUME_NODE_TABLE_NAME
        }
    }

    #[cfg(feature = "postgres-interop")]
    impl From<Row> for VolumeNodeRecord {
        fn from(row: Row) -> Self {
            VolumeNodeRecord {
                id: row.get::<_, i32>("id") as u32,
                volume_id: row.get::<_, i32>("volume_id") as u32,
                host_id: row.get::<_, i32>("host_id") as u32,
                path: row.get("path"),
                storage_resource_id: row
                    .get::<_, Option<i32>>("storage_resource_id")
                    .map(|x| x as u32),
                primary: row.get("primary"),
                _use: row.get("use"),
                not_deleted: row.get("not_deleted"),
            }
        }
    }

    /// Record from the `chroma_core_managedtargetmount` table
    #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
    pub struct ManagedTargetMountRecord {
        pub id: u32,
        pub host_id: u32,
        pub mount_point: Option<String>,
        pub volume_node_id: u32,
        pub primary: bool,
        pub target_id: u32,
        pub not_deleted: Option<bool>,
    }

    impl Id for ManagedTargetMountRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl NotDeleted for ManagedTargetMountRecord {
        fn not_deleted(&self) -> bool {
            not_deleted(self.not_deleted)
        }
    }

    #[cfg(feature = "postgres-interop")]
    impl From<Row> for ManagedTargetMountRecord {
        fn from(row: Row) -> Self {
            ManagedTargetMountRecord {
                id: row.get::<_, i32>("id") as u32,
                host_id: row.get::<_, i32>("host_id") as u32,
                mount_point: row.get("mount_point"),
                volume_node_id: row.get::<_, i32>("volume_node_id") as u32,
                primary: row.get("primary"),
                target_id: row.get::<_, i32>("target_id") as u32,
                not_deleted: row.get("not_deleted"),
            }
        }
    }

    pub const MANAGED_TARGET_MOUNT_TABLE_NAME: TableName =
        TableName("chroma_core_managedtargetmount");

    impl Name for ManagedTargetMountRecord {
        fn table_name() -> TableName<'static> {
            MANAGED_TARGET_MOUNT_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_managedtarget` table
    #[derive(serde::Deserialize, Debug)]
    pub struct ManagedTargetRecord {
        id: u32,
        state_modified_at: String,
        state: String,
        immutable_state: bool,
        name: Option<String>,
        uuid: Option<String>,
        ha_label: Option<String>,
        volume_id: u32,
        inode_size: Option<u32>,
        bytes_per_inode: Option<u32>,
        inode_count: Option<u64>,
        reformat: bool,
        active_mount_id: Option<u32>,
        not_deleted: Option<bool>,
        content_type_id: Option<u32>,
    }

    impl Id for ManagedTargetRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl NotDeleted for ManagedTargetRecord {
        fn not_deleted(&self) -> bool {
            not_deleted(self.not_deleted)
        }
    }

    pub const MANAGED_TARGET_TABLE_NAME: TableName = TableName("chroma_core_managedtarget");

    impl Name for ManagedTargetRecord {
        fn table_name() -> TableName<'static> {
            MANAGED_TARGET_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_ostpool` table
    #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
    pub struct OstPoolRecord {
        pub id: u32,
        pub name: String,
        pub filesystem_id: u32,
        pub not_deleted: Option<bool>,
        pub content_type_id: Option<u32>,
    }

    impl Id for OstPoolRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl Id for &OstPoolRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl NotDeleted for OstPoolRecord {
        fn not_deleted(&self) -> bool {
            not_deleted(self.not_deleted)
        }
    }

    #[cfg(feature = "postgres-interop")]
    impl From<Row> for OstPoolRecord {
        fn from(row: Row) -> Self {
            OstPoolRecord {
                id: row.get::<_, i32>("id") as u32,
                name: row.get("name"),
                filesystem_id: row.get::<_, i32>("filesystem_id") as u32,
                not_deleted: row.get("not_deleted"),
                content_type_id: row
                    .get::<_, Option<i32>>("content_type_id")
                    .map(|x| x as u32),
            }
        }
    }

    pub const OSTPOOL_TABLE_NAME: TableName = TableName("chroma_core_ostpool");

    impl Name for OstPoolRecord {
        fn table_name() -> TableName<'static> {
            OSTPOOL_TABLE_NAME
        }
    }

    impl Label for OstPoolRecord {
        fn label(&self) -> &str {
            &self.name
        }
    }

    impl Label for &OstPoolRecord {
        fn label(&self) -> &str {
            &self.name
        }
    }

    /// Record from the `chroma_core_ostpool_osts` table
    #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
    pub struct OstPoolOstsRecord {
        pub id: u32,
        pub ostpool_id: u32,
        pub managedost_id: u32,
    }

    impl Id for OstPoolOstsRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    #[cfg(feature = "postgres-interop")]
    impl From<Row> for OstPoolOstsRecord {
        fn from(row: Row) -> Self {
            OstPoolOstsRecord {
                id: row.get::<_, i32>("id") as u32,
                ostpool_id: row.get::<_, i32>("ostpool_id") as u32,
                managedost_id: row.get::<_, i32>("managedost_id") as u32,
            }
        }
    }

    pub const OSTPOOL_OSTS_TABLE_NAME: TableName = TableName("chroma_core_ostpool_osts");

    impl Name for OstPoolOstsRecord {
        fn table_name() -> TableName<'static> {
            OSTPOOL_OSTS_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_managedost` table
    #[derive(serde::Deserialize, Debug)]
    pub struct ManagedOstRecord {
        managedtarget_ptr_id: u32,
        index: u32,
        filesystem_id: u32,
    }

    impl Id for ManagedOstRecord {
        fn id(&self) -> u32 {
            self.managedtarget_ptr_id
        }
    }

    impl NotDeleted for ManagedOstRecord {
        fn not_deleted(&self) -> bool {
            true
        }
    }

    pub const MANAGED_OST_TABLE_NAME: TableName = TableName("chroma_core_managedost");

    impl Name for ManagedOstRecord {
        fn table_name() -> TableName<'static> {
            MANAGED_OST_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_managedmdt` table
    #[derive(serde::Deserialize, Debug)]
    pub struct ManagedMdtRecord {
        managedtarget_ptr_id: u32,
        index: u32,
        filesystem_id: u32,
    }

    impl Id for ManagedMdtRecord {
        fn id(&self) -> u32 {
            self.managedtarget_ptr_id
        }
    }

    impl NotDeleted for ManagedMdtRecord {
        fn not_deleted(&self) -> bool {
            true
        }
    }

    pub const MANAGED_MDT_TABLE_NAME: TableName = TableName("chroma_core_managedmdt");

    impl Name for ManagedMdtRecord {
        fn table_name() -> TableName<'static> {
            MANAGED_MDT_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_managedhost` table
    #[derive(serde::Deserialize, Debug)]
    pub struct ManagedHostRecord {
        id: u32,
        state_modified_at: String,
        state: String,
        immutable_state: bool,
        not_deleted: Option<bool>,
        content_type_id: Option<u32>,
        address: String,
        fqdn: String,
        nodename: String,
        boot_time: Option<String>,
        server_profile_id: Option<String>,
        needs_update: bool,
        install_method: String,
        properties: String,
        corosync_ring0: String,
    }

    impl Id for ManagedHostRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl NotDeleted for ManagedHostRecord {
        fn not_deleted(&self) -> bool {
            not_deleted(self.not_deleted)
        }
    }

    pub const MANAGED_HOST_TABLE_NAME: TableName = TableName("chroma_core_managedhost");

    impl Name for ManagedHostRecord {
        fn table_name() -> TableName<'static> {
            MANAGED_HOST_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_alertstate` table
    #[derive(serde::Deserialize, Debug)]
    pub struct AlertStateRecord {
        id: u32,
        alert_item_type_id: Option<u32>,
        alert_item_id: Option<u32>,
        alert_type: String,
        begin: String,
        end: Option<String>,
        active: Option<bool>,
        dismissed: bool,
        severity: u32,
        record_type: String,
        variant: Option<String>,
        lustre_pid: Option<u32>,
        message: Option<String>,
    }

    impl AlertStateRecord {
        pub fn is_active(&self) -> bool {
            self.active.is_some()
        }
    }

    impl Id for AlertStateRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    pub const ALERT_STATE_TABLE_NAME: TableName = TableName("chroma_core_alertstate");

    impl Name for AlertStateRecord {
        fn table_name() -> TableName<'static> {
            ALERT_STATE_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_stratagemconfiguration` table
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct StratagemConfiguration {
        pub id: u32,
        pub filesystem_id: u32,
        pub interval: u64,
        pub report_duration: Option<u64>,
        pub purge_duration: Option<u64>,
        pub immutable_state: bool,
        pub not_deleted: Option<bool>,
        pub state: String,
    }

    #[cfg(feature = "postgres-interop")]
    impl From<Row> for StratagemConfiguration {
        fn from(row: Row) -> Self {
            StratagemConfiguration {
                id: row.get::<_, i32>("id") as u32,
                filesystem_id: row.get::<_, i32>("filesystem_id") as u32,
                interval: row.get::<_, i64>("interval") as u64,
                report_duration: row
                    .get::<_, Option<i64>>("report_duration")
                    .map(|x| x as u64),
                purge_duration: row
                    .get::<_, Option<i64>>("purge_duration")
                    .map(|x| x as u64),
                immutable_state: row.get("immutable_state"),
                not_deleted: row.get("not_deleted"),
                state: row.get("state"),
            }
        }
    }

    impl Id for StratagemConfiguration {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl NotDeleted for StratagemConfiguration {
        fn not_deleted(&self) -> bool {
            not_deleted(self.not_deleted)
        }
    }

    pub const STRATAGEM_CONFIGURATION_TABLE_NAME: TableName =
        TableName("chroma_core_stratagemconfiguration");

    impl Name for StratagemConfiguration {
        fn table_name() -> TableName<'static> {
            STRATAGEM_CONFIGURATION_TABLE_NAME
        }
    }

    /// Record from the `chroma_core_lnetconfiguration` table
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct LnetConfigurationRecord {
        pub id: u32,
        pub state: String,
        pub host_id: u32,
        pub immutable_state: bool,
        pub not_deleted: Option<bool>,
        pub content_type_id: Option<u32>,
    }

    #[cfg(feature = "postgres-interop")]
    impl From<Row> for LnetConfigurationRecord {
        fn from(row: Row) -> Self {
            LnetConfigurationRecord {
                id: row.get::<_, i32>("id") as u32,
                state: row.get("state"),
                host_id: row.get::<_, i32>("host_id") as u32,
                immutable_state: row.get("immutable_state"),
                not_deleted: row.get("not_deleted"),
                content_type_id: row
                    .get::<_, Option<i32>>("content_type_id")
                    .map(|x| x as u32),
            }
        }
    }

    impl Id for LnetConfigurationRecord {
        fn id(&self) -> u32 {
            self.id
        }
    }

    impl NotDeleted for LnetConfigurationRecord {
        fn not_deleted(&self) -> bool {
            not_deleted(self.not_deleted)
        }
    }

    pub const LNET_CONFIGURATION_TABLE_NAME: TableName = TableName("chroma_core_lnetconfiguration");

    impl Name for LnetConfigurationRecord {
        fn table_name() -> TableName<'static> {
            LNET_CONFIGURATION_TABLE_NAME
        }
    }

    #[derive(
        Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq, Ord, PartialOrd, Clone, Hash,
    )]
    pub struct DeviceId(String);

    #[cfg(feature = "postgres-interop")]
    impl ToSql for DeviceId {
        fn to_sql(
            &self,
            ty: &Type,
            w: &mut BytesMut,
        ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
            <&str as ToSql>::to_sql(&&*self.0, ty, w)
        }

        fn accepts(ty: &Type) -> bool {
            <&str as ToSql>::accepts(ty)
        }

        to_sql_checked!();
    }

    #[cfg(feature = "postgres-interop")]
    impl<'a> FromSql<'a> for DeviceId {
        fn from_sql(
            ty: &Type,
            raw: &'a [u8],
        ) -> Result<DeviceId, Box<dyn std::error::Error + Sync + Send>> {
            FromSql::from_sql(ty, raw).map(DeviceId)
        }

        fn accepts(ty: &Type) -> bool {
            <String as FromSql>::accepts(ty)
        }
    }

    impl Deref for DeviceId {
        type Target = String;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct DeviceIds(pub BTreeSet<DeviceId>);

    impl Deref for DeviceIds {
        type Target = BTreeSet<DeviceId>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[cfg(feature = "postgres-interop")]
    impl ToSql for DeviceIds {
        fn to_sql(
            &self,
            ty: &Type,
            w: &mut BytesMut,
        ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
            let xs = self.0.iter().collect::<Vec<_>>();
            <&[&DeviceId] as ToSql>::to_sql(&&*xs, ty, w)
        }

        fn accepts(ty: &Type) -> bool {
            <&[&DeviceId] as ToSql>::accepts(ty)
        }

        to_sql_checked!();
    }

    #[cfg(feature = "postgres-interop")]
    impl<'a> FromSql<'a> for DeviceIds {
        fn from_sql(
            ty: &Type,
            raw: &'a [u8],
        ) -> Result<DeviceIds, Box<dyn std::error::Error + Sync + Send>> {
            <Vec<DeviceId> as FromSql>::from_sql(ty, raw)
                .map(|xs| DeviceIds(xs.into_iter().collect()))
        }

        fn accepts(ty: &Type) -> bool {
            <Vec<DeviceId> as FromSql>::accepts(ty)
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct Size(pub u64);

    #[cfg(feature = "postgres-interop")]
    impl ToSql for Size {
        fn to_sql(
            &self,
            ty: &Type,
            w: &mut BytesMut,
        ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
            <&str as ToSql>::to_sql(&&*self.0.to_string(), ty, w)
        }

        fn accepts(ty: &Type) -> bool {
            <&str as ToSql>::accepts(ty)
        }

        to_sql_checked!();
    }

    #[cfg(feature = "postgres-interop")]
    impl<'a> FromSql<'a> for Size {
        fn from_sql(
            ty: &Type,
            raw: &'a [u8],
        ) -> Result<Size, Box<dyn std::error::Error + Sync + Send>> {
            <String as FromSql>::from_sql(ty, raw).and_then(|x| {
                x.parse::<u64>()
                    .map(Size)
                    .map_err(|e| -> Box<dyn std::error::Error + Sync + Send> { Box::new(e) })
            })
        }

        fn accepts(ty: &Type) -> bool {
            <String as FromSql>::accepts(ty)
        }
    }

    /// The current type of Devices we support
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
    pub enum DeviceType {
        ScsiDevice,
        Partition,
        MdRaid,
        Mpath,
        VolumeGroup,
        LogicalVolume,
        Zpool,
        Dataset,
    }

    impl std::fmt::Display for DeviceType {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                DeviceType::ScsiDevice => write!(f, "scsi"),
                DeviceType::Partition => write!(f, "partition"),
                DeviceType::MdRaid => write!(f, "mdraid"),
                DeviceType::Mpath => write!(f, "mpath"),
                DeviceType::VolumeGroup => write!(f, "vg"),
                DeviceType::LogicalVolume => write!(f, "lv"),
                DeviceType::Zpool => write!(f, "zpool"),
                DeviceType::Dataset => write!(f, "dataset"),
            }
        }
    }

    #[cfg(feature = "postgres-interop")]
    impl ToSql for DeviceType {
        fn to_sql(
            &self,
            ty: &Type,
            w: &mut BytesMut,
        ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
            <String as ToSql>::to_sql(&format!("{}", self), ty, w)
        }

        fn accepts(ty: &Type) -> bool {
            <String as ToSql>::accepts(ty)
        }

        to_sql_checked!();
    }

    #[cfg(feature = "postgres-interop")]
    impl<'a> FromSql<'a> for DeviceType {
        fn from_sql(
            ty: &Type,
            raw: &'a [u8],
        ) -> Result<DeviceType, Box<dyn std::error::Error + Sync + Send>> {
            FromSql::from_sql(ty, raw).and_then(|x| match x {
                "scsi" => Ok(DeviceType::ScsiDevice),
                "partition" => Ok(DeviceType::Partition),
                "mdraid" => Ok(DeviceType::MdRaid),
                "mpath" => Ok(DeviceType::Mpath),
                "vg" => Ok(DeviceType::VolumeGroup),
                "lv" => Ok(DeviceType::LogicalVolume),
                "zpool" => Ok(DeviceType::Zpool),
                "dataset" => Ok(DeviceType::Dataset),
                _ => {
                    let e: Box<dyn std::error::Error + Sync + Send> = Box::new(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Unknown DeviceType variant",
                    ));

                    Err(e)
                }
            })
        }

        fn accepts(ty: &Type) -> bool {
            <String as FromSql>::accepts(ty)
        }
    }

    /// A device (Block or Virtual).
    /// These should be unique per cluster
    #[derive(Debug, PartialEq, Eq)]
    pub struct Device {
        pub id: DeviceId,
        pub size: Size,
        pub usable_for_lustre: bool,
        pub device_type: DeviceType,
        pub parents: DeviceIds,
        pub children: DeviceIds,
    }

    pub const DEVICE_TABLE_NAME: TableName = TableName("chroma_core_device");

    impl Name for Device {
        fn table_name() -> TableName<'static> {
            DEVICE_TABLE_NAME
        }
    }

    #[cfg(feature = "postgres-interop")]
    impl From<Row> for Device {
        fn from(row: Row) -> Self {
            Device {
                id: row.get("id"),
                size: row.get("size"),
                usable_for_lustre: row.get("usable_for_lustre"),
                device_type: row.get("device_type"),
                parents: row.get("parents"),
                children: row.get("children"),
            }
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct Paths(pub BTreeSet<PathBuf>);

    impl Deref for Paths {
        type Target = BTreeSet<PathBuf>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[cfg(feature = "postgres-interop")]
    impl ToSql for Paths {
        fn to_sql(
            &self,
            ty: &Type,
            w: &mut BytesMut,
        ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
            let xs = self.iter().map(|x| x.to_string_lossy()).collect::<Vec<_>>();
            <&[std::borrow::Cow<'_, str>] as ToSql>::to_sql(&&*xs, ty, w)
        }

        fn accepts(ty: &Type) -> bool {
            <&[std::borrow::Cow<'_, str>] as ToSql>::accepts(ty)
        }

        to_sql_checked!();
    }

    #[cfg(feature = "postgres-interop")]
    impl<'a> FromSql<'a> for Paths {
        fn from_sql(
            ty: &Type,
            raw: &'a [u8],
        ) -> Result<Paths, Box<dyn std::error::Error + Sync + Send>> {
            <Vec<String> as FromSql>::from_sql(ty, raw)
                .map(|xs| Paths(xs.into_iter().map(PathBuf::from).collect()))
        }

        fn accepts(ty: &Type) -> bool {
            <Vec<String> as FromSql>::accepts(ty)
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct MountPath(pub Option<PathBuf>);

    #[cfg(feature = "postgres-interop")]
    impl ToSql for MountPath {
        fn to_sql(
            &self,
            ty: &Type,
            w: &mut BytesMut,
        ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
            <&Option<String> as ToSql>::to_sql(
                &&self.0.clone().map(|x| x.to_string_lossy().into_owned()),
                ty,
                w,
            )
        }

        fn accepts(ty: &Type) -> bool {
            <&Option<String> as ToSql>::accepts(ty)
        }

        to_sql_checked!();
    }

    #[cfg(feature = "postgres-interop")]
    impl Deref for MountPath {
        type Target = Option<PathBuf>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    /// A pointer to a `Device` present on a host.
    /// Stores mount_path and paths to reach the pointed to `Device`.
    #[derive(Debug, PartialEq, Eq)]
    pub struct DeviceHost {
        pub device_id: DeviceId,
        pub fqdn: Fqdn,
        pub local: bool,
        pub paths: Paths,
        pub mount_path: MountPath,
        pub fs_type: Option<String>,
        pub fs_label: Option<String>,
        pub fs_uuid: Option<String>,
    }

    pub const DEVICE_HOST_TABLE_NAME: TableName = TableName("chroma_core_devicehost");

    impl Name for DeviceHost {
        fn table_name() -> TableName<'static> {
            DEVICE_HOST_TABLE_NAME
        }
    }

    #[cfg(feature = "postgres-interop")]
    impl From<Row> for DeviceHost {
        fn from(row: Row) -> Self {
            DeviceHost {
                device_id: row.get("device_id"),
                fqdn: Fqdn(row.get::<_, String>("fqdn")),
                local: row.get("local"),
                paths: row.get("paths"),
                mount_path: MountPath(
                    row.get::<_, Option<String>>("mount_path")
                        .map(PathBuf::from),
                ),
                fs_type: row.get::<_, Option<String>>("fs_type"),
                fs_label: row.get::<_, Option<String>>("fs_label"),
                fs_uuid: row.get::<_, Option<String>>("fs_uuid"),
            }
        }
    }
}

pub mod time {
    use super::RunState;

    #[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub enum Synced {
        Synced,
        Unsynced,
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct State {
        pub iml_configured: bool,
        pub ntp: (RunState, Option<Synced>, Option<Offset>),
        pub chrony: (RunState, Option<Synced>, Option<Offset>),
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    pub struct Offset(String);

    impl<T: ToString> From<T> for Offset {
        fn from(s: T) -> Self {
            Offset(s.to_string())
        }
    }
}

/// Types used for component checks
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ElementState {
    pub name: String,
    pub configurable: bool,
}

#[derive(Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UnitFileState {
    Disabled,
    Enabled,
}

#[derive(Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ActiveState {
    Inactive,
    Active,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RunState {
    Stopped,
    Enabled,
    Started,
    Setup, // Enabled + Started
}

impl Default for RunState {
    fn default() -> Self {
        RunState::Stopped
    }
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ServiceState {
    Unconfigured,
    Configured(RunState),
}

impl Default for ServiceState {
    fn default() -> Self {
        ServiceState::Unconfigured
    }
}

impl fmt::Display for ServiceState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ServiceState::Unconfigured => f.pad(&format!("{:?}", self)),
            ServiceState::Configured(r) => f.pad(&format!("{:?}", r)),
        }
    }
}

/// standard:provider:ocftype (e.g. ocf:heartbeat:ZFS, or stonith:fence_ipmilan)
#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone, Debug)]
pub struct ResourceAgentType {
    pub standard: String,         // e.g. ocf, lsb, stonith, etc..
    pub provider: Option<String>, // e.g. heartbeat, lustre, chroma
    pub ocftype: String,          // e.g. Lustre, ZFS
}

impl ResourceAgentType {
    pub fn new<'a>(
        standard: impl Into<Option<&'a str>>,
        provider: impl Into<Option<&'a str>>,
        ocftype: impl Into<Option<&'a str>>,
    ) -> Self {
        ResourceAgentType {
            standard: standard.into().map(str::to_string).unwrap_or_default(),
            provider: provider.into().map(str::to_string),
            ocftype: ocftype.into().map(str::to_string).unwrap_or_default(),
        }
    }
}

impl fmt::Display for ResourceAgentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.provider {
            Some(provider) => write!(f, "{}:{}:{}", self.standard, provider, self.ocftype),
            None => write!(f, "{}:{}", self.standard, self.ocftype),
        }
    }
}

impl PartialEq<String> for ResourceAgentType {
    fn eq(&self, other: &String) -> bool {
        self.to_string() == *other
    }
}

impl PartialEq<&str> for ResourceAgentType {
    fn eq(&self, other: &&str) -> bool {
        self.to_string() == *other
    }
}

/// Information about pacemaker resource agents
#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone, Debug)]
pub struct ResourceAgentInfo {
    pub agent: ResourceAgentType,
    pub group: Option<String>,
    pub id: String,
    pub args: HashMap<String, String>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConfigState {
    Unknown,
    Default, // components is in default configuration
    IML,     // matches what IML would do
    Other,
}

impl Default for ConfigState {
    fn default() -> Self {
        ConfigState::Unknown
    }
}

impl fmt::Display for ConfigState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{:?}", self))
    }
}

#[derive(Debug, Default, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct ComponentState<T: Default> {
    pub service: ServiceState,
    pub config: ConfigState,
    pub elements: Vec<ElementState>,
    pub info: String,
    pub state: T,
}

/// An OST Pool record from `/api/ostpool/`
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OstPoolApi {
    pub id: u32,
    pub resource_uri: String,
    #[serde(flatten)]
    pub ost: OstPool,
}

impl EndpointName for OstPoolApi {
    fn endpoint_name() -> &'static str {
        "ostpool"
    }
}

impl FlatQuery for OstPoolApi {}

impl std::fmt::Display for OstPoolApi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[#{}] {}", self.id, self.ost)
    }
}

/// Type Sent between ostpool agent daemon and service
/// FS Name -> Set of OstPools
pub type FsPoolMap = BTreeMap<String, BTreeSet<OstPool>>;

#[derive(Debug, Default, Clone, Eq, serde::Serialize, serde::Deserialize)]
pub struct OstPool {
    pub name: String,
    pub filesystem: String,
    pub osts: Vec<String>,
}

impl std::fmt::Display for OstPool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}.{} [{}]",
            self.filesystem,
            self.name,
            self.osts.join(", ")
        )
    }
}

impl Ord for OstPool {
    fn cmp(&self, other: &Self) -> Ordering {
        let x = self.filesystem.cmp(&other.filesystem);
        if x != Ordering::Equal {
            return x;
        }
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for OstPool {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for OstPool {
    fn eq(&self, other: &Self) -> bool {
        self.filesystem == other.filesystem && self.name == other.name
    }
}

pub mod warp_drive {
    use crate::{
        db::{
            Id, LnetConfigurationRecord, ManagedTargetMountRecord, OstPoolOstsRecord,
            OstPoolRecord, StratagemConfiguration, VolumeNodeRecord,
        },
        Alert, Filesystem, Host, LockChange, Target, TargetConfParam, Volume,
    };
    use im::{HashMap, HashSet};
    use std::ops::Deref;

    /// The current state of locks based on data from the locks queue
    pub type Locks = HashMap<String, HashSet<LockChange>>;

    #[derive(Default, serde::Serialize, serde::Deserialize, Debug, Clone)]
    pub struct Cache {
        pub active_alert: HashMap<u32, Alert>,
        pub filesystem: HashMap<u32, Filesystem>,
        pub host: HashMap<u32, Host>,
        pub lnet_configuration: HashMap<u32, LnetConfigurationRecord>,
        pub managed_target_mount: HashMap<u32, ManagedTargetMountRecord>,
        pub ost_pool: HashMap<u32, OstPoolRecord>,
        pub ost_pool_osts: HashMap<u32, OstPoolOstsRecord>,
        pub stratagem_config: HashMap<u32, StratagemConfiguration>,
        pub target: HashMap<u32, Target<TargetConfParam>>,
        pub volume: HashMap<u32, Volume>,
        pub volume_node: HashMap<u32, VolumeNodeRecord>,
    }

    impl Cache {
        /// Removes the record from the cache
        pub fn remove_record(&mut self, x: &RecordId) -> bool {
            match x {
                RecordId::ActiveAlert(id) => self.active_alert.remove(id).is_some(),
                RecordId::Filesystem(id) => self.filesystem.remove(id).is_some(),
                RecordId::Host(id) => self.host.remove(id).is_some(),
                RecordId::LnetConfiguration(id) => self.lnet_configuration.remove(id).is_some(),
                RecordId::ManagedTargetMount(id) => self.managed_target_mount.remove(id).is_some(),
                RecordId::OstPool(id) => self.ost_pool.remove(id).is_some(),
                RecordId::OstPoolOsts(id) => self.ost_pool_osts.remove(id).is_some(),
                RecordId::StratagemConfig(id) => self.stratagem_config.remove(id).is_some(),
                RecordId::Target(id) => self.target.remove(id).is_some(),
                RecordId::Volume(id) => self.volume.remove(id).is_some(),
                RecordId::VolumeNode(id) => self.volume_node.remove(id).is_some(),
            }
        }
        /// Inserts the record into the cache
        pub fn insert_record(&mut self, x: Record) {
            match x {
                Record::ActiveAlert(x) => {
                    self.active_alert.insert(x.id, x);
                }
                Record::Filesystem(x) => {
                    self.filesystem.insert(x.id, x);
                }
                Record::Host(x) => {
                    self.host.insert(x.id, x);
                }
                Record::LnetConfiguration(x) => {
                    self.lnet_configuration.insert(x.id(), x);
                }
                Record::ManagedTargetMount(x) => {
                    self.managed_target_mount.insert(x.id(), x);
                }
                Record::OstPool(x) => {
                    self.ost_pool.insert(x.id(), x);
                }
                Record::OstPoolOsts(x) => {
                    self.ost_pool_osts.insert(x.id(), x);
                }
                Record::StratagemConfig(x) => {
                    self.stratagem_config.insert(x.id(), x);
                }
                Record::Target(x) => {
                    self.target.insert(x.id, x);
                }
                Record::Volume(x) => {
                    self.volume.insert(x.id, x);
                }
                Record::VolumeNode(x) => {
                    self.volume_node.insert(x.id(), x);
                }
            }
        }
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
    #[serde(tag = "tag", content = "payload")]
    pub enum Record {
        ActiveAlert(Alert),
        Filesystem(Filesystem),
        Host(Host),
        ManagedTargetMount(ManagedTargetMountRecord),
        OstPool(OstPoolRecord),
        OstPoolOsts(OstPoolOstsRecord),
        StratagemConfig(StratagemConfiguration),
        Target(Target<TargetConfParam>),
        Volume(Volume),
        VolumeNode(VolumeNodeRecord),
        LnetConfiguration(LnetConfigurationRecord),
    }

    #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(tag = "tag", content = "payload")]
    pub enum RecordId {
        ActiveAlert(u32),
        Filesystem(u32),
        Host(u32),
        ManagedTargetMount(u32),
        OstPool(u32),
        OstPoolOsts(u32),
        StratagemConfig(u32),
        Target(u32),
        Volume(u32),
        VolumeNode(u32),
        LnetConfiguration(u32),
    }

    impl Deref for RecordId {
        type Target = u32;

        fn deref(&self) -> &u32 {
            match self {
                Self::ActiveAlert(x)
                | Self::Filesystem(x)
                | Self::Host(x)
                | Self::ManagedTargetMount(x)
                | Self::OstPool(x)
                | Self::OstPoolOsts(x)
                | Self::StratagemConfig(x)
                | Self::Target(x)
                | Self::Volume(x)
                | Self::VolumeNode(x)
                | Self::LnetConfiguration(x) => x,
            }
        }
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
    #[serde(tag = "tag", content = "payload")]
    pub enum RecordChange {
        Update(Record),
        Delete(RecordId),
    }

    /// Message variants.
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    #[serde(tag = "tag", content = "payload")]
    pub enum Message {
        Locks(Locks),
        Records(Cache),
        RecordChange(RecordChange),
    }
}
