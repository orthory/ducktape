//! the agent CLIs as a block device, derived from the operator's executors
//! directory.
//!
//! WHY A DEVICE AND NOT THE ROOTFS. The CLIs used to be baked into the shared
//! guest rootfs by `ops/build-guest-rootfs.sh`. That made the 500 MB image the
//! unit of installation — adding one CLI meant rebuilding all of it — and it
//! put the same fact in three places: the host `PATH` a node announced from,
//! the executors directory an operator filled, and the image bytes a run
//! actually exec'd. They drifted, and the drift was invisible until a run
//! produced nothing.
//!
//! The image is DERIVED, never authored: whatever the directory holds is what
//! the guest gets, and this module rebuilds it whenever the directory has moved
//! on. So there is no install step to forget and no staleness for an operator
//! to reason about — the directory is the only thing anyone edits.
//!
//! Shared read-only across concurrent runs, like the rootfs and for the same
//! reason: a writable copy would let one buyer's run edit the CLI another
//! buyer's run is about to exec.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

/// the image built from `dir` — its sibling, so a moved executors directory
/// takes its image with it.
pub fn path_for(dir: &Path) -> PathBuf {
    dir.with_extension("img")
}

/// The image for `dir`, built if it is missing or older than the directory.
///
/// Returns `None` for an executors directory that holds nothing: a node with no
/// agent CLI installed announces no provider capabilities, so there is no image
/// to build and nothing for a run to mount.
pub fn ensure(dir: &Path) -> Result<Option<PathBuf>, String> {
    let image = path_for(dir);
    let newest = newest_mtime(dir)?;
    let Some(newest) = newest else {
        return Ok(None);
    };
    let built = std::fs::metadata(&image)
        .ok()
        .and_then(|m| m.modified().ok());
    let is_current = built.is_some_and(|built| built >= newest);
    if is_current {
        return Ok(Some(image));
    }
    build(dir, &image)?;
    Ok(Some(image))
}

/// Build to a private path and rename over the target: two service daemons
/// discover their providers independently, so both may find the image stale at
/// once. Renaming makes the loser's work redundant rather than corrupting the
/// image the winner is already attaching to a live VM.
fn build(dir: &Path, image: &Path) -> Result<(), String> {
    refuse_foreign_binaries(dir)?;
    let bytes = crate::workspace_image::sized_for_read_only(dir)?;
    let staging = image.with_extension(format!("img.{}", std::process::id()));
    let started = std::time::Instant::now();
    crate::workspace_image::build(dir, &staging, bytes)?;
    std::fs::rename(&staging, image).map_err(|e| {
        let _ = std::fs::remove_file(&staging);
        format!("install {}: {e}", image.display())
    })?;
    tracing::info!(
        target: "ducktape::sandbox",
        event = "executor_image_built",
        image = %image.display(),
        bytes,
        build_ms = started.elapsed().as_millis() as u64,
        "built the agent CLI image from the executors directory"
    );
    Ok(())
}

