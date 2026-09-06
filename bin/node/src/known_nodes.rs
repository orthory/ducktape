//! Trust-on-first-use pin: `<duck>/keys/known-nodes`, `<http-base> <hex-key>`
//! one per line, 0600 like every other file under `keys/`.
//!
//! This is the fallback half of node-identity pinning
//! ([`crate::node_http::pinned_node_key`]): a target the operator's OWN
//! registry does not serve (no `node.toml` names it) has no local authority
//! to read the key from, so the FIRST answer this CLI ever sees for that url
//! is trusted and remembered — and every answer after that must match it,
//! or the caller is refusing to sign against whatever the dialled endpoint
//! claims today. Only `--trust-node` may overwrite an existing pin (#1824).
//!
//! Every function here takes `duck: &Path` explicitly, the way
//! `keystore::wallet` does everywhere except its one env-reading entry point
//! — a test then drives it against a temp dir with no `$DUCKTAPE_HOME`
//! mutation, and no risk of a test writing into an operator's real keystore.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn path(duck: &Path) -> PathBuf {
    keystore::wallet::keys_dir(duck).join("known-nodes")
}

fn load(duck: &Path) -> Result<BTreeMap<String, String>, String> {
    let path = path(duck);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    Ok(text
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(url, hex)| (url.to_string(), hex.to_string()))
        .collect())
}

/// Born 0600, rewritten whole — the file holds at most a few hundred lines of
/// url/key pairs, never a hot path.
fn save(duck: &Path, entries: &BTreeMap<String, String>) -> Result<(), String> {
    let path = path(duck);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let body: String = entries
        .iter()
        .map(|(url, hex)| format!("{url} {hex}\n"))
        .collect();
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let mut file = opts
        .open(&path)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    std::io::Write::write_all(&mut file, body.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))
}

/// this url's pinned key, if any has been trusted yet.
pub(crate) fn pinned(duck: &Path, base: &str) -> Result<Option<Vec<u8>>, String> {
    match load(duck)?.get(base) {
        Some(hex) => Ok(Some(duckfs_core::unhex(hex)?)),
        None => Ok(None),
    }
}

/// trust `key` for `base`, overwriting whatever (if anything) was pinned.
pub(crate) fn trust(duck: &Path, base: &str, key: &[u8]) -> Result<(), String> {
    let mut entries = load(duck)?;
    entries.insert(base.to_string(), duckfs_core::to_hex(key));
    save(duck, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpinned_then_pinned_roundtrips_and_updates_0600() {
        let duck = tempfile::TempDir::new().unwrap();
        let duck = duck.path();

        assert_eq!(pinned(duck, "http://node.example:8843").unwrap(), None);
        trust(duck, "http://node.example:8843", &[0xab; 32]).unwrap();
        assert_eq!(
            pinned(duck, "http://node.example:8843").unwrap(),
            Some(vec![0xab; 32])
        );
        // a second url leaves the first alone.
        trust(duck, "http://other:8843", &[0xcd; 32]).unwrap();
        assert_eq!(
            pinned(duck, "http://node.example:8843").unwrap(),
            Some(vec![0xab; 32])
        );
        // re-trusting overwrites in place.
        trust(duck, "http://node.example:8843", &[0xef; 32]).unwrap();
        assert_eq!(
            pinned(duck, "http://node.example:8843").unwrap(),
            Some(vec![0xef; 32])
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(path(duck)).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
