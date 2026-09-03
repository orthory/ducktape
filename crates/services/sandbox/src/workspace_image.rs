//! the run workspace as a block device.
//!
//! Firecracker's device model is deliberately minimal — virtio-block, -net,
//! -vsock, -balloon, -rng and a serial console, nothing else. There is no
//! virtio-fs, so the workspace is handed over as a per-run ext4 image: built
//! from the workdir before boot, mounted by the guest init, and read back after
//! the guest reports its exit code.
//!
//! Both directions are ROOTLESS and mount-free. `mke2fs -d` populates an image
//! from a directory without ever mounting it, and `debugfs -R rdump` walks one
//! back out. That matters: a loop mount would need root, and a node that needs
//! root to move a workspace is a node that runs as root.
//!
//! The round trip is CPU-bound, not I/O-bound — measured at 186 MB/s and 8,860
//! files/s against an NVMe that sustains 2.9 GB/s, with `mke2fs` at 99% of one
//! core. Do not reach for faster storage to speed this up; the spec's *Build
//! caches* section records the measurement and the conclusion drawn from it.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// the floor: ext4 metadata plus a journal does not fit in a few hundred KiB,
/// and `mke2fs` silently drops the journal below ~16 MiB ("Filesystem too small
/// for a journal"). A journal-less workspace image is a torn tree after a hard
/// VM kill, so the floor is above that threshold rather than at it.
pub const MIN_WORKSPACE_BYTES: u64 = 32 * 1024 * 1024;

/// the ceiling, refused before a VM boots. A run whose workspace does not fit
/// cannot be salvaged by retrying, so it must fail at submit-adjacent time.
pub const MAX_WORKSPACE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// headroom multiplier over the measured tree, for an image the guest WRITES
/// into: it needs room for the run's output, not just its input.
const HEADROOM: u64 = 3;

/// spare room over the measured tree for an image nothing writes into. Not
/// headroom — ext4's own metadata (inodes, bitmaps, the journal) does not fit
/// in the file bytes alone, and `mke2fs -d` fails with `No space left` if the
/// image is sized at exactly the payload.
const READ_ONLY_MARGIN_PERCENT: u64 = 20;

/// the image size for a WRITABLE tree: measured × [`HEADROOM`], floored at
/// [`MIN_WORKSPACE_BYTES`] and refused above [`MAX_WORKSPACE_BYTES`].
pub fn sized_for(workdir: &Path) -> Result<u64, String> {
    let measured = tree_bytes(workdir)?;
    size_or_refuse(
        "workspace",
        workdir,
        measured.saturating_mul(HEADROOM).max(MIN_WORKSPACE_BYTES),
    )
}

/// the image size for a READ-ONLY tree: measured plus a metadata margin, and
/// no headroom at all.
///
/// Tripling a read-only image was not merely wasteful — it decided runs. A
/// run's read-only inputs are its PATH commands, and on a machine where those
/// include a build directory they measure gigabytes; at ×3 the same tree
/// crossed the cap and the run was REFUSED for a size two thirds of which was
/// zeroes nothing could ever write to.
pub fn sized_for_read_only(dir: &Path) -> Result<u64, String> {
    let measured = tree_bytes(dir)?;
    let margin = measured / 100 * READ_ONLY_MARGIN_PERCENT;
    size_or_refuse(
        "read-only inputs",
        dir,
        measured
            .saturating_add(margin)
            .max(MIN_WORKSPACE_BYTES),
    )
}

/// the size decision alone, split out so the refusal is unit-testable without
/// materialising gigabytes on disk.
///
/// `what` and `dir` are in the message because there are two images and one
/// used to say "workspace" for both — sending a reader to inspect a 4 KiB
/// workspace while the 3 GB asset tree that actually blew the cap went
/// unnamed.
pub fn size_or_refuse(what: &str, dir: &Path, size: u64) -> Result<u64, String> {
    if size > MAX_WORKSPACE_BYTES {
        // countable, because "this node refuses every run" and "this one tree
        // is too big" look identical from the outside until you can count them.
        tracing::warn!(
            target: "ducktape::sandbox",
            reason = "image_over_cap",
            what,
            size,
            cap = MAX_WORKSPACE_BYTES,
            "refusing to build an image over the cap"
        );
        return Err(format!(
            "the {what} at {} need {size} bytes of image, over the \
             {MAX_WORKSPACE_BYTES}-byte cap",
            dir.display()
        ));
    }
    Ok(size)
}

