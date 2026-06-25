#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Op {
    StartAgenticDev { spec_entry: uid::Uid, params: String },
    TaskAssigned { task_id: uid::Uid, node: String },
    TaskResult { task_id: uid::Uid, outcome: String },
    TaskFailed { task_id: uid::Uid, error: String },
}
