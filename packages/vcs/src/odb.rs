//! the git object store — [`ObjectStore`] over a sha256-mode git repo.
//!
//! ## byte contract: git's loose-object form
//!
//! the bytes this store deals in are git's canonical loose-object form
//! `<type> <len>\0<body>`. an object's git oid is `sha256` of *exactly* that
//! form, so:
//!
//! - `put(bytes)` writes the object and returns its oid,
//! - `get(id)` reconstructs the same `<type> <len>\0<body>` bytes,
//! - therefore `id == sha256(get(id))` for **every** object type.
//!
//! that whole-object self-verification (`id == git_hash(bytes)`) is what the
//! blob transport (A3) needs to reject a byzantine peer's poisoned bytes. it
//! works uniformly for blobs, trees, and commits — not just blobs — which is
//! why the contract is the loose form, not a bare body.
//!
//! objects enter the ODB either via `put` (importing a known object) or via the
//! plumbing in [`crate::objects`] (`write-tree` / `commit-tree`); both land in
//! the same store and read back through `get`.

use std::path::{Path, PathBuf};

use objstore::ObjectStore;

use crate::git::{self, Error};
use crate::object::ObjectId;

/// a content-addressed git object store bound to one repo.
pub struct GitOdb {
    repo: PathBuf,
}

impl GitOdb {
    /// bind a store to an existing git repo at `repo`.
    pub fn open(repo: impl Into<PathBuf>) -> Self {
        Self { repo: repo.into() }
    }

    /// create a fresh **sha256-mode** repo at `repo` and bind a store to it.
    /// the object format is the one-way-door commitment behind `ObjectId`'s
    /// `[u8; 32]`; `--template=` keeps init hermetic (no sample hooks; no
    /// shared-template-dir race under parallel init).
    pub fn init(repo: impl Into<PathBuf>) -> Result<Self, Error> {
        let repo = repo.into();
        std::fs::create_dir_all(&repo)?;
        git::run(
            &repo,
            &["init", "--object-format=sha256", "--template="],
            None,
        )?;
        Ok(Self { repo })
    }

    /// the repo this store operates on.
    pub fn repo(&self) -> &Path {
        &self.repo
    }
}

impl ObjectStore<ObjectId> for GitOdb {
    type Error = Error;

    /// store loose-object bytes `<type> <len>\0<body>`; returns the git oid,
    /// which is `sha256` of exactly those bytes. the declared length is ignored
    /// (git recomputes it from the body), so the contract is really `<type> ` +
    /// anything + `\0` + body.
    fn put(&self, bytes: Vec<u8>) -> Result<ObjectId, Error> {
        let (typ, body) = split_loose(&bytes).ok_or(Error::MalformedObject)?;
        let out = git::run(
            &self.repo,
            &["hash-object", "-w", "-t", typ, "--stdin"],
            Some(body),
        )?;
        git::parse_oid(&out.stdout)
    }

    /// fetch an object as loose-object bytes, or `Ok(None)` if absent. the type
    /// is detected (`cat-file -t`) so trees and commits round-trip too, not just
    /// blobs.
    fn get(&self, id: &ObjectId) -> Result<Option<Vec<u8>>, Error> {
        if !self.has(id)? {
            return Ok(None);
        }
        let hex = id.to_hex();
        let typ_out = git::run(&self.repo, &["cat-file", "-t", &hex], None)?;
        let typ = String::from_utf8_lossy(&typ_out.stdout).trim().to_string();
        let body = git::run(&self.repo, &["cat-file", &typ, &hex], None)?.stdout;
        Ok(Some(loose(&typ, &body)))
    }

    /// existence via `cat-file -e` — cheaper than reading the object, and a
    /// non-zero exit is "absent", not a fault.
    fn has(&self, id: &ObjectId) -> Result<bool, Error> {
        git::run_ok(&self.repo, &["cat-file", "-e", &id.to_hex()])
    }
}

/// split git loose-object bytes `<type> <len>\0<body>` into `(type, body)`.
/// returns `None` if there's no `<type> ` … `\0` header.
fn split_loose(bytes: &[u8]) -> Option<(&str, &[u8])> {
    let space = bytes.iter().position(|&b| b == b' ')?;
    let nul = bytes.iter().position(|&b| b == 0)?;
    if nul < space {
        return None;
    }
    let typ = std::str::from_utf8(&bytes[..space]).ok()?;
    Some((typ, &bytes[nul + 1..]))
}

