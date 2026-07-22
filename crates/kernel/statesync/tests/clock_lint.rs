//! Clock-seam guard for statesync.
//!
//! statesync's serve monitor reads time through an injected clock handle, not
//! the wall clock, so its recency/expiry logic can be advanced by a controlled
//! clock. A raw `Instant::now()` / `SystemTime::now()` anywhere in the crate
//! sources bypasses that seam. This source-parsing test walks `src/` and fails
//! if the hole reopens (PR5, layer-contract-standardization).

use std::path::Path;

const BANNED: &[&str] = &["Instant::now(", "SystemTime::now("];

fn scan(dir: &Path, hits: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read statesync source dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            scan(&path, hits);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read source file");
        for (n, line) in src.lines().enumerate() {
            for pat in BANNED {
                if line.contains(pat) {
                    hits.push(format!("{}:{} {}", path.display(), n + 1, line.trim()));
                }
            }
        }
    }
}

#[test]
fn statesync_reads_clock_through_the_seam() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    scan(&root, &mut hits);
    assert!(
        hits.is_empty(),
        "raw wall-clock reads bypass the Clock seam — inject a clock handle:\n{}",
        hits.join("\n"),
    );
}
