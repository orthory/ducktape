use std::sync::Arc;

use document::Document;
use serde::Serialize;

#[derive(thiserror::Error, Debug)]
pub enum EntryError {
    #[error("EntryError: {0}")]
    Invariant(anyhow::Error),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Entry {
    /// A leaf containing a parsed `Document`. Owned inline, not behind an
    /// `Arc` — the parent `Directory`'s `Arc<Entry>` already provides
    /// structural sharing for everything below it.
    File(Document),

    /// A directory's children, paired with their basename. Each child sits
    /// behind an `Arc` so cloning a `Directory` only reallocates the
    /// immediate Vec and ref-bumps each subtree — no recursive deep copy.
    /// `Tree` relies on this for copy-on-write versioning: a write copies
    /// only the path it touches, untouched subtrees stay shared with the
    /// previous version.
    Directory(Vec<(String, Arc<Entry>)>),
}
