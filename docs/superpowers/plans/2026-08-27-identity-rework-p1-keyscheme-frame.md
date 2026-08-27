# Identity Rework — Phase 1: `keyscheme` crate + frame scheme byte — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One `KeyScheme { Ed25519, Secp256k1, Secp256r1 }` verifier crate shared by the kernel frame codec and the identity module; op frames carry a scheme byte so any of the three key kinds can be a frame origin; identity adopts `KeyScheme` (dropping `P256`, `WebauthnP256`, `MemberProof`, the RP-id pin) in the same PR so no second verifier survives in tree.

**Architecture:** New crate `crates/kernel/keyscheme` (below modules — the frame codec cannot depend on a module) owns the closed scheme enum, per-scheme proof parsing, and a `testkit` feature with signing helpers every test suite reuses. `crates/kernel/node` prepends one scheme byte to the frame preimage and dispatches verification through `KeyScheme::verify`; `Origin::External(pubkey)` stays raw bytes, so no origin consumer moves. `identity` deletes `src/scheme.rs` and re-exports `keyscheme::KeyScheme`; every `MemberAuth`/`AddMemberKey`/`MemberKeyView` carries `scheme` + proof BYTES. Consumers are a rename sweep; the wasm guests regenerate once.

**Tech Stack:** Rust workspace; commonware-cryptography (ed25519 namespaced verify, `union_unique`), RustCrypto `p256` (WebAuthn), `k256` + `sha3` (EIP-191 `personal_sign` recovery), borsh (stored record codec), serde (wire); guest-builder + wasm-tools for the wasm guests.

**Spec:** `docs/superpowers/specs/2026-08-27-identity-rework-design.md` — sections "`KeyScheme` — one crate, one verifier", "Frame codec", phase 1 in "Phases".

## Global Constraints

- Zero live networks: NO compat arm, NO versioned enum variant, NO alias. Replace, never keep. (`CLAUDE.md` "No Legacy, No Compat".)
- Work in a worktree at `<primary-checkout>/.worktree/feat-keyscheme-frame`, branched from `origin/dev`; PR against `dev`. Cargo `target/` lives in the worktree (disk-backed).
- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`. Never `cargo fmt --all`; format only files you touched (`rustfmt <file>`).
- `tracing`, never `println!`, in library code. CLI stdout stays `println!`.
- Rust house rules: early return, named predicates, one `match` per discriminant, no `_` wildcard on a scheme/tag match (a new scheme must fail the build until routed).
- Wire tags are `Ed25519 = 0`, `Secp256k1 = 1`, `Secp256r1 = 2`. Proof encodings: Ed25519 = exactly 64 bytes; Secp256k1 = exactly 65 bytes `r‖s‖v` (`v ∈ {0,1,27,28}`); Secp256r1 = `u32-LE len ‖ authenticator_data ‖ u32-LE len ‖ client_data_json ‖ 64-byte raw R‖S`.
- Frame preimage = `scheme_tag(1) ‖ u64-LE len ‖ origin ‖ u64-LE seq ‖ u64-LE len ‖ target ‖ u64-LE len ‖ payload`; the proof bytes follow verbatim. `pub fn encode_frame(signer: &PrivateKey, seq: u64, msg: &Msg) -> Vec<u8> {` keeps EXACTLY that signature text (`tests/no_continuation_lane.rs` pins it).
- Commit messages end with:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM
  ```
- If rustc dies with SIGSEGV/SIGBUS in an unrelated dep, `cargo clean -p <crate>` and rerun with `CARGO_INCREMENTAL=0 -j 8` (known host flake).
- Gate delivery on cargo's exit status (`${PIPESTATUS[0]}`), never on a grep of its output.

---

## File map

| path | responsibility |
|---|---|
| `crates/kernel/keyscheme/Cargo.toml` | new crate manifest; `testkit` feature |
| `crates/kernel/keyscheme/src/lib.rs` | `KeyScheme` enum, `tag/from_tag/pubkey_wellformed/verify`, the ed25519 arm |
| `crates/kernel/keyscheme/src/eth.rs` | EIP-191 `personal_sign` message + digest + recovery verify |
| `crates/kernel/keyscheme/src/webauthn.rs` | assertion envelope encode/decode/verify, `webauthn_challenge` (moved from identity) |
| `crates/kernel/keyscheme/src/testkit.rs` | signing helpers for tests (`ed25519_proof`, `eth_*`, `passkey_*`) |
| `Cargo.toml` | workspace member + alias, `k256`/`sha3` workspace deps |
| `crates/kernel/node/Cargo.toml`, `src/lib.rs` | frame codec: scheme byte, `pub frame_preimage`, `pub FRAME_NS`, verify dispatch |
| `crates/kernel/node/tests/frame_schemes.rs` | new: non-ed25519 frames decode, bad tags reject |
| `crates/modules/system/identity/{Cargo.toml, src/interface.rs, src/lib.rs, src/testkit.rs, src/tests.rs, src/guest.rs, tests/sync_round_trip.rs}` | adopt `KeyScheme`; delete `src/scheme.rs` |
| consumer sweep (Task 7) | gateway module + tests, `bin/node` gateway_plane/userkey_cli, workspace-config, acl/governance/host/noded/simnode tests, labs comment |
| `crates/modules/*/component.wasm`, `crates/kernel/host/tests/fixtures/*.component.wasm` | regenerated guests |

---

### Task 1: Worktree, branch, spec commit

**Files:**
- Create: `.worktree/feat-keyscheme-frame/` (git worktree)
- Add: `docs/superpowers/specs/2026-08-27-identity-rework-design.md`, `docs/superpowers/plans/2026-08-27-identity-rework-p1-keyscheme-frame.md` (both exist untracked in the primary checkout — copy them in)

- [ ] **Step 1: Create the worktree from origin/dev**

```bash
cd /home/eddy/dev/ducktape/ducktape
git fetch origin dev
git worktree add .worktree/feat-keyscheme-frame -b feat/keyscheme-frame origin/dev
```

- [ ] **Step 2: Copy the spec and this plan into the worktree and commit**

```bash
cp docs/superpowers/specs/2026-08-27-identity-rework-design.md .worktree/feat-keyscheme-frame/docs/superpowers/specs/
cp docs/superpowers/plans/2026-08-27-identity-rework-p1-keyscheme-frame.md .worktree/feat-keyscheme-frame/docs/superpowers/plans/
cd .worktree/feat-keyscheme-frame
git add docs/superpowers/specs/2026-08-27-identity-rework-design.md docs/superpowers/plans/2026-08-27-identity-rework-p1-keyscheme-frame.md
git commit -m "docs: identity rework design + phase 1 plan

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM"
```

All later tasks run inside `.worktree/feat-keyscheme-frame`.

---

### Task 2: `keyscheme` crate — enum, tags, well-formedness, ed25519 arm

**Files:**
- Create: `crates/kernel/keyscheme/Cargo.toml`, `crates/kernel/keyscheme/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members` block near line 83; `[workspace.dependencies]` near lines 244 and 282)

**Interfaces:**
- Produces: `keyscheme::KeyScheme` with `tag(self) -> u8`, `from_tag(u8) -> Option<KeyScheme>`, `pubkey_wellformed(self, &[u8]) -> bool`, `verify(self, pubkey: &[u8], ns: &[u8], preimage: &[u8], proof: &[u8]) -> bool`. Derives `Serialize, Deserialize (snake_case), Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize`.

- [ ] **Step 1: Register the crate in the workspace**

In `Cargo.toml` `members`, after `"crates/kernel/index-guest",` add:

```toml
    # the closed signature-scheme set + ONE verifier: shared by the kernel
    # frame codec (below modules) and the identity module (a module).
    "crates/kernel/keyscheme",
```

In `[workspace.dependencies]`, after `p256 = "0.13"` add:

```toml
# secp256k1 with public-key RECOVERY: an Ethereum wallet's `personal_sign`
# proof carries r‖s‖v and no pubkey, so verify = recover + compare.
k256 = { version = "0.13", features = ["ecdsa"] }
# keccak-256 for the EIP-191 personal-message digest.
sha3 = "0.10"
```

After `sdk = { path = "crates/kernel/sdk" }` add:

```toml
keyscheme = { path = "crates/kernel/keyscheme" }
```

- [ ] **Step 2: Write the manifest**

`crates/kernel/keyscheme/Cargo.toml`:

```toml
[package]
name = "keyscheme"
edition.workspace = true
version.workspace = true

# the CLOSED set of signature schemes a ducktape key can carry, and the one
# `(scheme, pubkey, proof) -> verified?` dispatch every signed artifact rides:
# op frames (kernel/node), account association proofs (identity), and later
# request proofs and git push certificates. below modules on purpose — the
# frame codec cannot depend on a module. pure, deterministic, wasm-clean.
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
borsh = { workspace = true }
# ed25519: commonware's namespaced EdDSA, the same verify the frame codec used.
commonware-cryptography = { workspace = true }
commonware-codec = { workspace = true }
# union_unique — the exact namespaced preimage commonware's verify hashes,
# reused as the EIP-191 message body so a wallet proof and an ed25519 proof
# are domain-separated identically.
commonware-utils = { workspace = true }
# secp256r1 (WebAuthn assertion envelope) — pure-Rust p256 on every arch.
p256 = { workspace = true }
# secp256k1 (Ethereum wallet `personal_sign`) — recovery-based verify.
k256 = { workspace = true }
sha2 = { workspace = true }
sha3 = { workspace = true }
# base64url-decoding the WebAuthn challenge out of clientDataJSON.
base64 = { workspace = true }

[features]
# test-only SIGNING helpers (src/testkit.rs). consumers enable it as a
# dev-dependency feature; a shipping build never compiles signing code here.
testkit = []

[dev-dependencies]
keyscheme = { path = ".", features = ["testkit"] }
```

- [ ] **Step 3: Write the failing enum test**

`crates/kernel/keyscheme/src/lib.rs` (whole file; the two arms marked `todo!` are filled by Tasks 3 and 4 — the tests in this task exercise only ed25519 and the tag/wellformed surface):