/// Refuse to build an image around an executable the guest could not exec.
///
/// The guest is Linux on every host, so a macOS operator who copies their own
/// `claude` into the executors directory has installed a Mach-O binary. Nothing
/// downstream can tell: the file is executable, the node announces the
/// capability, the VM boots, and the run ends having produced nothing. Caught
/// here it is one sentence naming the file.
///
/// Refusing the whole image rather than skipping the file is deliberate — the
/// directory is small and hand-curated, so a stray binary in it is a mistake to
/// report, not a condition to route around.
fn refuse_foreign_binaries(dir: &Path) -> Result<(), String> {
    const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
        let path = entry.path();
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        let is_command = meta.is_file() && meta.permissions().mode() & 0o111 != 0;
        if !is_command {
            continue;
        }
        let mut head = [0u8; 4];
        let read = std::fs::File::open(&path)
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head));
        if read.is_ok() && head == ELF_MAGIC {
            continue;
        }
        return Err(format!(
            "{} is not a Linux executable, and every run executes inside a Linux guest; \
             replace it with `ducktape agent install {}` or remove it",
            path.display(),
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    Ok(())
}

/// the newest mtime among the directory's top-level entries, or `None` when it
/// holds none. Top level only, because that is what a PATH directory means and
/// what the image is built from.
///
/// Public because it is the ONE staleness signal for this directory: the guest
/// image rebuilds on it ([`ensure`]) and the service daemon re-derives its
/// hello on it. Two answers keyed off the same clock can never disagree about
/// whether a newly installed CLI exists.
pub fn newest_mtime(dir: &Path) -> Result<Option<std::time::SystemTime>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // an operator who has installed nothing has no directory, which is a
        // node with no providers — not an error.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", dir.display())),
    };
    let mut newest = None;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        newest = newest.max(Some(modified));
    }
    Ok(newest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dt-exec-img-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// an executable that passes the guest-executable check: only its first
    /// four bytes are ever read.
    fn install(dir: &Path, name: &str) {
        install_bytes(dir, name, b"\x7fELF and then some");
    }

    fn install_bytes(dir: &Path, name: &str, body: &[u8]) {
        let path = dir.join(name);
        std::fs::write(&path, body).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    /// A node with nothing installed has no image and no error: it announces no
    /// provider capabilities, so a run that could mount one cannot exist.
    #[test]
    fn an_empty_executors_directory_builds_no_image() {
        let root = scratch("empty");
        let dir = root.join("executors");
        assert_eq!(ensure(&dir).expect("absent dir is not an error"), None);
        std::fs::create_dir_all(&dir).expect("dir");
        assert_eq!(ensure(&dir).expect("empty dir is not an error"), None);
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    /// The image is DERIVED: installing a CLI after it was built must produce a
    /// new image without anyone asking for one. This is the failure the old
    /// bake-into-the-rootfs design turned into "the run produced nothing".
    #[test]
    fn a_directory_that_moved_on_rebuilds_the_image() {
        if crate::host_tools::find_system_tool("mke2fs").is_none() {
            return; // e2fsprogs is a host prerequisite, not a test dependency
        }
        let dir = scratch("stale").join("executors");
        std::fs::create_dir_all(&dir).expect("dir");
        install(&dir, "codex");

        let image = ensure(&dir).expect("build").expect("an image");
        let first = std::fs::metadata(&image).unwrap().modified().unwrap();

        // unchanged directory: the same image, not a rebuild.
        assert_eq!(ensure(&dir).expect("reuse"), Some(image.clone()));
        assert_eq!(
            std::fs::metadata(&image).unwrap().modified().unwrap(),
            first
        );

        // a newly installed CLI is newer than the image, so it is rebuilt.
        let ahead = first + std::time::Duration::from_secs(60);
        install(&dir, "claude");
        std::fs::File::open(dir.join("claude"))
            .unwrap()
            .set_modified(ahead)
            .unwrap();
        ensure(&dir).expect("rebuild");
        assert!(
            std::fs::metadata(&image).unwrap().modified().unwrap() > first,
            "an executors directory that moved on must produce a new image"
        );

        std::fs::remove_dir_all(dir.parent().unwrap()).expect("cleanup");
    }

    /// The guest is Linux on every host, so a Mach-O binary an operator copied
    /// in by hand is a run that produces nothing — unless it is named here.
    #[test]
    fn a_binary_the_guest_could_not_exec_is_named_and_refused() {
        let dir = scratch("foreign").join("executors");
        std::fs::create_dir_all(&dir).expect("dir");
        install(&dir, "codex");
        // Mach-O 64-bit's magic, which is what a macOS operator's own CLI is.
        install_bytes(&dir, "claude", b"\xcf\xfa\xed\xfe and then some");

        let error = ensure(&dir).expect_err("a foreign binary must refuse");
        assert!(error.contains("claude"), "names the file: {error}");
        assert!(
            error.contains("ducktape agent install"),
            "names the fix: {error}"
        );
        // A non-executable file is data, not a foreign binary, and passes.
        std::fs::remove_file(dir.join("claude")).expect("rm");
        std::fs::write(dir.join("notes.txt"), b"not a binary").expect("write");
        assert!(ensure(&dir).is_ok() || crate::host_tools::find_system_tool("mke2fs").is_none());

        std::fs::remove_dir_all(dir.parent().unwrap()).expect("cleanup");
    }
}
