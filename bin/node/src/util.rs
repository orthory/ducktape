use commonware_cryptography::ed25519;
use consensus::{Digest, digest_of};
use sdk::StateRoot;

/// a fail-stop: the node cannot continue and a human must go fix something.
///
/// this is THE event an operator most needs — *the node died, here is why* — and
/// as a bare `eprintln!` it reached stderr (and so `daemon.log`) but never the
/// `LogRing`, which means it never reached the app's Logs tab: the only surface
/// most users have. routing it through `tracing` puts it in both.
///
/// the literal "FATAL" text stays in the message on purpose: it is how a
/// `daemon.log` reader classifies a dead node, and `bin/node`'s e2e suites
/// assert on it.
///
/// NOTE: `process::exit` runs no destructors, so the ring's ws subscribers may
/// not be scheduled before we go. That is fine and deliberate — stderr is
/// unbuffered, so `daemon.log` always has the line, and the shell reads its tail
/// precisely to report a death (`daemon.rs::SpawnFailure::log_tail`).
macro_rules! fatal {
    // the same stop, carrying the snake_case `reason` token an operator greps
    // for. Only the causes a reader is meant to COUNT get one; the rest say it
    // in the message.
    ($label:expr, reason = $reason:expr, $($arg:tt)*) => {{
        tracing::error!(
            target: "ducktape::node",
            node = %$label,
            reason = $reason,
            "FATAL: {}", format_args!($($arg)*)
        );
        std::process::exit(1)
    }};
    ($label:expr, $($arg:tt)*) => {{
        tracing::error!(
            target: "ducktape::node",
            node = %$label,
            "FATAL: {}", format_args!($($arg)*)
        );
        std::process::exit(1)
    }};
}
pub(crate) use fatal;

/// the per-epoch genesis floor: domain-separated by namespace AND epoch, so a
/// respawned engine can never confuse an old epoch's certificates with its own
/// (an old-epoch floor fails `Floor::assert` against the new epoch).
pub(crate) fn epoch_floor(namespace: &[u8], epoch: u64) -> Digest {
    digest_of(
        &[
            b"ducktape:consensus:genesis:v1:".as_ref(),
            namespace,
            b":epoch:",
            &epoch.to_le_bytes(),
        ]
        .concat(),
    )
}

/// the orchestrator's current epoch participant set as raw key bytes — what
/// checkpoints, cutover records, and the statesync manifest carry.
pub(crate) fn participant_bytes(
    orchestrator: &consensus::ValsetOrchestrator<ed25519::PublicKey>,
) -> Vec<Vec<u8>> {
    orchestrator
        .current_members()
        .iter()
        .map(|k| k.as_ref().to_vec())
        .collect()
}

pub(crate) fn resident_bytes(
    orchestrator: &consensus::ValsetOrchestrator<ed25519::PublicKey>,
) -> Vec<Vec<u8>> {
    orchestrator
        .current_residents()
        .iter()
        .map(|k| k.as_ref().to_vec())
        .collect()
}

/// hex-encode a state root for a stable, greppable log line.
pub(crate) fn hex(root: &StateRoot) -> String {
    duckfs_core::to_hex(&root.0)
}

/// every module the host runs, as the status projection reports it — the
/// registry's sorted-id order, the same set and order the root-hash composes
/// over. a module admitted after genesis is a row like any other.
pub(crate) fn module_statuses(host: &host::Host) -> Vec<noded::ModuleStatus> {
    host.module_roots()
        .into_iter()
        .map(|(id, root)| noded::ModuleStatus {
            category: noded::ModuleCategory::of(&id),
            root: hex(&root),
            id,
        })
        .collect()
}

/// the same set as the rpc status's id → hex-root map.
pub(crate) fn module_roots_hex(host: &host::Host) -> std::collections::BTreeMap<String, String> {
    host.module_roots()
        .into_iter()
        .map(|(id, root)| (id, hex(&root)))
        .collect()
}

