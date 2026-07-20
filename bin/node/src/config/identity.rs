//! identity files — a persisted ed25519 secret, hex-encoded — plus the
//! user-key authorizer helpers the CLI mints and the chain-id mint.

use std::path::Path;

use commonware_codec::{DecodeExt as _, Encode as _};
use commonware_cryptography::{Signer as _, ed25519};

use super::{hex_bytes, unhex};


/// load the identity at `path`, or generate one there from OS randomness.
/// returns the signer and whether it was freshly generated. written 0600 on
/// unix — this is the NODE's identity (mesh/valset/frame-signing key) only.
/// the user's identity is a separate keypair held by the app
/// (`~/.ducktape/user.key`) and bound to this node's key through the
/// `identity` module (`crates/modules/system/identity`); this file never holds it.
pub fn load_or_generate_identity(path: &Path) -> Result<(ed25519::PrivateKey, bool), String> {
    if path.exists() {
        return load_identity(path).map(|k| (k, false));
    }
    let mut raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut raw);
    // every 32-byte string is a valid ed25519 seed (the scheme clamps), so
    // decode cannot fail on fresh OS randomness.
    let key = ed25519::PrivateKey::decode(raw.as_slice()).expect("32 random bytes decode");
    let encoded = hex_bytes(key.encode().as_ref());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {dir:?}: {e}"))?;
    }
    // the file is BORN 0600 (no write-then-chmod window a co-tenant could
    // read the secret through), and create_new makes exists-then-create
    // race-free: a concurrent keygen loses cleanly instead of clobbering.
    {
        use std::io::Write as _;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(path)
            .map_err(|e| format!("create {path:?}: {e}"))?;
        if let Err(e) = f.write_all(format!("{encoded}\n").as_bytes()) {
            // a partial file would shadow every future load with a decode
            // error; remove it so the next run regenerates cleanly.
            let _ = std::fs::remove_file(path);
            return Err(format!("write {path:?}: {e}"));
        }
    }
    Ok((key, true))
}

pub fn load_identity(path: &Path) -> Result<ed25519::PrivateKey, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let raw = unhex(text.trim()).map_err(|e| format!("{path:?}: {e}"))?;
    ed25519::PrivateKey::decode(raw.as_slice())
        .map_err(|e| format!("{path:?} is not an ed25519 secret: {e}"))
}

/// wrap an ed25519 user key's signature over `preimage` (under `namespace`) as
/// the [`identity::MemberAuth`] every account op carries -- the node's user key
/// is always an ed25519 account member, so this is the one authorizer shape the
/// CLI mints.
pub fn ed25519_member_auth(
    user: &ed25519::PrivateKey,
    namespace: &[u8],
    preimage: &[u8],
) -> identity::MemberAuth {
    identity::MemberAuth {
        key: user.public_key().as_ref().to_vec(),
        kind: identity::KeyKind::Ed25519,
        proof: identity::MemberProof::Signature {
            sig: user.sign(namespace, preimage).as_ref().to_vec(),
        },
    }
}

/// the possession proof an ed25519 key produces over `preimage` -- what a NEW
/// device signs to prove it holds the key it is asking to enroll (the other
/// half of an `AddMemberKey`, alongside an existing member's [`ed25519_member_auth`]).
pub fn ed25519_possession(
    user: &ed25519::PrivateKey,
    namespace: &[u8],
    preimage: &[u8],
) -> identity::MemberProof {
    identity::MemberProof::Signature {
        sig: user.sign(namespace, preimage).as_ref().to_vec(),
    }
}

