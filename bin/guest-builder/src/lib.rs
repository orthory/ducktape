//! the componentizer: a core wasm module in, a component out.
//!
//! every committed component came out of this one function at the
//! `wit-component` version this crate's manifest pins, so a rebuild on any box
//! writes the same bytes. the encoder writes the component's own sections and
//! they move between releases (0.253 and 0.258 spell `target_features`
//! differently), which makes it a build input exactly like the rust channel:
//! under another version one cdylib becomes other bytes, and every module
//! another hash. so it is pinned exactly, linked rather than found on a PATH,
//! and moved only with a `make wasm-modules` sweep committed as one change.
//!
//! `wasm-tools component new` is this same call through this same crate, and
//! `wasm-tools 1.x.y` ships with `wit-component 0.x.y`: the CLI of the matching
//! release writes identical bytes. the Makefile builds that CLI for the view
//! bundler and the guest image installs it for runs, both off the version
//! pinned here, so the number is written once —
//! `the_componentizer_version_is_written_once` holds the tree to it.

/// wrap a core module (a `wasm32-unknown-unknown` cdylib carrying its
/// component type section) as a component. the encoder's defaults are
/// `wasm-tools component new`'s: validated output, legacy names accepted,
/// realloc through the module's own allocator, no adapters.
pub fn componentize(core: &[u8]) -> Result<Vec<u8>, String> {
    wit_component::ComponentEncoder::default()
        .validate(true)
        .module(core)
        .map_err(|e| format!("reading the core module: {e:#}"))?
        .encode()
        .map_err(|e| format!("encoding the component: {e:#}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    /// the componentizer's version is written in this crate's manifest and
    /// nowhere else. a workflow, a script or a document naming the number by
    /// hand is a second pin, and a second pin drifts; the `wasm-tools` CLI is
    /// the same release, so its number is held to the same rule.
    #[test]
    fn the_componentizer_version_is_written_once() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest = fs::read_to_string(crate_root.join("Cargo.toml")).unwrap();
        let pinned = manifest
            .lines()
            .find_map(|line| line.strip_prefix("wit-component = \"="))
            .and_then(|rest| rest.split('"').next())
            .expect("the manifest pins wit-component to an exact version");
        let (_, release) = pinned.split_once('.').expect("a semver version");
        let spellings = [format!("0.{release}"), format!("1.{release}")];

        let platform_root = crate_root.join("../..");
        let tracked = Command::new("git")
            .arg("-C")
            .arg(&platform_root)
            .args(["ls-files", "-z"])
            .output()
            .unwrap();
        assert!(tracked.status.success());
        for path in tracked.stdout.split(|byte| *byte == 0) {
            let path = String::from_utf8_lossy(path);
            let is_cargo_s_own = path.ends_with("Cargo.toml") || path.ends_with(".lock");
            let is_an_artifact = path.ends_with(".wasm");
            if path.is_empty() || is_cargo_s_own || is_an_artifact {
                continue;
            }
            let Ok(text) = fs::read_to_string(platform_root.join(&*path)) else {
                continue;
            };
            for spelling in &spellings {
                assert!(
                    !text.contains(spelling),
                    "{path} names the componentizer version {spelling} by hand; the pin is bin/guest-builder/Cargo.toml"
                );
            }
        }
    }
}
