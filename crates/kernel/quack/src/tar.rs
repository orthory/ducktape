//! The deterministic ustar codec — the tar reader/writer a `.quack` capsule is.
//!
//! [`build_tar`] writes the canonical form: directory entries derived from the
//! file paths, everything sorted by path, `mtime`/`uid`/`gid` zeroed, regular
//! files + dirs only, terminated by two zero blocks. Two builds of the same
//! files (in any insertion order) are byte-identical, so a `.quack` is
//! reproducible. [`open_tar`] parses one back into a [`Capsule`].
//!
//! The codec is hardened against malformed or hostile archives: an oversized
//! path or numeric field is a clean [`CapsuleError`] (never a panic), and
//! [`open_tar`] rejects entries with an absolute path or a `..` component
//! before any future extract-to-disk feature can trust them.

use std::collections::BTreeMap;

use crate::capsule::{Capsule, CapsuleError};

/// A tar block is 512 bytes; every header and data run is a multiple of it.
const BLOCK: usize = 512;
const TYPE_FILE: u8 = b'0';
const TYPE_DIR: u8 = b'5';

/// Parse a `.quack` (deterministic ustar) back into a capsule. Only regular
/// files are retained; directory entries are structural and dropped. Rejects
/// unsafe entry paths (absolute, or with a `..` component).
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
        if is_unsafe_path(&name) {
            return Err(CapsuleError::UnsafePath { path: name });
        }
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
/// zeroed, terminated by two zero blocks. Errors ([`CapsuleError::PathTooLong`]
/// / [`CapsuleError::FieldOverflow`]) on a path past the 100-byte ustar name
/// field or a size that overflows its header field, so a deep or huge tree
/// fails cleanly instead of panicking.
pub fn build_tar(c: &Capsule) -> Result<Vec<u8>, CapsuleError> {
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
            Entry::Dir => write_header(&mut out, name, TYPE_DIR, 0o755, 0)?,
            Entry::File(bytes) => {
                write_header(&mut out, name, TYPE_FILE, 0o644, bytes.len() as u64)?;
                out.extend_from_slice(bytes);
                let pad = (BLOCK - (bytes.len() % BLOCK)) % BLOCK;
                out.resize(out.len() + pad, 0);
            }
        }
    }
    // two zero blocks mark end-of-archive.
    out.resize(out.len() + BLOCK * 2, 0);
    Ok(out)
}

enum Entry<'a> {
    Dir,
    File(&'a [u8]),
}

/// A capsule path is safe to extract iff it is relative (no leading `/`) and has
/// no `..` component. Today capsules stay in memory, so this only guards a
/// future extract-to-disk; enforcing it at parse keeps that path trustworthy.
fn is_unsafe_path(path: &str) -> bool {
    path.starts_with('/') || path.split('/').any(|c| c == "..")
}

// ---------------------------------------------------------------------------
// ustar header codec — minimal, hand-rolled (regular files + dirs only).
// ---------------------------------------------------------------------------

fn write_header(
    out: &mut Vec<u8>,
    name: &str,
    typeflag: u8,
    mode: u32,
    size: u64,
) -> Result<(), CapsuleError> {
    let nb = name.as_bytes();
    if nb.len() > 100 {
        return Err(CapsuleError::PathTooLong {
            path: name.to_string(),
        });
    }
    let mut h = [0u8; BLOCK];
    h[..nb.len()].copy_from_slice(nb);
    put_octal(&mut h[100..108], u64::from(mode))?; // mode
    put_octal(&mut h[108..116], 0)?; // uid
    put_octal(&mut h[116..124], 0)?; // gid
    put_octal(&mut h[124..136], size)?; // size
    put_octal(&mut h[136..148], 0)?; // mtime
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
    Ok(())
}

/// Right-justified, zero-padded octal filling `field.len() - 1` digits, NUL
/// terminated — the ustar numeric-field convention. A value too wide for the
/// field is a checked [`CapsuleError::FieldOverflow`] (the 12-byte size field
/// caps at 8 GiB), never a silent truncation.
fn put_octal(field: &mut [u8], value: u64) -> Result<(), CapsuleError> {
    let width = field.len() - 1;
    let s = format!("{value:0width$o}");
    if s.len() > width {
        return Err(CapsuleError::FieldOverflow { value });
    }
    field[..s.len()].copy_from_slice(s.as_bytes());
    field[width] = 0;
    Ok(())
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
        assert_eq!(build_tar(&c).unwrap(), build_tar(&c).unwrap());
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
        assert_eq!(
            build_tar(&a).unwrap(),
            build_tar(&b).unwrap(),
            "and byte-identical tars"
        );
    }

    #[test]
    fn tar_round_trips_the_files() {
        let c = sample();
        let bytes = build_tar(&c).unwrap();
        // well-formed ustar: length is a whole number of 512-byte blocks.
        assert_eq!(bytes.len() % BLOCK, 0);
        let back = open_tar(&bytes).expect("re-open");
        assert_eq!(back, c);
    }

    #[test]
    fn round_trips_a_file_larger_than_one_block() {
        let mut c = Capsule::new();
        c.insert("big.bin", vec![7u8; BLOCK + 13]);
        let back = open_tar(&build_tar(&c).unwrap()).expect("re-open");
        assert_eq!(back, c);
    }

    #[test]
    fn open_tar_rejects_a_tampered_header() {
        let mut bytes = build_tar(&sample()).unwrap();
        bytes[0] ^= 0xff; // corrupt the first header's name byte
        assert_eq!(open_tar(&bytes), Err(CapsuleError::ChecksumMismatch));
    }

    #[test]
    fn build_tar_rejects_a_path_over_the_ustar_limit() {
        // a 104-byte path exceeds the 100-byte name field: a clean error, not a
        // panic, so a deep package tree fails cleanly.
        let long = format!("{}.bin", "a".repeat(100));
        let mut c = Capsule::new();
        c.insert(long.clone(), b"x".to_vec());
        assert_eq!(build_tar(&c), Err(CapsuleError::PathTooLong { path: long }));
    }

    #[test]
    fn rejects_unsafe_extract_paths() {
        // the sanitizer, directly.
        for bad in ["../x", "a/../b", "..", "/abs", "/etc/passwd"] {
            assert!(is_unsafe_path(bad), "{bad:?} should be unsafe");
        }
        for ok in ["a", "a/b/c", "prompts/x.md", "harness/golden.json"] {
            assert!(!is_unsafe_path(ok), "{ok:?} should be safe");
        }
        // and through open_tar on a crafted archive with a traversal path.
        let mut c = Capsule::new();
        c.insert("../evil", b"x".to_vec());
        let tar = build_tar(&c).expect("build");
        assert!(matches!(
            open_tar(&tar),
            Err(CapsuleError::UnsafePath { .. })
        ));
    }

    #[test]
    fn put_octal_rejects_a_field_overflow() {
        // the 12-byte size field holds 11 octal digits: 0o77777777777 = 2^33-1.
        let mut field = [0u8; 12];
        assert!(put_octal(&mut field, (1u64 << 33) - 1).is_ok());
        assert_eq!(
            put_octal(&mut field, 1u64 << 33),
            Err(CapsuleError::FieldOverflow { value: 1u64 << 33 })
        );
        // the same overflow surfaces through the header writer — exercised
        // directly with a synthetic size, so no multi-GiB file is allocated.
        let mut out = Vec::new();
        assert_eq!(
            write_header(&mut out, "big.bin", TYPE_FILE, 0o644, 1u64 << 33),
            Err(CapsuleError::FieldOverflow { value: 1u64 << 33 })
        );
    }
}
