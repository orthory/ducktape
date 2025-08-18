pub mod document;
mod rand;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub enum Identifier {
    Document(document::DocumentID),
}

pub trait Identifiable {
    fn identifier(&self) -> Option<Identifier>;
}
