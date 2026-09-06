//! path normalization and authority (task 4): NFC-normalized absolute paths,
//! segment/depth/byte caps, and the `/home/<principal>/**` write-authority
//! rule over authenticated account, key, module, and system authority.

use unicode_normalization::UnicodeNormalization;

use crate::wire::{MAX_DEPTH, MAX_NAME_BYTES, MAX_PATH_BYTES};

/// validate a consensus path and split it into its segments. paths are strict
/// consensus data: utf-8 (given by `&str`), NFC-normalized, absolute,
/// `/`-separated, with no empty / `.` / `..` segments and no NUL bytes. this
/// only ever rejects — it never rewrites (no trimming, no collapsing). a bare
/// `/` (root) yields an empty segment list, which read queries like Ls use.
///
/// rewriting belongs on the client side of the wire, where an OS filename first
/// becomes a path: `duckfs_client::scan::duckfs_join` composes an NFD name (what
/// macOS hands back) to NFC before it ever reaches here.
pub fn canonical(path: &str) -> Result<Vec<String>, String> {
    // absolute: every path is anchored at root, so a leading '/' is required.
    if !path.starts_with('/') {
        return Err("files: path must be absolute (start with '/')".to_string());
    }
    // NFC once over the whole path: reject any string that is not already in
    // composed form so that a byte sequence maps to exactly one path.
    if path.chars().nfc().collect::<String>() != path {
        return Err("files: path is not NFC-normalized".to_string());
    }
    // a '/' can never survive the split below, but a NUL can, so guard it here
    // over the whole path (paths address entries in a real tree).
    if path.contains('\0') {
        return Err("files: path must not contain a NUL byte".to_string());
    }
    // total byte length is capped before we do any per-segment work.
    if path.len() > MAX_PATH_BYTES {
        return Err(format!(
            "files: path exceeds the {MAX_PATH_BYTES}-byte length limit"
        ));
    }
    // root has no segments; return early so the split does not see an empty tail.
    if path == "/" {
        return Ok(Vec::new());
    }
    // strip the anchoring '/', then every '/' delimits a segment. a doubled or
    // trailing slash surfaces as an empty segment and is rejected below.
    let mut segments = Vec::new();
    for segment in path[1..].split('/') {
        // empty (`//`), current (`.`), and parent (`..`) segments are illegal:
        // there is no relative or self-referential navigation in a canonical path.
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err("files: path contains an empty or dot segment".to_string());
        }
        // name cap is on encoded BYTES, so a multi-byte utf-8 name counts its
        // full utf-8 length, not its char count.
        if segment.len() > MAX_NAME_BYTES {
            return Err(format!(
                "files: segment name exceeds the {MAX_NAME_BYTES}-byte limit"
            ));
        }
        segments.push(segment.to_string());
    }
    // depth is the number of segments; the byte cap already bounds the count.
    if segments.len() > MAX_DEPTH {
        return Err(format!(
            "files: path exceeds the maximum depth of {MAX_DEPTH}"
        ));
    }
    Ok(segments)
}

/// the two STRUCTURAL namespace roots: `/home` and `/shared`.
///
/// They are not ordinary directories anybody made — [`check_authority`] refuses
/// to write either of them ("root is not writable"), and nothing materializes
/// them in the tree either, so on a fresh filesystem they exist in the rule and
/// not in the store. That asymmetry is the whole reason this predicate exists:
/// a READ of one must answer like the filesystem root does (an empty listing),
/// not `path not found`, which would tell a caller to create something the
/// authority rule forbids them from creating.
///
/// Exactly one segment. `/shared/nope` is a genuinely absent path and must
/// still say so.
pub fn is_namespace_root(segments: &[String]) -> bool {
    matches!(segments, [only] if only == "home" || only == "shared")
}

/// Decide whether authenticated `authority` may write the canonical `segments`.
/// System writes anywhere; a home uses its actor label or actual signer key, and the
/// home root itself (`/home` or `/home/<o>`) is never a writable file — only
/// paths strictly under it; `/shared/**` (≥ 2 segments) is writable by anyone;
/// everything else (including the filesystem root) is rejected. authority never
/// re-derives or mutates the path.
pub fn check_authority(authority: &crate::Authority, segments: &[String]) -> Result<(), String> {
    // system bypasses authority entirely (the path was still canonicalized).
    if matches!(authority, crate::Authority::System) {
        return Ok(());
    }
    match segments.first().map(String::as_str) {
        Some("home") => match segments.get(1) {
            // a home tree needs the owner segment AND at least one entry under
            // it: `["home", o, ..]`. the home root itself is not a file.
            Some(o) if segments.len() >= 3 => {
                if authority.owns_home(o) {
                    Ok(())
                } else {
                    Err(format!(
                        "files: actor '{}' is not the home owner '{o}'",
                        authority.actor()
                    ))
                }
            }
            // `/home` or `/home/<o>` on their own: writing the home root rejects.
            _ => Err("files: home root is not writable".to_string()),
        },
        // `/shared/**` is public, but the shared root itself is not a target.
        Some("shared") => {
            if segments.len() >= 2 {
                Ok(())
            } else {
                Err("files: shared root is not writable".to_string())
            }
        }
        // anything else (including the empty-segment filesystem root) is out.
        _ => Err("files: path is outside /home and /shared".to_string()),
    }
}
