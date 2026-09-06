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
/// workspace). A directory that came back from `rm -rf` is empty, so the next
/// read finds no token file and mints a fresh one on the spot — a random
/// collision with the token from before deletion is not a real possibility.
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

    /// This directory's own identity token: read back one a prior boot left
    /// behind, or mint and persist a fresh one when none is there yet — which
    /// is exactly what happens the instant a deleted-and-recreated directory
    /// is looked at again. A filesystem error persisting the fresh token
    /// (read-only mount, no space) still returns it for THIS process's
    /// comparisons; it just won't survive a restart, same ambiguity the mark
    /// already had before this fix.
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

    /// Is the pinned directory still the one sitting at `dir`? False when the
    /// path is gone AND when something else — a re-created empty workspace —
    /// now occupies it.
    pub(crate) fn still_at(&self, dir: &std::path::Path) -> bool {
        Self::read(dir) == Some(*self)
    }
}

#[cfg(test)]
mod workspace_mark_tests {
    use super::WorkspaceMark;

    #[test]
    fn a_pinned_workspace_is_still_itself() {
        let dir = tempfile::tempdir().unwrap();
        let mark = WorkspaceMark::read(dir.path()).expect("a fresh dir stats");
        assert!(mark.still_at(dir.path()));
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
        assert!(!mark.still_at(&path), "a missing path is not the workspace");

        std::fs::create_dir_all(path.join("storage")).unwrap();
        assert!(
            !mark.still_at(&path),
            "a re-created workspace is a different one"
        );
    }

    #[test]
    fn an_unreadable_directory_marks_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(WorkspaceMark::read(&dir.path().join("absent")), None);
    }

    /// The scenario `a_deleted_workspace_is_gone_even_once_the_path_is_back`
    /// depends on the recreated directory getting a mark that differs from
    /// the deleted one's even when the filesystem hands back the exact same
    /// (device, inode) pair — which ext4 does, measured directly on this box.
    /// Pin the mechanism directly: two marks that share device AND inode but
    /// carry different tokens must still compare unequal.
    #[test]
    fn a_reused_inode_is_not_the_same_workspace() {
        let same_device = 1;
        let reused_inode = 42;
        let born_first = WorkspaceMark {
            device: same_device,
            inode: reused_inode,
            token: [1; 16],
        };
        let born_later = WorkspaceMark {
            device: same_device,
            inode: reused_inode,
            token: [2; 16],
        };
        assert_ne!(
            born_first, born_later,
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