```rust
//! the CLOSED, versioned set of signature schemes a ducktape key can carry,
//! and the ONE verifier every signed artifact dispatches through.
//!
//! a validator must recognize EVERY scheme it might see or two honest nodes
//! disagree on an op's validity, so schemes are a compiled enum, never a
//! runtime table: adding one is a coordinated protocol change (every node
//! ships the new verify arm). every verify here is a pure boolean over
//! bytes — no clock, no RNG, no I/O — so every validator reaches the same
//! verdict.
//!
//! `proof` is SCHEME-OWNED bytes; each arm parses its own envelope:
//! - `Ed25519`: 64-byte commonware signature over `union_unique(ns, preimage)`.
//! - `Secp256k1`: 65-byte `r‖s‖v` from a wallet's `personal_sign` over the
//!   same `union_unique(ns, preimage)` bytes (see [`eth`]).
//! - `Secp256r1`: a WebAuthn assertion envelope whose challenge is
//!   `SHA-256(ns ‖ preimage)` (see [`webauthn`]).

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

mod eth;
mod webauthn;
#[cfg(feature = "testkit")]
pub mod testkit;

pub use eth::{eip191_digest, personal_message};
pub use webauthn::{webauthn_challenge, webauthn_proof};

/// the closed scheme set. borsh rides along for stored records (identity's
/// member meta); serde is the wire form. borsh numbers variants by declaration
/// order, so the declaration order IS the stored tag — never reorder.
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    BorshSerialize,
    BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum KeyScheme {
    /// everything native: device keys, node keys, SSH keys. 32-byte pubkey.
    Ed25519,
    /// an Ethereum wallet. SEC1 33/65-byte pubkey; proof is `personal_sign`.
    Secp256k1,
    /// a WebAuthn passkey. SEC1 33/65-byte pubkey; proof is the assertion envelope.
    Secp256r1,
}

impl KeyScheme {
    /// the one-byte wire tag: folded into signing preimages and the frame
    /// header. NEVER renumber — only append.
    pub fn tag(self) -> u8 {
        match self {
            KeyScheme::Ed25519 => 0,
            KeyScheme::Secp256k1 => 1,
            KeyScheme::Secp256r1 => 2,
        }
    }

    /// the inverse of [`KeyScheme::tag`]; `None` for a tag no scheme owns.
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(KeyScheme::Ed25519),
            1 => Some(KeyScheme::Secp256k1),
            2 => Some(KeyScheme::Secp256r1),
            _ => None,
        }
    }

    /// a fast, allocation-free well-formedness check on a public key's bytes
    /// for this scheme — rules out bytes that could never be a key of this
    /// scheme. NOT a substitute for [`KeyScheme::verify`].
    pub fn pubkey_wellformed(self, pubkey: &[u8]) -> bool {
        match self {
            KeyScheme::Ed25519 => pubkey.len() == 32,
            KeyScheme::Secp256k1 => k256::ecdsa::VerifyingKey::from_sec1_bytes(pubkey).is_ok(),
            KeyScheme::Secp256r1 => p256::ecdsa::VerifyingKey::from_sec1_bytes(pubkey).is_ok(),
        }
    }

    /// does `proof` demonstrate that the holder of `pubkey` (read as this
    /// scheme) authorized `preimage` under `ns`? a proof whose envelope does
    /// not fit this scheme is a categorical `false`.
    pub fn verify(self, pubkey: &[u8], ns: &[u8], preimage: &[u8], proof: &[u8]) -> bool {
        match self {
            KeyScheme::Ed25519 => verify_ed25519(pubkey, ns, preimage, proof),
            KeyScheme::Secp256k1 => eth::verify_personal_sign(pubkey, ns, preimage, proof),
            KeyScheme::Secp256r1 => webauthn::verify_assertion(pubkey, ns, preimage, proof),
        }
    }
}

/// commonware namespaced EdDSA over the raw preimage: exactly 64 proof bytes.
fn verify_ed25519(pubkey: &[u8], ns: &[u8], preimage: &[u8], proof: &[u8]) -> bool {
    use commonware_codec::DecodeExt as _;
    use commonware_cryptography::{
        Verifier as _,
        ed25519::{PublicKey, Signature},
    };
    let (Ok(pk), Ok(sig)) = (PublicKey::decode(pubkey), Signature::decode(proof)) else {
        return false;
    };
    pk.verify(ns, preimage, &sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;

    const NS: &[u8] = b"ducktape-test-ns-v1";
    const OTHER_NS: &[u8] = b"ducktape-other-ns-v1";

    #[test]
    fn tags_are_stable_and_round_trip() {
        assert_eq!(KeyScheme::Ed25519.tag(), 0);
        assert_eq!(KeyScheme::Secp256k1.tag(), 1);
        assert_eq!(KeyScheme::Secp256r1.tag(), 2);
        for s in [KeyScheme::Ed25519, KeyScheme::Secp256k1, KeyScheme::Secp256r1] {
            assert_eq!(KeyScheme::from_tag(s.tag()), Some(s));
        }
        assert_eq!(KeyScheme::from_tag(3), None);
        assert_eq!(KeyScheme::from_tag(255), None);
    }

    #[test]
    fn borsh_tag_matches_wire_tag() {
        // the stored record codec numbers variants by declaration order; the
        // declaration order must equal `tag()` or a stored scheme lies.
        for s in [KeyScheme::Ed25519, KeyScheme::Secp256k1, KeyScheme::Secp256r1] {
            assert_eq!(borsh::to_vec(&s).unwrap(), vec![s.tag()]);
        }
    }

    #[test]
    fn ed25519_verifies_and_is_namespace_and_preimage_bound() {
        let signer = commonware_cryptography::ed25519::PrivateKey::from_seed(7);
        let pk = signer.public_key();
        let pk = pk.as_ref();
        let preimage = b"chain|scheme|newkey|gen";
        let proof = signer.sign(NS, preimage).as_ref().to_vec();

        assert!(KeyScheme::Ed25519.verify(pk, NS, preimage, &proof));
        assert!(!KeyScheme::Ed25519.verify(pk, OTHER_NS, preimage, &proof));
        assert!(!KeyScheme::Ed25519.verify(pk, NS, b"different", &proof));
        // wrong scheme for the same bytes is a categorical no.
        assert!(!KeyScheme::Secp256k1.verify(pk, NS, preimage, &proof));
        assert!(!KeyScheme::Secp256r1.verify(pk, NS, preimage, &proof));
        // a 63-byte proof is not an ed25519 signature.
        assert!(!KeyScheme::Ed25519.verify(pk, NS, preimage, &proof[..63]));
    }

    #[test]
    fn wellformed_by_scheme() {
        assert!(KeyScheme::Ed25519.pubkey_wellformed(&[0u8; 32]));
        assert!(!KeyScheme::Ed25519.pubkey_wellformed(&[0u8; 33]));
        let r1 = p256::ecdsa::SigningKey::from_slice(&[0x11u8; 32]).unwrap();
        assert!(KeyScheme::Secp256r1.pubkey_wellformed(&r1.verifying_key().to_sec1_bytes()));
        assert!(!KeyScheme::Secp256r1.pubkey_wellformed(&[0u8; 33]));
        let k1 = k256::ecdsa::SigningKey::from_slice(&[0x22u8; 32]).unwrap();
        assert!(KeyScheme::Secp256k1.pubkey_wellformed(&k1.verifying_key().to_sec1_bytes()));
        assert!(!KeyScheme::Secp256k1.pubkey_wellformed(&[0u8; 33]));
    }
}
```

Create placeholder modules so the crate compiles for this task's tests — they are REPLACED in Tasks 3 and 4:

`crates/kernel/keyscheme/src/eth.rs`:
```rust
//! filled in by Task 3.
pub fn personal_message(ns: &[u8], preimage: &[u8]) -> Vec<u8> {
    commonware_utils::union_unique(ns, preimage)
}
pub fn eip191_digest(_message: &[u8]) -> [u8; 32] {
    unimplemented!("task 3")
}
pub(crate) fn verify_personal_sign(_pubkey: &[u8], _ns: &[u8], _preimage: &[u8], _proof: &[u8]) -> bool {
    false
}
```

`crates/kernel/keyscheme/src/webauthn.rs`:
```rust
//! filled in by Task 4.
pub fn webauthn_challenge(_ns: &[u8], _preimage: &[u8]) -> [u8; 32] {
    unimplemented!("task 4")
}
pub fn webauthn_proof(_authenticator_data: &[u8], _client_data_json: &[u8], _signature: &[u8]) -> Vec<u8> {
    unimplemented!("task 4")
}
pub(crate) fn verify_assertion(_pubkey: &[u8], _ns: &[u8], _preimage: &[u8], _proof: &[u8]) -> bool {
    false
}
```

`crates/kernel/keyscheme/src/testkit.rs`:
```rust
//! filled in by Tasks 3 and 4.
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p keyscheme`
Expected: 4 passed (`tags_are_stable_and_round_trip`, `borsh_tag_matches_wire_tag`, `ed25519_verifies_and_is_namespace_and_preimage_bound`, `wellformed_by_scheme`).

- [ ] **Step 5: Lint and commit**

Run: `cargo clippy -p keyscheme --tests --no-deps` — expected: no warnings (an `unimplemented!` placeholder is allowed by clippy).

```bash
git add Cargo.toml Cargo.lock crates/kernel/keyscheme
git commit -m "feat(keyscheme): the closed KeyScheme set with the ed25519 arm

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM"
```

---

### Task 3: `Secp256k1` — EIP-191 `personal_sign` recovery verify + testkit

**Files:**
- Replace: `crates/kernel/keyscheme/src/eth.rs`
- Modify: `crates/kernel/keyscheme/src/testkit.rs`

**Interfaces:**
- Consumes: `KeyScheme::verify` dispatch from Task 2.
- Produces: `keyscheme::personal_message(ns, preimage) -> Vec<u8>` (the bytes a wallet is asked to `personal_sign`), `keyscheme::eip191_digest(message) -> [u8; 32]`; testkit `eth_key(seed: u8) -> k256::ecdsa::SigningKey`, `eth_pubkey(&SigningKey) -> Vec<u8>` (33-byte compressed SEC1), `eth_proof(&SigningKey, ns, preimage) -> Vec<u8>` (65 bytes, `v` = 27/28), `ed25519_proof(&ed25519::PrivateKey, ns, preimage) -> Vec<u8>`.

- [ ] **Step 1: Write the failing tests** (append to `eth.rs` — the file is written whole in Step 3; write the tests block first and watch them fail against the Task 2 placeholder)

