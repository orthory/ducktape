//! The `.quack` capsule — a package's `path -> bytes` file set and the digest
//! checks over it.
//!
//! A capsule is just `path -> bytes` (regular files only; directories are
//! implied by paths). [`open_dir`] reads a source directory; the [`crate::tar`]
//! module reads/writes the deterministic ustar a `.quack` is. [`verify_digests`]
//! checks every manifest `hash = "sha256:..."` field against the real file
//! bytes — module artifacts, prompts, action schemas, and the golden proof.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::manifest::PackageManifest;

/// The reserved manifest path inside every capsule.
pub const MANIFEST_PATH: &str = "quack.toml";

/// The reserved harness-proof path inside a capsule: the golden script a
/// recipient replays before activation. A capsule that ships one MUST pin it
/// via the manifest's `golden` field ([`verify_digests`] enforces it), so the
/// proof the recipient runs is the one the signature commits to.
pub const GOLDEN_PATH: &str = "harness/golden.json";

/// A package's file set: `path -> raw bytes`, sorted (a `BTreeMap`) so every
/// derived form is deterministic regardless of insertion order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capsule {
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Capsule {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert (or replace) a file.
    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.files.insert(path.into(), bytes.into());
    }

    /// The raw `quack.toml` bytes, if present — the manifest's canonical form.
    pub fn manifest_bytes(&self) -> Option<&[u8]> {
        self.files.get(MANIFEST_PATH).map(Vec::as_slice)
    }
}

/// Everything that can go wrong reading, writing, or verifying a capsule.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CapsuleError {
    #[error("{0}")]
    Io(String),
    #[error("path {0:?} is not valid utf-8")]
    NonUtf8Path(String),
    #[error("truncated tar: a header or data run runs past the end of the archive")]
    Truncated,
    #[error("malformed tar header: {0}")]
    BadHeader(String),
    #[error("tar header checksum mismatch")]
    ChecksumMismatch,
    #[error("file {path:?} referenced by the manifest is missing from the capsule")]
    MissingFile { path: String },
    #[error("manifest hash field {value:?} for {path:?} is malformed (want sha256:<64 hex>)")]
    BadHashField { path: String, value: String },
    #[error("digest mismatch for {path:?}: manifest says {expected}, bytes hash to {actual}")]
    DigestMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("capsule ships a harness proof at {path:?} that the manifest does not pin")]
    UnpinnedGolden { path: String },
    #[error("capsule path {path:?} exceeds the 100-byte ustar name field")]
    PathTooLong { path: String },
    #[error("unsafe tar path {path:?}: absolute, or contains a \"..\" component")]
    UnsafePath { path: String },
    #[error("numeric field overflow: {value} does not fit its ustar header field")]
    FieldOverflow { value: u64 },
}

/// The raw sha256 of a file's bytes — the digest every `hash = "sha256:..."`
/// field commits to (same discipline as `files` chunk digests).
pub fn file_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Read a package *source directory* into a capsule: every regular file under
/// `path`, keyed by its `/`-separated path relative to `path`.
pub fn open_dir(path: &Path) -> Result<Capsule, CapsuleError> {
    let mut capsule = Capsule::new();
    walk_dir(path, path, &mut capsule.files)?;
    Ok(capsule)
}

fn walk_dir(
    root: &Path,
    dir: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), CapsuleError> {
    let entries = std::fs::read_dir(dir).map_err(|e| CapsuleError::Io(format!("{dir:?}: {e}")))?;
    for entry in entries {
        let entry = entry.map_err(|e| CapsuleError::Io(format!("{dir:?}: {e}")))?;
        let file_type = entry
            .file_type()
            .map_err(|e| CapsuleError::Io(format!("{:?}: {e}", entry.path())))?;
        let child = entry.path();
        if file_type.is_dir() {
            walk_dir(root, &child, out)?;
        } else if file_type.is_file() {
            let rel = child
                .strip_prefix(root)
                .expect("child is under root")
                .to_str()
                .ok_or_else(|| CapsuleError::NonUtf8Path(child.display().to_string()))?
                // normalize to '/' so the same source builds an identical tar
                // on Windows and unix alike.
                .replace(std::path::MAIN_SEPARATOR, "/");
            let bytes =
                std::fs::read(&child).map_err(|e| CapsuleError::Io(format!("{child:?}: {e}")))?;
            out.insert(rel, bytes);
        }
        // symlinks / devices / etc. are ignored: capsules are files + dirs only.
    }
    Ok(())
}

