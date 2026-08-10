//! `guest-builder` — build a module crate's `ducktape:module` component
//! without a checked-in packaging crate.
//!
//! a ported module carries its whole guest surface itself: a `src/guest.rs`
//! behind a wasm-only `guest` feature (the dispatch shell + the component
//! export). what used to be a per-module `crates/guests/<id>-wasm` crate is
//! pure packaging — a cdylib manifest, a one-line lib, a standalone
//! `[workspace]` table, and the wasm32 dep patches — identical across modules.
//! this tool synthesizes that packaging into a scratch workspace, builds it
//! for `wasm32-unknown-unknown`, and componentizes the result:
//!
//! ```text
//! guest-builder <module-dir> [--out <component.wasm>]
//!               [--scratch <dir>] [--platform-root <dir>]
//! ```
//!
//! the synthesized workspace is EPHEMERAL by design: guest workspaces never
//! committed lockfiles — the committed artifact is the canonical bytes
//! (`wasm-modules-check` guards the copies) — so regenerating the packaging
//! loses nothing. the standalone workspace is what keeps wasm32 dep
//! resolution, feature unification, and the `[patch.crates-io]` stubs
//! (getrandom, blst) out of the host workspace; the patch set is applied
//! uniformly — cargo warns "unused patch" for modules whose graphs never pull
//! those crates, which is expected and harmless.
//!
//! that scratch workspace also holds `tree/` — a snapshot of the platform
//! checkout — and the build compiles THAT, never the checkout in place. The
//! committed artifact is therefore identical from any checkout path, which the
//! copies alone never were: cargo hashes a path package's absolute location
//! into `-C metadata` and so into every symbol name unless the package sits
//! under the workspace being built. See [`synthesize`] and [`remap_flags`].
//!
//! `--platform-root` points at the ducktape checkout supplying `guest-adapter`
//! and the patch crates; it defaults to the checkout this binary was built
//! from, so in-tree use (`cargo run -p guest-builder`) needs no flags and an
//! out-of-tree module directory is buildable by passing its path.
//!
//! `--index` builds the module's INDEX guest instead: the fluentabi mapper
//! behind the crate's `index-guest` feature (a `src/index_guest.rs` — see the
//! index-guest crate). same synthesis, different feature, and the artifact
//! stays core wasm (`index.wasm`, no componentize step): the fluent31 engine
//! executes plain wasm32 modules, not components.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

/// the platform checkout, copied under the scratch workspace root — see
/// [`snapshot`] for why the build compiles the copy and not the checkout.
const TREE: &str = "tree";

const USAGE: &str = "usage: guest-builder <module-dir> [--index] \
     [--out <artifact.wasm>] [--scratch <dir>] [--platform-root <dir>]";

/// which of a module's two guests to package. the consensus component and the
/// index mapper share the synthesis pipeline; everything mode-specific —
/// contract feature, artifact name, whether the cdylib is componentized —
/// hangs off this one discriminant.
#[derive(Clone, Copy, PartialEq)]
enum GuestKind {
    /// the `ducktape:module` consensus component (`guest` feature).
    Component,
    /// the fluentabi index mapper (`index-guest` feature), core wasm.
    Index,
}

impl GuestKind {
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

