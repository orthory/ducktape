//! `guest-builder` — build one module's wasm guest out of the platform
//! repository at a revision.
//!
//! a module carries its whole guest surface itself: a `src/guest.rs` behind a
//! wasm-only `guest` feature (the dispatch shell + the component export) over
//! the module SDK (`crates/module-sdk`). packaging that as a cdylib is
//! identical across modules — a manifest, a one-line lib, a `[workspace]`
//! table, the wasm32 patch set — so none of it is checked in: this tool
//! synthesizes it into a scratch workspace, builds for
//! `wasm32-unknown-unknown`, componentizes, and writes the artifact:
//!
//! ```text
//! guest-builder <module-dir> [--index] [--rev <sha>]
//!               [--out <artifact.wasm>] [--scratch <dir>]
//! ```
//!
//! the shell's ONE dependency is the module, reached out of the platform
//! repository as a git source ([`PLATFORM_GIT`]) at the revision the shell
//! lock pins — never out of the checkout in place. that is what makes a module
//! independently buildable and its bytes reproducible: the build inputs are
//! the module's revision, its lock, and the toolchain, and nothing else.
//!
//! * every platform crate the module reads (the SDK, a sibling's wire types,
//!   the wasm32 patch stubs) resolves inside that one git source at that one
//!   revision — a path dependency inside a git checkout IS the git source —
//!   so a module's platform is one revision by construction.
//! * a git source's location is no part of a symbol hash (a path dependency's
//!   absolute location is), so bytes do not depend on where a checkout lives.
//!   the directory cargo unpacks the revision into is remapped out of panic
//!   paths by [`remap_flags`].
//! * the shell names the module WITHOUT a `rev`: cargo hashes a git reference
//!   as written into `-C metadata`, so a revision in the manifest would change
//!   every symbol name — and every artifact's bytes — on every commit. the
//!   revision lives in the lock (`cargo update --precise`), which is not
//!   hashed, so a module rebuilt at a later revision that changed none of the
//!   sources it compiles yields byte-identical output.
//! * the shell lock is the module's `guest.lock`, committed beside its
//!   artifacts: the record of the revision and the registry versions an
//!   artifact came from, and the seed of the next build, so a crates.io
//!   publish between two rebuilds does not move the bytes. a canonical build
//!   writes artifact and lock together; `--out` writes the artifact alone.
//!
//! the revision defaults to the checkout's HEAD and must be reachable at
//! [`PLATFORM_GIT`]: push before building. a module whose sources are modified
//! in the working tree is refused, since the build would silently compile
//! HEAD instead.
//!
//! `--index` builds the module's INDEX guest instead: the fluentabi mapper
//! behind the crate's `index-guest` feature (a `src/index_guest.rs` — see the
//! index-guest crate). same shell, a second member, and the artifact stays
//! core wasm (`index.wasm`, no componentize step): the fluent31 engine
//! executes plain wasm32 modules, not components.
//!
//! a module authored outside this repository needs none of this: its crate is
//! the cdylib, it pins `ducktape-module-sdk` and the patch stubs by git
//! revision in its own manifest, and plain cargo + `wasm-tools component new`
//! build it.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// the platform repository every shell reaches the module and the platform
/// crates through. a constant, not the checkout's remote: the URL is part of
/// every symbol hash, so two builders spelling it differently would produce
/// different bytes for the same revision.
const PLATFORM_GIT: &str = "https://github.com/orthory/ducktape";

const USAGE: &str = "usage: guest-builder <module-dir> [--index] [--rev <sha>] \
     [--out <artifact.wasm>] [--scratch <dir>]";

/// which of a module's two guests to build. the consensus component and the
/// index mapper share the shell; everything guest-specific — contract
/// feature, shell member, artifact name, whether the cdylib is componentized —
/// hangs off this one discriminant.
#[derive(Clone, Copy, PartialEq, Debug)]
enum GuestKind {
    /// the `ducktape:module` consensus component (`guest` feature).
    Component,
    /// the fluentabi index mapper (`index-guest` feature), core wasm.
    Index,
}

impl GuestKind {
    const ALL: [GuestKind; 2] = [GuestKind::Component, GuestKind::Index];

