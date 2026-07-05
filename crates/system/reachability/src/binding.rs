//! Deterministic (chain_id, epoch, members) commitments. The node has no
//! consensus-state admission-root concept, so the `wireguard-upgrade`
//! `ActiveValidatorSet` binding is DERIVED: domain-separated hashes over the
//! chain id, the epoch, and the sorted member identities. Every node computes
//! the same binding from the same cutover event — no state lookup, no
//! coordination — and a node on a different member set simply cannot verify
//! the mesh (its roots differ), which is the binding's whole job.

use sha2::{Digest as _, Sha256};
use wireguard_upgrade::{
    ActiveValidatorSet, AdmissionRoot, PortPolicy, Root, UpgradeError, ValidatorIdentity,
};

use nat_traversal::NodeKey;

const VALSET_ROOT_NS: &str = "ducktape:reachability-valset-root:v1";
const ADMISSION_ROOT_NS: &str = "ducktape:reachability-admission-root:v1";
const IFNAME_NS: &str = "ducktape:reachability-ifname:v1";

/// The derived valset commitment for `(chain_id, epoch, members)`.
pub fn valset_root(chain_id: &str, epoch: u64, members: &[ValidatorIdentity]) -> Root {
    Root(members_commitment(VALSET_ROOT_NS, chain_id, epoch, members))
}

/// The derived admission commitment for `(chain_id, epoch, members)`.
/// Distinct domain from [`valset_root`] so the two can never be confused for
/// one another even though today they commit to the same inputs.
pub fn admission_root(chain_id: &str, epoch: u64, members: &[ValidatorIdentity]) -> AdmissionRoot {
    AdmissionRoot(members_commitment(ADMISSION_ROOT_NS, chain_id, epoch, members))
}

/// The full `ActiveValidatorSet` for a cutover event: derived roots + the
/// member identities. `chain_id` doubles as the advertisement namespace,
/// exactly as it does for the commonware mesh.
pub fn active_set(
    chain_id: &str,
    epoch: u64,
    members: Vec<ValidatorIdentity>,
) -> Result<ActiveValidatorSet, UpgradeError> {
    let root = valset_root(chain_id, epoch, &members);
    let admission = admission_root(chain_id, epoch, &members);
    ActiveValidatorSet::new(chain_id, epoch, root, admission, members)
}

/// The dedicated interface name for this chain: `dt-` + the first 8 hex chars
/// of a chain-scoped hash. NEVER wg0/tailscale0 — the whole coexistence story
/// rests on ducktape owning its own interface — and 11 chars stays well under
/// Linux's 15-char IFNAMSIZ. Two networks on one host get two interfaces.
pub fn interface_name(chain_id: &str) -> String {
    let mut pre = Vec::new();
    put_str(&mut pre, IFNAME_NS);
    put_str(&mut pre, chain_id);
    let h: [u8; 32] = Sha256::digest(&pre).into();
    let hex: String = h[..4].iter().map(|b| format!("{b:02x}")).collect();
    format!("dt-{hex}")
}

/// The staged plane's endpoint policy: every port, loopback and private
/// addresses allowed. It MUST be a uniform constant — `port_policy_hash` is
/// cross-checked in every tunnel handshake, so two members constructing
/// different policies (say, from their own local ports) could never
/// handshake. Nodes advertise arbitrary operator-chosen ports and dev
/// networks live on private/loopback addresses, hence open; pinning a
/// descriptor-carried policy per network is the hardening follow-up.
pub fn open_port_policy() -> PortPolicy {
    let all_ports: Vec<u16> = (1..=u16::MAX).collect();
    PortPolicy {
        name: "ducktape-open".into(),
        allowed_control_tcp_ports: all_ports.clone(),
        allowed_wireguard_udp_ports: all_ports,
        allow_loopback: true,
        allow_private_ip: true,
    }
}

/// A member's rendezvous key on the nat-traversal plane IS its ed25519
/// identity bytes — one identity everywhere, so the coordinator's advert book
/// and the mesh view can never disagree about who an address belongs to.
pub fn node_key(identity: ValidatorIdentity) -> NodeKey {
    NodeKey(identity.0)
}

/// Map a commonware ed25519 public key to the `wireguard-upgrade` identity
/// type (both are the raw 32 public-key bytes).
pub fn identity_of(pk: &commonware_cryptography::ed25519::PublicKey) -> ValidatorIdentity {
    let mut raw = [0u8; 32];
    raw.copy_from_slice(pk.as_ref());
    ValidatorIdentity(raw)
}

fn members_commitment(
    domain: &str,
    chain_id: &str,
    epoch: u64,
    members: &[ValidatorIdentity],
) -> [u8; 32] {
    // sorted so the commitment is a SET commitment: every node derives it
    // from its own member ordering and still agrees.
    let mut sorted: Vec<[u8; 32]> = members.iter().map(|m| m.0).collect();
    sorted.sort();
    let mut pre = Vec::new();
    put_str(&mut pre, domain);
    put_str(&mut pre, chain_id);
    pre.extend_from_slice(&epoch.to_be_bytes());
    pre.extend_from_slice(&(sorted.len() as u64).to_be_bytes());
    for member in sorted {
        pre.extend_from_slice(&member);
    }
    Sha256::digest(&pre).into()
}

// length-prefixed so no two (domain, chain_id) pairs can collide by
// concatenation — the same discipline as wireguard-upgrade's preimages.
fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u64).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(bytes: &[u8]) -> Vec<ValidatorIdentity> {
        bytes.iter().map(|b| ValidatorIdentity([*b; 32])).collect()
    }

    #[test]
    fn commitments_are_order_free_and_input_sensitive() {
        let forward = ids(&[1, 2, 3]);
        let reversed = ids(&[3, 2, 1]);
        assert_eq!(
            valset_root("net#1", 7, &forward),
            valset_root("net#1", 7, &reversed)
        );
        assert_eq!(
            admission_root("net#1", 7, &forward),
            admission_root("net#1", 7, &reversed)
        );

        // every input moves the commitment; the two domains never coincide.
        assert_ne!(valset_root("net#1", 7, &forward), valset_root("net#2", 7, &forward));
        assert_ne!(valset_root("net#1", 7, &forward), valset_root("net#1", 8, &forward));
        assert_ne!(
            valset_root("net#1", 7, &forward),
            valset_root("net#1", 7, &ids(&[1, 2]))
        );
        assert_ne!(
            valset_root("net#1", 7, &forward).0,
            admission_root("net#1", 7, &forward).0
        );
    }

    #[test]
    fn active_set_binds_the_derived_roots() {
        let members = ids(&[1, 2, 3]);
        let set = active_set("net#1", 7, members.clone()).unwrap();
        assert_eq!(set.valset_root, valset_root("net#1", 7, &members));
        assert_eq!(set.admission_root, admission_root("net#1", 7, &members));
        assert_eq!(set.namespace, "net#1");
        assert_eq!(set.epoch, 7);
    }

    #[test]
    fn interface_name_is_chain_scoped_and_ifnamsiz_safe() {
        let name = interface_name("ducktape#a1b2c3d4");
        assert!(name.starts_with("dt-"));
        assert_eq!(name.len(), 11);
        assert_eq!(name, interface_name("ducktape#a1b2c3d4"));
        assert_ne!(name, interface_name("ducktape#zzzzzzzz"));
    }

    #[test]
    fn node_key_is_the_identity_bytes() {
        let identity = ValidatorIdentity([9u8; 32]);
        assert_eq!(node_key(identity).0, identity.0);
    }
}