    /// suffix of the synthesized packaging crate (and its scratch dir).
    fn shell_suffix(self) -> &'static str {
        match self {
            GuestKind::Component => "wasm",
            GuestKind::Index => "index",
        }
    }

    fn missing_feature_hint(self, name: &str) -> String {
        match self {
            GuestKind::Component => format!(
                "module `{name}` declares no `guest` feature — the port lives in the \
                 module crate (a `src/guest.rs` behind `guest = [\"dep:guest-adapter\"]`); \
                 see crates/modules/apps/tasks for the shape"
            ),
            GuestKind::Index => format!(
                "module `{name}` declares no `index-guest` feature — the index mapper \
                 lives in the module crate (a `src/index_guest.rs` behind \
                 `index-guest = [\"dep:index-guest\"]`); see crates/modules/apps/tasks \
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
    let module_dir = canonical(&args.module_dir)?;
    let platform_root = match args.platform_root {
        Some(root) => canonical(&root)?,
        None => default_platform_root()?,
    };
    let module = read_module(&module_dir, kind)?;

    let scratch = match args.scratch {
        Some(dir) => dir,
        None => platform_root.join("target/guest-builder").join(format!(
            "{}-{}",
            module.name,
            kind.shell_suffix()
        )),
    };
    synthesize(&scratch, &module_dir, &module.name, &platform_root, kind)?;
    build(&scratch, &remap_flags(&module_dir, &scratch))?;

    let out = match args.out {
        Some(path) => path,
        None => module_dir.join(kind.artifact()),
    };
    let cdylib = cdylib_path(&scratch, &module.name, kind);
    match kind {
        GuestKind::Component => componentize(&cdylib, &out)?,
        GuestKind::Index => copy_cdylib(&cdylib, &out)?,
    }
    println!("{}", out.display());
    Ok(())
}

// ============================================================================
// argument parsing
// ============================================================================

struct Args {
    module_dir: PathBuf,
    kind: GuestKind,
    out: Option<PathBuf>,
    scratch: Option<PathBuf>,
    platform_root: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut module_dir = None;
    let mut kind = GuestKind::Component;
    let mut out = None;
    let mut scratch = None;
    let mut platform_root = None;

    let mut argv = env::args().skip(1);
    while let Some(arg) = argv.next() {
        let flag_value = |argv: &mut dyn Iterator<Item = String>| {
            argv.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--index" => kind = GuestKind::Index,
            "--out" => out = Some(flag_value(&mut argv)?),
            "--scratch" => scratch = Some(flag_value(&mut argv)?),
            "--platform-root" => platform_root = Some(flag_value(&mut argv)?),
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
        out,
        scratch,
        platform_root,
    })
}

// ============================================================================
// module introspection — name + the `guest` feature contract
// ============================================================================

struct Module {
    name: String,
}

/// read the module's package name via `cargo metadata` and verify it declares
/// the requested guest's contract feature. a module without it has no such
/// port to build, so fail with the wiring instruction rather than a
/// downstream compile error.
fn read_module(module_dir: &Path, kind: GuestKind) -> Result<Module, String> {
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

    let has_contract_feature = pkg["features"].get(kind.feature()).is_some();
    if !has_contract_feature {
        return Err(kind.missing_feature_hint(name));
    }
    Ok(Module {
        name: name.to_string(),
    })
}

// ============================================================================
// synthesis — the packaging workspace every module shares
// ============================================================================

/// write the scratch packaging workspace: a cdylib crate whose only dependency
/// is the module (the contract feature on, native off) plus the uniform wasm32
/// patch set. regenerated on every run — nothing here is hand-maintained state.
///
/// Every path dependency is reached through [`snapshot`] — `tree/...`, INSIDE
/// the scratch workspace — because cargo hashes a path package's location into
/// its `-C metadata`, and thus into every symbol name, relative to the
/// workspace root when the package sits under it and ABSOLUTELY when it does
/// not. Depending on the checkout directly is what made two checkouts of the
/// same source produce different bytes; no rustc flag can undo it, since the
/// hash is cargo's and is fixed before rustc runs.
fn synthesize(
    scratch: &Path,
    module_dir: &Path,
    name: &str,
    platform_root: &Path,
    kind: GuestKind,
) -> Result<(), String> {
    let src = scratch.join("src");
    fs::create_dir_all(&src).map_err(|e| format!("creating {}: {e}", src.display()))?;
    snapshot(platform_root, &scratch.join(TREE))?;

    // an out-of-tree module is not ours to relocate: it keeps its own path, and
    // with it the checkout-dependence this snapshot exists to remove.
    let module_path = match module_dir.strip_prefix(platform_root) {
        Ok(rel) => format!("{TREE}/{}", rel.display()),
        Err(_) => module_dir.display().to_string(),
    };
    let stubs = format!("{TREE}/crates/guests/stubs");
    let blst = format!("{TREE}/patches/blst");
    let feature = kind.feature();
    let suffix = kind.shell_suffix();
    let manifest = format!(
        r#"# synthesized by guest-builder — do not edit; regenerated on every build.
# the packaging shell only: the module logic and its guest port live in the
# module crate (feature "{feature}"). this standalone workspace gives the cdylib
# its own wasm32 dep resolution, feature unification, and patch set — none of
# which may leak into the host workspace.
[package]
name = "{name}-{suffix}"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
{name} = {{ path = "{module_path}", default-features = false, features = ["{feature}"] }}

# `exclude` is load-bearing: without it THIS workspace claims the snapshot's
# crates, and their `workspace = true` inheritance resolves against this bare
# table instead of the platform manifest one directory down.
[workspace]
exclude = ["{TREE}"]

# the uniform wasm32 patch set (see crates/guests/stubs and patches/blst):
# applied to every synthesized guest; cargo's "unused patch" warning on
# modules whose graphs never pull these crates is expected and harmless.
[patch.crates-io]
getrandom = {{ path = "{stubs}/getrandom-02" }}
getrandom-03 = {{ package = "getrandom", path = "{stubs}/getrandom-03" }}
getrandom-04 = {{ package = "getrandom", path = "{stubs}/getrandom-04" }}
blst = {{ path = "{blst}" }}
"#
    );
    let lib = format!(
        "// synthesized by guest-builder — link the module crate for its guest export.\n\
         extern crate {} as _;\n",
        snake(name)
    );

    write(&scratch.join("Cargo.toml"), &manifest)?;
    write(&src.join("lib.rs"), &lib)
}

/// refresh `<scratch>/tree` with a copy of the platform checkout — the source
/// the guest build actually compiles, and the reason its bytes do not depend on
/// where the checkout lives.
///
/// tar rather than a hand-rolled walk: it carries symlinks and mtimes over
/// verbatim, and the mtimes are what keep the next build incremental. `target`,
/// `.git` and `.worktree` are build output, history and other checkouts —
/// copying them would drag tens of GB along and recurse into the snapshot
/// itself. What is left costs ~40 MB of disk per module.
///
/// The exclude patterns are UNANCHORED on purpose — no leading `./`. Anchored,
/// they skip only the top-level `target`, and the nested
/// `crates/guests/*-wasm/target` dirs (~145 MB each) ride along into every
/// snapshot: 646 MB per module instead of 42 MB.
fn snapshot(platform_root: &Path, tree: &Path) -> Result<(), String> {
    // wiped first: tar overwrites, it never removes, so a file deleted from the
    // checkout would otherwise live on in here forever.
    if tree.exists() {
        fs::remove_dir_all(tree).map_err(|e| format!("clearing {}: {e}", tree.display()))?;
    }
    fs::create_dir_all(tree).map_err(|e| format!("creating {}: {e}", tree.display()))?;

    let mut pack = Command::new("tar")
        .args([
            "-cf",
            "-",
            "--exclude=target",
            "--exclude=.git",
            "--exclude=.worktree",
            "-C",
        ])
        .arg(platform_root)
        .arg(".")
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("running tar: {e}"))?;
    let Some(packed) = pack.stdout.take() else {
        return Err("tar produced no output stream".to_string());
    };
    let unpack = Command::new("tar")
        .args(["-xf", "-", "-C"])
        .arg(tree)
        .stdin(packed)
        .status()
        .map_err(|e| format!("running tar: {e}"))?;
    let packing = pack.wait().map_err(|e| format!("waiting on tar: {e}"))?;
    if !packing.success() || !unpack.success() {
        return Err(format!(
            "snapshotting {} into {} failed",
            platform_root.display(),
            tree.display()
        ));
    }
    Ok(())
}

// ============================================================================
// build + componentize
// ============================================================================

/// `--remap-path-prefix` mappings that keep every builder-local absolute path
/// out of the artifact's CONTENT — panic locations name their source file, so
/// without these the bytes carry the builder's `/home/<user>/...` around
/// forever. Half of path-independence; [`synthesize`]'s snapshot is the other
/// half (symbol names). `ops/wasm-repro-check.sh` and the host-path scan in
/// `make wasm-modules-check` are the gates.
///
/// The scratch mapping covers the snapshot with it, so the platform checkout
/// needs no mapping of its own — the build never names it.
fn remap_flags(module_dir: &Path, scratch: &Path) -> String {
    let home = env::var("HOME").unwrap_or_default();
    let tool_home =
        |key: &str, dir: &str| env::var(key).unwrap_or_else(|_| format!("{home}/{dir}"));
    let mappings = [
        (tool_home("CARGO_HOME", ".cargo"), "/cargo"),
        (tool_home("RUSTUP_HOME", ".rustup"), "/rustup"),
        (scratch.display().to_string(), "/ducktape"),
        // an out-of-tree module is compiled where it lies (see `synthesize`).
        (module_dir.display().to_string(), "/module"),
    ];
    let flags: Vec<String> = mappings
        .iter()
        .map(|(from, to)| format!("--remap-path-prefix={from}={to}"))
        .collect();
    // the ENCODED form's separator: plain `RUSTFLAGS` splits on whitespace, so
    // a checkout path containing a space would tear one flag into two.
    flags.join("\x1f")
}

fn build(scratch: &Path, rustflags: &str) -> Result<(), String> {
    let status = Command::new(cargo())
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
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

fn cdylib_path(scratch: &Path, name: &str, kind: GuestKind) -> PathBuf {
    scratch
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{}_{}.wasm", snake(name), kind.shell_suffix()))
}

// ============================================================================
// small helpers
// ============================================================================

/// the cargo that invoked us (`cargo run` sets `CARGO`), else PATH's.
fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

/// the ducktape checkout this binary was built from — the default source of
/// guest-adapter and the patch crates for in-tree use.
fn default_platform_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest_dir.parent().and_then(Path::parent) else {
        return Err("cannot derive the platform root; pass --platform-root".to_string());
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
