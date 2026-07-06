//! the tasks module's public wire surface -- types only.
//! writes go via [`TaskMsg`]; reads via [`TaskQuery`] -> [`TaskReply`].
//! as the first built-in package-action OWNER, this surface also names the
//! action tags routed here and their payload schemas ([`CreateTaskAction`],
//! [`UpdateStatusAction`]) — the shapes a `PackageActionQuery::Probe` /
//! `PackageActionMsg::Apply` payload must decode into.

use serde::{Deserialize, Serialize};

// ---- the owned action tags (design D6) -----------------------------------

/// the open action tag for creating a task (a builtin package route).
pub const ACTION_TASKS_CREATE: &str = "tasks.create";
/// the open action tag for moving a task (a builtin package route).
pub const ACTION_TASKS_UPDATE_STATUS: &str = "tasks.update_status";

/// the [`ACTION_TASKS_CREATE`] payload schema.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CreateTaskAction {
    pub task_id: String,
    pub title: String,
}

/// the [`ACTION_TASKS_UPDATE_STATUS`] payload schema. `status` is the wire
/// name of a [`TaskStatus`]: `"open"`, `"in_progress"`, or `"done"`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpdateStatusAction {
    pub task_id: String,
    pub status: String,
}

/// the [`TaskStatus`] an [`UpdateStatusAction`]'s wire name carries.
pub fn task_status_from_wire(name: &str) -> Option<TaskStatus> {
    match name {
        "open" => Some(TaskStatus::Open),
        "in_progress" => Some(TaskStatus::InProgress),
        "done" => Some(TaskStatus::Done),
        _ => None,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    InProgress,
    Done,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskMsg {
    CreateTask { task_id: String, title: String },
    UpdateStatus { task_id: String, status: TaskStatus },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskQuery {
    List,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskReply {
    Tasks(Vec<Task>),
}

pub fn encode_msg(m: &TaskMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<TaskMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &TaskQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<TaskQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &TaskReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<TaskReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_create_action(a: &CreateTaskAction) -> Vec<u8> {
    serde_json::to_vec(a).expect("serializable")
}

pub fn decode_create_action(b: &[u8]) -> Result<CreateTaskAction, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_update_status_action(a: &UpdateStatusAction) -> Vec<u8> {
    serde_json::to_vec(a).expect("serializable")
}

pub fn decode_update_status_action(b: &[u8]) -> Result<UpdateStatusAction, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