/// Check every manifest content digest against the real capsule bytes: each
/// module `artifact`+`hash` pair, each prompt's `hash`, each action's
/// `schema`+`schema_hash` pair, and the golden proof at [`GOLDEN_PATH`].
/// Missing files, a malformed `sha256:` field, or a mismatch all fail. Assumes
/// a [`crate::validate`]d manifest (so `schema`/`schema_hash` travel together);
/// an unpinned schema is simply not checked here, exactly as an unpinned module
/// artifact is not.
///
/// The golden proof is enforced tighter than the other files: a capsule that
/// ships one at [`GOLDEN_PATH`] MUST pin it (`golden` set), so an attacker
/// cannot inject a trivially-passing harness under an otherwise-valid
/// signature.
pub fn verify_digests(c: &Capsule, m: &PackageManifest) -> Result<(), CapsuleError> {
    for me in &m.modules {
        if let (Some(artifact), Some(hash)) = (&me.artifact, &me.hash) {
            check_digest(c, artifact, hash)?;
        }
    }
    for pe in &m.prompts {
        check_digest(c, &pe.path, &pe.hash)?;
    }
    for ae in &m.actions {
        if let (Some(schema), Some(hash)) = (&ae.schema, &ae.schema_hash) {
            check_digest(c, schema, hash)?;
        }
    }
    verify_golden(c, m.golden.as_deref())?;
    Ok(())
}

/// Enforce the golden proof invariant: the pin and the file at [`GOLDEN_PATH`]
/// must agree on presence. A pin checks the file's digest; a present-but-
/// unpinned proof is rejected; a pin with no file is a missing-file error;
/// neither present is a package with no harness proof (fine).
fn verify_golden(c: &Capsule, pin: Option<&str>) -> Result<(), CapsuleError> {
    match (pin, c.files.contains_key(GOLDEN_PATH)) {
        (Some(hash), _) => check_digest(c, GOLDEN_PATH, hash),
        (None, true) => Err(CapsuleError::UnpinnedGolden {
            path: GOLDEN_PATH.to_string(),
        }),
        (None, false) => Ok(()),
    }
}

fn check_digest(c: &Capsule, path: &str, hash_field: &str) -> Result<(), CapsuleError> {
    let bytes = c.files.get(path).ok_or_else(|| CapsuleError::MissingFile {
        path: path.to_string(),
    })?;
    let expected = parse_sha256(hash_field).ok_or_else(|| CapsuleError::BadHashField {
        path: path.to_string(),
        value: hash_field.to_string(),
    })?;
    let actual = file_digest(bytes);
    if actual != expected {
        return Err(CapsuleError::DigestMismatch {
            path: path.to_string(),
            expected: format!("sha256:{}", crate::to_hex(&expected)),
            actual: format!("sha256:{}", crate::to_hex(&actual)),
        });
    }
    Ok(())
}