    fn feature(self) -> &'static str {
        match self {
            GuestKind::Component => "guest",
            GuestKind::Index => "index-guest",
        }
    }

    fn artifact(self) -> &'static str {
        match self {
            GuestKind::Component => "component.wasm",
            GuestKind::Index => "index.wasm",
        }
    }

    /// the shell workspace member (and the cdylib's name suffix) for this guest.
    fn member(self) -> &'static str {
        match self {
            GuestKind::Component => "component",
            GuestKind::Index => "index",
        }
    }

    fn missing_feature_hint(self, name: &str) -> String {
        match self {
            GuestKind::Component => format!(
                "module `{name}` declares no `guest` feature — the port lives in the \
                 module crate (a `src/guest.rs` behind `guest = [\"dep:ducktape-module-sdk\"]`); \
                 see crates/modules/apps/tasks for the shape"
            ),
            GuestKind::Index => format!(
                "module `{name}` declares no `index-guest` feature — the index mapper \
                 lives in the module crate (a `src/index_guest.rs` behind \
                 `index-guest = [\"index_guest/guest\"]`); see crates/modules/apps/tasks \
                 for the shape"
            ),
        }
    }
}

fn main() {
    let Err(err) = run() else { return };
    eprintln!("guest-builder: {err}");
    process::exit(1);
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let kind = args.kind;
    let platform_root = default_platform_root()?;
    let module_dir = canonical(&args.module_dir)?;
    let module = read_module(&platform_root, &module_dir)?;
    let declares_requested_guest = module.guests.contains(&kind);
    if !declares_requested_guest {
        return Err(kind.missing_feature_hint(&module.name));
    }

    let builds_head = args.rev.is_none();
    if builds_head {
        refuse_modified_sources(&platform_root, &module)?;
    }
    let rev = match args.rev {
        Some(rev) => rev,
        None => head(&platform_root)?,
    };

    let scratch = match args.scratch {
        Some(dir) => dir,
        None => platform_root
            .join("target/guest-builder")
            .join(&module.name),
    };
    eprintln!(
        "guest-builder: {} {} at {rev}",
        module.name,
        kind.artifact()
    );
    synthesize(&scratch, &module, &module_dir)?;
    pin(&scratch, &module.name, &rev)?;
    let checkout = checkout_root(&scratch, &module)?;
    build(
        &scratch,
        &module.name,
        kind,
        &remap_flags(&scratch, &checkout),
    )?;

    let cdylib = cdylib_path(&scratch, &module.name, kind);
    // the canonical artifact and its lock are written together: the lock is
    // the record of THOSE bytes. a one-off `--out` build leaves the module
    // directory untouched, so a check that rebuilds every guest keeps the
    // tree clean.
    let out = match args.out {
        Some(path) => {
            write_artifact(kind, &cdylib, &path)?;
            path
        }
        None => {
            let canonical = module_dir.join(kind.artifact());
            write_artifact(kind, &cdylib, &canonical)?;
            record_lock(&scratch, &module_dir)?;
            canonical
        }
    };
    println!("{}", out.display());
    Ok(())
}

fn write_artifact(kind: GuestKind, cdylib: &Path, out: &Path) -> Result<(), String> {
    match kind {
        GuestKind::Component => componentize(cdylib, out),
        GuestKind::Index => copy_cdylib(cdylib, out),
    }
}

// ============================================================================
// argument parsing
// ============================================================================

struct Args {
    module_dir: PathBuf,
    kind: GuestKind,
    rev: Option<String>,
    out: Option<PathBuf>,
    scratch: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut module_dir = None;
    let mut kind = GuestKind::Component;
    let mut rev = None;
    let mut out = None;
    let mut scratch = None;

    let mut argv = env::args().skip(1);
    while let Some(arg) = argv.next() {
        let flag_value = |argv: &mut dyn Iterator<Item = String>| {
            argv.next()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--index" => kind = GuestKind::Index,
            "--rev" => rev = Some(flag_value(&mut argv)?),
            "--out" => out = Some(PathBuf::from(flag_value(&mut argv)?)),
            "--scratch" => scratch = Some(PathBuf::from(flag_value(&mut argv)?)),
            flag if flag.starts_with("--") => {
                return Err(format!("unknown flag {flag}\n{USAGE}"));
            }
            positional => {
                let unclaimed = module_dir.is_none();
                if !unclaimed {
                    return Err(format!("unexpected argument {positional}\n{USAGE}"));
                }
                module_dir = Some(PathBuf::from(positional));
            }
        }
    }

