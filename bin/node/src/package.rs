//! `ducktape-node package …` — inspect, build, verify, and test Quack
//! packages.
//!
//! A thin CLI over the `quack` crate: read a package from a source directory or
//! a `.quack` capsule, print its manifest, pack a deterministic `.quack`
//! (optionally signed), verify content digests + the signature, and `test` —
//! verify, then replay the capsule's `harness/golden.json` in-process on a
//! `quack_harness::PackageTestBed` whose extra modules come from this binary's
//! native catalog (v1: the docs package's modules).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config::{hex_bytes, load_identity};

/// Dispatch a `package` subcommand.
pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("inspect") => cmd_inspect(&args[1..]),
        Some("build") => cmd_build(&args[1..]),
        Some("verify") => cmd_verify(&args[1..]),
        Some("test") => cmd_test(&args[1..]),
        Some(other) => Err(format!(
            "unknown package subcommand {other:?} (want inspect|build|verify|test <dir|.quack>)"
        )
        .into()),
        None => Err("package needs a subcommand: inspect|build|verify|test <dir|.quack>".into()),
    }
}

/// `package inspect <dir|.quack>` — print the manifest summary, the manifest
/// hash, and the signature status.
fn cmd_inspect(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let opts = Opts::parse(args)?;
    let source = opts.require_source()?;
    let capsule = load_capsule(source)?;
    let toml = capsule
        .manifest_bytes()
        .ok_or("no quack.toml in the package")?;
    let manifest = quack::parse_manifest(toml)?;
    quack::validate(&manifest)?;

    println!("package  {} {}", manifest.package, manifest.version);
    println!("schema   {}", manifest.schema);
    if let Some(harness) = &manifest.harness {
        println!("harness  {harness}");
    }
    println!("manifest sha256:{}", hex_bytes(&quack::manifest_hash(toml)));
    println!(
        "requires protocol_min={} modules={:?} capabilities={:?}",
        manifest.requires.protocol_min, manifest.requires.modules, manifest.requires.capabilities
    );

    println!("modules ({}):", manifest.modules.len());
    for m in &manifest.modules {
        let kind = match m.kind {
            quack::ModuleKind::Native => "native",
            quack::ModuleKind::Wasm => "wasm",
        };
        println!("  {} -> {} [{}]", m.logical, m.default_id, kind);
    }
    println!("prompts ({}):", manifest.prompts.len());
    for p in &manifest.prompts {
        println!("  {} <- {}", p.logical, p.path);
    }
    println!("actions ({}):", manifest.actions.len());
    for a in &manifest.actions {
        println!("  {} -> {}", a.tag, a.owner);
    }
    println!("agents ({}):", manifest.agents.len());
    for a in &manifest.agents {
        println!(
            "  {} \"{}\" cap={} actions={:?} [{}]",
            a.id, a.display_name, a.capability, a.actions, a.status
        );
    }
    println!("engagements ({}):", manifest.engagements.len());
    for e in &manifest.engagements {
        println!("  {}.{} -> {} ({})", e.source, e.event, e.agent, e.policy);
    }
    println!("signature {}", signature_status(&capsule, toml));
    Ok(())
}

/// `package build <dir> [-o out.quack] [--key <keyfile>] [--emit-hashes]` —
/// pack a deterministic `.quack` from a source directory. `--emit-hashes`
/// prints the computed content digests instead of building (bootstrap the
/// manifest's `hash` fields).
fn cmd_build(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let opts = Opts::parse(args)?;
    let dir = opts.require_source()?;
    if !dir.is_dir() {
        return Err(format!("build expects a source directory, got {dir:?}").into());
    }
    let mut capsule = quack::open_dir(dir)?;
    let toml = capsule
        .manifest_bytes()
        .ok_or("no quack.toml in the package")?
        .to_vec();
    let manifest = quack::parse_manifest(&toml)?;
    quack::validate(&manifest)?;

    if opts.emit_hashes {
        for m in &manifest.modules {
            if let Some(artifact) = &m.artifact {
                emit_hash(&capsule, &m.logical, artifact)?;
            }
        }
        for p in &manifest.prompts {
            emit_hash(&capsule, &p.logical, &p.path)?;
        }
        return Ok(());
    }

    // a built package must be internally consistent: every declared digest
    // matches the file it points at.
    quack::verify_digests(&capsule, &manifest)?;

    if let Some(keyfile) = &opts.key {
        let signer = load_identity(keyfile).map_err(|e| format!("--key {keyfile:?}: {e}"))?;
        let sig = quack::sign_manifest(&signer, &quack::manifest_hash(&toml));
        let json = serde_json::to_vec_pretty(&sig)?;
        capsule.insert("signatures/package.sig", json);
        println!("signed by {}", hex_bytes(&sig.signer));
    }

    let out = opts
        .out
        .clone()
        .unwrap_or_else(|| default_out(dir, &manifest));
    std::fs::write(&out, quack::build_tar(&capsule))?;
    println!("built {} ({} files)", out.display(), capsule.files.len());
    Ok(())
}