pub(crate) fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The workspace directory this node booted on, pinned by identity rather than
/// by path: the device and inode `stat` reported at boot, plus a random token
/// this process stamps into the directory itself.
///
/// A path check alone cannot answer "is my workspace still there", because the
/// node RECREATES it: every journal write does `create_dir_all` on the storage
/// root under it, so seconds after an `rm -rf` the path exists again — empty,
/// on a NEW inode, with the chain's history gone. (device, inode) alone is not
/// enough to tell the two apart: ext4 hands a freed inode number straight back
/// out, so a delete-then-recreate can land on the exact same pair — measured
/// directly on this box, repeatable. The inode's birth time doesn't save it
/// either: `Metadata::created()` came back IDENTICAL across the same
/// delete-then-recreate here too (ext4 does not reliably refresh crtime on
/// inode reuse), so it is not a real second component. What IS unforgeable is
/// a value nothing but this process could have written: at boot this mints a
/// random 128-bit token into `<workspace>/workspace.mark` (or reads one back,
/// if the directory already carries one from an earlier boot of the same
/// workspace). Every later check COMPARES that pinned token against what the
/// directory carries — see [`WorkspaceMark::presence`], which is where the
/// three outcomes and the deliberate limit of this scheme are spelled out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceMark {
    device: u64,
    inode: u64,
    token: [u8; 16],
}

impl WorkspaceMark {
    const MARK_FILE: &'static str = "workspace.mark";

    /// Pin a directory as it is now. `None` when it cannot be stat'd at all —
    /// the caller then runs unguarded rather than fail-stopping on a boot-time
    /// filesystem oddity.
    pub(crate) fn read(dir: &std::path::Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        let meta = std::fs::metadata(dir).ok()?;
        Some(Self {
            device: meta.dev(),
            inode: meta.ino(),
            token: Self::token(dir),
        })
    }

    /// This directory's own identity token, minted ONCE at boot: read back one
    /// a prior boot left behind, or mint and persist a fresh one when none is
    /// there yet. A filesystem error persisting it (read-only mount, no space)
    /// still returns it for this process's comparisons; it just won't survive
    /// a restart.
    fn token(dir: &std::path::Path) -> [u8; 16] {
        let path = dir.join(Self::MARK_FILE);
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(existing) = bytes.try_into()
        {
            return existing;
        }
        let fresh: [u8; 16] = rand::random();
        let _ = std::fs::write(&path, fresh);
        fresh
    }

    /// Is the pinned directory still the one this node booted on? COMPARES the
    /// pinned token — it never re-mints, because minting is what a boot does:
    /// a check that re-derives the token from scratch reports "different
    /// workspace" for every state in which the mark file simply cannot be read
    /// back, and fail-stops a healthy node.
    ///
    /// A missing or short mark file is therefore [`Presence::MarkLost`], not a
    /// deletion: a backup that skipped an unrecognized extensionless file, a
    /// full or read-only filesystem that never let the boot write it, a crash
    /// between truncate and write. The cost of that ruling is the one case a
    /// deleted-and-recreated directory lands back on the SAME (device, inode)
    /// — ext4 hands a freed inode number straight out — and comes back empty:
    /// that reads as MarkLost too. A recreated workspace far more often gets a
    /// different inode, and the alternative was killing live nodes over a
    /// missing 16-byte file.
    pub(crate) fn presence(&self, dir: &std::path::Path) -> Presence {
        use std::os::unix::fs::MetadataExt as _;
        let Ok(meta) = std::fs::metadata(dir) else {
            return Presence::Vanished;
        };
        let same_directory = meta.dev() == self.device && meta.ino() == self.inode;
        if !same_directory {
            return Presence::Vanished;
        }
        match std::fs::read(dir.join(Self::MARK_FILE)) {
            Ok(bytes) if bytes == self.token => Presence::Intact,
            // a FULL-length token that is not ours was stamped by another
            // process on this inode — a different workspace wearing our path.
            Ok(bytes) if bytes.len() == self.token.len() => Presence::Vanished,
            _ => Presence::MarkLost,
        }
    }

    /// Put the pinned token back, so a workspace whose mark file was lost to a
    /// backup or a transient write failure heals on the next check. `false`
    /// when the filesystem still refuses it.
    pub(crate) fn restore(&self, dir: &std::path::Path) -> bool {
        std::fs::write(dir.join(Self::MARK_FILE), self.token).is_ok()
    }
}

/// What a [`WorkspaceMark`] check found at the pinned path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Presence {
    /// The pinned directory, carrying the token this node stamped into it.
    Intact,
    /// The pinned directory, but its mark file is gone or truncated. NOT a
    /// deletion — the node keeps running and rewrites the mark.
    MarkLost,
    /// Gone, unreadable, or a different (device, inode) — a re-created
    /// workspace wearing the same path.
    Vanished,
}