```rust
#[cfg(test)]
mod tests {
    use crate::testkit::{eth_key, eth_proof, eth_pubkey};
    use crate::{KeyScheme, eip191_digest, personal_message};

    const NS: &[u8] = b"ducktape-test-ns-v1";

    #[test]
    fn eip191_digest_matches_the_known_vector() {
        // keccak256("\x19Ethereum Signed Message:\n11hello world") — the
        // canonical `personal_sign("hello world")` digest every wallet produces.
        let d = eip191_digest(b"hello world");
        let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "d9eba16ed0ecae432b71fe008c98cc872bb4cc214d3220a36f365326cf807d68");
    }

    #[test]
    fn personal_sign_proof_verifies_and_binds_namespace_and_preimage() {
        let sk = eth_key(3);
        let pk = eth_pubkey(&sk);
        let preimage = b"chain|scheme|newkey|gen";
        let proof = eth_proof(&sk, NS, preimage);
        assert_eq!(proof.len(), 65);
        assert!(KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof));
        assert!(!KeyScheme::Secp256k1.verify(&pk, b"other-ns", preimage, &proof));
        assert!(!KeyScheme::Secp256k1.verify(&pk, NS, b"different", &proof));
        // another wallet's key does not verify this proof.
        assert!(!KeyScheme::Secp256k1.verify(&eth_pubkey(&eth_key(4)), NS, preimage, &proof));
    }

    #[test]
    fn both_v_conventions_are_accepted() {
        let sk = eth_key(5);
        let pk = eth_pubkey(&sk);
        let preimage = b"v-test";
        let mut proof = eth_proof(&sk, NS, preimage);
        assert!(proof[64] == 27 || proof[64] == 28);
        assert!(KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof));
        proof[64] -= 27; // the 0/1 convention some signers emit
        assert!(KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof));
        proof[64] = 9; // neither convention
        assert!(!KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof));
    }

    #[test]
    fn uncompressed_pubkey_is_accepted_too() {
        let sk = eth_key(6);
        let uncompressed = sk.verifying_key().to_encoded_point(false).as_bytes().to_vec();
        assert_eq!(uncompressed.len(), 65);
        let preimage = b"sec1-test";
        let proof = eth_proof(&sk, NS, preimage);
        assert!(KeyScheme::Secp256k1.verify(&uncompressed, NS, preimage, &proof));
    }

    #[test]
    fn wrong_length_and_tampered_proofs_fail() {
        let sk = eth_key(7);
        let pk = eth_pubkey(&sk);
        let preimage = b"tamper";
        let proof = eth_proof(&sk, NS, preimage);
        assert!(!KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof[..64]));
        let mut tampered = proof.clone();
        tampered[10] ^= 0xff;
        assert!(!KeyScheme::Secp256k1.verify(&pk, NS, preimage, &tampered));
        // the message a wallet is shown is the commonware-namespaced preimage.
        assert_eq!(personal_message(NS, preimage), commonware_utils::union_unique(NS, preimage));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p keyscheme eth::` 
Expected: compile error (testkit `eth_key` not defined) — that is the failure; proceed.

- [ ] **Step 3: Write `eth.rs` whole**

```rust
//! the `Secp256k1` arm: an Ethereum wallet's `personal_sign` as a proof.
//!
//! a wallet never signs our bytes directly — `personal_sign` wraps them in
//! the EIP-191 envelope `"\x19Ethereum Signed Message:\n" ‖ len ‖ msg` and
//! keccak-256 hashes that. the signature is `r‖s‖v` and carries no public
//! key, so verification RECOVERS the key from the signature and compares it
//! to the registered one. `msg` is [`personal_message`] — commonware's
//! `union_unique(ns, preimage)`, the same domain separation the ed25519 arm
//! gets from its namespaced verify — so a wallet proof minted for one
//! namespace can never pass under another.
//!
//! deterministic: pure-Rust k256 on every arch. low-S is NOT required (a
//! malleated signature authorizes the same bytes, which is harmless here);
//! a high-S signature is normalized before recovery and its parity bit
//! flipped accordingly.

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use sha3::{Digest, Keccak256};

const EIP191_PREFIX: &[u8] = b"\x19Ethereum Signed Message:\n";
/// r(32) ‖ s(32) ‖ v(1)
const PROOF_LEN: usize = 65;

/// the exact bytes a wallet is asked to `personal_sign` for `(ns, preimage)`
/// — commonware's namespaced preimage, so the enrollment side and this
/// verifier share one source of truth.
pub fn personal_message(ns: &[u8], preimage: &[u8]) -> Vec<u8> {
    commonware_utils::union_unique(ns, preimage)
}

/// `keccak256("\x19Ethereum Signed Message:\n" ‖ decimal(len(message)) ‖ message)`.
pub fn eip191_digest(message: &[u8]) -> [u8; 32] {
    let mut h = Keccak256::new();
    h.update(EIP191_PREFIX);
    h.update(message.len().to_string().as_bytes());
    h.update(message);
    h.finalize().into()
}

/// `v` as wallets emit it (27/28) or as raw parity (0/1); anything else is
/// not a recovery id.
fn recovery_id(v: u8) -> Option<RecoveryId> {
    let parity = match v {
        0 | 1 => v,
        27 | 28 => v - 27,
        _ => return None,
    };
    RecoveryId::from_byte(parity)
}

pub(crate) fn verify_personal_sign(pubkey: &[u8], ns: &[u8], preimage: &[u8], proof: &[u8]) -> bool {
    if proof.len() != PROOF_LEN {
        return false;
    }
    let Ok(expected) = VerifyingKey::from_sec1_bytes(pubkey) else {
        return false;
    };
    let Ok(sig) = Signature::from_slice(&proof[..64]) else {
        return false;
    };
    let Some(recid) = recovery_id(proof[64]) else {
        return false;
    };
    // a high-S signature recovers to the wrong point unless S is normalized
    // and the parity bit flipped with it.
    let (sig, recid) = match sig.normalize_s() {
        Some(low) => (low, RecoveryId::new(!recid.is_y_odd(), recid.is_x_reduced())),
        None => (sig, recid),
    };
    let digest = eip191_digest(&personal_message(ns, preimage));
    match VerifyingKey::recover_from_prehash(&digest, &sig, recid) {
        Ok(recovered) => recovered == expected,
        Err(_) => false,
    }
}

// (the #[cfg(test)] mod tests block from Step 1 goes here, verbatim)
```

- [ ] **Step 4: Write the testkit signing helpers**

`crates/kernel/keyscheme/src/testkit.rs` (whole file; the passkey helpers are added in Task 4):

```rust
//! test-only SIGNING helpers — one place every suite (keyscheme, node,
//! identity, the wasm parity proofs) mints proofs from, so "what a signer
//! produces" is written once and matches the verifier by construction.

use commonware_cryptography::{Signer as _, ed25519};

/// an ed25519 proof: commonware's namespaced signature, 64 bytes.
pub fn ed25519_proof(signer: &ed25519::PrivateKey, ns: &[u8], preimage: &[u8]) -> Vec<u8> {
    signer.sign(ns, preimage).as_ref().to_vec()
}

/// a deterministic secp256k1 signing key from a non-zero seed byte.
pub fn eth_key(seed: u8) -> k256::ecdsa::SigningKey {
    assert_ne!(seed, 0, "seed 0 is not a valid scalar");
    k256::ecdsa::SigningKey::from_slice(&[seed; 32]).expect("valid scalar")
}

/// the 33-byte compressed SEC1 point — the form a wallet registers.
pub fn eth_pubkey(sk: &k256::ecdsa::SigningKey) -> Vec<u8> {
    sk.verifying_key().to_encoded_point(true).as_bytes().to_vec()
}

/// exactly what a wallet's `personal_sign` returns for
/// [`crate::personal_message`]`(ns, preimage)`: `r‖s‖v` with `v ∈ {27, 28}`.
pub fn eth_proof(sk: &k256::ecdsa::SigningKey, ns: &[u8], preimage: &[u8]) -> Vec<u8> {
    let digest = crate::eip191_digest(&crate::personal_message(ns, preimage));
    let (sig, recid) = sk
        .sign_prehash_recoverable(&digest)
        .expect("signing a 32-byte digest");
    let mut proof = sig.to_bytes().to_vec();
    proof.push(recid.to_byte() + 27);
    proof
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p keyscheme`
Expected: all Task 2 tests + 5 `eth::tests::*` pass. If `eip191_digest_matches_the_known_vector` fails, the prefix or the decimal length is wrong — the vector is the standard `personal_sign("hello world")` digest (independently recomputed with `sha3 0.10` while writing this plan); do NOT change the vector.

- [ ] **Step 6: Lint and commit**

Run: `cargo clippy -p keyscheme --tests --no-deps`

```bash
git add crates/kernel/keyscheme
git commit -m "feat(keyscheme): Secp256k1 — EIP-191 personal_sign recovery verify

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM"
```

---

### Task 4: `Secp256r1` — WebAuthn assertion envelope (moved from identity) + testkit

**Files:**
- Replace: `crates/kernel/keyscheme/src/webauthn.rs`
- Modify: `crates/kernel/keyscheme/src/testkit.rs` (append passkey helpers)
- Reference (read, do not edit yet — deleted in Task 6): `crates/modules/system/identity/src/scheme.rs:158-176, 272-338` (the verifier being moved), `:475-520` (the `make_webauthn_proof` fixture being moved)

**Interfaces:**
- Produces: `keyscheme::webauthn_challenge(ns, preimage) -> [u8; 32]`, `keyscheme::webauthn_proof(authenticator_data, client_data_json, signature) -> Vec<u8>`; testkit `passkey(seed: u8) -> p256::ecdsa::SigningKey`, `passkey_pubkey(&SigningKey) -> Vec<u8>` (33-byte SEC1), `passkey_proof(&SigningKey, rp_id: &str, ns, preimage, user_present: bool) -> Vec<u8>`, `passkey_proof_typed(&SigningKey, rp_id, ns, preimage, client_type: &str) -> Vec<u8>`.

- [ ] **Step 1: Write the failing tests** (the `tests` block of the new `webauthn.rs`)

