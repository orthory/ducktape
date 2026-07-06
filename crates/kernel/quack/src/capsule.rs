//! The `.quack` capsule — a package's files, and the deterministic tar around
//! them.
//!
//! A capsule is just `path -> bytes` (regular files only; directories are
//! implied by paths). [`open_dir`] reads a source directory, [`open_tar`] reads
//! a `.quack`, and [`build_tar`] writes the canonical form: a **deterministic
//! ustar** — entries sorted by path, `mtime = 0`, `uid`/`gid = 0`, no user
//! names, regular files + dirs only. Two builds of the same files (in any
//! insertion order) produce byte-identical output, so a `.quack` is
//! reproducible. [`verify_digests`] checks every manifest `hash = "sha256:..."`
//! field against the real file bytes.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::manifest::PackageManifest;

/// The reserved manifest path inside every capsule.
pub const MANIFEST_PATH: &str = "quack.toml";

/// A tar block is 512 bytes; every header and data run is a multiple of it.
const BLOCK: usize = 512;
const TYPE_FILE: u8 = b'0';
const TYPE_DIR: u8 = b'5';

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

/// Parse a `.quack` (deterministic ustar) back into a capsule. Only regular
/// files are retained; directory entries are structural and dropped.
pub fn open_tar(bytes: &[u8]) -> Result<Capsule, CapsuleError> {
    let mut files = BTreeMap::new();
    let mut pos = 0;
    while pos + BLOCK <= bytes.len() {
        let header = &bytes[pos..pos + BLOCK];
        if header.iter().all(|&b| b == 0) {
            break; // the archive's trailing zero blocks
        }
        verify_checksum(header)?;
        let name = read_name(header)?;
        let typeflag = header[156];
        let size = read_octal(&header[124..136])? as usize;
        pos += BLOCK;
        let data_end = pos
            .checked_add(size)
            .filter(|&end| end <= bytes.len())
            .ok_or(CapsuleError::Truncated)?;
        if typeflag == TYPE_FILE || typeflag == 0 {
            files.insert(name, bytes[pos..data_end].to_vec());
        }
        // advance past the data, rounded up to the next block boundary.
        pos += size.div_ceil(BLOCK) * BLOCK;
    }
    Ok(Capsule { files })
}

/// Write the capsule as a deterministic ustar `.quack`: directory entries
/// derived from the file paths, everything sorted by path, `mtime`/`uid`/`gid`
/// zeroed, terminated by two zero blocks. Panics only on a path longer than the
/// 100-byte ustar name field — an invariant callers control (capsule paths are
/// short relative names).
pub fn build_tar(c: &Capsule) -> Vec<u8> {
    // one sorted stream of entries: directories (with a trailing '/') sort
    // immediately before their contents, so the archive is globally
    // path-ordered and dirs are always created before the files inside them.
    let mut entries: BTreeMap<String, Entry<'_>> = BTreeMap::new();
    for path in c.files.keys() {
        let mut acc = String::new();
        let comps: Vec<&str> = path.split('/').collect();
        for comp in &comps[..comps.len().saturating_sub(1)] {
            acc.push_str(comp);
            acc.push('/');
            entries.entry(acc.clone()).or_insert(Entry::Dir);
        }
    }
    for (path, bytes) in &c.files {
        entries.insert(path.clone(), Entry::File(bytes));
    }

    let mut out = Vec::new();
    for (name, entry) in &entries {
        match entry {
            Entry::Dir => write_header(&mut out, name, TYPE_DIR, 0o755, 0),
            Entry::File(bytes) => {
                write_header(&mut out, name, TYPE_FILE, 0o644, bytes.len() as u64);
                out.extend_from_slice(bytes);
                let pad = (BLOCK - (bytes.len() % BLOCK)) % BLOCK;
                out.resize(out.len() + pad, 0);
            }
        }
    }
    // two zero blocks mark end-of-archive.
    out.resize(out.len() + BLOCK * 2, 0);
    out
}

enum Entry<'a> {
    Dir,
    File(&'a [u8]),
}

