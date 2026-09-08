//! Per-binding **scoped service keys**: the credential an attached personal
//! session signs its delivery receipts with.
//!
//! ## why not the owner's wallet key
//!
//! A receipt is emitted by a daemon reacting to a provider event — no human, no
//! terminal, no password. A wallet key costs an argon2id pass over 64 MiB and a
//! prompt, so it cannot be the signer here. Handing the daemon the owner's
//! account key would be worse still: it would let an attached session act as the
//! whole account, when what it needs is to advance one conversation's delivery
//! records.
//!
//! So a binding authorizes a key of its own. The OWNER signs the `Bind` that
//! names this key's public half; the scoped key never authorizes itself, and
//! replacing a binding retires the old key by generation.
//!
//! ## the trust model is the workspace, not the port
//!
//! The key is 32 bytes, hex, mode 0600, under the node's workspace — the same
//! shape and the same boundary as `service-link.token` and `admin.token`
//! (`noded::services`). "Can read the node's own workspace" is the whole claim.
//! A sandbox guest reaching the node's listener through its vsock tunnel holds
//! no workspace and so holds no key.
//!
//! Unlike those two, a service key **persists across boots**: its public half is
//! committed on-chain in the binding, so re-minting it at boot would silently
//! revoke every attachment on this host.
//!
//! ## a key is scoped to one NETWORK as well as one binding
//!
//! `chain_id` is part of the binding's identity here, not decoration. One
//! workspace can be pointed at a second network, and conversation and
//! participant ids are module-defined strings that carry no network in them —
//! so `("standup", "alice")` on network A and on network B are the same two
//! strings. Without the chain id they would name one key file.
//!
//! That matters because a submit frame is NOT chain-bound: `user_frame` signs
//! `signer ‖ seq ‖ target ‖ payload` and nothing else, so a frame minted for
//! one network verifies byte-identically on another. Sharing one key across
//! both would make a `Send` or `Acknowledge` signed for A replayable as the
//! same binding's op on B. Fencing the KEY is what stops it: B's binding
//! commits a different public key, so A's frame is signed by a key B's module
//! does not recognise, and B refuses it.
//!
//! The fence is the workspace's own `network.toml` (`chain_id`, which that
//! file's own doc calls the namespace) — immutable per network and readable
//! with no node running.
//!
//! ## the filename is a digest
//!
//! A conversation id and a participant id are module-defined strings; core puts
//! no charset on them. Rather than constrain core's id space or hand-roll an
//! escaping rule, the file is named by
//! `sha256(chain_id ‖ 0x1f ‖ conversation ‖ 0x1f ‖ participant)` — no id can
//! walk out of the directory, and nothing here has to be listed by name.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};
use sha2::Digest as _;

/// where a workspace keeps its scoped messaging keys. `collab-keys` rather than
/// `keys`, which is the WALLET keystore's directory: these are not wallets and
/// must never be offered as one.
const DIR: &str = "collab-keys";

/// One binding's identity: the network it lives on, and the two ids that name
/// it there on-chain.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BindingRef<'a> {
    /// the workspace's `network.toml` `chain_id`. Two networks never share a
    /// scoped key — see this module's header.
    pub(crate) network: &'a str,
    pub(crate) conversation: &'a str,
    pub(crate) participant: &'a str,
}