/// `package verify <dir|.quack>` — check content digests and signature status.
/// Exits non-zero on a digest mismatch or an invalid signature.
fn cmd_verify(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let opts = Opts::parse(args)?;
    let source = opts.require_source()?;
    let capsule = load_capsule(source)?;
    let toml = capsule
        .manifest_bytes()
        .ok_or("no quack.toml in the package")?;
    let manifest = quack::parse_manifest(toml)?;
    quack::validate(&manifest)?;
    quack::verify_digests(&capsule, &manifest)?;
    println!("digests  ok ({} referenced files)", digest_count(&manifest));

    let status = signature_status(&capsule, toml);
    println!("signature {status}");
    if status.starts_with("invalid") {
        return Err("signature verification failed".into());
    }
    println!("ok {} {}", manifest.package, manifest.version);
    Ok(())
}

/// `package test <dir|.quack>` — verify (digests + signature), then replay the
/// capsule's `harness/golden.json` against an in-process testbed and print a
/// per-step pass/fail table. Exits non-zero on any verification or step
/// failure.
fn cmd_test(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let opts = Opts::parse(args)?;
    let source = opts.require_source()?;
    let capsule = load_capsule(source)?;
    let toml = capsule
        .manifest_bytes()
        .ok_or("no quack.toml in the package")?;
    let manifest = quack::parse_manifest(toml)?;
    quack::validate(&manifest)?;
    quack::verify_digests(&capsule, &manifest)?;
    println!("digests  ok ({} referenced files)", digest_count(&manifest));
    let status = signature_status(&capsule, toml);
    println!("signature {status}");
    if status.starts_with("invalid") {
        return Err("signature verification failed".into());
    }

    let fixture = quack_harness::GoldenFixture::from_capsule(&capsule)
        .map_err(|e| format!("golden fixture: {e}"))?;
    let extras = native_catalog(&manifest, &fixture.bindings)?;
    let labels: Vec<&'static str> = fixture.steps.iter().map(|s| s.label()).collect();
    println!("golden   {} steps against the native catalog", labels.len());

    // the testbed owns its deterministic runtime; the whole golden run is
    // in-process and side-effect free.
    let outcome = quack_harness::PackageTestBed::run(extras, |mut bed| async move {
        quack_harness::run_golden(&mut bed, &capsule, &fixture).await
    });
    match outcome {
        Ok(run) => {
            for (i, label) in run.steps.iter().enumerate() {
                println!("  ok   {:>3} {label}", i + 1);
            }
            println!(
                "ok {} {} ({} steps)",
                manifest.package,
                manifest.version,
                run.steps.len()
            );
            Ok(())
        }
        Err(e) => {
            for (i, label) in labels.iter().take(e.step - 1).enumerate() {
                println!("  ok   {:>3} {label}", i + 1);
            }
            println!("  FAIL {:>3} {}: {}", e.step, e.label, e.message);
            Err(format!("golden harness failed at step {} ({})", e.step, e.label).into())
        }
    }
}

