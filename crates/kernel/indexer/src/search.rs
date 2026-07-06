//! shared text-index helpers for materialized views: tokenization and
//! posting-list intersection. domain-agnostic — a mapper decides its posting
//! key shape; this module only agrees on the convention that every token's
//! postings share a common prefix and an identical per-target suffix (the
//! "rest"), so AND-intersection is exact-key probes, never set merges.

use std::collections::{BTreeMap, BTreeSet};

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
/// `{key_ns}{token}/` head) and one posting value for that target.
pub struct PostingHit {
    pub rest: String,
    pub value: Vec<u8>,
}

/// the target a posting key addresses: everything after its `{key_ns}{token}/`
/// head. postings share `key_ns` (e.g. `tok/`) and store the token as the first
/// `/`-delimited segment after it, so the `rest` is the mapper's own target
/// suffix — `{channel}/{seq}`, `{doc}/{block}`, a bare `{block}`, whatever it
/// chose. the `/` after the token is found at the byte level: `/` (0x2f) is
/// never a utf-8 continuation byte, so a non-ascii token or target is safe.
fn target_rest(key: &[u8], ns_len: usize) -> Option<String> {
    let after_ns = key.get(ns_len..)?;
    let slash = after_ns.iter().position(|&b| b == b'/')?;
    Some(String::from_utf8_lossy(&after_ns[slash + 1..]).into_owned())
}

/// AND-intersect token PREFIX matches. each query token matches any indexed
/// token that STARTS WITH it (search-as-you-type: `tes` finds `testing`); a
/// target survives only when EVERY query token has some such match on it.
///
/// `key_ns` is the shared posting namespace (`tok/`); `tokens` are the query's
/// prefixes. for each token this scans `{key_ns}{token}` (up to `cap` postings)
/// and folds its postings down to the distinct set of targets, then intersects
/// those sets. a target that carries several matching words (`test`, `tester`,
/// `testing` all match `tes`) collapses to ONE hit — the map dedups by target.
/// callers apply their own scope filter (channel/doc/page) to the returned
/// values, whose stored refs name the target in full.
pub fn intersect_prefix(
    reader: &ViewReader,
    key_ns: &str,
    tokens: &[String],
    cap: usize,
) -> Result<Vec<PostingHit>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    let ns_len = key_ns.len();
    let mut acc: Option<BTreeMap<String, Vec<u8>>> = None;
    for token in tokens {
        let scan_prefix = format!("{key_ns}{token}");
        let mut targets: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut cursor: Option<String> = None;
        let mut walked = 0usize;
        'walk: loop {
            let page = reader.scan(
                scan_prefix.as_bytes(),
                cursor.as_deref().map(str::as_bytes),
                crate::MAX_SCAN_LIMIT,
            )?;
            for (key, value) in &page.entries {
                if walked == cap {
                    break 'walk;
                }
                walked += 1;
                if let Some(rest) = target_rest(key, ns_len) {
                    targets.entry(rest).or_insert_with(|| value.clone());
                }
            }
            match page.next_after {
                Some(next) if page.has_more => cursor = Some(next),
                _ => break,
            }
        }
        acc = Some(match acc {
            None => targets,
            // keep only targets present for this token too; carry the earlier
            // value (any posting of a target names it identically).
            Some(prev) => prev
                .into_iter()
                .filter(|(target, _)| targets.contains_key(target))
                .collect(),
        });
        // an empty intersection can only stay empty — stop scanning.
        if acc.as_ref().is_some_and(BTreeMap::is_empty) {
            break;
        }
    }
    Ok(acc
        .unwrap_or_default()
        .into_iter()
        .map(|(rest, value)| PostingHit { rest, value })
        .collect())
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
