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
/// by path: the device and inode `stat` reported at boot.
///
/// A path check alone cannot answer "is my workspace still there", because the
/// node RECREATES it: every journal write does `create_dir_all` on the storage
/// root under it, so seconds after an `rm -rf` the path exists again — empty,
/// on a NEW inode, with the chain's history gone. Comparing the inode is what
/// tells the two apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceMark {
    device: u64,
    inode: u64,
}

impl WorkspaceMark {
    /// Pin a directory as it is now. `None` when it cannot be stat'd at all —
    /// the caller then runs unguarded rather than fail-stopping on a boot-time
    /// filesystem oddity.
    pub(crate) fn read(dir: &std::path::Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        let meta = std::fs::metadata(dir).ok()?;
        Some(Self {
            device: meta.dev(),
            inode: meta.ino(),
        })
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
}