```rust
#[cfg(test)]
mod tests {
    use crate::testkit::{passkey, passkey_proof, passkey_proof_typed, passkey_pubkey};
    use crate::{KeyScheme, webauthn_proof};

    const NS: &[u8] = b"ducktape-test-ns-v1";
    const OTHER_NS: &[u8] = b"ducktape-other-ns-v1";

    #[test]
    fn assertion_verifies_and_binds_challenge() {
        let sk = passkey(0x21);
        let pk = passkey_pubkey(&sk);
        let preimage = b"chain|scheme|newkey|gen-0";
        let proof = passkey_proof(&sk, "auth.ducktape.byeongsu.dev", NS, preimage, true);
        assert!(KeyScheme::Secp256r1.verify(&pk, NS, preimage, &proof));
        assert!(!KeyScheme::Secp256r1.verify(&pk, OTHER_NS, preimage, &proof));
        assert!(!KeyScheme::Secp256r1.verify(&pk, NS, b"chain|scheme|newkey|gen-1", &proof));
        // the same envelope is not a k1 or ed25519 proof.
        assert!(!KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof));
        assert!(!KeyScheme::Ed25519.verify(&pk, NS, preimage, &proof));
    }

    #[test]
    fn user_presence_is_required() {
        let sk = passkey(0x22);
        let pk = passkey_pubkey(&sk);
        let proof = passkey_proof(&sk, "rp", NS, b"up", false);
        assert!(!KeyScheme::Secp256r1.verify(&pk, NS, b"up", &proof));
    }

    #[test]
    fn registration_type_is_rejected() {
        let sk = passkey(0x23);
        let pk = passkey_pubkey(&sk);
        let proof = passkey_proof_typed(&sk, "rp", NS, b"type", "webauthn.create");
        assert!(!KeyScheme::Secp256r1.verify(&pk, NS, b"type", &proof));
    }

    #[test]
    fn tampered_signature_and_malformed_envelopes_fail() {
        let sk = passkey(0x24);
        let pk = passkey_pubkey(&sk);
        let proof = passkey_proof(&sk, "rp", NS, b"tamper", true);
        let mut tampered = proof.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        assert!(!KeyScheme::Secp256r1.verify(&pk, NS, b"tamper", &tampered));
        // a truncated envelope, and a length prefix pointing past the end.
        assert!(!KeyScheme::Secp256r1.verify(&pk, NS, b"tamper", &proof[..proof.len() - 1]));
        let forged = webauthn_proof(&[0u8; 36], b"{}", &[0u8; 64]); // authData under the 37-byte minimum
        assert!(!KeyScheme::Secp256r1.verify(&pk, NS, b"tamper", &forged));
        let mut bad_len = proof.clone();
        bad_len[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(!KeyScheme::Secp256r1.verify(&pk, NS, b"tamper", &bad_len));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p keyscheme webauthn::`
Expected: compile error (testkit `passkey` not defined).

- [ ] **Step 3: Write `webauthn.rs` whole**

```rust
//! the `Secp256r1` arm: a WebAuthn passkey's ASSERTION as a proof.
//!
//! a passkey never signs our bytes. it signs the fixed WebAuthn structure
//! `authenticatorData ‖ SHA-256(clientDataJSON)`, and the only field we
//! control is `clientDataJSON.challenge` — so our preimage is HASHED into the
//! challenge ([`webauthn_challenge`]) and verification is an ENVELOPE check:
//! parse clientDataJSON, match the challenge, require the `webauthn.get`
//! type and the User-Present flag, then verify raw ECDSA-P256 over the
//! reconstructed signed bytes. the signature is raw `R‖S` (the transport
//! normalizes the authenticator's DER away before it reaches consensus).
//!
//! no RP-id pin: a passkey is scoped to its RP by construction, so its
//! public key can never answer under another RP.
//!
//! envelope on the wire (the scheme-owned proof bytes):
//! `u32-LE len ‖ authenticator_data ‖ u32-LE len ‖ client_data_json ‖ sig(64)`.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const WEBAUTHN_GET_TYPE: &str = "webauthn.get";
/// authenticatorData is at minimum rpIdHash(32) ‖ flags(1) ‖ signCount(4).
const AUTH_DATA_MIN_LEN: usize = 37;
/// flags bit 0: User Present.
const FLAG_USER_PRESENT: u8 = 0x01;
const SIG_LEN: usize = 64;

/// the challenge a passkey must sign for `(ns, preimage)`: `SHA-256(ns ‖ preimage)`.
/// public so the enrollment side computes it from the exact bytes the
/// verifier checks against.
pub fn webauthn_challenge(ns: &[u8], preimage: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(ns);
    h.update(preimage);
    h.finalize().into()
}

/// frame an assertion as the scheme-owned proof bytes.
pub fn webauthn_proof(authenticator_data: &[u8], client_data_json: &[u8], signature: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + authenticator_data.len() + client_data_json.len() + signature.len());
    push(&mut out, authenticator_data);
    push(&mut out, client_data_json);
    out.extend_from_slice(signature);
    out
}

fn push(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn take<'a>(buf: &mut &'a [u8]) -> Option<&'a [u8]> {
    let (head, rest) = buf.split_at_checked(4)?;
    let len = u32::from_le_bytes(head.try_into().expect("split of 4")) as usize;
    let (bytes, rest) = rest.split_at_checked(len)?;
    *buf = rest;
    Some(bytes)
}

struct Assertion<'a> {
    authenticator_data: &'a [u8],
    client_data_json: &'a [u8],
    signature: &'a [u8],
}

fn split(proof: &[u8]) -> Option<Assertion<'_>> {
    let mut buf = proof;
    let authenticator_data = take(&mut buf)?;
    let client_data_json = take(&mut buf)?;
    let is_exact_signature = buf.len() == SIG_LEN;
    if !is_exact_signature {
        return None;
    }
    Some(Assertion {
        authenticator_data,
        client_data_json,
        signature: buf,
    })
}

pub(crate) fn verify_assertion(pubkey: &[u8], ns: &[u8], preimage: &[u8], proof: &[u8]) -> bool {
    use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier as _};

    let Some(Assertion {
        authenticator_data,
        client_data_json,
        signature,
    }) = split(proof)
    else {
        return false;
    };

    // 1. authenticatorData shape + user presence.
    if authenticator_data.len() < AUTH_DATA_MIN_LEN {
        return false;
    }
    let user_present = authenticator_data[32] & FLAG_USER_PRESENT != 0;
    if !user_present {
        return false;
    }

    // 2. clientDataJSON: a `get` assertion whose challenge is exactly ours.
    #[derive(Deserialize)]
    struct ClientData {
        #[serde(rename = "type")]
        type_: String,
        challenge: String,
    }
    let Ok(client) = serde_json::from_slice::<ClientData>(client_data_json) else {
        return false;
    };
    if client.type_ != WEBAUTHN_GET_TYPE {
        return false;
    }
    let Ok(challenge) = URL_SAFE_NO_PAD.decode(client.challenge.as_bytes()) else {
        return false;
    };
    if challenge != webauthn_challenge(ns, preimage) {
        return false;
    }

    // 3. raw ECDSA-P256-SHA256 over `authenticatorData ‖ SHA-256(clientDataJSON)`.
    let (Ok(vk), Ok(sig)) = (
        VerifyingKey::from_sec1_bytes(pubkey),
        Signature::from_slice(signature),
    ) else {
        return false;
    };
    let mut signed = Vec::with_capacity(authenticator_data.len() + 32);
    signed.extend_from_slice(authenticator_data);
    signed.extend_from_slice(&Sha256::digest(client_data_json));
    vk.verify(&signed, &sig).is_ok()
}

// (the #[cfg(test)] mod tests block from Step 1 goes here, verbatim)
```

- [ ] **Step 4: Append the passkey helpers to `testkit.rs`**

```rust
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest as _, Sha256};

/// a deterministic P-256 signing key from a non-zero seed byte.
pub fn passkey(seed: u8) -> p256::ecdsa::SigningKey {
    assert_ne!(seed, 0, "seed 0 is not a valid scalar");
    p256::ecdsa::SigningKey::from_slice(&[seed; 32]).expect("valid scalar")
}

/// the 33-byte compressed SEC1 point the transport lifts out of the COSE key.
pub fn passkey_pubkey(sk: &p256::ecdsa::SigningKey) -> Vec<u8> {
    sk.verifying_key().to_sec1_bytes().to_vec()
}

/// a self-consistent `webauthn.get` assertion for `(ns, preimage)` under
/// `rp_id` — exactly what an authenticator produces, so a passing verify
/// proves the envelope reconstruction matches real signing.
pub fn passkey_proof(
    sk: &p256::ecdsa::SigningKey,
    rp_id: &str,
    ns: &[u8],
    preimage: &[u8],
    user_present: bool,
) -> Vec<u8> {
    assertion(sk, rp_id, ns, preimage, user_present, "webauthn.get")
}

/// the same envelope with a caller-chosen clientData `type` (a
/// `webauthn.create` must NOT verify as a possession proof).
pub fn passkey_proof_typed(
    sk: &p256::ecdsa::SigningKey,
    rp_id: &str,
    ns: &[u8],
    preimage: &[u8],
    client_type: &str,
) -> Vec<u8> {
    assertion(sk, rp_id, ns, preimage, true, client_type)
}

fn assertion(
    sk: &p256::ecdsa::SigningKey,
    rp_id: &str,
    ns: &[u8],
    preimage: &[u8],
    user_present: bool,
    client_type: &str,
) -> Vec<u8> {
    use p256::ecdsa::{Signature, signature::Signer as _};
    let challenge = crate::webauthn_challenge(ns, preimage);
    let client_data_json = format!(
        r#"{{"type":"{client_type}","challenge":"{}","origin":"https://{rp_id}"}}"#,
        URL_SAFE_NO_PAD.encode(challenge)
    )
    .into_bytes();
    let mut authenticator_data = Vec::new();
    authenticator_data.extend_from_slice(&Sha256::digest(rp_id.as_bytes()));
    authenticator_data.push(if user_present { 0x01 } else { 0 });
    authenticator_data.extend_from_slice(&0u32.to_be_bytes()); // signCount
    let mut signed = authenticator_data.clone();
    signed.extend_from_slice(&Sha256::digest(&client_data_json));
    // RustCrypto signs deterministically (RFC6979), low-S; `.to_bytes()` is raw R‖S.
    let sig: Signature = sk.sign(&signed);
    crate::webauthn_proof(&authenticator_data, &client_data_json, &sig.to_bytes())
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p keyscheme`
Expected: every test in the crate passes (Task 2: 4, Task 3: 5, Task 4: 4).

- [ ] **Step 6: Lint, format, commit**

Run: `cargo clippy -p keyscheme --tests --no-deps && rustfmt crates/kernel/keyscheme/src/*.rs`

```bash
git add crates/kernel/keyscheme
git commit -m "feat(keyscheme): Secp256r1 — the WebAuthn assertion envelope, moved out of identity

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM"
```

---

### Task 5: Frame codec — scheme byte, public preimage, multi-scheme decode

**Files:**
- Modify: `crates/kernel/node/Cargo.toml` (deps), `crates/kernel/node/src/lib.rs:96` (`FRAME_NS`), `:118-135` (frame doc comment), `:169-235` (`frame_preimage`, `encode_frame`, `decode_frame`), `:240-247` (`frame_origin_seq`)
- Create: `crates/kernel/node/tests/frame_schemes.rs`

