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
//! `--platform-root` points at the ducktape checkout supplying `guest-adapter`
//! and the patch crates; it defaults to the checkout this binary was built
//! from, so in-tree use (`cargo run -p guest-builder`) needs no flags and an
//! out-of-tree module directory is buildable by passing its path.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const USAGE: &str = "usage: guest-builder <module-dir> \
     [--out <component.wasm>] [--scratch <dir>] [--platform-root <dir>]";

fn main() {
    let Err(err) = run() else { return };
    eprintln!("guest-builder: {err}");
    process::exit(1);
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let module_dir = canonical(&args.module_dir)?;
    let platform_root = match args.platform_root {
        Some(root) => canonical(&root)?,
        None => default_platform_root()?,
    };
    let module = read_module(&module_dir)?;

    let scratch = match args.scratch {
        Some(dir) => dir,
        None => platform_root
            .join("target/guest-builder")
            .join(format!("{}-wasm", module.name)),
    };
    synthesize(&scratch, &module_dir, &module.name, &platform_root)?;
    build(&scratch)?;

    let out = match args.out {
        Some(path) => path,
        None => module_dir.join("component.wasm"),
    };
    componentize(&scratch, &module.name, &out)?;
    println!("{}", out.display());
    Ok(())
}

// ============================================================================
// argument parsing
// ============================================================================

struct Args {
    module_dir: PathBuf,
    out: Option<PathBuf>,
    scratch: Option<PathBuf>,
    platform_root: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut module_dir = None;
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
/// the `guest` feature — the contract this tool packages. a module without it
/// has no port to build, so fail with the wiring instruction rather than a
/// downstream compile error.
fn read_module(module_dir: &Path) -> Result<Module, String> {
    let manifest = module_dir.join("Cargo.toml");
    let output = Command::new(cargo())
        .args(["metadata", "--format-version", "1", "--no-deps", "--manifest-path"])
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
    let Some(pkg) = meta["packages"].as_array().into_iter().flatten().find(is_this_module)
    else {
        return Err(format!("{} is not a cargo package", module_dir.display()));
    };
    let Some(name) = pkg["name"].as_str() else {
        return Err(format!("{}: package name missing from metadata", manifest.display()));
    };

    let has_guest_feature = pkg["features"].get("guest").is_some();
    if !has_guest_feature {
        return Err(format!(
            "module `{name}` declares no `guest` feature — the port lives in the \
             module crate (a `src/guest.rs` behind `guest = [\"dep:guest-adapter\"]`); \
             see crates/modules/apps/tasks for the shape"
        ));
    }
    Ok(Module {
        name: name.to_string(),
    })
}

// ============================================================================
// synthesis — the packaging workspace every module shares
// ============================================================================

/// write the scratch packaging workspace: a cdylib crate whose only dependency
/// is the module (guest feature on, native off) plus the uniform wasm32 patch
/// set. regenerated on every run — nothing here is hand-maintained state.
fn synthesize(
    scratch: &Path,
    module_dir: &Path,
    name: &str,
    platform_root: &Path,
) -> Result<(), String> {
    let src = scratch.join("src");
    fs::create_dir_all(&src).map_err(|e| format!("creating {}: {e}", src.display()))?;

    let module_path = module_dir.display();
    let stubs = platform_root.join("crates/guests/stubs");
    let stubs = stubs.display();
    let blst = platform_root.join("patches/blst");
    let blst = blst.display();
    let manifest = format!(
        r#"# synthesized by guest-builder — do not edit; regenerated on every build.
# the packaging shell only: the module logic and its guest port live in the
# module crate (feature "guest"). this standalone workspace gives the cdylib
# its own wasm32 dep resolution, feature unification, and patch set — none of
# which may leak into the host workspace.
[package]
name = "{name}-wasm"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
{name} = {{ path = "{module_path}", default-features = false, features = ["guest"] }}

[workspace]

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

// ============================================================================
// build + componentize
// ============================================================================

fn build(scratch: &Path) -> Result<(), String> {
    let status = Command::new(cargo())
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .current_dir(scratch)
        .status()
        .map_err(|e| format!("running cargo build: {e}"))?;
    if !status.success() {
        return Err(format!("wasm32 build failed in {}", scratch.display()));
    }
    Ok(())
}

fn componentize(scratch: &Path, name: &str, out: &Path) -> Result<(), String> {
    let cdylib = scratch
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{}_wasm.wasm", snake(name)));
    let status = Command::new("wasm-tools")
        .arg("component")
        .arg("new")
        .arg(&cdylib)
        .arg("-o")
        .arg(out)
        .status()
        .map_err(|e| format!("running wasm-tools (cargo install wasm-tools): {e}"))?;
    if !status.success() {
        return Err(format!("componentizing {} failed", cdylib.display()));
    }
    Ok(())
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