    let Some(module_dir) = module_dir else {
        return Err(USAGE.to_string());
    };
    Ok(Args {
        module_dir,
        kind,
        rev,
        out,
        scratch,
    })
}

// ============================================================================
// module introspection — name, place in the repository, declared guests
// ============================================================================

struct Module {
    name: String,
    /// the module directory relative to the platform root: its place in the
    /// repository, which is where the build reads it from.
    path: PathBuf,
    /// the guests the crate declares, by contract feature.
    guests: Vec<GuestKind>,
}

/// read the module's package name and contract features via `cargo metadata`
/// on the working-tree manifest. the build compiles the repository, so the
/// module must be in it; a crate outside the platform checkout is an
/// out-of-tree module, which is its own cdylib and needs no shell.
fn read_module(platform_root: &Path, module_dir: &Path) -> Result<Module, String> {
    let Ok(path) = module_dir.strip_prefix(platform_root) else {
        return Err(format!(
            "{} is outside the platform checkout {} — guest-builder builds the modules of \
             this repository; a module authored elsewhere is its own cdylib crate pinning \
             ducktape-module-sdk by git revision, built with cargo and wasm-tools directly",
            module_dir.display(),
            platform_root.display()
        ));
    };
    let manifest = module_dir.join("Cargo.toml");
    let output = Command::new(cargo())
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .map_err(|e| format!("running cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata on {}: {}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("parsing cargo metadata output: {e}"))?;

    // metadata on a workspace member lists every member; the module is the
    // package whose manifest is the one we asked about.
    let is_this_module =
        |pkg: &&serde_json::Value| pkg["manifest_path"].as_str() == manifest.to_str();
    let Some(pkg) = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(is_this_module)
    else {
        return Err(format!("{} is not a cargo package", module_dir.display()));
    };
    let Some(name) = pkg["name"].as_str() else {
        return Err(format!(
            "{}: package name missing from metadata",
            manifest.display()
        ));
    };
    let guests = GuestKind::ALL
        .into_iter()
        .filter(|kind| pkg["features"].get(kind.feature()).is_some())
        .collect();
    Ok(Module {
        name: name.to_string(),
        path: path.to_path_buf(),
        guests,
    })
}

/// the checkout's HEAD: the revision a build without `--rev` compiles.
fn head(platform_root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(platform_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("running git rev-parse: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git rev-parse HEAD in {}: {}",
            platform_root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// a build without `--rev` compiles HEAD, so a module edited in the working
/// tree would come out unchanged with no sign of it: refuse. the module's own
/// artifacts and lock are exempt — a sweep rewrites them module by module.
fn refuse_modified_sources(platform_root: &Path, module: &Module) -> Result<(), String> {
    let status = Command::new("git")
        .arg("-C")
        .arg(platform_root)
        .args(["diff", "--quiet", "HEAD", "--"])
        .arg(&module.path)
        .args([":(exclude)*.wasm", ":(exclude)*guest.lock"])
        .status()
        .map_err(|e| format!("running git diff: {e}"))?;
    let Some(code) = status.code() else {
        return Err("git diff was killed by a signal".to_string());
    };
    match code {
        0 => Ok(()),
        1 => Err(format!(
            "{} has uncommitted source changes; the build compiles the module out of \
             {PLATFORM_GIT} at HEAD, never out of this checkout — commit and push them \
             first (or pass --rev to build a specific revision)",
            module.path.display()
        )),
        other => Err(format!("git diff exited {other}")),
    }
}

// ============================================================================
// synthesis — the shell workspace every module shares
// ============================================================================

/// write the shell workspace: one cdylib member per guest the module
/// declares, each depending on the module alone (its contract feature on,
/// defaults off) out of the platform git source, plus the uniform wasm32 patch
/// set from the same source. regenerated on every run — nothing here is
/// hand-maintained state, except the lock, which is seeded from the module's
/// committed `guest.lock` when there is one.
fn synthesize(scratch: &Path, module: &Module, module_dir: &Path) -> Result<(), String> {
    for kind in &module.guests {
        let member = scratch.join(kind.member());
        let src = member.join("src");
        fs::create_dir_all(&src).map_err(|e| format!("creating {}: {e}", src.display()))?;
        write(
            &member.join("Cargo.toml"),
            &member_manifest(&module.name, *kind),
        )?;
        write(&src.join("lib.rs"), &member_lib(&module.name))?;
    }
    write(
        &scratch.join("Cargo.toml"),
        &workspace_manifest(&module.guests),
    )?;

    let committed_lock = module_dir.join("guest.lock");
    if committed_lock.is_file() {
        fs::copy(&committed_lock, scratch.join("Cargo.lock"))
            .map(|_| ())
            .map_err(|e| {
                format!(
                    "seeding the shell lock from {}: {e}",
                    committed_lock.display()
                )
            })?;
    }
    Ok(())
}

fn workspace_manifest(guests: &[GuestKind]) -> String {
    let members: Vec<String> = guests
        .iter()
        .map(|kind| format!("\"{}\"", kind.member()))
        .collect();
    format!(
        r#"# synthesized by guest-builder — do not edit; regenerated on every build.
# the packaging shell only: the module logic and its guest ports live in the
# module crate, read out of the platform repository at the revision the lock
# pins. one member per guest the module declares.
[workspace]
members = [{members}]
resolver = "2"

# the uniform wasm32 patch set (crates/module-sdk/stubs in the platform
# repository, at the module's own revision): applied to every guest; cargo's
# "unused patch" warning on a module whose graph never pulls one of these
# crates is expected and harmless.
[patch.crates-io]
getrandom-02 = {{ package = "getrandom", version = "0.2", git = "{PLATFORM_GIT}" }}
getrandom-03 = {{ package = "getrandom", version = "0.3", git = "{PLATFORM_GIT}" }}
getrandom-04 = {{ package = "getrandom", version = "0.4", git = "{PLATFORM_GIT}" }}
blst = {{ git = "{PLATFORM_GIT}" }}
"#,
        members = members.join(", ")
    )
}

/// the member's manifest. the module is named by git source ALONE — no `rev`,
/// no `branch` — because cargo hashes a written reference into every symbol
/// name; the lock carries the revision.
fn member_manifest(name: &str, kind: GuestKind) -> String {
    let feature = kind.feature();
    let member = kind.member();
    format!(
        r#"# synthesized by guest-builder — do not edit; regenerated on every build.
[package]
name = "{name}-{member}"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
{name} = {{ git = "{PLATFORM_GIT}", default-features = false, features = ["{feature}"] }}
"#
    )
}

fn member_lib(name: &str) -> String {
    format!(
        "// synthesized by guest-builder — link the module crate for its guest export.\n\
         extern crate {} as _;\n",
        snake(name)
    )
}

/// pin the platform git source to `rev` in the shell lock. every package the
/// shell reads from the repository (the module, the SDK, siblings, the patch
/// stubs) shares that one source, so this moves them together.
fn pin(scratch: &Path, name: &str, rev: &str) -> Result<(), String> {
    let spec = format!("{PLATFORM_GIT}#{name}");
    let status = Command::new(cargo())
        .args(["update", "-p", &spec, "--precise", rev])
        .current_dir(scratch)
        .status()
        .map_err(|e| format!("running cargo update: {e}"))?;
    if !status.success() {
        return Err(format!(
            "pinning {name} to {rev} failed — the revision must be reachable at \
             {PLATFORM_GIT} (push it first): the build compiles the module out of the \
             repository, never out of this checkout"
        ));
    }
    Ok(())
}

/// where cargo unpacked the pinned revision: the platform checkout the build
/// actually compiles, found through the module's manifest in the resolved
/// graph. [`remap_flags`] maps it to a stable token.
fn checkout_root(scratch: &Path, module: &Module) -> Result<PathBuf, String> {
    let output = Command::new(cargo())
        .args(["metadata", "--format-version", "1"])
        .current_dir(scratch)
        .output()
        .map_err(|e| format!("running cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata in {}: {}",
            scratch.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("parsing cargo metadata output: {e}"))?;
    let is_the_module_from_git = |pkg: &&serde_json::Value| {
        let named = pkg["name"].as_str() == Some(module.name.as_str());
        let from_git = pkg["source"]
            .as_str()
            .is_some_and(|source| source.starts_with("git+"));
        named && from_git
    };
    let Some(pkg) = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(is_the_module_from_git)
    else {
        return Err(format!(
            "{} is not in the shell's resolved graph as a git package",
            module.name
        ));
    };
    let Some(manifest_path) = pkg["manifest_path"].as_str() else {
        return Err(format!(
            "{}: manifest path missing from metadata",
            module.name
        ));
    };
    checkout_root_of(Path::new(manifest_path), &module.path)
}

/// the checkout a git package's manifest sits in: its manifest path minus the
/// module's place in the repository.
fn checkout_root_of(manifest_path: &Path, module_path: &Path) -> Result<PathBuf, String> {
    let module_manifest = module_path.join("Cargo.toml");
    let depth = module_manifest.components().count();
    let Some(root) = manifest_path.ancestors().nth(depth) else {
        return Err(format!(
            "{} is shallower than {}",
            manifest_path.display(),
            module_manifest.display()
        ));
    };
    let sits_at_its_repository_place = root.join(&module_manifest) == manifest_path;
    if !sits_at_its_repository_place {
        return Err(format!(
            "{} does not end in {}",
            manifest_path.display(),
            module_manifest.display()
        ));
    }
    Ok(root.to_path_buf())
}

// ============================================================================
// build + componentize
// ============================================================================

/// `--remap-path-prefix` mappings that keep every builder-local absolute path
/// out of the artifact's CONTENT — panic locations name their source file, so
/// without these the bytes carry the builder's `/home/<user>/...` around
/// forever. `ops/wasm-repro-check.sh` and the host-path scan in
/// `make wasm-modules-check` are the gates.
///
/// the checkout mapping is last on purpose: the checkout sits under
/// CARGO_HOME and rustc takes the LAST matching mapping, so the
/// revision-specific `git/checkouts/<repo>-<hash>/<rev>` directory becomes the
/// stable `/ducktape` — the same token at every revision — instead of a path
/// that would move the bytes on every commit.
fn remap_flags(scratch: &Path, checkout: &Path) -> String {
    let home = env::var("HOME").unwrap_or_default();
    let tool_home =
        |key: &str, dir: &str| env::var(key).unwrap_or_else(|_| format!("{home}/{dir}"));
    let mappings = [
        (tool_home("CARGO_HOME", ".cargo"), "/cargo"),
        (tool_home("RUSTUP_HOME", ".rustup"), "/rustup"),
        (scratch.display().to_string(), "/guest-builder"),
        (checkout.display().to_string(), "/ducktape"),
    ];
    let flags: Vec<String> = mappings
        .iter()
        .map(|(from, to)| format!("--remap-path-prefix={from}={to}"))
        .collect();
    // the ENCODED form's separator: plain `RUSTFLAGS` splits on whitespace, so
    // a path containing a space would tear one flag into two.
    flags.join("\x1f")
}

fn build(scratch: &Path, name: &str, kind: GuestKind, rustflags: &str) -> Result<(), String> {
    let member = format!("{name}-{}", kind.member());
    let status = Command::new(cargo())
        // `--locked`: the lock is complete after `pin`, and the bytes are
        // only reproducible if this build changes nothing in it.
        .args([
            "build",
            "--locked",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "-p",
            &member,
        ])
        .env("CARGO_ENCODED_RUSTFLAGS", rustflags)
        // the encoded form wins over the plain one, but an inherited
        // `RUSTFLAGS` would be a confusing dead passenger.
        .env_remove("RUSTFLAGS")
        .current_dir(scratch)
        .status()
        .map_err(|e| format!("running cargo build: {e}"))?;
    if !status.success() {
        return Err(format!("wasm32 build failed in {}", scratch.display()));
    }
    Ok(())
}

fn componentize(cdylib: &Path, out: &Path) -> Result<(), String> {
    let status = Command::new("wasm-tools")
        .arg("component")
        .arg("new")
        .arg(cdylib)
        .arg("-o")
        .arg(out)
        .status()
        .map_err(|e| format!("running wasm-tools (cargo install wasm-tools): {e}"))?;
    if !status.success() {
        return Err(format!("componentizing {} failed", cdylib.display()));
    }
    Ok(())
}

/// an index guest ships as the built cdylib itself — fluentabi is core wasm,
/// so there is nothing to componentize.
fn copy_cdylib(cdylib: &Path, out: &Path) -> Result<(), String> {
    fs::copy(cdylib, out)
        .map(|_| ())
        .map_err(|e| format!("copying {} to {}: {e}", cdylib.display(), out.display()))
}

/// the shell lock, written back beside the module as its `guest.lock`: the
/// record of what the artifact was built from, and the seed of the next build.
fn record_lock(scratch: &Path, module_dir: &Path) -> Result<(), String> {
    let lock = module_dir.join("guest.lock");
    fs::copy(scratch.join("Cargo.lock"), &lock)
        .map(|_| ())
        .map_err(|e| format!("recording the shell lock as {}: {e}", lock.display()))
}

fn cdylib_path(scratch: &Path, name: &str, kind: GuestKind) -> PathBuf {
    scratch
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{}_{}.wasm", snake(name), kind.member()))
}

// ============================================================================
// small helpers
// ============================================================================

/// the cargo that invoked us (`cargo run` sets `CARGO`), else PATH's.
fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

/// the ducktape checkout this binary was built from: the source of the
/// default revision, and the tree a module directory must sit in.
fn default_platform_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest_dir.parent().and_then(Path::parent) else {
        return Err("cannot derive the platform root from the build location".to_string());
    };
    canonical(root)
}

fn canonical(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn snake(name: &str) -> String {
    name.replace('-', "_")
}

fn write(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("writing {}: {e}", path.display()))
}

// ============================================================================
// tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// a written git reference is hashed into every symbol name, so the shell
    /// must name the module by source alone and leave the revision to the lock.
    #[test]
    fn the_shell_names_the_module_by_source_alone() {
        let manifest = member_manifest("chat", GuestKind::Component);
        assert!(manifest.contains(
            "chat = { git = \"https://github.com/orthory/ducktape\", default-features = false, features = [\"guest\"] }"
        ));
        assert!(!manifest.contains("rev ="));
        assert!(!manifest.contains("branch ="));
        assert!(manifest.contains("name = \"chat-component\""));
    }

    #[test]
    fn the_workspace_has_one_member_per_declared_guest() {
        let both = workspace_manifest(&[GuestKind::Component, GuestKind::Index]);
        assert!(both.contains("members = [\"component\", \"index\"]"));
        let component_only = workspace_manifest(&[GuestKind::Component]);
        assert!(component_only.contains("members = [\"component\"]"));
        // the patch stubs ride the same source, with no reference either
        assert!(both.contains("getrandom-02 = { package = \"getrandom\", version = \"0.2\", git = \"https://github.com/orthory/ducktape\" }"));
        assert!(!both.contains("rev ="));
    }

    /// rustc takes the last matching mapping, and the checkout lives under
    /// CARGO_HOME: the checkout's stable token must come after CARGO_HOME's.
    #[test]
    fn the_checkout_mapping_comes_after_cargo_home() {
        let flags = remap_flags(
            Path::new("/scratch"),
            Path::new("/home/u/.cargo/git/checkouts/ducktape-1234/abcdef0"),
        );
        let flags: Vec<&str> = flags.split('\x1f').collect();
        let cargo_home = flags
            .iter()
            .position(|f| f.ends_with("=/cargo"))
            .expect("cargo home mapping");
        let checkout = flags
            .iter()
            .position(|f| f.ends_with("=/ducktape"))
            .expect("checkout mapping");
        assert!(checkout > cargo_home, "{flags:?}");
        assert_eq!(
            flags[checkout],
            "--remap-path-prefix=/home/u/.cargo/git/checkouts/ducktape-1234/abcdef0=/ducktape"
        );
    }

    #[test]
    fn the_checkout_root_is_the_manifest_minus_the_repository_place() {
        let root = checkout_root_of(
            Path::new("/home/u/.cargo/git/checkouts/ducktape-1234/abcdef0/crates/modules/apps/chat/Cargo.toml"),
            Path::new("crates/modules/apps/chat"),
        )
        .expect("root");
        assert_eq!(
            root,
            Path::new("/home/u/.cargo/git/checkouts/ducktape-1234/abcdef0")
        );

        let wrong_place = checkout_root_of(
            Path::new("/somewhere/else/tasks/Cargo.toml"),
            Path::new("crates/modules/apps/chat"),
        );
        assert!(wrong_place.is_err());
    }
}