**Interfaces:**
- Consumes: `keyscheme::KeyScheme`, testkit `eth_*`, `passkey_*`.
- Produces: `pub const node::FRAME_NS: &[u8]`; `pub fn node::frame_preimage(scheme: KeyScheme, origin: &[u8], seq: u64, msg: &Msg) -> Vec<u8>`; `encode_frame` (unchanged signature, tag 0); `decode_frame` accepting all three schemes; `frame_origin_seq` skipping the tag byte.

- [ ] **Step 1: Add the dependency**

In `crates/kernel/node/Cargo.toml` `[dependencies]`, replace the two-line comment + `commonware-cryptography`/`commonware-codec` entries with:

```toml
# op frames are SIGNED: encode signs (ed25519, the node/device key), decode
# verifies under the frame's declared scheme, so the host's Origin::External
# is authenticated authorship under any scheme a key can carry.
keyscheme = { workspace = true }
commonware-cryptography = { workspace = true }
```

(`commonware-codec` is no longer used by `lib.rs` after this task — remove it; re-add only if `cargo check -p node` names it.) In `[dev-dependencies]` add:

```toml
# non-ed25519 frame proofs in tests/frame_schemes.rs.
keyscheme = { workspace = true, features = ["testkit"] }
```

- [ ] **Step 2: Write the failing tests**

`crates/kernel/node/tests/frame_schemes.rs`:

```rust
//! a frame's origin may be ANY scheme in `keyscheme`: the frame declares its
//! scheme in the first byte, the proof bytes follow the preimage, and
//! `decode_frame` verifies under that scheme. `Origin::External` stays the
//! raw pubkey bytes — no consumer learns or needs the scheme.

use keyscheme::KeyScheme;
use keyscheme::testkit::{eth_key, eth_proof, eth_pubkey, passkey, passkey_proof, passkey_pubkey};
use sdk::{Msg, Origin};

fn msg() -> Msg {
    Msg {
        target: "kv".into(),
        payload: b"{\"set\":{\"k\":\"v\"}}".to_vec(),
    }
}

#[test]
fn an_ed25519_frame_declares_tag_zero() {
    use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
    let signer = PrivateKey::from_seed(1);
    let frame = node::encode_frame(&signer, 5, &msg());
    assert_eq!(frame[0], KeyScheme::Ed25519.tag());
    let (origin, m) = node::decode_frame(&frame).expect("decodes");
    assert_eq!(origin, Origin::External(signer.public_key().as_ref().to_vec()));
    assert_eq!(m, msg());
    assert_eq!(node::frame_origin_seq(&frame), Some((signer.public_key().as_ref().to_vec(), 5)));
}

#[test]
fn a_wallet_signed_frame_decodes_to_the_wallet() {
    let sk = eth_key(9);
    let pk = eth_pubkey(&sk);
    let mut frame = node::frame_preimage(KeyScheme::Secp256k1, &pk, 7, &msg());
    let proof = eth_proof(&sk, node::FRAME_NS, &frame);
    frame.extend_from_slice(&proof);
    let (origin, m) = node::decode_frame(&frame).expect("a wallet frame decodes");
    assert_eq!(origin, Origin::External(pk.clone()));
    assert_eq!(m, msg());
    assert_eq!(node::frame_origin_seq(&frame), Some((pk, 7)));
}

#[test]
fn a_passkey_signed_frame_decodes_to_the_passkey() {
    let sk = passkey(0x31);
    let pk = passkey_pubkey(&sk);
    let mut frame = node::frame_preimage(KeyScheme::Secp256r1, &pk, 1, &msg());
    let proof = passkey_proof(&sk, "auth.ducktape.byeongsu.dev", node::FRAME_NS, &frame, true);
    frame.extend_from_slice(&proof);
    let (origin, _) = node::decode_frame(&frame).expect("a passkey frame decodes");
    assert_eq!(origin, Origin::External(pk));
}

#[test]
fn an_unknown_scheme_tag_is_rejected() {
    let sk = eth_key(9);
    let pk = eth_pubkey(&sk);
    let mut frame = node::frame_preimage(KeyScheme::Secp256k1, &pk, 7, &msg());
    let proof = eth_proof(&sk, node::FRAME_NS, &frame);
    frame.extend_from_slice(&proof);
    frame[0] = 9;
    assert!(node::decode_frame(&frame).is_err());
    assert_eq!(node::frame_origin_seq(&frame), None);
}

#[test]
fn a_key_under_the_wrong_scheme_is_rejected() {
    // an ed25519 key claiming to be a passkey: well-formedness (32 bytes is
    // not a SEC1 point) and the verify both refuse.
    use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
    let signer = PrivateKey::from_seed(2);
    let pk = signer.public_key().as_ref().to_vec();
    let mut frame = node::frame_preimage(KeyScheme::Secp256r1, &pk, 0, &msg());
    let proof = signer.sign(node::FRAME_NS, &frame).as_ref().to_vec();
    frame.extend_from_slice(&proof);
    assert!(node::decode_frame(&frame).is_err());
    // and a wallet key with an ed25519-length proof under tag 0.
    let sk = eth_key(3);
    let mut frame = node::frame_preimage(KeyScheme::Ed25519, &eth_pubkey(&sk), 0, &msg());
    frame.extend_from_slice(&[0u8; 64]);
    assert!(node::decode_frame(&frame).is_err());
}

#[test]
fn a_tampered_wallet_frame_is_rejected() {
    let sk = eth_key(4);
    let pk = eth_pubkey(&sk);
    let mut frame = node::frame_preimage(KeyScheme::Secp256k1, &pk, 2, &msg());
    let proof = eth_proof(&sk, node::FRAME_NS, &frame);
    frame.extend_from_slice(&proof);
    let last_payload_byte = frame.len() - 65 - 1;
    frame[last_payload_byte] ^= 0x01;
    assert!(node::decode_frame(&frame).is_err());
}
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p node --test frame_schemes`
Expected: compile errors — `node::FRAME_NS` and `node::frame_preimage` are private / have the old signature.

- [ ] **Step 4: Rewrite the codec**

In `crates/kernel/node/src/lib.rs`:

Replace the `use` block at lines 83-88 with:

```rust
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use keyscheme::KeyScheme;
use sdk::Origin;
```

Replace line 96 (`const FRAME_NS`) with:

```rust
/// the signing domain for op frames. domain-separated so an op signature can
/// never double as a consensus vote, an endpoint advertisement, or any other
/// signed artifact in the system. the ONE codec: a scheme tag, then
/// length-prefixed `(origin, seq, target, payload)` and nothing else. a frame
/// carries EXACTLY ONE op — there is no envelope continuation section, so a
/// frame cannot append a second op that dispatches under a caller-chosen
/// `Origin::Module` (see `no_continuation_lane.rs`). PUBLIC so an external
/// signer (a wallet, a passkey page) signs the exact namespace the decoder
/// verifies against.
pub const FRAME_NS: &[u8] = b"ducktape:op-frame:v1";
```