/// mint a chain-id: the human-readable name plus a short salt, so two
/// unrelated networks that pick the same name still get distinct namespaces
/// (their handshakes fail cleanly instead of colliding). the salt hashes the
/// initiator's identity and the wall clock — unique enough for a network id,
/// no rng plumbing needed.
pub fn mint_chain_id(name: &str, initiator: &ed25519::PublicKey) -> String {
    use commonware_cryptography::{Hasher as _, Sha256};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock is past the epoch")
        .as_nanos();
    let mut hasher = Sha256::default();
    hasher.update(initiator.as_ref());
    hasher.update(&nanos.to_le_bytes());
    let digest = hasher.finalize();
    format!("{name}#{}", hex_bytes(&digest.as_ref()[..4]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ducktape-config-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn identity_roundtrips_and_reuses() {
        let dir = tmp("identity");
        let path = dir.join("identity.key");
        let (a, generated) = load_or_generate_identity(&path).expect("generate");
        assert!(generated);
        let (b, generated) = load_or_generate_identity(&path).expect("reuse");
        assert!(
            !generated,
            "an existing identity is reused, never clobbered"
        );
        assert_eq!(a.public_key(), b.public_key());
    }

    // ---- user-key bind/unbind certificates ---------------------------------

    #[test]
    fn mint_bind_cert_verifies_against_module_preimage() {
        use commonware_cryptography::{
            Signer as _, Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(1);
        let node_pub = [9u8; 32];
        let cert = user
            .sign(
                identity::IDENTITY_BIND_NS,
                &identity::bind_preimage("chain-a", &node_pub, 0),
            )
            .as_ref()
            .to_vec();
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage = identity::bind_preimage("chain-a", &node_pub, 0);
        assert!(user.public_key().verify(identity::IDENTITY_BIND_NS, &preimage, &sig));
    }

    #[test]
    fn mint_bind_cert_is_chain_scoped() {
        use commonware_cryptography::{
            Signer as _, Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(1);
        let node_pub = [9u8; 32];
        let cert = user
            .sign(
                identity::IDENTITY_BIND_NS,
                &identity::bind_preimage("chain-a", &node_pub, 0),
            )
            .as_ref()
            .to_vec();
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        // a cert minted for chain-a must NOT verify against chain-b's preimage.
        let preimage_b = identity::bind_preimage("chain-b", &node_pub, 0);
        assert!(!user.public_key().verify(identity::IDENTITY_BIND_NS, &preimage_b, &sig));
    }

    #[test]
    fn mint_bind_cert_does_not_verify_under_unbind_namespace() {
        use commonware_cryptography::{
            Signer as _, Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(1);
        let node_pub = [9u8; 32];
        let cert = user
            .sign(
                identity::IDENTITY_BIND_NS,
                &identity::bind_preimage("chain-a", &node_pub, 0),
            )
            .as_ref()
            .to_vec();
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage = identity::bind_preimage("chain-a", &node_pub, 0);
        // signed under IDENTITY_BIND_NS -- must NOT verify under the unbind ns.
        assert!(!user.public_key().verify(identity::IDENTITY_UNBIND_NS, &preimage, &sig));
    }

    #[test]
    fn mint_unbind_cert_verifies_against_module_preimage() {
        use commonware_cryptography::{
            Signer as _, Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(2);
        let node_pub = [11u8; 32];
        let cert = user
            .sign(
                identity::IDENTITY_UNBIND_NS,
                &identity::unbind_preimage("chain-a", &node_pub, 3),
            )
            .as_ref()
            .to_vec();
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage = identity::unbind_preimage("chain-a", &node_pub, 3);
        assert!(user.public_key().verify(identity::IDENTITY_UNBIND_NS, &preimage, &sig));
    }

    #[test]
    fn mint_unbind_cert_is_chain_scoped() {
        use commonware_cryptography::{
            Signer as _, Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(2);
        let node_pub = [11u8; 32];
        let cert = user
            .sign(
                identity::IDENTITY_UNBIND_NS,
                &identity::unbind_preimage("chain-a", &node_pub, 3),
            )
            .as_ref()
            .to_vec();
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage_b = identity::unbind_preimage("chain-b", &node_pub, 3);
        assert!(!user.public_key().verify(identity::IDENTITY_UNBIND_NS, &preimage_b, &sig));
    }

    #[test]
    fn mint_unbind_cert_does_not_verify_under_bind_namespace() {
        use commonware_cryptography::{
            Signer as _, Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(2);
        let node_pub = [11u8; 32];
        let cert = user
            .sign(
                identity::IDENTITY_UNBIND_NS,
                &identity::unbind_preimage("chain-a", &node_pub, 3),
            )
            .as_ref()
            .to_vec();
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage = identity::unbind_preimage("chain-a", &node_pub, 3);
        // signed under IDENTITY_UNBIND_NS -- must NOT verify under the bind ns.
        assert!(!user.public_key().verify(identity::IDENTITY_BIND_NS, &preimage, &sig));
    }

    /// mirrors exactly what `cmd_user_sign_bind`/`cmd_user_sign_unbind` build,
    /// so this is a stand-in for hand-verifying the CLI's JSON output: encode
    /// through `identity::encode_msg`, decode through `identity::decode_msg`,
    /// and check the message the module actually consumes round-trips.
    #[test]
    fn user_sign_messages_round_trip_through_identity_codec() {
        let user = ed25519::PrivateKey::from_seed(3);
        let node_pub = [42u8; 32];

        let bind_msg = identity::IdentityMsg::BindNode {
            authorizer: ed25519_member_auth(
                &user,
                identity::IDENTITY_BIND_NS,
                &identity::bind_preimage("test@abc", &node_pub, 0),
            ),
        };
        let encoded = identity::encode_msg(&bind_msg);
        // the wire contract: a single utf-8 JSON line, decodable as-is.
        assert_eq!(String::from_utf8(encoded.clone()).unwrap().lines().count(), 1);
        assert_eq!(identity::decode_msg(&encoded).unwrap(), bind_msg);

        let unbind_msg = identity::IdentityMsg::UnbindNode {
            node_key: node_pub.to_vec(),
            authorizer: ed25519_member_auth(
                &user,
                identity::IDENTITY_UNBIND_NS,
                &identity::unbind_preimage("test@abc", &node_pub, 1),
            ),
        };
        let encoded = identity::encode_msg(&unbind_msg);
        assert_eq!(String::from_utf8(encoded.clone()).unwrap().lines().count(), 1);
        assert_eq!(identity::decode_msg(&encoded).unwrap(), unbind_msg);
    }

    #[cfg(unix)]
    #[test]
    fn identity_file_is_born_private() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tmp("perms");
        let path = dir.join("identity.key");
        load_or_generate_identity(&path).expect("generate");
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "secret must never be world-readable, even transiently"
        );
    }
}
