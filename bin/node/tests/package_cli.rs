//! Smoke tests for the `ducktape-node package inspect|build|verify|test`
//! verbs.
//!
//! Drives the real built binary (`CARGO_BIN_EXE_ducktape-node`) against the
//! reference `packages/docs/` source: verify passes, inspect prints the
//! manifest surface, build is byte-deterministic and re-verifies, test runs
//! the capsule's golden harness in-process and prints a per-step pass table,
//! and a tampered package fails both verbs with a non-zero exit.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ducktape-node")
}

fn fixture() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packages/docs"))
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("run ducktape-node package")
}

#[test]
fn verify_accepts_the_reference_package() {
    let out = run(&["package", "verify", fixture().to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "verify failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("digests  ok"), "stdout: {stdout}");
    assert!(
        stdout.contains("ok org.ducktape.docs 0.1.0"),
        "stdout: {stdout}"
    );
}

#[test]
fn inspect_prints_modules_actions_and_agents() {
    let out = run(&["package", "inspect", fixture().to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "inspect failed: {stdout}");
    for needle in [
        "org.ducktape.docs 0.1.0",
        "pages -> pages [native]",
        "docs-harness -> docs-harness [native]",
        "pages.comment.add -> docs-harness",
        "pages.thread.resolve -> docs-harness",
        "docs.editor",
    ] {
        assert!(stdout.contains(needle), "missing {needle:?} in:\n{stdout}");
    }
}

#[test]
fn build_is_deterministic_and_reverifies() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let a = tmp.path().join("a.quack");
    let b = tmp.path().join("b.quack");
    let fixture = fixture();
    let src = fixture.to_str().unwrap();

    for out in [&a, &b] {
        let r = run(&["package", "build", src, "-o", out.to_str().unwrap()]);
        assert!(
            r.status.success(),
            "build failed: {}",
            String::from_utf8_lossy(&r.stderr)
        );
    }
    let ba = std::fs::read(&a).unwrap();
    let bb = std::fs::read(&b).unwrap();
    assert_eq!(ba, bb, "two builds of the same source are byte-identical");

    let r = run(&["package", "verify", a.to_str().unwrap()]);
    assert!(
        r.status.success(),
        "verify of built .quack failed: {}",
        String::from_utf8_lossy(&r.stderr)
    );
}

#[test]
fn test_runs_the_golden_harness_and_prints_a_step_table() {
    let out = run(&["package", "test", fixture().to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "package test failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // the verification preamble ran.
    assert!(stdout.contains("digests  ok"), "stdout: {stdout}");
    // the per-step pass table names the fixture's step kinds.
    for needle in ["install", "expect_job", "oracle", "snapshot_roundtrip"] {
        assert!(
            stdout.contains(&format!(" {needle}")),
            "missing step {needle:?} in table:\n{stdout}"
        );
    }
    assert!(stdout.contains("ok org.ducktape.docs"), "stdout: {stdout}");
}

#[test]
fn test_also_accepts_a_built_capsule() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let quack = tmp.path().join("docs.quack");
    let r = run(&[
        "package",
        "build",
        fixture().to_str().unwrap(),
        "-o",
        quack.to_str().unwrap(),
    ]);
    assert!(r.status.success());
    let out = run(&["package", "test", quack.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "package test of the .quack failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn test_rejects_a_tampered_package_before_running_any_step() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("docs");
    copy_dir(&fixture(), &dir);
    let prompt = dir.join("prompts/docs-editor.md");
    let mut body = std::fs::read(&prompt).unwrap();
    body.extend_from_slice(b"tampered");
    std::fs::write(&prompt, body).unwrap();

    let out = run(&["package", "test", dir.to_str().unwrap()]);
    assert!(
        !out.status.success(),
        "test must fail on a tampered package"
    );
    // the digest check runs BEFORE the golden replay: the failure must name
    // the tampered file, and stdout must show no step (not even the
    // "digests  ok" preamble) ever ran.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("digest mismatch") && stderr.contains("prompts/docs-editor.md"),
        "the error must name the digest/tamper failure, got stderr:\n{stderr}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.is_empty(),
        "no step (or the verification preamble) may have run, got stdout:\n{stdout}"
    );
}

#[test]
fn test_names_a_module_outside_the_native_catalog() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("docs");
    copy_dir(&fixture(), &dir);
    // rename the harness module to something no binary constructor knows.
    let manifest = dir.join("quack.toml");
    let toml = std::fs::read_to_string(&manifest)
        .unwrap()
        .replace("docs-harness", "ghost-harness");
    std::fs::write(&manifest, toml).unwrap();

    let out = run(&["package", "test", dir.to_str().unwrap()]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("native catalog") && stderr.contains("ghost-harness"),
        "a readable catalog rejection, got:\n{stderr}"
    );
}

#[test]
fn verify_rejects_a_tampered_package() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("docs");
    copy_dir(&fixture(), &dir);
    // append to the prompt so it no longer hashes to the manifest's digest.
    let prompt = dir.join("prompts/docs-editor.md");
    let mut body = std::fs::read(&prompt).unwrap();
    body.extend_from_slice(b"tampered");
    std::fs::write(&prompt, body).unwrap();

    let out = run(&["package", "verify", dir.to_str().unwrap()]);
    assert!(
        !out.status.success(),
        "verify must fail on a tampered package"
    );
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dst = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &dst);
        } else {
            std::fs::copy(entry.path(), dst).unwrap();
        }
    }
}