In the frame doc comment (lines 118-135) change the first sentence to: `// a wire frame: the ordered unit. carries the submitter's public key ("origin") under a declared SCHEME (the first byte — ed25519 for nodes and device keys, secp256k1 for a wallet, secp256r1 for a passkey), a per-origin monotonic "seq" ...` and change the last paragraph's `with the 64-byte signature appended` to `with the scheme's proof bytes appended (64 for ed25519, 65 for a wallet, an assertion envelope for a passkey)`.

Replace `frame_preimage` (lines 169-186) with:

```rust
/// the signed preimage AND the frame's wire prefix: the scheme tag, then
/// length-prefixed fields so no two (seq, target, payload) triples can
/// collide across a moving boundary. a frame is exactly these bytes with the
/// scheme's proof appended, so [`decode_frame`] verifies against the received
/// prefix without rebuilding anything. PUBLIC so a wallet or passkey client
/// signs the exact bytes the decoder verifies — never a reconstruction.
pub fn frame_preimage(scheme: KeyScheme, origin: &[u8], seq: u64, msg: &Msg) -> Vec<u8> {
    let target = msg.target.as_bytes();
    let mut out = Vec::with_capacity(1 + 8 * 3 + origin.len() + target.len() + msg.payload.len());
    out.push(scheme.tag());
    out.extend_from_slice(&(origin.len() as u64).to_le_bytes());
    out.extend_from_slice(origin);
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&(target.len() as u64).to_le_bytes());
    out.extend_from_slice(target);
    out.extend_from_slice(&(msg.payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&msg.payload);
    out
}
```

Replace `encode_frame` body (keep the signature line byte-for-byte):

```rust
/// frame and SIGN a locally-originated msg for the ordered lane with an
/// ed25519 key (a node's or a device's). the signer's public key becomes the
/// frame's origin under tag 0; the frame bytes are the signed preimage with
/// the 64-byte signature appended. other schemes sign [`frame_preimage`]
/// externally and append their own proof.
pub fn encode_frame(signer: &PrivateKey, seq: u64, msg: &Msg) -> Vec<u8> {
    let origin = signer.public_key();
    let mut frame = frame_preimage(KeyScheme::Ed25519, origin.as_ref(), seq, msg);
    let sig = signer.sign(FRAME_NS, &frame);
    frame.extend_from_slice(sig.as_ref());
    frame
}
```

Replace `decode_frame` with:

```rust
/// decode a delivered frame back to `(Origin, Msg)`, VERIFYING the proof
/// first under the frame's declared scheme. rejects deterministically on: an
/// unknown scheme tag, a parse failure, TRAILING BYTES between the payload
/// and the proof (exactly one valid encoding per frame — this is what makes
/// an appended continuation section unrepresentable; every scheme's proof is
/// self-delimiting so the boundary is the preimage's own end), an origin
/// malformed for its scheme, or a proof that does not bind the whole
/// preimage. the ordered drain treats any rejection as a deterministic no-op.
/// the verified `origin` becomes the block's `Origin::External(pubkey)` — raw
/// key bytes, scheme not surfaced (a key's bytes cannot collide across
/// schemes without a discrete log on the other curve); the `seq` is
/// ordering/replay metadata, not surfaced.
pub fn decode_frame(bytes: &[u8]) -> Result<(Origin, Msg), Error> {
    let parse_err = || Error::Host(sdk::Error::Module("frame does not parse".into()));
    let mut buf = bytes;
    let (tag, rest) = buf.split_first().ok_or_else(parse_err)?;
    buf = rest;
    let scheme = KeyScheme::from_tag(*tag)
        .ok_or_else(|| Error::Host(sdk::Error::Module(format!("frame scheme tag {tag} is unknown"))))?;
    let origin = take_slice(&mut buf).ok_or_else(parse_err)?;
    // seq is ordering/replay metadata, consumed but not surfaced.
    let Some(_seq) = take_u64(&mut buf) else {
        return Err(parse_err());
    };
    let target = std::str::from_utf8(take_slice(&mut buf).ok_or_else(parse_err)?)
        .map_err(|_| parse_err())?;
    let payload = take_slice(&mut buf).ok_or_else(parse_err)?;
    let preimage_len = bytes.len() - buf.len();
    if !scheme.pubkey_wellformed(origin) {
        return Err(Error::Host(sdk::Error::Module(
            "frame origin is malformed for its scheme".into(),
        )));
    }
    if !scheme.verify(origin, FRAME_NS, &bytes[..preimage_len], buf) {
        return Err(Error::Host(sdk::Error::Module(
            "frame proof does not bind this op to its origin".into(),
        )));
    }
    Ok((
        Origin::External(origin.to_vec()),
        Msg {
            target: target.to_string(),
            payload: payload.to_vec(),
        },
    ))
}
```

Replace `frame_origin_seq` with:

```rust
pub fn frame_origin_seq(bytes: &[u8]) -> Option<(Vec<u8>, u64)> {
    let (tag, mut buf) = bytes.split_first()?;
    KeyScheme::from_tag(*tag)?;
    let origin = take_slice(&mut buf)?;
    let seq = take_u64(&mut buf)?;
    Some((origin.to_vec(), seq))
}
```

Search the file for any other use of `Signature::decode` / `PublicKey::decode` (there should be none left) and for the comment at lines 127-129 mentioning "per-origin nonce enforcement IN STATE is the planned successor" — leave it.

- [ ] **Step 5: Run the whole node test suite**

Run: `cargo test -p node`
Expected: all pass, including the pre-existing `no_continuation_lane`, `submit_frame`, `frame_size_guard`, `batch_aggregation`, `submit_decoded` suites (a frame grew by one byte; `MAX_FRAME_BYTES` carries 16 KiB of headroom over a full chunk, so `frame_size_guard` is unaffected).

Then: `cargo test -p node --test frame_schemes` — expected 6 passed.

- [ ] **Step 6: Anything else that decodes a frame?**

Run: `grep -rn "take_slice\|frame_preimage\|FRAME_NS" --include=*.rs bin crates app | grep -v "crates/kernel/node/"`
Expected: no hits outside `crates/kernel/node` (the relay lane, the validator drain, userkey_cli's `sign-frame`, and the app's signer all call `encode_frame`/`decode_member`, which are unchanged in signature). If a hit appears, it is a private codec duplicate — replace it with a call into `node::`.

- [ ] **Step 7: Lint, format, commit**

Run: `cargo clippy -p node --tests --no-deps && rustfmt crates/kernel/node/src/lib.rs crates/kernel/node/tests/frame_schemes.rs`

```bash
git add crates/kernel/node
git commit -m "feat(node): op frames declare their key scheme; any KeyScheme is a frame origin

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM"
```

---

### Task 6: identity adopts `KeyScheme` — delete `scheme.rs`, proofs become bytes, no RP pin

**Files:**
- Delete: `crates/modules/system/identity/src/scheme.rs`
- Modify: `crates/modules/system/identity/Cargo.toml`; `src/interface.rs:1-31` (docs), `:44-56` (`MemberKeyView`), `:92-104` (`MemberAuth`), `:122-131` (`AddMemberKey`), `:212-268` (preimages; delete `add_member_signing_payload`), `:290-375` (tests); `src/lib.rs:1-10` (docs), `:76-81` (mod/re-exports), `:148-158` (`MemberMeta`), `:328-356` (`authorize`), `:365-372` (`account_view`), `:528-560` (founding), `:645-725` (`add_member_key`); `src/testkit.rs`; `src/guest.rs:21-24` (doc); `src/tests.rs:60-145, 208, 241-249, 296-304, 364-387, 415-431, 480-487, 548-554, 777-785`; `tests/sync_round_trip.rs:22-80, 191-198`

**Interfaces:**
- Consumes: `keyscheme::KeyScheme`, `keyscheme::testkit::*`.
- Produces (identity's wire, phase-1 shape):
  ```rust
  pub use keyscheme::KeyScheme;
  pub struct MemberAuth { pub key: Vec<u8>, pub scheme: KeyScheme, pub proof: Vec<u8> }
  pub struct MemberKeyView { pub pubkey: Vec<u8>, pub scheme: KeyScheme, pub label: Option<String>, pub added_at: u64 }
  IdentityMsg::AddMemberKey { new_key: Vec<u8>, new_scheme: KeyScheme, new_label: Option<String>, possession: Vec<u8>, authorizer: MemberAuth }
  pub fn add_member_preimage(chain_id: &str, account_id: &[u8], new_key: &[u8], new_scheme: KeyScheme, nonce: u64) -> Vec<u8>
  ```
  Everything else in identity's wire is unchanged in this phase (`BindNode`, `UnbindNode`, `RemoveMemberKey`, `SetAccountName`, `SetProfile`, `SetNodeLabel`, queries, `bind_preimage`, `unbind_preimage`, `remove_member_preimage`, the four `IDENTITY_*_NS`).

- [ ] **Step 1: Manifest**

In `crates/modules/system/identity/Cargo.toml`: replace the header comment's "commonware-cryptography (to verify ed25519/p256 member certificates deterministically) + p256/base64 (for the bespoke WebAuthn passkey envelope ...)" with "keyscheme (the one verifier every member proof rides)". Add `keyscheme = { workspace = true }` to `[dependencies]`. Remove `p256`, `base64`, `sha2`, `commonware-utils`, `commonware-codec` and their comments. Keep `commonware-cryptography` (the `testkit` feature signs with it). Add to `[dev-dependencies]`: `keyscheme = { workspace = true, features = ["testkit"] }`.

- [ ] **Step 2: Delete the old verifier**

```bash
git rm crates/modules/system/identity/src/scheme.rs
```

In `src/lib.rs` lines 76-81 replace the `mod scheme;` block and its `pub use` with:

```rust
// the one verifier every member proof rides — shared with the kernel frame
// codec, so an account key and a frame origin are verified identically.
pub use keyscheme::KeyScheme;
```

- [ ] **Step 3: `interface.rs`**

Line 20: `pub use crate::scheme::{KeyKind, MemberProof};` → `pub use keyscheme::KeyScheme;`.

Header doc (lines 3-16): replace "collects many MEMBER KEYS of different schemes (an ed25519 seed key, a WebAuthn passkey, ...)" with "collects many MEMBER KEYS of different [`KeyScheme`]s (an ed25519 device key, an Ethereum wallet, a WebAuthn passkey)"; replace "(which key, which scheme, and the proof over the op's chain-and-nonce-scoped preimage)" with "(which key, its scheme, and the scheme-owned proof BYTES over the op's chain-and-nonce-scoped preimage — `KeyScheme::verify` parses them)".

`MemberKeyView`:
```rust
pub struct MemberKeyView {
    pub pubkey: Vec<u8>,
    pub scheme: KeyScheme,
    pub label: Option<String>,
    pub added_at: u64,
}
```

`MemberAuth`:
```rust
/// an authorization by one member key: which key, its scheme, and its
/// scheme-owned proof bytes over the operation's preimage. the account it
/// speaks for is resolved from this key's membership -- never carried as a
/// spoofable payload field.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemberAuth {
    pub key: Vec<u8>,
    pub scheme: KeyScheme,
    pub proof: Vec<u8>,
}
```

`AddMemberKey` variant:
```rust
    /// add `new_key` (of `new_scheme`, optional `new_label`) to the account
    /// `authorizer` belongs to. TWO consents over [`add_member_preimage`]: the
    /// existing member `authorizer` admits it, and `possession` (the new key's
    /// own proof bytes) proves the new key holds itself. bumps the nonce.
    AddMemberKey {
        new_key: Vec<u8>,
        new_scheme: KeyScheme,
        new_label: Option<String>,
        possession: Vec<u8>,
        authorizer: MemberAuth,
    },
```

`add_member_preimage`: rename the parameter `new_kind: KeyKind` → `new_scheme: KeyScheme` and `out.push(new_kind.tag())` → `out.push(new_scheme.tag())`; doc "its one-byte kind tag" → "its one-byte scheme tag". Delete `add_member_signing_payload` entirely (lines 249-268) — its only caller was the `p256-payload` verb, deleted in Task 7.

Tests (lines 290-375): `KeyKind::WebauthnP256` → `KeyScheme::Secp256r1`, `KeyKind::P256` → `KeyScheme::Secp256k1`, `KeyKind::Ed25519` → `KeyScheme::Ed25519`; in `msg_codec_roundtrips` the auth becomes
```rust
        let auth = MemberAuth {
            key: vec![7; 32],
            scheme: KeyScheme::Ed25519,
            proof: vec![9; 64],
        };
```
and the `AddMemberKey` arm becomes
```rust
            IdentityMsg::AddMemberKey {
                new_key: vec![2; 33],
                new_scheme: KeyScheme::Secp256r1,
                new_label: Some("phone".into()),
                possession: vec![3; 120],
                authorizer: auth.clone(),
            },
```

- [ ] **Step 4: `lib.rs`**

Header doc line 5: "(an ed25519 seed key, a WebAuthn passkey, a native P-256 key -- see [`KeyKind`])" → "(an ed25519 device key, an Ethereum wallet, a WebAuthn passkey -- see [`KeyScheme`])". Line 41: `[`KeyKind::pubkey_wellformed`]` → `[`KeyScheme::pubkey_wellformed`]`.

`MemberMeta` (lines 148-158):
```rust
/// per-member metadata; the public key is the map key, so it is not
/// repeated. serialized verbatim inside [`AccountRecord`].
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
struct MemberMeta {
    scheme: KeyScheme,
    label: Option<String>,
    added_at: u64,
}
```

`authorize` (lines 328-356):
```rust
    fn authorize(
        record: &AccountRecord,
        namespace: &[u8],
        preimage: &[u8],
        auth: &MemberAuth,
    ) -> Result<(), Error> {
        let meta = record
            .member_keys
            .get(&auth.key)
            .ok_or_else(|| Error::Module("authorizer is not a member of this account".into()))?;
        let scheme_matches_registration = meta.scheme == auth.scheme;
        if !scheme_matches_registration {
            return Err(Error::Module(
                "authorizer scheme does not match its registered scheme".into(),
            ));
        }
        if !auth.scheme.verify(&auth.key, namespace, preimage, &auth.proof) {
            return Err(Error::Module(
                "authorizer certificate does not verify".into(),
            ));
        }
        Ok(())
    }