#[cfg(test)]
mod workspace_mark_tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::{Presence, WorkspaceMark};

    #[test]
    fn a_pinned_workspace_is_still_itself() {
        let dir = tempfile::tempdir().unwrap();
        let mark = WorkspaceMark::read(dir.path()).expect("a fresh dir stats");
        assert_eq!(mark.presence(dir.path()), Presence::Intact);
    }

    #[test]
    fn a_deleted_workspace_is_gone_even_once_the_path_is_back() {
        // exactly the shape a running node lands in: the directory is deleted
        // underneath it, and its own next journal write puts the PATH back.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace");
        std::fs::create_dir(&path).unwrap();
        let mark = WorkspaceMark::read(&path).expect("a fresh dir stats");

        std::fs::remove_dir_all(&path).unwrap();
        assert_eq!(
            mark.presence(&path),
            Presence::Vanished,
            "a missing path is not the workspace"
        );

        std::fs::create_dir_all(path.join("storage")).unwrap();
        // a re-created path is never the pinned workspace: on a fresh inode it
        // is `Vanished`; when the filesystem hands the freed inode straight
        // back (ext4 routinely does) it reads as `MarkLost`, the documented
        // ceiling of the (device, inode) pin. Which one the kernel picks is
        // not this test's to assert — that it is not `Intact` is.
        assert_ne!(
            mark.presence(&path),
            Presence::Intact,
            "a re-created workspace is never the pinned one"
        );
    }

    #[test]
    fn a_deleted_mark_file_is_not_a_deleted_workspace() {
        // a backup that skipped the unrecognized extensionless file, or an
        // operator tidying it away. the directory is untouched, so the node
        // must keep running — and must be able to put the mark back.
        let dir = tempfile::tempdir().unwrap();
        let mark = WorkspaceMark::read(dir.path()).expect("a fresh dir stats");
        std::fs::remove_file(dir.path().join(WorkspaceMark::MARK_FILE)).unwrap();

        assert_eq!(mark.presence(dir.path()), Presence::MarkLost);
        assert!(
            mark.restore(dir.path()),
            "a writable dir takes the mark back"
        );
        assert_eq!(mark.presence(dir.path()), Presence::Intact);
    }

    #[test]
    fn a_truncated_mark_file_is_not_a_deleted_workspace() {
        // a crash between truncate and write leaves 0 bytes behind.
        let dir = tempfile::tempdir().unwrap();
        let mark = WorkspaceMark::read(dir.path()).expect("a fresh dir stats");
        std::fs::write(dir.path().join(WorkspaceMark::MARK_FILE), b"").unwrap();

        assert_eq!(mark.presence(dir.path()), Presence::MarkLost);
    }

    #[test]
    fn a_boot_that_could_not_write_the_mark_does_not_fail_stop() {
        // ENOSPC or a read-only mount at boot: `read` mints a token it never
        // manages to persist. Every later check must still say "this is my
        // workspace", not kill the node within a second of boot.
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let readonly = std::fs::Permissions::from_mode(0o555);
        std::fs::set_permissions(&workspace, readonly).unwrap();

        let mark = WorkspaceMark::read(&workspace).expect("an unwritable dir still stats");
        assert!(
            !workspace.join(WorkspaceMark::MARK_FILE).exists(),
            "the boot write could not land"
        );
        assert_eq!(mark.presence(&workspace), Presence::MarkLost);
        assert!(
            !mark.restore(&workspace),
            "and the rewrite cannot land either"
        );

        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn an_unreadable_directory_marks_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(WorkspaceMark::read(&dir.path().join("absent")), None);
    }

    /// The scenario `a_deleted_workspace_is_gone_even_once_the_path_is_back`
    /// depends on the (device, inode) pair NOT being the discriminator: ext4
    /// hands a freed inode number straight back out, measured directly on this
    /// box. Pin the mechanism directly — another process's token under our
    /// pinned path is a different workspace even when the pair matches.
    #[test]
    fn a_reused_inode_carrying_another_token_is_not_the_same_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let mark = WorkspaceMark::read(dir.path()).expect("a fresh dir stats");
        std::fs::write(dir.path().join(WorkspaceMark::MARK_FILE), [0xAAu8; 16]).unwrap();
        assert_eq!(
            mark.presence(dir.path()),
            Presence::Vanished,
            "a reused inode is not the workspace it used to be"
        );
    }

    #[test]
    fn a_reboot_on_the_same_untouched_workspace_reads_back_the_same_token() {
        // the token must survive a process restart on the SAME workspace —
        // otherwise every boot would look like a brand-new directory.
        let dir = tempfile::tempdir().unwrap();
        let first_boot = WorkspaceMark::read(dir.path()).expect("a fresh dir stats");
        let second_boot = WorkspaceMark::read(dir.path()).expect("the same dir stats again");
        assert_eq!(
            first_boot, second_boot,
            "an untouched workspace keeps its identity across boots"
        );
    }
}
