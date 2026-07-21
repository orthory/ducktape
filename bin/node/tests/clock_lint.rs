//! Clock-seam guard for the validator loop.
//!
//! The validator pump holds a commonware context; every wall-clock read must
//! go through its `Clock` seam (`context.current()`). A raw `Instant::now()` /
//! `SystemTime::now()` bypasses that seam, so lease/settle/timeout logic can no
//! longer be advanced by a controlled clock. This source-parsing test walks the
//! validator sources and fails if the hole reopens (PR5,
//! layer-contract-standardization).

use std::path::Path;

const BANNED: &[&str] = &["Instant::now(", "SystemTime::now("];

fn scan(dir: &Path, hits: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read validator source dir") {
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
fn validator_loop_reads_clock_through_the_seam() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/validator");
    let mut hits = Vec::new();
    scan(&root, &mut hits);
    assert!(
        hits.is_empty(),
        "raw wall-clock reads bypass the Clock seam — use `context.current()`:\n{}",
        hits.join("\n"),
    );
}