impl BindingRef<'_> {
    /// the digest that names this binding's key file. `0x1f` separated so no
    /// shift of a boundary between the three parts can collide — `("ab", "c")`
    /// with `("a", "bc")`, or a chain id ending in a conversation id's prefix.
    fn file_name(&self) -> String {
        let mut hasher = sha2::Sha256::new();
        hasher.update(self.network.as_bytes());
        hasher.update([0x1f]);
        hasher.update(self.conversation.as_bytes());
        hasher.update([0x1f]);
        hasher.update(self.participant.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

fn key_path(workspace: &std::path::Path, binding: BindingRef<'_>) -> std::path::PathBuf {
    workspace.join(DIR).join(binding.file_name())
}

/// Load this binding's service key, or mint one if it has none.
///
/// Idempotent on purpose: `collab attach` is re-runnable, and a second attach
/// to the same binding must present the SAME public key or the on-chain
/// binding would stop matching the key that signs its receipts. Minting a fresh
/// key per call would make every re-attach silently break the previous one.
pub(crate) fn ensure(
    workspace: &std::path::Path,
    binding: BindingRef<'_>,
) -> Result<ed25519::PrivateKey, String> {
    if let Some(existing) = load(workspace, binding)? {
        return Ok(existing);
    }
    mint(workspace, binding)
}

/// This binding's service key, or `None` when it has never been attached here.
///
/// `None` is an ordinary state — a binding on another device — and never an
/// error, so a caller can tell "not attached here" from "attached and
/// unreadable" (a permissions or corruption problem, which IS an error).
pub(crate) fn load(
    workspace: &std::path::Path,
    binding: BindingRef<'_>,
) -> Result<Option<ed25519::PrivateKey>, String> {
    let path = key_path(workspace, binding);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("service key {}: {error}", path.display())),
    };
    // the PATH, never the contents: this error reaches a terminal and a log.
    let seed = crate::config::unhex(text.trim())
        .map_err(|_| format!("service key {} is not hex", path.display()))?;
    let key = ed25519::PrivateKey::decode(seed.as_slice())
        .map_err(|_| format!("service key {} is not an ed25519 key", path.display()))?;
    Ok(Some(key))
}

/// Mint a fresh service key for this binding, 0600.
///
/// `create_new` is the whole concurrency story: two `collab attach` runs racing
/// on one binding cannot both mint, because the loser's create fails and it
/// reads the winner's key instead of overwriting it. Overwriting would strand
/// the on-chain binding on a public key nothing holds any more.
fn mint(
    workspace: &std::path::Path,
    binding: BindingRef<'_>,
) -> Result<ed25519::PrivateKey, String> {
    let dir = workspace.join(DIR);
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let path = key_path(workspace, binding);
    let seed = noded::services::new_secret();
    match write_owner_only(&path, &seed) {
        Ok(()) => {}
        // lost the race: the winner's key is the binding's key.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return load(workspace, binding)?
                .ok_or_else(|| format!("service key {} vanished mid-mint", path.display()));
        }
        Err(error) => return Err(format!("service key {}: {error}", path.display())),
    }
    let bytes = crate::config::unhex(&seed).map_err(|error| error.to_string())?;
    ed25519::PrivateKey::decode(bytes.as_slice())
        .map_err(|_| "a fresh 32-byte secret is not a valid ed25519 key".to_string())
}

/// Create 0600 from the start — a world-readable window, however short, is what
/// this file exists to avoid. `create_new` so an existing key is never clobbered.
#[cfg(unix)]
fn write_owner_only(path: &std::path::Path, secret: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?
        .write_all(secret.as_bytes())
}

#[cfg(not(unix))]
fn write_owner_only(path: &std::path::Path, secret: &str) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, secret.as_bytes()))
}