```

`account_view` (line 370): `kind: meta.kind,` → `scheme: meta.scheme,`.

Founding branch of `bind_node` (lines 528-555): `authorizer.kind.pubkey_wellformed` → `authorizer.scheme.pubkey_wellformed`; error text "founding key is malformed for its kind" → "... for its scheme"; DELETE the `let rp_id_hash = ...` block; the inserted meta becomes
```rust
                    MemberMeta {
                        scheme: authorizer.scheme,
                        label: None,
                        added_at: ctx.env().consensus_time,
                    },
```

`add_member_key` (lines 645-725): signature `new_kind: KeyKind` → `new_scheme: KeyScheme`, `possession: MemberProof` → `possession: Vec<u8>`; `new_kind.pubkey_wellformed(&new_key)` → `new_scheme.pubkey_wellformed(&new_key)` ("malformed for its scheme"); `add_member_preimage(..., new_kind, ...)` → `new_scheme`; the possession check becomes
```rust
        // ... and the new key proves it holds itself.
        if !new_scheme.verify(&new_key, IDENTITY_ADD_MEMBER_NS, &preimage, &possession) {
            return Err(Error::Module("possession proof does not verify".into()));
        }
```
DELETE the `let rp_id_hash = ...` block; the inserted meta becomes `MemberMeta { scheme: new_scheme, label, added_at: ctx.env().consensus_time }`.

The `execute` dispatch arm for `AddMemberKey` (around line 425) renames the destructured fields: `new_kind` → `new_scheme`.

`src/guest.rs` lines 21-24: "the WebAuthn / P-256 member verifies run IN the guest — pure-Rust p256, deterministic on wasm32" → "every member proof verifies IN the guest through `keyscheme` — pure-Rust p256/k256 and commonware ed25519, deterministic on wasm32".

- [ ] **Step 5: `testkit.rs`**

```rust
use crate::{IDENTITY_BIND_NS, KeyScheme, MemberAuth};
// ... keep the existing builder shape, with:
        scheme: KeyScheme::Ed25519,
        proof: keyscheme::testkit::ed25519_proof(user, ns, preimage),