/// Check every manifest content digest against the real capsule bytes: each
/// module `artifact`+`hash` pair and each prompt's `hash`. Missing files, a
/// malformed `sha256:` field, or a mismatch all fail.
pub fn verify_digests(c: &Capsule, m: &PackageManifest) -> Result<(), CapsuleError> {
    for me in &m.modules {
        if let (Some(artifact), Some(hash)) = (&me.artifact, &me.hash) {
            check_digest(c, artifact, hash)?;
        }
    }
    for pe in &m.prompts {
        check_digest(c, &pe.path, &pe.hash)?;
    }
    Ok(())
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

// ---------------------------------------------------------------------------
// ustar header codec — minimal, hand-rolled (regular files + dirs only).
// ---------------------------------------------------------------------------

fn write_header(out: &mut Vec<u8>, name: &str, typeflag: u8, mode: u32, size: u64) {
    let nb = name.as_bytes();
    assert!(
        nb.len() <= 100,
        "capsule path {name:?} exceeds the 100-byte ustar name limit"
    );
    let mut h = [0u8; BLOCK];
    h[..nb.len()].copy_from_slice(nb);
    put_octal(&mut h[100..108], u64::from(mode)); // mode
    put_octal(&mut h[108..116], 0); // uid
    put_octal(&mut h[116..124], 0); // gid
    put_octal(&mut h[124..136], size); // size
    put_octal(&mut h[136..148], 0); // mtime
    h[156] = typeflag;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    // the checksum is computed with its own field taken as eight spaces.
    for b in &mut h[148..156] {
        *b = b' ';
    }
    let sum: u32 = h.iter().map(|&b| u32::from(b)).sum();
    // canonical ustar: six octal digits, a NUL, then a space.
    let cks = format!("{sum:06o}\0 ");
    h[148..156].copy_from_slice(cks.as_bytes());
    out.extend_from_slice(&h);
}

/// Right-justified, zero-padded octal filling `field.len() - 1` digits, NUL
/// terminated — the ustar numeric-field convention.
fn put_octal(field: &mut [u8], value: u64) {
    let width = field.len() - 1;
    let s = format!("{value:0width$o}");
    // our values (mode/size, uid/gid/mtime = 0) always fit; guard anyway.
    debug_assert!(s.len() <= width, "octal value {value} overflows tar field");
    field[..s.len()].copy_from_slice(s.as_bytes());
    field[width] = 0;
}

fn verify_checksum(header: &[u8]) -> Result<(), CapsuleError> {
    let stored = read_octal(&header[148..156])?;
    let mut sum: u32 = 0;
    for (i, &b) in header.iter().enumerate() {
        // the checksum field itself counts as eight spaces.
        sum += if (148..156).contains(&i) {
            u32::from(b' ')
        } else {
            u32::from(b)
        };
    }
    if u64::from(sum) == stored {
        Ok(())
    } else {
        Err(CapsuleError::ChecksumMismatch)
    }
}

fn read_name(header: &[u8]) -> Result<String, CapsuleError> {
    // our writer never uses the prefix field, but honor it if present.
    let name = trim_nul(&header[0..100]);
    let prefix = trim_nul(&header[345..500]);
    let raw = if prefix.is_empty() {
        name.to_vec()
    } else {
        let mut joined = prefix.to_vec();
        joined.push(b'/');
        joined.extend_from_slice(name);
        joined
    };
    let mut s =
        String::from_utf8(raw).map_err(|_| CapsuleError::BadHeader("non-utf8 path".to_string()))?;
    // directory entries carry a trailing '/'; drop it for the name key.
    if s.ends_with('/') {
        s.pop();
    }
    Ok(s)
}

fn trim_nul(field: &[u8]) -> &[u8] {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    &field[..end]
}

fn read_octal(field: &[u8]) -> Result<u64, CapsuleError> {
    let mut value: u64 = 0;
    let mut any = false;
    for &b in field {
        match b {
            b'0'..=b'7' => {
                value = value * 8 + u64::from(b - b'0');
                any = true;
            }
            b' ' | 0 => {
                if any {
                    break; // trailing padding after the digits
                }
                // leading space padding: keep scanning.
            }
            _ => {
                return Err(CapsuleError::BadHeader(
                    "non-octal numeric field".to_string(),
                ));
            }
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::parse_manifest;

    fn sample() -> Capsule {
        let mut c = Capsule::new();
        c.insert("quack.toml", b"schema = 1\n".to_vec());
        c.insert("prompts/a.md", b"hello prompt".to_vec());
        c.insert("actions/x.schema.json", b"{}".to_vec());
        c
    }

    #[test]
    fn tar_is_byte_identical_across_two_builds() {
        let c = sample();
        assert_eq!(build_tar(&c), build_tar(&c));
    }

    #[test]
    fn tar_is_independent_of_insertion_order() {
        let mut a = Capsule::new();
        a.insert("quack.toml", b"x".to_vec());
        a.insert("prompts/a.md", b"y".to_vec());
        a.insert("actions/x.json", b"z".to_vec());

        let mut b = Capsule::new();
        b.insert("actions/x.json", b"z".to_vec());
        b.insert("prompts/a.md", b"y".to_vec());
        b.insert("quack.toml", b"x".to_vec());

        assert_eq!(a, b, "same files, different insertion order, equal capsule");
        assert_eq!(build_tar(&a), build_tar(&b), "and byte-identical tars");
    }

    #[test]
    fn tar_round_trips_the_files() {
        let c = sample();
        let bytes = build_tar(&c);
        // well-formed ustar: length is a whole number of 512-byte blocks.
        assert_eq!(bytes.len() % BLOCK, 0);
        let back = open_tar(&bytes).expect("re-open");
        assert_eq!(back, c);
    }

    #[test]
    fn round_trips_a_file_larger_than_one_block() {
        let mut c = Capsule::new();
        c.insert("big.bin", vec![7u8; BLOCK + 13]);
        let back = open_tar(&build_tar(&c)).expect("re-open");
        assert_eq!(back, c);
    }

    #[test]
    fn open_tar_rejects_a_tampered_header() {
        let mut bytes = build_tar(&sample());
        bytes[0] ^= 0xff; // corrupt the first header's name byte
        assert_eq!(open_tar(&bytes), Err(CapsuleError::ChecksumMismatch));
    }

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

    #[test]
    fn opens_the_on_disk_fixture_dir() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/docs");
        let c = open_dir(Path::new(dir)).expect("fixture dir opens");
        let toml = c.manifest_bytes().expect("has quack.toml");
        let m = parse_manifest(toml).expect("manifest parses");
        crate::manifest::validate(&m).expect("manifest validates");
        verify_digests(&c, &m).expect("fixture digests match");
        // and the built tar re-opens to the same file set.
        assert_eq!(open_tar(&build_tar(&c)).unwrap(), c);
    }
}
