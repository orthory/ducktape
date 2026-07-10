fn main() {
    // tauri_build validates bundle.externalBin at COMPILE time, but the node
    // sidecar is another workspace crate's artifact and cargo gives no build
    // ordering between them. materialize an empty placeholder so plain
    // `cargo build` succeeds; real bundles get the fresh binary because
    // beforeBuildCommand runs scripts/prepare-sidecar.sh, which overwrites it.
    let triple = std::env::var("TARGET").expect("cargo sets TARGET");
    let executable_suffix = if triple.contains("windows") { ".exe" } else { "" };
    for binary in ["ducktape-node", "duckdnsd"] {
        let path = std::path::PathBuf::from(format!(
            "binaries/{binary}-{triple}{executable_suffix}"
        ));
        if !path.exists() {
            std::fs::create_dir_all("binaries").expect("create binaries dir");
            std::fs::write(&path, []).expect("write sidecar placeholder");
        }
    }
    tauri_build::build()
}
