use serde::Serialize;

#[derive(Serialize, Clone)]
pub enum Identifier {
    Document(String),
}
