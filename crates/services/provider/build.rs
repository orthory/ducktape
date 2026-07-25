//! embed every `specs/*.toml` as a built-in capability spec, sorted by file
//! name. the glob is the point: no Rust source — including this script —
//! names an executor. adding or removing a built-in is a file add/remove in
//! `specs/`, nothing else.

use std::io::Write as _;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let specs_dir = std::path::Path::new(&manifest).join("specs");
    println!("cargo:rerun-if-changed={}", specs_dir.display());

    let mut files: Vec<_> = std::fs::read_dir(&specs_dir)
        .expect("provider-host/specs directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let out = std::path::Path::new(&out_dir).join("builtin_specs.rs");
    let mut f = std::fs::File::create(&out).expect("create builtin_specs.rs");
    // an array-literal EXPRESSION consumed by include!() in builtin_specs().
    writeln!(f, "[").expect("write");
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 spec file name");
        writeln!(f, "    (\"embedded:{name}\", include_str!({path:?})),").expect("write");
    }
    writeln!(f, "]").expect("write");
}