/// The public half — what a `Bind` op carries on-chain as the binding's
/// authorized signer.
pub(crate) fn public_hex(key: &ed25519::PrivateKey) -> String {
    crate::config::hex_bytes(key.public_key().as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("temp workspace")
    }

    const NET: &str = "ducktape#a1b2c3d4";

    fn binding<'a>(conversation: &'a str, participant: &'a str) -> BindingRef<'a> {
        on_network(NET, conversation, participant)
    }

    fn on_network<'a>(
        network: &'a str,
        conversation: &'a str,
        participant: &'a str,
    ) -> BindingRef<'a> {
        BindingRef {
            network,
            conversation,
            participant,
        }
    }

    /// Re-attaching must present the SAME public key. A fresh key per call
    /// would leave the committed binding naming a key nothing holds, and every
    /// receipt it signed afterwards would be refused.
    #[test]
    fn ensure_is_idempotent_for_one_binding() {
        let dir = workspace();
        let first = ensure(dir.path(), binding("c1", "p1")).expect("mints");
        let second = ensure(dir.path(), binding("c1", "p1")).expect("loads");
        assert_eq!(public_hex(&first), public_hex(&second));
    }

    /// A key is scoped to ONE binding: that is what "scoped" means, and it is
    /// why a compromised session cannot acknowledge another conversation's
    /// deliveries.
    #[test]
    fn each_binding_gets_its_own_key() {
        let dir = workspace();
        let a = ensure(dir.path(), binding("c1", "p1")).expect("mints");
        let other_conversation = ensure(dir.path(), binding("c2", "p1")).expect("mints");
        let other_participant = ensure(dir.path(), binding("c1", "p2")).expect("mints");
        assert_ne!(public_hex(&a), public_hex(&other_conversation));
        assert_ne!(public_hex(&a), public_hex(&other_participant));
    }

    /// The parts are separated before hashing, so a shift of any boundary
    /// between them cannot name the same file.
    #[test]
    fn the_id_parts_cannot_collide_by_shifting_a_boundary() {
        assert_ne!(
            binding("ab", "c").file_name(),
            binding("a", "bc").file_name()
        );
        assert_ne!(
            on_network("net", "a", "p").file_name(),
            on_network("ne", "ta", "p").file_name()
        );
    }

    /// One workspace pointed at a second network must not reuse the first
    /// network's scoped key for the same two ids.
    ///
    /// A submit frame is not chain-bound (`user_frame` signs signer, seq,
    /// target and payload — no chain id), so one shared key would make a `Send`
    /// or `Acknowledge` minted for one network replayable as the same binding's
    /// op on the other. Distinct keys are what refuse it: the second network's
    /// binding commits a different public key.
    #[test]
    fn the_same_binding_on_another_network_gets_another_key() {
        let dir = workspace();
        let here = ensure(
            dir.path(),
            on_network("ducktape#aaaa1111", "standup", "alice"),
        )
        .expect("mints");
        let elsewhere = ensure(
            dir.path(),
            on_network("ducktape#bbbb2222", "standup", "alice"),
        )
        .expect("mints");
        assert_ne!(
            public_hex(&here),
            public_hex(&elsewhere),
            "one workspace, two networks, two keys"
        );
    }

    /// A module-defined id is an arbitrary string. It must not be able to name
    /// a file outside the key directory.
    #[test]
    fn a_hostile_id_cannot_escape_the_key_directory() {
        let dir = workspace();
        let hostile = binding("../../../etc/passwd", "../../root/.ssh/id_ed25519");
        let path = key_path(dir.path(), hostile);
        let key_dir = dir.path().join(DIR);
        assert_eq!(
            path.parent(),
            Some(key_dir.as_path()),
            "every key lands in the key directory: {}",
            path.display()
        );
        assert!(
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.len() == 64
                    && name.chars().all(|c| c.is_ascii_hexdigit())),
            "the file name is a digest, not an id"
        );
        ensure(dir.path(), hostile).expect("a hostile id is ordinary input here");
    }

    /// "Not attached on this device" is a state, not a failure — a binding can
    /// live on another laptop.
    #[test]
    fn an_unattached_binding_loads_as_none() {
        let dir = workspace();
        assert!(
            load(dir.path(), binding("never", "attached"))
                .expect("absence is not an error")
                .is_none()
        );
    }

    /// A key that IS there and cannot be read is a real problem and must not
    /// read as "not attached here" — that would silently mint a second key and
    /// strand the committed binding.
    #[test]
    fn a_corrupt_key_is_an_error_not_an_absence() {
        let dir = workspace();
        let target = binding("c1", "p1");
        ensure(dir.path(), target).expect("mints");
        std::fs::write(key_path(dir.path(), target), "not hex at all").expect("corrupt it");
        let failure = load(dir.path(), target).expect_err("a corrupt key is an error");
        assert!(failure.contains("not hex"), "{failure}");
    }

    /// 0600 is the whole trust model: the key is admissible because only this
    /// workspace's owner can read it.
    #[cfg(unix)]
    #[test]
    fn a_minted_key_is_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = workspace();
        let target = binding("c1", "p1");
        ensure(dir.path(), target).expect("mints");
        let mode = std::fs::metadata(key_path(dir.path(), target))
            .expect("stat")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "mode was {:o}", mode & 0o777);
    }
}