fn tree_bytes(dir: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let entries = std::fs::read_dir(&next)
            .map_err(|e| format!("measure workspace {}: {e}", next.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("measure workspace: {e}"))?;
            let meta = entry
                .metadata()
                .map_err(|e| format!("measure {}: {e}", entry.path().display()))?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

/// build an ext4 image of exactly `bytes` populated from `workdir`.
pub fn build(workdir: &Path, image: &Path, bytes: u64) -> Result<(), String> {
    let started = std::time::Instant::now();
    let tool = crate::host_tools::find_system_tool("mke2fs")
        .ok_or_else(|| "mke2fs is not on PATH; install e2fsprogs".to_string())?;
    let blocks = bytes.div_ceil(4096);
    let out = Command::new(&tool)
        .args(["-q", "-t", "ext4", "-b", "4096", "-d"])
        .arg(workdir)
        .arg(image)
        .arg(blocks.to_string())
        .output()
        .map_err(|e| format!("run mke2fs: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "mke2fs exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    // the other half of a run's fixed cost, beside the copy back.
    tracing::debug!(
        target: "ducktape::sandbox",
        bytes,
        build_ms = started.elapsed().as_millis() as u64,
        "workspace image built"
    );
    Ok(())
}

/// walk `image` back out into `dest`, which is created if absent.
pub fn read_back(image: &Path, dest: &Path) -> Result<(), String> {
    let tool = crate::host_tools::find_system_tool("debugfs")
        .ok_or_else(|| "debugfs is not on PATH; install e2fsprogs".to_string())?;
    std::fs::create_dir_all(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    // `rdump / <dest>` lands the image root's entries DIRECTLY in dest — no
    // wrapper directory. Verified on a real tree: the round trip is
    // byte-identical, modes included, once `lost+found` is dropped.
    let out = Command::new(&tool)
        .arg("-R")
        .arg(format!("rdump / {}", dest.display()))
        .arg(image)
        .output()
        .map_err(|e| format!("run debugfs: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "debugfs exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    drop_lost_found(dest)
}

/// build the per-run READ-ONLY asset image: the context doc, the skills tree,
/// and any host PATH directories the run declared.
///
/// Its layout mirrors the guest's: entry N of `assets` lands at `ro<N>/`, and a
/// FILE lands at the image root under its own name — one level above the
/// workspace, so a `workspace-parent` context doc still resolves as
/// `../<name>`. An empty `workspace/` directory is always created, because the
/// workspace image is mounted on top of it.
///
/// Never read back: nothing the run writes here survives, which is the point of
/// it being a separate device rather than part of the workspace. Handing the
/// skills tree back to the buyer as if the run had produced it would be wrong,
/// and on a second round trip it would nest.
pub fn build_assets(assets: &[GuestAsset], image: &Path, staging: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(staging);
    std::fs::create_dir_all(staging.join("workspace"))
        .map_err(|e| format!("stage {}: {e}", staging.display()))?;

    for (index, asset) in assets.iter().enumerate() {
        let target = staging.join(format!("ro{index}"));
        match asset {
            GuestAsset::Commands(source) => stage_commands(source, &target)?,
            GuestAsset::Whole(source) => stage_whole(source, staging, &target)?,
        }
    }

    // Removed on EVERY exit, not just the happy one. Staged inputs are a copy
    // of what already exists elsewhere, and the tree runs to gigabytes when a
    // PATH entry is a build directory — a `?` straight to the caller left one
    // behind per failed run, and they accumulate under the run root until
    // something notices the disk. Measured: 21 GB across one afternoon's runs.
    let built = sized_for_read_only(staging).and_then(|size| build(staging, image, size));
    let _ = std::fs::remove_dir_all(staging);
    built
}

/// one read-only input, and HOW MUCH of it crosses into the guest.
///
/// The distinction exists because a VM copies where a container bind-mounted.
/// Under a bind mount the size of a declared directory cost nothing, so nobody
/// had to think about it; a copy makes it the run's latency and the node's
/// disk, per run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestAsset {
    /// a PATH entry. Only the executables at its TOP LEVEL cross.
    ///
    /// That is not a heuristic, it is what a PATH entry means: resolution never
    /// recurses into a subdirectory and never resolves a non-executable, so
    /// nothing else in the tree is nameable from inside the guest. Measured on
    /// this repo's own node binary, whose PATH entry is `target/debug`: a 39 GB
    /// tree holding exactly ONE file — 0.95 GB — that a run could ever invoke.
    /// Copying the tree filled a 9.1 GB tmpfs and failed the run with
    /// `No space left on device` while copying an `.rlib`.
    Commands(PathBuf),
    /// a tree the run reads, or a single file: the skills root, the assembled
    /// context doc. Copied entire — every byte of these is readable input.
    Whole(PathBuf),
}

impl GuestAsset {
    pub fn path(&self) -> &Path {
        match self {
            Self::Commands(path) | Self::Whole(path) => path,
        }
    }
}

/// stage the executables a PATH entry actually offers, and nothing else.
fn stage_commands(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    let entries = std::fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", from.display()))?;
        let source = entry.path();
        // metadata(), not symlink_metadata(): a PATH directory of symlinks into
        // a versioned store is the normal shape, and the target is the command.
        let Ok(meta) = std::fs::metadata(&source) else {
            continue;
        };
        let is_command = meta.is_file() && meta.permissions().mode() & 0o111 != 0;
        if !is_command {
            continue;
        }
        let target = to.join(entry.file_name());
        std::fs::copy(&source, &target).map_err(|e| format!("copy {}: {e}", source.display()))?;
    }
    Ok(())
}

/// stage a whole input: a directory tree at `to`, or a file at the image root.
///
/// A file lands one level ABOVE the workspace under its own name, so a
/// `workspace-parent` context doc still resolves as `../<name>`.
fn stage_whole(source: &Path, staging: &Path, to: &Path) -> Result<(), String> {
    let meta =
        std::fs::metadata(source).map_err(|e| format!("stat asset {}: {e}", source.display()))?;
    if !meta.is_file() {
        return copy_tree(source, to);
    }
    let name = source
        .file_name()
        .ok_or_else(|| format!("asset {} has no file name", source.display()))?;
    std::fs::copy(source, staging.join(name))
        .map_err(|e| format!("stage asset {}: {e}", source.display()))?;
    Ok(())
}

/// copy a directory tree, preserving mode bits. Symlinks are followed rather
/// than recreated: a link inside a staged tree would point at a HOST path that
/// does not exist in the guest, which is a broken input rather than a preserved
/// one.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    let entries = std::fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", from.display()))?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let meta = match std::fs::metadata(&source) {
            Ok(meta) => meta,
            // a dangling symlink in a skills tree is not worth failing a run
            Err(_) => continue,
        };
        if meta.is_dir() {
            copy_tree(&source, &target)?;
            continue;
        }
        std::fs::copy(&source, &target).map_err(|e| format!("copy {}: {e}", source.display()))?;
    }
    Ok(())
}

/// `lost+found` is an ext4 artifact `mke2fs` creates, not something the run
/// produced. Handing it back would add a directory to the buyer's workspace
/// that nobody put there — and on a second round trip it would persist and
/// multiply.
fn drop_lost_found(dest: &Path) -> Result<(), String> {
    let stray = dest.join("lost+found");
    match std::fs::remove_dir_all(&stray) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("remove {}: {e}", stray.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ducktape-wsimg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn have_e2fsprogs() -> bool {
        crate::host_tools::find_system_tool("mke2fs").is_some()
            && crate::host_tools::find_system_tool("debugfs").is_some()
    }

    /// The round trip is the whole contract: what the guest writes has to come
    /// A PATH entry offers exactly the commands at its top level. Copying the
    /// tree instead is not merely wasteful, it is unbounded: the node's own
    /// binary sits in a build directory whose tree measured 39 GB against the
    /// 0.95 GB of it a run can name, and copying it filled a tmpfs and failed
    /// the run with `No space left on device`.
    #[test]
    fn a_path_entry_hands_over_its_commands_and_nothing_else() {
        let root = scratch("path-entry");
        let from = root.join("bin");
        std::fs::create_dir_all(from.join("deps")).expect("tree");
        std::fs::write(from.join("tool"), b"#!/bin/sh\n").expect("command");
        std::fs::set_permissions(from.join("tool"), std::fs::Permissions::from_mode(0o755))
            .expect("chmod");
        std::fs::write(from.join("notes.txt"), b"not a command").expect("data file");
        std::fs::write(from.join("deps").join("huge.rlib"), b"not reachable").expect("nested");

        let to = root.join("staged");
        stage_commands(&from, &to).expect("stage");

        let mut staged: Vec<String> = std::fs::read_dir(&to)
            .expect("read staged")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        staged.sort();
        assert_eq!(
            staged,
            vec!["tool".to_string()],
            "PATH resolution never recurses and never resolves a non-executable"
        );
        let mode = std::fs::metadata(to.join("tool"))
            .expect("stat")
            .permissions()
            .mode();
        assert!(mode & 0o111 != 0, "it must still be executable: {mode:o}");
        std::fs::remove_dir_all(root).ok();
    }

    /// back byte-identical, with the mode bits intact — an agent run that
    /// produces an executable must not hand back a non-executable one.
    #[test]
    fn a_workspace_survives_the_round_trip_with_modes_and_nesting() {
        use std::os::unix::fs::PermissionsExt as _;
        if !have_e2fsprogs() {
            return;
        }
        let root = scratch("round-trip");
        let src = root.join("src");
        std::fs::create_dir_all(src.join("nested/deeper")).expect("nested");
        std::fs::write(src.join("plain.txt"), b"hello workspace").expect("plain");
        std::fs::write(src.join("nested/deeper/leaf.bin"), [0u8, 1, 2, 255]).expect("leaf");
        let script = src.join("run.sh");
        std::fs::write(&script, b"#!/bin/sh\necho hi\n").expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");

        let image = root.join("ws.img");
        let size = sized_for(&src).expect("size");
        build(&src, &image, size).expect("build");

        let out = root.join("out");
        read_back(&image, &out).expect("read back");

        assert_eq!(
            std::fs::read(out.join("plain.txt")).expect("plain back"),
            b"hello workspace"
        );
        assert_eq!(
            std::fs::read(out.join("nested/deeper/leaf.bin")).expect("leaf back"),
            [0u8, 1, 2, 255]
        );
        let mode = std::fs::metadata(out.join("run.sh"))
            .expect("script back")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755, "the executable bit must survive");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// `lost+found` is the filesystem's, not the run's. It must never appear in
    /// a workspace handed back to a buyer.
    #[test]
    fn the_read_back_does_not_hand_back_lost_and_found() {
        if !have_e2fsprogs() {
            return;
        }
        let root = scratch("lost-found");
        let src = root.join("src");
        std::fs::create_dir_all(&src).expect("src");
        std::fs::write(src.join("only.txt"), b"x").expect("only");

        let image = root.join("ws.img");
        build(&src, &image, sized_for(&src).expect("size")).expect("build");
        let out = root.join("out");
        read_back(&image, &out).expect("read back");

        let mut names: Vec<String> = std::fs::read_dir(&out)
            .expect("list")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["only.txt".to_string()]);

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A tree over the cap cannot be salvaged by retrying, so it is refused
    /// before anything is materialised on disk — and the refusal NAMES which
    /// of the run's two images it is about. There are two, they blow the cap
    /// for entirely different reasons, and a message that said "workspace" for
    /// both sent a reader to inspect a 4 KiB directory while the 3 GB asset
    /// tree that actually refused went unnamed.
    #[test]
    fn an_oversized_tree_is_refused_before_any_image_exists_and_names_itself() {
        let refused = size_or_refuse("read-only inputs", Path::new("/run/assets"), MAX_WORKSPACE_BYTES + 1)
            .expect_err("must refuse");
        assert!(refused.contains("over the"), "{refused}");
        assert!(refused.contains("read-only inputs"), "names the tree: {refused}");
        assert!(refused.contains("/run/assets"), "names the directory: {refused}");

        size_or_refuse("workspace", Path::new("/run/ws"), MAX_WORKSPACE_BYTES)
            .expect("the cap itself is allowed");
    }

    /// A read-only image gets a metadata margin, NOT the writable image's ×3.
    ///
    /// This decided real runs: a run's read-only inputs are its PATH commands,
    /// and where those include a build directory they measure gigabytes. At ×3
    /// the same tree crossed the cap and the run was refused over a size two
    /// thirds of which was zeroes nothing could ever write to.
    #[test]
    fn a_read_only_image_is_not_sized_for_writes_that_cannot_happen() {
        let root = scratch("ro-size");
        // sparse and well over MIN_WORKSPACE_BYTES, so the FLOOR is not what
        // either answer is — the measurement is. `tree_bytes` reads the logical
        // length, so this costs no disk.
        let payload: u64 = 512 * 1024 * 1024;
        std::fs::File::create(root.join("blob"))
            .expect("create")
            .set_len(payload)
            .expect("set_len");

        let writable = sized_for(&root).expect("writable size");
        let read_only = sized_for_read_only(&root).expect("read-only size");
        assert_eq!(writable, payload * 3, "writable is measured × HEADROOM");
        assert!(
            read_only < writable,
            "read-only {read_only} must be under writable {writable}"
        );
        assert!(
            read_only >= payload,
            "…but still hold the payload: {read_only} < {payload}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// An empty workspace still needs a journal, so the floor applies rather
    /// than the measured zero.
    #[test]
    fn an_empty_workspace_is_floored_not_zero_sized() {
        let root = scratch("floor");
        assert_eq!(sized_for(&root).expect("size"), MIN_WORKSPACE_BYTES);
        let _ = std::fs::remove_dir_all(&root);
    }
}