/// The binary's native module catalog: map each manifest `[[modules]]` entry
/// to what this binary can supply. Platform logicals resolve to the testbed's
/// standard set; package logicals resolve to in-binary constructors (v1 knows
/// the docs package's modules). Anything else is a clear rejection.
fn native_catalog(
    manifest: &quack::PackageManifest,
    bindings: &BTreeMap<String, String>,
) -> Result<Vec<Box<dyn sdk::Module>>, Box<dyn std::error::Error>> {
    let mut extras: Vec<Box<dyn sdk::Module>> = Vec::new();
    for entry in &manifest.modules {
        let concrete = bindings
            .get(&entry.logical)
            .unwrap_or(&entry.default_id)
            .clone();
        match entry.logical.as_str() {
            // the testbed's standard platform set already registers pages.
            "pages" => {
                if concrete != "pages" {
                    return Err(format!(
                        "module {:?} binds to {concrete:?}, but the testbed's platform set \
                         registers it as \"pages\"",
                        entry.logical
                    )
                    .into());
                }
            }
            "docs-harness" => extras.push(Box::new(docs_harness::DocsHarness::new(
                concrete, "package", "agent", "jobs", "memory", "pages", "runs",
            ))),
            other => {
                return Err(format!(
                    "module {other:?} is not in this binary's native catalog \
                     (v1 knows: pages, docs-harness)"
                )
                .into());
            }
        }
    }
    Ok(extras)
}

/// Describe the signature state of a capsule: unsigned, signed-by, or invalid.
fn signature_status(capsule: &quack::Capsule, toml: &[u8]) -> String {
    let Some(raw) = capsule.files.get("signatures/package.sig") else {
        return "unsigned".to_string();
    };
    let sig: quack::PackageSig = match serde_json::from_slice(raw) {
        Ok(sig) => sig,
        Err(e) => return format!("invalid (malformed package.sig: {e})"),
    };
    if quack::verify_manifest_sig(&sig, &quack::manifest_hash(toml)) {
        format!("signed by {}", hex_bytes(&sig.signer))
    } else {
        format!("invalid (bad signature from {})", hex_bytes(&sig.signer))
    }
}

fn emit_hash(
    capsule: &quack::Capsule,
    logical: &str,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = capsule
        .files
        .get(path)
        .ok_or_else(|| format!("{logical}: {path} is missing from the package"))?;
    println!(
        "{logical} {path} = \"sha256:{}\"",
        hex_bytes(&quack::file_digest(bytes))
    );
    Ok(())
}

fn digest_count(m: &quack::PackageManifest) -> usize {
    m.prompts.len()
        + m.modules
            .iter()
            .filter(|e| e.artifact.is_some() && e.hash.is_some())
            .count()
}

fn default_out(dir: &Path, m: &quack::PackageManifest) -> PathBuf {
    // `<last path component or package id>.quack`, in the current directory.
    let stem = dir
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && *s != ".")
        .map(str::to_string)
        .unwrap_or_else(|| m.package.clone());
    PathBuf::from(format!("{stem}.quack"))
}

fn load_capsule(path: &Path) -> Result<quack::Capsule, Box<dyn std::error::Error>> {
    if path.is_dir() {
        Ok(quack::open_dir(path)?)
    } else {
        let bytes = std::fs::read(path).map_err(|e| format!("read {path:?}: {e}"))?;
        Ok(quack::open_tar(&bytes)?)
    }
}

/// The package verbs' flag set — a small, self-contained parser (the shared
/// `parse_flags` requires a value for every flag, which `--emit-hashes` is not,
/// and does not know `-o`).
#[derive(Default)]
struct Opts {
    positional: Vec<String>,
    out: Option<PathBuf>,
    key: Option<PathBuf>,
    emit_hashes: bool,
}

impl Opts {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut o = Opts::default();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "-o" | "--out" => {
                    o.out = Some(PathBuf::from(next(&mut it, "--out")?));
                }
                "--key" => {
                    o.key = Some(PathBuf::from(next(&mut it, "--key")?));
                }
                "--emit-hashes" => o.emit_hashes = true,
                other if other.starts_with('-') => {
                    return Err(format!("unknown flag {other:?}"));
                }
                other => o.positional.push(other.to_string()),
            }
        }
        Ok(o)
    }

    fn require_source(&self) -> Result<&Path, String> {
        match self.positional.as_slice() {
            [one] => Ok(Path::new(one)),
            [] => Err("missing <dir|.quack> argument".to_string()),
            many => Err(format!("expected one <dir|.quack>, got {many:?}")),
        }
    }
}

fn next<'a>(it: &mut std::slice::Iter<'a, String>, flag: &str) -> Result<&'a String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
}
