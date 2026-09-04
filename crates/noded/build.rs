//! Stamp this build's identity into the binary, as a DIAGNOSTIC.
//!
//! `noded::services::build_identity` reads it back so `ducktape service status`
//! can print a daemon's build beside the node's own and make ordinary dev-loop
//! skew visible. Nothing is refused for it — see that function for why build
//! equality is not an admission rule.
//!
//! The package version cannot carry this: the repo pins version numbering at v1
//! permanently, so `CARGO_PKG_VERSION` is a constant and every pair of builds
//! would compare equal. The commit — plus a working-tree digest, since a dirty
//! build is not the commit it sits on — is the identity that actually moves.
//!
//! Git absent (a source tarball, a vendored build, Docker without `.git`) is
//! NOT an error and NOT a fallback to anything: the env var is simply left
//! unset, `build_identity()` is `None`, and the node reports its build as
//! `unknown` while serving every service plane normally.
//!
//! And stage the FOUNDING SET beside the binaries this library links into.
//!
//! A ducktape binary embeds no wasm (`AGENTS.md`, "No Embedded Wasm"): a
//! network's wasm is its genesis, and the one place bare wasm files are read
//! is the founding set `node init` composes a genesis from (and the daemons
//! that run no network compose directly from). `cargo build` is what puts
//! that set where a freshly built binary looks — `target/<profile>/modules`,
//! beside the binary (`workspace_config::modules_dir`) — so a built node is
//! complete without an install step. The set is the checkout's committed
//! artifacts (`make wasm-modules` refreshes them): one component per wasm
//! module the topology names, plus one index guest per module whose crate
//! declares one by carrying `src/index_guest.rs`. A declared artifact the
//! checkout lacks fails the build here, naming the path, instead of `node
//! init` later.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // re-run when HEAD moves. `--git-path` resolves correctly inside a git
    // worktree, where `.git` is a file pointing elsewhere.
    for path in ["HEAD", "index"] {
        if let Some(resolved) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={resolved}");
        }
    }

    if let Some(build) = build_id() {
        println!("cargo:rustc-env=DUCKTAPE_BUILD={build}");
    }

    stage_founding_set();
}

/// copy every declared artifact into `<profile dir>/modules`, under the
/// founding-set names `workspace_config::genesis` reads
/// (`<id>.component.wasm`, `<id>.index.wasm`, `netstack.component.wasm`).
///
/// The profile dir is `OUT_DIR`'s third ancestor
/// (`target[/<triple>]/<profile>/build/<pkg>-<hash>/out`): cargo exposes no
/// variable for the directory a binary lands in, and this is the one fixed
/// relation between a build script's output and that directory.
fn stage_founding_set() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("OUT_DIR sits three levels under the profile dir");
    let dest = profile_dir.join("modules");
    std::fs::create_dir_all(&dest).expect("create the staged founding set dir");
    let checkout = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .join("../..");

    for spec in topology::TOPOLOGY.modules {
        if spec.code != topology::Code::Wasm {
            continue;
        }
        let module_dir = module_dir(&checkout, spec.id);
        stage(
            &module_dir.join("component.wasm"),
            &dest.join(format!("{}.component.wasm", spec.id)),
        );
        if declares_index_guest(&module_dir) {
            stage(
                &module_dir.join("index.wasm"),
                &dest.join(format!("{}.index.wasm", spec.id)),
            );
        }
    }
    stage(
        &checkout.join("crates/networking/netstack-machine/component.wasm"),
        &dest.join("netstack.component.wasm"),
    );
}

/// a module declares its index guest by carrying the guest's engine shell:
/// `src/index_guest.rs` (built by `guest-builder --index` into the crate's
/// committed `index.wasm`). the file is the declaration, so a module cannot
/// ship a mapper the founding set omits or be declared to ship one it lacks.
/// the file is a rerun trigger too: adding or removing the shell re-stages.
fn declares_index_guest(module_dir: &Path) -> bool {
    let shell = module_dir.join("src/index_guest.rs");
    println!("cargo:rerun-if-changed={}", shell.display());
    shell.is_file()
}

/// the checkout directory a module's committed artifacts live in: the
/// product modules under `crates/modules/apps`, the system ones under
/// `crates/modules/system`. Neither holding the module is a build error
/// naming both, since the topology declared a module the tree does not carry.
fn module_dir(checkout: &Path, id: &str) -> PathBuf {
    let candidates = [
        checkout.join("crates/modules/apps").join(id),
        checkout.join("crates/modules/system").join(id),
    ];
    candidates
        .iter()
        .find(|dir| dir.join("component.wasm").is_file())
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "module {id} is in the topology but neither {} nor {} holds a component.wasm \
                 (run `make wasm-modules`)",
                candidates[0].display(),
                candidates[1].display()
            )
        })
}

/// copy `src` to `dest` when the bytes differ, atomically (tmp + rename), and
/// give `dest` the source's mtime so a staged file never reads as newer than
/// this run to cargo's rerun check. Both paths are rerun triggers: a changed
/// artifact re-stages, and so does a deleted staged copy.
fn stage(src: &Path, dest: &Path) {
    println!("cargo:rerun-if-changed={}", src.display());
    println!("cargo:rerun-if-changed={}", dest.display());
    let bytes = std::fs::read(src)
        .unwrap_or_else(|e| panic!("read {} (run `make wasm-modules`): {e}", src.display()));
    let already_staged = std::fs::read(dest).is_ok_and(|have| have == bytes);
    if already_staged {
        return;
    }
    let tmp = dest.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, &bytes).unwrap_or_else(|e| panic!("write {}: {e}", tmp.display()));
    std::fs::rename(&tmp, dest)
        .unwrap_or_else(|e| panic!("rename {} -> {}: {e}", tmp.display(), dest.display()));
    let modified = std::fs::metadata(src).and_then(|m| m.modified());
    if let Ok(modified) = modified {
        let _ = std::fs::File::open(dest).and_then(|f| f.set_modified(modified));
    }
}

/// `<short sha>`, or `<short sha>-<digest>` when the working tree differs from
/// it. A bare `-dirty` marker would be useless here: the whole point is that
/// two DIFFERENT uncommitted trees at one commit must not compare equal, which
/// is the ordinary dev-loop skew (edit, rebuild the node, leave yesterday's
/// daemon running). Digesting the diff makes them differ.
fn build_id() -> Option<String> {
    let commit = git(&["rev-parse", "--short", "HEAD"])?;
    // tracked changes only: untracked scratch files are not part of the build.
    let diff = git(&["diff", "HEAD"]).unwrap_or_default();
    if diff.is_empty() {
        return Some(commit);
    }
    // `DefaultHasher` is not stable across toolchains, and that is fine now
    // that this is only ever DISPLAYED: two builds of the same dirty tree under
    // different toolchains render as different stamps, which reads as skew and
    // costs nothing. stdlib, so no build-dependency for one hash.
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    diff.hash(&mut hasher);
    Some(format!("{commit}-{:x}", hasher.finish()))
}

/// Run one git command, returning its trimmed stdout. `None` for any failure —
/// git missing, not a repository, or a non-zero exit.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8(out.stdout).ok()?.trim().to_string())
}
