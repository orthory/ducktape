//! the shared `git` shell-out primitive.
//!
//! every git invocation in this crate — porcelain verbs ([`crate::cmd`]), the
//! object store ([`crate::odb`]), the bytes bridge ([`crate::objects`]) — goes
//! through [`run`]. it is **binary-safe**: stdout is returned as raw `Vec<u8>`,
//! because git emits non-utf8 data (packfiles, raw tree/commit object bodies).
//! decoding to a `String` happens only at call sites that know their output is
//! text (the porcelain in [`crate::cmd`]).

use std::io::Write;
use std::path::Path;
use std::process::{Command as Process, Stdio};

use crate::object::ObjectId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("git io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("git exited with status {0}: {1}")]
    NonZeroExit(i32, String),
    #[error("git returned an unparseable oid: {0}")]
    BadOid(String),
    #[error("malformed loose object (missing '<type> <len>\\0' header)")]
    MalformedObject,
}

/// raw, binary-safe captured output of a successful `git` invocation.
pub struct RawOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// run `git <args>` in `repo`, optionally feeding `stdin`, capturing stdout as
/// raw bytes. non-zero exit maps to [`Error::NonZeroExit`] (stderr is lossy-
/// decoded for the message only).
///
/// stdin is written on a separate thread, then the pipe is dropped (EOF), so a
/// payload larger than the OS pipe buffer can't deadlock against git blocking on
/// its own stdout. matters because `pack-objects` / `unpack-objects` stream.
pub fn run(repo: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<RawOutput, Error> {
    let mut child = Process::new("git")
        .current_dir(repo)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let writer = stdin.map(|bytes| {
        let mut pipe = child.stdin.take().expect("stdin piped above");
        let owned = bytes.to_vec();
        // pipe is moved in and dropped at thread end -> git sees EOF.
        std::thread::spawn(move || pipe.write_all(&owned))
    });

    let output = child.wait_with_output()?;
    if let Some(handle) = writer {
        // surface a stdin write failure (e.g. git closed the pipe early).
        handle.join().expect("stdin writer thread panicked")?;
    }

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(Error::NonZeroExit(
            code,
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(RawOutput {
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

/// run `git <args>` for its exit status only (output discarded). used for
/// existence checks like `cat-file -e`, where a non-zero exit is a normal
/// answer ("absent"), not a fault. only a spawn failure is an [`Error`].
pub fn run_ok(repo: &Path, args: &[&str]) -> Result<bool, Error> {
    let status = Process::new("git")
        .current_dir(repo)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(status.success())
}

/// parse a git oid printed on stdout (hex + trailing whitespace/newline) into an
/// [`ObjectId`]. shared by [`crate::odb`] and [`crate::objects`].
pub(crate) fn parse_oid(stdout: &[u8]) -> Result<ObjectId, Error> {
    let hex = String::from_utf8_lossy(stdout);
    let hex = hex.trim();
    ObjectId::from_hex(hex).map_err(|_| Error::BadOid(hex.to_string()))
}

/// fallback identity for object-creating actions (commit, commit-tree, annotated
/// tag, non-ff merge) so they work on hosts with no global git identity (CI).
/// injected via `-c` per-invocation; never touches global config.
pub fn identity_args(name: &str) -> [String; 4] {
    [
        "-c".to_string(),
        format!("user.name={name}"),
        "-c".to_string(),
        format!("user.email={name}@localhost"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sha256_repo() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vcs-git-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        // `--template=` keeps init hermetic (no shared-template-dir race).
        run(&dir, &["init", "--object-format=sha256", "--template="], None).expect("init sha256");
        dir
    }

    // the load-bearing property: stdout is captured as raw bytes, so a blob
    // with NON-utf8 content survives a write -> read round-trip intact. a
    // String-based capture (from_utf8_lossy) would corrupt this silently.
    #[test]
    fn binary_stdout_is_not_corrupted() {
        let repo = sha256_repo();
        let payload: Vec<u8> = vec![0x00, 0xff, 0xfe, 0x80, b'h', b'i', 0x01, 0x00];

        let oid_out = run(&repo, &["hash-object", "-w", "--stdin"], Some(&payload)).expect("write");
        let oid = String::from_utf8(oid_out.stdout).unwrap();
        let oid = oid.trim();
        assert_eq!(oid.len(), 64, "sha256 oid");

        let back = run(&repo, &["cat-file", "blob", oid], None).expect("read");
        assert_eq!(back.stdout, payload, "non-utf8 bytes must round-trip exactly");

        fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn run_ok_reports_existence() {
        let repo = sha256_repo();
        let oid_out = run(&repo, &["hash-object", "-w", "--stdin"], Some(b"hi\n")).expect("write");
        let oid = String::from_utf8(oid_out.stdout).unwrap();
        let oid = oid.trim();

        assert!(run_ok(&repo, &["cat-file", "-e", oid]).unwrap(), "present");
        let absent = "0".repeat(64);
        assert!(
            !run_ok(&repo, &["cat-file", "-e", &absent]).unwrap(),
            "absent -> false, not an error"
        );

        fs::remove_dir_all(&repo).ok();
    }
}
