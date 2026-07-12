use commonware_cryptography::ed25519;
use consensus::{Digest, digest_of};
use sdk::StateRoot;

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

pub(crate) fn diag_log(line: impl AsRef<str>) {
    let Ok(path) = std::env::var("DUCKTAPE_DIAG_LOG") else {
        return;
    };
    let line = line.as_ref();
    println!("{line}");
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut file) => {
            use std::io::Write as _;
            if let Err(e) = writeln!(file, "{line}") {
                eprintln!("DUCKTAPE_DIAG_LOG append failed for {path}: {e}");
            }
        }
        Err(e) => eprintln!("DUCKTAPE_DIAG_LOG open failed for {path}: {e}"),
    }
}

pub(crate) fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
