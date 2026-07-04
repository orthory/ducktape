//! shared text-index helpers for materialized views: tokenization and
//! posting-list intersection. domain-agnostic — a mapper decides its posting
//! key shape; this module only agrees on the convention that every token's
//! postings share a common prefix and an identical per-target suffix (the
//! "rest"), so AND-intersection is exact-key probes, never set merges.

use std::collections::BTreeSet;

use crate::{Result, ViewReader};

/// cap on distinct tokens indexed per text (alphabetical truncation beyond
/// it) — bounds the fan-out of a pathological message/block.
pub const MAX_TOKENS_PER_TEXT: usize = 256;
/// default cap on how many postings of the first (driving) token one search
/// walks. results beyond it are silently out of reach — callers that care
/// should surface their own "narrow the query" signal.
pub const DEFAULT_POSTING_CAP: usize = 4096;

/// lowercase alphanumeric tokens of `text`, deduplicated, single-char noise
/// dropped, capped at [`MAX_TOKENS_PER_TEXT`].
pub fn tokens(text: &str) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(str::to_string)
        .collect();
    while set.len() > MAX_TOKENS_PER_TEXT {
        set.pop_last();
    }
    set
}

/// one intersection survivor: the per-target `rest` (posting key minus its
/// token prefix) and the driving token's posting value.
pub struct PostingHit {
    pub rest: String,
    pub value: Vec<u8>,
}

/// AND-intersect token postings: walk the first prefix's postings (up to
/// `cap`), keep each target only when EVERY other prefix holds the same
/// `rest` — an exact-key probe per (token, target), no posting-set
/// materialization. prefixes must therefore share `rest` semantics: same
/// target, same suffix, whatever the mapper chose it to be.
pub fn intersect(reader: &ViewReader, prefixes: &[String], cap: usize) -> Result<Vec<PostingHit>> {
    let Some((first, others)) = prefixes.split_first() else {
        return Ok(Vec::new());
    };
    let mut hits = Vec::new();
    let mut cursor: Option<String> = None;
    let mut walked = 0usize;
    'walk: loop {
        let page = reader.scan(
            first.as_bytes(),
            cursor.as_deref().map(str::as_bytes),
            crate::MAX_SCAN_LIMIT,
        )?;
        for (key, value) in &page.entries {
            if walked == cap {
                break 'walk;
            }
            walked += 1;
            let rest = String::from_utf8_lossy(&key[first.len()..]).into_owned();
            let mut everywhere = true;
            for other in others {
                if reader.get(format!("{other}{rest}").as_bytes())?.is_none() {
                    everywhere = false;
                    break;
                }
            }
            if everywhere {
                hits.push(PostingHit {
                    rest,
                    value: value.clone(),
                });
            }
        }
        match page.next_after {
            Some(next) if page.has_more => cursor = Some(next),
            _ => break,
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_lowercase_dedup_and_drop_noise() {
        let toks = tokens("Hello, hello WORLD! a b1 -- code_path");
        let want: BTreeSet<String> = ["hello", "world", "b1", "code", "path"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(toks, want);
    }

    #[test]
    fn tokens_cap_is_enforced() {
        let text: String = (0..600).map(|i| format!("tok{i:03} ")).collect();
        assert_eq!(tokens(&text).len(), MAX_TOKENS_PER_TEXT);
    }
}