/// assemble git loose-object bytes `<type> <len>\0<body>` from a type and body.
fn loose(typ: &str, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(typ.len() + 24 + body.len());
    out.extend_from_slice(typ.as_bytes());
    out.push(b' ');
    out.extend_from_slice(body.len().to_string().as_bytes());
    out.push(0);
    out.extend_from_slice(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_repo() -> GitOdb {
        // process-wide counter for collision-proof temp dirs: see cmd.rs::tmpdir
        // — {pid}-{nanos} alone can collide under parallel init on macos.
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vcs-odb-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        GitOdb::init(&dir).expect("init sha256 odb")
    }

    fn cleanup(odb: &GitOdb) {
        std::fs::remove_dir_all(odb.repo()).ok();
    }

    #[test]
    fn blob_loose_round_trips_and_is_content_addressed() {
        let odb = fresh_repo();
        let body = b"hello world\n".to_vec();
        let loose_bytes = loose("blob", &body);

        let id = odb.put(loose_bytes.clone()).expect("put");
        // 64-char sha256 oid
        assert_eq!(id.to_hex().len(), 64);
        assert!(odb.has(&id).expect("has"));
        // get reconstructs the exact loose form -> id == sha256(get(id))
        assert_eq!(odb.get(&id).expect("get"), Some(loose_bytes));
        // re-putting identical bytes yields the same oid (content-addressed)
        assert_eq!(odb.put(loose("blob", &body)).unwrap(), id);

        cleanup(&odb);
    }

    #[test]
    fn non_utf8_blob_round_trips() {
        // a binary body through the loose contract: proves the store never
        // lossily decodes object bytes to a String.
        let odb = fresh_repo();
        let body = vec![0x00u8, 0xff, 0xfe, 0x80, b'x', 0x00, 0x01];

        let id = odb.put(loose("blob", &body)).expect("put");
        let got = odb.get(&id).expect("get").expect("present");
        assert_eq!(got, loose("blob", &body));
        // the body extracted from the loose form equals the original bytes.
        assert_eq!(split_loose(&got).unwrap().1, &body[..]);

        cleanup(&odb);
    }

    #[test]
    fn tree_loose_round_trips_design_y() {
        // the load-bearing Design-Y proof for a NON-blob object: build a real
        // tree, feed its loose form to put, and get the SAME oid back. this is
        // what makes per-object self-verification work for trees/commits.
        let odb = fresh_repo();
        let repo = odb.repo().to_path_buf();
        std::fs::write(repo.join("f.txt"), b"content\n").unwrap();
        git::run(&repo, &["add", "-A"], None).unwrap();
        let tree_hex = String::from_utf8(git::run(&repo, &["write-tree"], None).unwrap().stdout)
            .unwrap()
            .trim()
            .to_string();
        let tree_id = ObjectId::from_hex(&tree_hex).unwrap();
        let tree_body = git::run(&repo, &["cat-file", "tree", &tree_hex], None)
            .unwrap()
            .stdout;

        // put the loose tree form -> identical oid, and get round-trips.
        let put_id = odb.put(loose("tree", &tree_body)).expect("put tree");
        assert_eq!(put_id, tree_id, "tree loose form is content-addressed");
        assert_eq!(odb.get(&tree_id).unwrap(), Some(loose("tree", &tree_body)));

        cleanup(&odb);
    }

    #[test]
    fn absent_get_is_none_not_error() {
        let odb = fresh_repo();
        let absent = ObjectId::from_hex(&"0".repeat(64)).unwrap();
        assert_eq!(odb.get(&absent).expect("get"), None);
        assert!(!odb.has(&absent).expect("has"));
        cleanup(&odb);
    }

    #[test]
    fn malformed_loose_bytes_rejected() {
        let odb = fresh_repo();
        // no "<type> <len>\0" header -> MalformedObject, not a git error.
        assert!(matches!(
            odb.put(b"no-nul-header".to_vec()),
            Err(Error::MalformedObject)
        ));
        cleanup(&odb);
    }
}
