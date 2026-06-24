#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Init,
    Add { paths: Vec<String> },
    Commit { message: String, author: String },
    Branch { name: String },
    Checkout { reference: String },
    Merge { from: String },
    Tag { name: String, message: Option<String> },
}