```
(`keyscheme::testkit` is behind the `testkit` feature — add `keyscheme/testkit` to identity's `testkit` feature: `testkit = ["keyscheme/testkit"]`.)

- [ ] **Step 6: `src/tests.rs`**

- Lines 60-145: delete `P256Native`, `p256`, `p256_pub`, `p256_auth`; delete `wa_key`/`wa_pub`/`wa_proof` bodies and re-express them over the testkit:
  ```rust
  fn ed_proof(k: &Ed, ns: &[u8], preimage: &[u8]) -> Vec<u8> {
      keyscheme::testkit::ed25519_proof(k, ns, preimage)
  }
  fn ed_auth(k: &Ed, ns: &[u8], preimage: &[u8]) -> MemberAuth {
      MemberAuth { key: ed_pub(k), scheme: KeyScheme::Ed25519, proof: ed_proof(k, ns, preimage) }
  }
  fn wa_key(seed: u8) -> p256::ecdsa::SigningKey { keyscheme::testkit::passkey(seed) }
  fn wa_pub(k: &p256::ecdsa::SigningKey) -> Vec<u8> { keyscheme::testkit::passkey_pubkey(k) }
  fn wa_proof(k: &p256::ecdsa::SigningKey, rp_id: &str, ns: &[u8], preimage: &[u8]) -> Vec<u8> {
      keyscheme::testkit::passkey_proof(k, rp_id, ns, preimage, true)
  }
  fn wa_auth(k: &p256::ecdsa::SigningKey, rp_id: &str, ns: &[u8], preimage: &[u8]) -> MemberAuth {
      MemberAuth { key: wa_pub(k), scheme: KeyScheme::Secp256r1, proof: wa_proof(k, rp_id, ns, preimage) }
  }
  ```
  (`p256` stays a dev-dependency of identity for the `SigningKey` type in these helpers — add `p256 = { workspace = true }` under `[dev-dependencies]`.)
- Every `KeyKind::Ed25519` → `KeyScheme::Ed25519`; `KeyKind::WebauthnP256` → `KeyScheme::Secp256r1`; every `.kind` on a view/auth → `.scheme`; `new_kind:` → `new_scheme:`.
- Delete the test `a_native_p256_key_can_found_and_authorize` (line ~415-435) — there is no raw P-256 scheme.
- The "claimed kind mismatch" test (lines ~480-487): `auth.kind = KeyKind::P256;` → `auth.scheme = KeyScheme::Secp256k1;` and the comment "claim the founder is a P256 member" → "claim the founder is a wallet".
- Any test asserting the RP-id pin (grep `rp_id` beyond the helper) is deleted — there are none in this file today.

- [ ] **Step 7: `tests/sync_round_trip.rs`**

Lines 22 (imports: drop `MemberProof`, `KeyKind` → `KeyScheme`), 45-46 (`scheme: KeyScheme::Ed25519, proof: keyscheme::testkit::ed25519_proof(...)`), 60-80 (`wa_proof` → `keyscheme::testkit::passkey_proof(k, rp_id, ns, preimage, true)` returning `Vec<u8>`), 191-198 (`KeyKind::WebauthnP256` → `KeyScheme::Secp256r1`, `new_kind` → `new_scheme`).

- [ ] **Step 8: Build and test identity**

Run: `cargo test -p identity --features testkit`
Expected: every test passes; the compiler names any site missed above — fix it, no wildcard matches.

Run: `cargo clippy -p identity --tests --no-deps` — no warnings.

- [ ] **Step 9: Commit**

```bash
rustfmt crates/modules/system/identity/src/*.rs crates/modules/system/identity/tests/*.rs
git add crates/modules/system/identity
git commit -m "feat(identity): member keys carry a KeyScheme and proof bytes; scheme.rs moves to keyscheme

Drops the raw P-256 kind and the RP-id pin (a passkey is RP-scoped by
construction).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM"
```

---

### Task 7: Consumer sweep — everything that named `KeyKind` / `MemberProof`

**Files:**
- Modify: `crates/modules/system/gateway/src/module.rs:59-60, 470-495, 573-600`; `crates/modules/system/gateway/tests/module.rs:14, 71`; `crates/modules/system/gateway/tests/sync_round_trip.rs:27, 57`; `bin/node/src/gateway_plane.rs:570-600, 1990`; `bin/node/src/userkey_cli.rs:37-52, 160-200, 273-280, 853-861, 880-930, 983-1045, 1046-1125, 1384, 1447-1455`; `bin/node/Cargo.toml`; `crates/workspace-config/src/identity.rs:75-105`; `crates/modules/system/acl/tests/dispatch_gate.rs:24, 262-263`; `crates/modules/system/governance/tests/governance_shares.rs:17, 49, 75`; `crates/kernel/host/tests/wasm_gateway_parity.rs:40, 152-153`; `crates/kernel/host/tests/wasm_governance_parity.rs:39, 235-236`; `crates/kernel/host/tests/wasm_identity_parity.rs:32, 129-180, 453-490, 663, 697-717`; `crates/noded/tests/router.rs:768`; `bin/simnode/tests/module_gaps.rs:17, 78`; `crates/labs/src/multisig/key.rs:16`

**Interfaces:**
- Consumes: identity's phase-1 wire from Task 6; `keyscheme::testkit`.

- [ ] **Step 1: gateway module**

`src/module.rs:59-60` imports: drop `KeyKind, MemberProof, verify_authority`; add `KeyScheme`. At both authorization sites (route publication ~470-495, credential ~573-600):

```rust
        let signer_is_current = account
            .member_keys
            .iter()
            .any(|member| member.pubkey == authorization.signer && member.scheme == KeyScheme::Ed25519);
        if !signer_is_current {
            return Err(Error::Module(
                "gateway: signer is not a current Ed25519 account member".into(),
            ));
        }
        let preimage = route_signing_preimage(&statement).map_err(Error::Module)?;
        if !KeyScheme::Ed25519.verify(
            &authorization.signer,
            GATEWAY_ROUTE_NS,
            &preimage,
            &authorization.signature,
        ) {
```
(keep the surrounding error text; the credential site uses its own namespace constant and preimage exactly as today — only the verifier call changes: `verify_authority(KeyKind::Ed25519, &authorization.signer, None, NS, &preimage, &proof)` → `KeyScheme::Ed25519.verify(&authorization.signer, NS, &preimage, &authorization.signature)`, and the `let proof = MemberProof::Signature { ... }` lines are deleted.)

Tests `tests/module.rs:71`, `tests/sync_round_trip.rs:57`: `kind: KeyKind::Ed25519` → `scheme: KeyScheme::Ed25519` (and the import lists).

Run: `cargo test -p gateway` — expected green.

- [ ] **Step 2: `bin/node` — gateway_plane + userkey_cli**

`bin/node/Cargo.toml`: add `keyscheme = { workspace = true }` to `[dependencies]`.

`gateway_plane.rs:570-600` (`revalidate_route_authority`):
```rust
    let signer_is_current = account.member_keys.iter().any(|member| {
        member.scheme == identity::KeyScheme::Ed25519 && member.pubkey == authorization.signer
    });
    let node_is_current = account
        .nodes
        .iter()
        .any(|node| node.node_key == statement.publisher_node);
    let preimage =
        gateway::route_signing_preimage(statement).map_err(GatewayFailure::Unavailable)?;
    let authority_verifies = identity::KeyScheme::Ed25519.verify(
        &authorization.signer,
        gateway::GATEWAY_ROUTE_NS,
        &preimage,
        &authorization.signature,
    );
    if account.account_id != statement.account_id
        || !signer_is_current
        || !node_is_current
        || !authority_verifies
    {
```
Line 1990 test fixture: `kind: identity::KeyKind::Ed25519` → `scheme: identity::KeyScheme::Ed25519`.

`userkey_cli.rs`:
- Delete the `P256Payload` verb: the enum variant (line 52), the dispatch arm (line 280), `user_p256_payload` + `cmd_user_p256_payload` (1018-1045), and its test (`p256_payload_*`, ~1100-1125).
- `parse_kind` (853-861) → 
  ```rust
  /// parse a `--new-scheme` flag value into a [`identity::KeyScheme`].
  fn parse_scheme(s: &str) -> Result<identity::KeyScheme, Box<dyn std::error::Error>> {
      match s {
          "ed25519" => Ok(identity::KeyScheme::Ed25519),
          "secp256k1" | "wallet" => Ok(identity::KeyScheme::Secp256k1),
          "secp256r1" | "passkey" => Ok(identity::KeyScheme::Secp256r1),
          other => Err(format!("unknown key scheme {other:?} (ed25519 | secp256k1 | secp256r1)").into()),
      }
  }
  ```
- `AddMemberArgs` (160-185): `new_kind` field/flag → `new_scheme` / `--new-scheme`, doc "ed25519 | secp256k1 | secp256r1"; `possession` doc → "the new key's possession proof (hex of the scheme's proof bytes)". In `cmd_user_sign_add_member` (~905-930): `let possession = config::unhex(&args.possession)?;` replaces the `serde_json::from_str::<MemberProof>` parse; `new_kind` → `new_scheme` throughout; the message field `new_kind:` → `new_scheme:`.
- `user_webauthn_challenge` (984-1010): `identity::KeyKind::WebauthnP256` → `identity::KeyScheme::Secp256r1`; `identity::webauthn_challenge(...)` → `keyscheme::webauthn_challenge(...)`; its test (~1046-1090) likewise.
- Line 1384: `authorizer.kind` → `authorizer.scheme`; lines 1447-1455: `identity::verify_authority(identity::KeyKind::Ed25519, key, None, NS, &preimage, &identity::MemberProof::Signature { sig })` → `identity::KeyScheme::Ed25519.verify(key, NS, &preimage, &sig)`.

Run: `cargo check -p node-bin --all-targets` (`node-bin` is the `bin/node` package; its binary is `ducktape`) — expected: green after the compiler walks you to any leftover site.

- [ ] **Step 3: workspace-config**

`crates/workspace-config/src/identity.rs:75-105`:
```rust
pub fn ed25519_member_auth(
    user: &ed25519::PrivateKey,
    namespace: &[u8],
    preimage: &[u8],
) -> identity::MemberAuth {
    identity::MemberAuth {
        key: user.public_key().as_ref().to_vec(),
        scheme: identity::KeyScheme::Ed25519,
        proof: user.sign(namespace, preimage).as_ref().to_vec(),
    }
}

/// the possession proof an ed25519 key produces over `preimage` -- what a NEW
/// device signs to prove it holds the key it is asking to enroll.
pub fn ed25519_possession(user: &ed25519::PrivateKey, namespace: &[u8], preimage: &[u8]) -> Vec<u8> {
    user.sign(namespace, preimage).as_ref().to_vec()
}
```

- [ ] **Step 4: the test-only sites**

Mechanical, one file at a time (the compiler is the checklist):
- `acl/tests/dispatch_gate.rs:24` import `KeyKind, MemberAuth, MemberProof` → `KeyScheme, MemberAuth`; `:262-263` → `scheme: KeyScheme::Ed25519, proof: <the sig bytes vec>` (the existing `MemberProof::Signature { sig: X }` becomes `proof: X`).
- `governance/tests/governance_shares.rs:17, 49, 75` — `KeyKind` → `KeyScheme`, `kind:` → `scheme:`.
- `host/tests/wasm_gateway_parity.rs:40, 152-153` and `wasm_governance_parity.rs:39, 235-236` — same two edits as dispatch_gate.
- `host/tests/wasm_identity_parity.rs`: `:32` imports; `:129-180` helpers → the same testkit-backed shape as Task 6 Step 6 (`ed_proof`/`wa_proof` return `Vec<u8>`, `wa_auth` uses `KeyScheme::Secp256r1`); `:453-490` `KeyKind::*` → `KeyScheme::*`, `new_kind` → `new_scheme`; `:663` `forged_kind.kind = KeyKind::P256` → `forged.scheme = KeyScheme::Secp256k1` (rename the local too); `:697-717` `proof: MemberProof::Signature { sig: vec![0; 64] }` → `proof: vec![0; 64]`, `possession: MemberProof::Signature { sig: vec![0; 64] }` → `possession: vec![0; 64]`. Add `keyscheme = { workspace = true, features = ["testkit"] }` and keep `p256` in host's `[dev-dependencies]`.
- `noded/tests/router.rs:768`, `simnode/tests/module_gaps.rs:17, 78` — `KeyKind` → `KeyScheme`, `kind:` → `scheme:`.
- `labs/src/multisig/key.rs:16` comment: "the identity module's deliberately CLOSED `KeyKind` enum and warns that adding `Secp256k1` would be a protocol change" → "`keyscheme::KeyScheme` now carries `Secp256k1` for account association; this labs key is still NOT a member key — it is verified only by Ethereum `ecrecover` in the Safe flow".

- [ ] **Step 5: Whole-workspace check**

Run: `cargo check --workspace --all-targets`
Expected: green. Any remaining `KeyKind`/`MemberProof`/`verify_authority`/`webauthn_rp_id_hash`/`add_member_signing_payload` reference is a site this plan missed — fix it the same way; then

Run: `grep -rn "KeyKind\|MemberProof\|verify_authority\|rp_id_hash\|add_member_signing_payload\|WebauthnP256" --include=*.rs --include=*.md bin crates app docs/superpowers/specs/2026-08-27-identity-rework-design.md`
Expected: only the spec's "what exists today" table rows. (Older specs under `docs/superpowers/specs/2026-07-*` are superseded and are not edited.)

- [ ] **Step 6: Test the touched crates natively** (parity tests still use the OLD wasm bytes and will fail until Task 8 — skip them here)

```bash
cargo test -p gateway && cargo test -p workspace-config && cargo test -p acl && cargo test -p governance
cargo test -p host --test cross_module
cargo test -p node-bin userkey_cli
```
Expected: green.

- [ ] **Step 7: Lint, format, commit**

```bash
for c in gateway workspace-config acl governance; do cargo clippy -p $c --tests --no-deps || exit 1; done
cargo clippy -p node-bin --tests --no-deps
git add -A crates bin
git commit -m "refactor: every KeyKind/MemberProof consumer speaks KeyScheme + proof bytes

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM"
```

---

### Task 8: Regenerate the wasm guests, run the parity proofs, pin the artifacts

**Files:**
- Regenerated: every `component.wasm` under `crates/modules/**` and `crates/examples/directory`, every `crates/kernel/host/tests/fixtures/*.component.wasm`, every `index.wasm` in `INDEX_MODULES` (the Makefile sweep refreshes the whole set — bytes are toolchain-dependent and must be committed together)

- [ ] **Step 1: Build the guests**

Run: `make wasm-modules`
Expected: exit 0. A failure naming a native-only dependency in `keyscheme` (e.g. `getrandom` from `k256`) means a wasm32 stub is missing — `k256`'s `ecdsa` feature pulls `signature`/`rfc6979` which are pure; if `getrandom` appears, guest-builder's existing `getrandom-0[2-4]` stubs (`crates/guests/stubs`) cover it. Do NOT add a `std`/`os_rng` feature to k256.

- [ ] **Step 2: Run every parity proof**

Run: `cargo test -p host --test wasm_identity_parity --test wasm_gateway_parity --test wasm_governance_parity`
Expected: green — the guest and native identity commit IDENTICAL roots with the new `MemberMeta { scheme, label, added_at }` borsh shape, and the passkey (h7) block verifies through `keyscheme` inside the guest.

- [ ] **Step 3: Mutual-consistency gate**

Run: `make wasm-modules-check`
Expected: exit 0 (module dir copies == fixture copies).

- [ ] **Step 4: Commit the artifacts**

```bash
git add -A crates/modules crates/examples crates/kernel/host/tests/fixtures crates/kernel/index-guest
git commit -m "build(wasm): regenerate guests for the KeyScheme wire

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM"
```

---

### Task 9: Full gates and the PR

- [ ] **Step 1: Workspace gates**

```bash
cargo check --workspace --all-targets; echo exit=$?
cargo test -p keyscheme -p node -p identity --features identity/testkit; echo exit=$?
cargo test -p host; echo exit=$?
cargo check -p files --no-default-features; echo exit=$?
make wasm-modules-check; echo exit=$?
```
Every `exit=0`. Then the app's own lane, which signs frames with `node::encode_frame`: `cargo test -p ducktape-app` — expected green (no app source changed; this proves the one-byte frame growth is invisible to it).

- [ ] **Step 2: Push and open the PR**

```bash
git push -u origin feat/keyscheme-frame
gh pr create --base dev --title "identity rework phase 1: keyscheme crate + frame scheme byte" --body "$(cat <<'EOF'
## Summary
- new `crates/kernel/keyscheme`: closed `KeyScheme { Ed25519, Secp256k1, Secp256r1 }`, one `verify` dispatch, scheme-owned proof bytes (commonware ed25519 / EIP-191 `personal_sign` recovery / WebAuthn assertion envelope), `testkit` signing helpers
- op frames carry a leading scheme byte; `frame_preimage` and `FRAME_NS` are public so wallets and passkeys sign the exact bytes; `Origin::External` stays raw pubkey bytes
- identity adopts `KeyScheme`: `MemberAuth { key, scheme, proof: Vec<u8> }`, `AddMemberKey { new_scheme, possession: Vec<u8>, .. }`, `MemberKeyView.scheme`; `src/scheme.rs`, `KeyKind::P256`, `MemberProof`, the RP-id pin and `add_member_signing_payload` are deleted
- consumer sweep + regenerated wasm guests

Spec: `docs/superpowers/specs/2026-08-27-identity-rework-design.md` (phase 1 of 6).

## Test plan
- [ ] `cargo test -p keyscheme` (EIP-191 known vector, both v conventions, passkey UP/type/tamper)
- [ ] `cargo test -p node` incl. `tests/frame_schemes.rs`
- [ ] `cargo test -p identity --features testkit`
- [ ] `cargo test -p host` (wasm parity: identity/gateway/governance)
- [ ] `make wasm-modules-check`
- [ ] `cargo test -p ducktape-app`

🤖 Generated with [Claude Code](https://claude.com/claude-code)

https://claude.ai/code/session_013CnF7sWAUCBsYDCaU2dPGM
EOF
)"
```

- [ ] **Step 3: Report**

State in the PR comment / final report: which gates ran and their exit codes; that phases 2–6 follow in separate PRs; and that the worktree `.worktree/feat-keyscheme-frame` is to be removed by `ops/worktree-clean.sh --yes` once merged.
