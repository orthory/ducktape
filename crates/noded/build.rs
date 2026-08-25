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