/// Parse a `sha256:<64 hex>` field into its 32 raw bytes.
fn parse_sha256(field: &str) -> Option<[u8; 32]> {
    let hex = field.strip_prefix("sha256:")?;
    let bytes = crate::from_hex(hex)?;
    <[u8; 32]>::try_from(bytes.as_slice()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::parse_manifest;
    use crate::tar::{build_tar, open_tar};

    const DIGEST_MANIFEST: &str = r#"
schema = 1
package = "org.ducktape.docs"
version = "0.1.0"

[requires]
protocol_min = 1

[[prompts]]
logical = "p"
path = "prompts/a.md"
hash = "PLACEHOLDER"

[install]
register_modules = true
seed_state = true
register_agents = true
register_actions = true
wire_hooks = true
enable_jobs = true
run_harness = true

[uninstall]
remove_hooks = true
pause_agents = true
unregister_actions = true
pending_runs = "drain"
user_data = "preserve"
package_state = "tombstone"
"#;

    #[test]
    fn verify_digests_accepts_matching_bytes() {
        let body = b"hello prompt".to_vec();
        let hash = format!("sha256:{}", crate::to_hex(&file_digest(&body)));
        let toml = DIGEST_MANIFEST.replace("PLACEHOLDER", &hash);
        let m = parse_manifest(toml.as_bytes()).unwrap();

        let mut c = Capsule::new();
        c.insert("prompts/a.md", body);
        verify_digests(&c, &m).expect("digests match");
    }

    #[test]
    fn verify_digests_detects_a_tampered_file() {
        let hash = format!("sha256:{}", crate::to_hex(&file_digest(b"hello prompt")));
        let toml = DIGEST_MANIFEST.replace("PLACEHOLDER", &hash);
        let m = parse_manifest(toml.as_bytes()).unwrap();

        let mut c = Capsule::new();
        c.insert("prompts/a.md", b"tampered".to_vec());
        assert!(matches!(
            verify_digests(&c, &m),
            Err(CapsuleError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn verify_digests_flags_a_missing_file() {
        let hash = format!("sha256:{}", crate::to_hex(&file_digest(b"hello prompt")));
        let toml = DIGEST_MANIFEST.replace("PLACEHOLDER", &hash);
        let m = parse_manifest(toml.as_bytes()).unwrap();
        assert_eq!(
            verify_digests(&Capsule::new(), &m),
            Err(CapsuleError::MissingFile {
                path: "prompts/a.md".into()
            })
        );
    }

    // a manifest that pins an action's schema file and the golden proof, with
    // `SCHEMA`/`GOLDEN` placeholders the tests fill with real digests.
    const PINNED_MANIFEST: &str = r#"
schema = 1
package = "org.ducktape.docs"
version = "0.1.0"
golden = "GOLDEN"

[requires]
protocol_min = 1

[[modules]]
logical = "h"
default_id = "h"
kind = "native"

[[actions]]
tag = "x.do"
owner = "h"
schema = "actions/x.schema.json"
schema_hash = "SCHEMA"

[install]
register_modules = true
seed_state = true
register_agents = true
register_actions = true
wire_hooks = true
enable_jobs = true
run_harness = true

[uninstall]
remove_hooks = true
pause_agents = true
unregister_actions = true
pending_runs = "drain"
user_data = "preserve"
package_state = "tombstone"
"#;

    // schema + golden bytes and a capsule/manifest that pin both.
    fn pinned() -> (Capsule, PackageManifest) {
        let schema = br#"{"type":"object"}"#.to_vec();
        let golden = br#"{"schema":1,"steps":[]}"#.to_vec();
        let toml = PINNED_MANIFEST
            .replace(
                "SCHEMA",
                &format!("sha256:{}", crate::to_hex(&file_digest(&schema))),
            )
            .replace(
                "GOLDEN",
                &format!("sha256:{}", crate::to_hex(&file_digest(&golden))),
            );
        let m = parse_manifest(toml.as_bytes()).unwrap();
        crate::manifest::validate(&m).expect("pinned manifest validates");
        let mut c = Capsule::new();
        c.insert("actions/x.schema.json", schema);
        c.insert(GOLDEN_PATH, golden);
        (c, m)
    }

    #[test]
    fn verify_digests_accepts_pinned_schema_and_golden() {
        let (c, m) = pinned();
        verify_digests(&c, &m).expect("schema + golden pins match");
    }

    #[test]
    fn verify_digests_detects_a_tampered_schema() {
        let (mut c, m) = pinned();
        c.insert("actions/x.schema.json", br#"{"type":"array"}"#.to_vec());
        assert!(matches!(
            verify_digests(&c, &m),
            Err(CapsuleError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn verify_digests_detects_a_tampered_golden() {
        let (mut c, m) = pinned();
        c.insert(
            GOLDEN_PATH,
            br#"{"schema":1,"steps":[{"deliver":{}}]}"#.to_vec(),
        );
        assert!(matches!(
            verify_digests(&c, &m),
            Err(CapsuleError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn verify_digests_rejects_an_unpinned_golden() {
        // a capsule that ships a proof under a manifest that does not pin it —
        // the injected-golden-under-a-valid-signature case.
        let (c, mut m) = pinned();
        m.golden = None;
        assert_eq!(
            verify_digests(&c, &m),
            Err(CapsuleError::UnpinnedGolden {
                path: GOLDEN_PATH.into(),
            })
        );
    }

    #[test]
    fn verify_digests_flags_a_missing_golden_file() {
        let (mut c, m) = pinned();
        c.files.remove(GOLDEN_PATH);
        assert_eq!(
            verify_digests(&c, &m),
            Err(CapsuleError::MissingFile {
                path: GOLDEN_PATH.into(),
            })
        );
    }

    #[test]
    fn verify_digests_ok_with_no_golden_anywhere() {
        // no golden file and no pin: a package with no harness proof verifies.
        let (mut c, mut m) = pinned();
        c.files.remove(GOLDEN_PATH);
        m.golden = None;
        verify_digests(&c, &m).expect("a proof-less package verifies");
    }

    #[test]
    fn opens_the_on_disk_fixture_dir() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/docs");
        let c = open_dir(Path::new(dir)).expect("fixture dir opens");
        let toml = c.manifest_bytes().expect("has quack.toml");
        let m = parse_manifest(toml).expect("manifest parses");
        crate::manifest::validate(&m).expect("manifest validates");
        verify_digests(&c, &m).expect("fixture digests match");
        // and the built tar re-opens to the same file set.
        assert_eq!(open_tar(&build_tar(&c).unwrap()).unwrap(), c);
    }
}
