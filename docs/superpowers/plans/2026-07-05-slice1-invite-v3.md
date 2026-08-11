# Slice 1 — v3 signed invite + typed `Reach` (config wire encoding)

Status: implementation plan of record for **Slice 1** of the
`epic/p3-private-cutover` epic. Worktree
`<repo>/.claude/worktrees/epic+p3-private-cutover`, branch
`slice/1-invite-v3`.

Design of record:
`docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md`,
Component 2 ("Typed reach hint + v3 signed invite").

## Scope

Slice 1 lands the **invite wire format** and the **typed reachability model** in
`bin/node/src/config.rs`, plus the one CLI call-site that mints an invite
(`bin/node/src/main.rs::cmd_invite`, which already loads the inviter key). It
does **not** stand up the coordinator, the STUN client, hole-punch, or the relay
effect — those are Slices 0/2/3. Slice 1 makes the descriptor able to *represent*
and *resolve* a `Coordinated` reach (dial the coordinator, expect the target's
key) so the later slices have a wire to carry.

Everything lands in `config.rs` except a ~6-line change to `cmd_invite` and its
signature-changed `encode_invite` caller — an unavoidable, minimal cross-file
edit because the inviter must now *sign* the invite (`cmd_invite` already holds
`config::load_identity(...)`). No consensus, no `NetworkDescriptor` fingerprint
change, no app/Tauri change (the desktop wrapper passes blobs through verbatim
and `decode_invite` keeps accepting v2 pastes).

## The four requirements, mapped

1. **Typed `Reach { Direct | Fronted | Coordinated(CoordRef{coord_addr, coord_key}) }`
   and `ReachHint { expected_key, reach }`** — new public types in `config.rs`
   (Task 1). `CoordRef.coord_key` and `ReachHint.expected_key` are
   `ed25519::PublicKey`.
2. **v3 packed invite** — version byte `3`, per-hint reach `tag+payload`, trailing
   `expires` + embedded `inviter_key` + `inviter_sig` over the
   payload-minus-signature with domain separation. `decode` verifies signature,
   inviter-is-a-genesis-validator, and expiry; **fails closed**. Reuses the v2
   `InviteReader` bounds/trailing-byte rigor (Tasks 4–5).
3. **v2 stays PARSE-ONLY** — a v2 blob decodes to all-`Direct`, unsigned hints; v3
   and v2 are never confusable (distinct prefix **and** version byte, and the two
   must *agree* on decode). No prod v2 encoder survives (Task 6).
4. **Reach stays ADVISORY / out of the genesis fingerprint; additive descriptor
   fields; `bootstrap: Vec<String>` keeps working; `Coordinated` changes the dial
   target** — new `#[serde(default)] reach: Vec<String>` field, typed accessor
   `reach_hints()` that *synthesizes* all-`Direct` hints from `bootstrap` when
   `reach` is empty, and `reach_entries()` that resolves a hint to
   `(expected_key, dial_addr)` — `Coordinated` dials `coord_addr` while still
   expecting `expected_key` end-to-end (Tasks 2, 3, 7).

## Key design decisions (and tradeoffs)

- **Descriptor stores `reach` as canonical `Vec<String>`, parsed to typed
  `ReachHint` at the boundary.** This mirrors the existing codebase exactly:
  `validators` and `bootstrap` are `Vec<String>` (hex / `hexkey@host:port`) parsed
  by `validator_keys()` / `bootstrap_entries()`. TOML then serialises trivially
  (an array of strings) and diffs stay stable, avoiding serde's ugly
  externally-tagged-enum-in-TOML representation. The typed `Reach` / `ReachHint`
  live as the in-memory + wire form. Tradeoff: one parse/format pair
  (`ReachHint::parse` / `to_canonical`); accepted because it is the house style
  and keeps serde boring.
  - Canonical forms: `direct:<hex>@<host:port>`, `fronted:<hex>@<host:port>`,
    `coordinated:<hex>@<coord_host:port>#<coord_hex>`. `@` and `#` never occur in a
    `host:port` (IPv6 uses `[..]:port`), so the split is unambiguous.

- **`reach_hints()` synthesises from `bootstrap` when `reach` is empty.** One
  dial source of truth. A v2/legacy descriptor (only `bootstrap`) therefore yields
  all-`Direct` hints (requirement 3) *without* duplicating data into `reach`, so
  there is never a double-dial. `resolve_network_shape` switches its one
  `bootstrap_entries()` call to `reach_entries()` (which falls through to the same
  synthesis), so existing v2 behaviour is byte-for-byte preserved and its tests
  stay green.

- **Self-verifying invite: embed `inviter_key`, sign the whole envelope, then
  require `inviter_key ∈ validators`.** The blob carries the inviter's public key;
  the signature covers everything before it (including it). On decode we (a) verify
  the signature against the embedded key — so no box on the path (coordinator
  included) can swap an `expected_key`, a `coord_key`, or the expiry without
  invalidating it; and (b) require the inviter to be one of the genesis
  `validators` — binding invite authenticity to consensus-recognised identity
  rather than to OOB delivery alone. Tradeoff: an invite can only be minted by a
  genesis validator (the founder pre-genesis, any member post-genesis) — which is
  exactly the membership model (valset == membership), so it is never a real
  restriction, and it is a strong fail-closed check. Documented as revisitable
  policy.

- **Domain separation** via commonware's namespaced signing:
  `INVITE_SIG_NS = b"ducktape:invite:v3:"`, matching the
  `crates/system/wireguard-upgrade` convention (`ENDPOINT_NS`,
  `UPGRADE_REQUEST_NS`, …). `signer.sign(INVITE_SIG_NS, &payload_wo_sig)` /
  `inviter_key.verify(INVITE_SIG_NS, &payload_wo_sig, &sig)`.

- **Non-confusability at two layers.** v3 uses prefix `ducktape-invite-v3:` and
  version byte `3`; v2 uses `ducktape-invite-v2:` and byte `2`. `decode_invite`
  selects the codec by prefix, then reads the version byte and **requires
  agreement** (a v2 payload under a v3 prefix, or vice-versa, is rejected). The
  version byte is inside the signed region; the prefix is transport framing.

- **Clock-injected decode core** `decode_invite_at(blob, now_unix)` so expiry is
  deterministically testable; `decode_invite(blob)` reads the real clock and
  delegates. Mirrors `MeshView::verify(.., current_view)` in `wireguard-upgrade`.

- **Reach is excluded from `genesis_namespace()`.** Confirmed: `genesis_namespace`
  (config.rs:210-233) hashes `b"ducktape:genesis:v1:" ‖ scheme ‖ sorted
  validators` only — `bootstrap` is already excluded, and the new `reach` field is
  never referenced there. Task 3 locks this with a guard test.

## Exact v3 byte layout

```
offset  bytes  field
------  -----  -----------------------------------------------------------------
 0       1     version = 3
 1       1     chain_id length  L_cid                         (u8)
 2       L_cid chain_id (ascii, e.g. "ducktape#a1b2c3d4")
 ..      1     validator count  V                             (u8)
 ..      32*V  raw ed25519 validator pubkeys (NOT hex)
 ..      1     reach-hint count H                             (u8)
              repeated H times:
                32   expected_key : raw ed25519 pubkey
                 1   reach tag: 0=Direct, 1=Fronted, 2=Coordinated
                 -- Direct / Fronted --
                 1   addr length A                            (u8)
                 A   addr bytes (host:port; hostname allowed, resolved at dial)
                 -- Coordinated --
                 1   coord_addr length A                      (u8)
                 A   coord_addr bytes (host:port)
                32   coord_key : raw ed25519 pubkey
 ..      8     expires_unix_secs                              (u64, little-endian)
 ..     32     inviter_key : raw ed25519 pubkey
--- end of signed region (signed_len == bytes.len() - 64) ---
 ..     64     inviter_sig : ed25519 signature over bytes[0..signed_len],
              domain-separated by INVITE_SIG_NS
```

Signed message = `bytes[0..signed_len]` where `signed_len` is the offset right
after `inviter_key`. In `pack_invite_v3` we build that prefix into `out`, sign it,
then append the 64 signature bytes — so `bytes[..len-64]` on decode is identical to
the signed bytes. Every field is length-delimited, so the reader lands exactly at
`signed_len`, takes 64 sig bytes, and asserts `done()` (no trailing bytes).

## Files touched

- `bin/node/src/config.rs` — all types, wire format, resolution, helpers, tests.
- `bin/node/src/main.rs` — two small touches: `cmd_init`'s `NetworkDescriptor {}`
  literal gains `reach: Vec::new()` (Task 2, a new-field requirement), and
  `cmd_invite` signs the v3 invite with `--ttl-days` (Task 8). No other `main.rs`
  change.

## Gate command

Per-task and final (node-bin is a large crate; slow compiles are expected and
acceptable):

```
cargo test -p node-bin --bin ducktape-node config:: && cargo clippy -p node-bin --all-targets -- -D warnings
```

`--bin ducktape-node config::` runs only the `config::tests` unit tests in the bin
target (it does **not** build the heavy `tests/` process-e2e suite). Clippy
`--all-targets -D warnings` is the thorough lint (it does compile `tests/`, the
slow path — accepted). Run this at the end of every task before committing.

---

## Task 1 — Typed `Reach` / `CoordRef` / `ReachHint` + canonical parse/format

**Goal.** Introduce the typed reachability model and its canonical string codec.
No wire, no descriptor field yet.

**Preflight (imports).** At the top of `config.rs`, extend the crypto import so
verification is in scope later:

```rust
use commonware_cryptography::{Signer as _, Verifier as _, ed25519};
```

`commonware_codec::{DecodeExt as _, Encode as _}` is already imported (used for
`PublicKey::decode` / `Signature::decode`).

**Red — write these tests first** (append to `mod tests` in `config.rs`):

```rust
#[test]
fn reach_hint_canonical_roundtrips_every_kind() {
    let ek = ed25519::PrivateKey::from_seed(11).public_key();
    let ck = ed25519::PrivateKey::from_seed(12).public_key();
    let cases = [
        ReachHint { expected_key: ek.clone(), reach: Reach::Direct("127.0.0.1:9000".into()) },
        ReachHint { expected_key: ek.clone(), reach: Reach::Fronted("front.example.com:443".into()) },
        ReachHint {
            expected_key: ek.clone(),
            reach: Reach::Coordinated(CoordRef {
                coord_addr: "p2p.ducktape.industries:7777".into(),
                coord_key: ck.clone(),
            }),
        },
    ];
    for h in cases {
        let s = h.to_canonical();
        assert_eq!(ReachHint::parse(&s).expect("parse"), h, "roundtrip {s}");
    }
}

#[test]
fn reach_hint_parse_rejects_malformed() {
    assert!(ReachHint::parse("nope").is_err(), "no tag");
    assert!(ReachHint::parse("direct:deadbeef").is_err(), "no @addr");
    assert!(ReachHint::parse("bogus:00@host:1").is_err(), "unknown tag");
    assert!(ReachHint::parse("direct:zz@host:1").is_err(), "bad hex key");
    // coordinated without the #coord_key delimiter:
    assert!(ReachHint::parse("coordinated:00@host:1").is_err(), "missing #coord_key");
}
```

**Green — implement** (place near the invite section, before `encode_invite`):

```rust
/// how to reach a member's REAL node. advisory (never part of the genesis
/// fingerprint); the mesh still authenticates the peer by its ed25519 key
/// end-to-end regardless of which socket got dialed.
#[derive(Clone, Debug, PartialEq)]
pub enum Reach {
    /// dial this `host:port` directly (today's bootstrap behaviour).
    Direct(String),
    /// dial a transport forwarder that splices to the target.
    Fronted(String),
    /// dial a coordinator (`coord_addr`) and ask it for a path to the target.
    Coordinated(CoordRef),
}

/// how to reach a coordinator, plus the key it authenticates its channel with.
#[derive(Clone, Debug, PartialEq)]
pub struct CoordRef {
    pub coord_addr: String,
    pub coord_key: ed25519::PublicKey,
}

/// a signed-invite reach hint: the REAL node identity a joiner must end up
/// authenticating, plus how to get a path to it.
#[derive(Clone, Debug, PartialEq)]
pub struct ReachHint {
    pub expected_key: ed25519::PublicKey,
    pub reach: Reach,
}

impl ReachHint {
    /// canonical single-line form stored in `network.toml`'s `reach` array and
    /// parsed by [`ReachHint::parse`]. `@` separates the expected key from the
    /// address, `#` separates a coordinator address from its key; neither char
    /// occurs in a host:port, so the split is unambiguous.
    pub fn to_canonical(&self) -> String {
        let ek = hex_bytes(self.expected_key.as_ref());
        match &self.reach {
            Reach::Direct(a) => format!("direct:{ek}@{a}"),
            Reach::Fronted(a) => format!("fronted:{ek}@{a}"),
            Reach::Coordinated(c) => {
                format!("coordinated:{ek}@{}#{}", c.coord_addr, hex_bytes(c.coord_key.as_ref()))
            }
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let (tag, rest) = s
            .split_once(':')
            .ok_or_else(|| format!("reach hint {s:?} missing a tag"))?;
        let (ek_hex, addr_part) = rest
            .split_once('@')
            .ok_or_else(|| format!("reach hint {s:?} is not tag:key@addr"))?;
        let expected_key = decode_key(ek_hex)?;
        let reach = match tag {
            "direct" => Reach::Direct(addr_part.to_string()),
            "fronted" => Reach::Fronted(addr_part.to_string()),
            "coordinated" => {
                let (coord_addr, ck_hex) = addr_part
                    .rsplit_once('#')
                    .ok_or_else(|| format!("coordinated hint {s:?} missing #coord_key"))?;
                Reach::Coordinated(CoordRef {
                    coord_addr: coord_addr.to_string(),
                    coord_key: decode_key(ck_hex)?,
                })
            }
            other => return Err(format!("unknown reach tag {other:?} in {s:?}")),
        };
        Ok(Self { expected_key, reach })
    }
}
```

**Gate + commit.**

```
cargo test -p node-bin --bin ducktape-node config:: && cargo clippy -p node-bin --all-targets -- -D warnings
git add -A && git commit -m "feat(invite-v3): typed Reach/CoordRef/ReachHint + canonical string codec"
```

---

## Task 2 — Additive `NetworkDescriptor.reach` field + `reach_hints()` + `add_reach`

**Goal.** Carry reach hints on the descriptor additively, keep `bootstrap`
working, and expose the typed view that synthesises `Direct` hints from
`bootstrap` when `reach` is empty.

**Red:**

```rust
#[test]
fn reach_field_defaults_empty_and_toml_roundtrips() {
    let a = ed25519::PrivateKey::from_seed(21).public_key();
    let mut d = NetworkDescriptor {
        chain_id: "r#00000000".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(a.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    // an existing network.toml without a [reach] array still parses (serde default),
    // and an empty reach is not serialised (skip_serializing_if).
    assert!(!d.to_toml().contains("reach"));
    d.add_reach(&ReachHint { expected_key: a.clone(), reach: Reach::Direct("10.0.0.1:9000".into()) });
    let back = NetworkDescriptor::from_toml(&d.to_toml()).expect("roundtrip");
    assert_eq!(back.reach, d.reach);
}

#[test]
fn reach_hints_synthesizes_direct_from_bootstrap_when_reach_empty() {
    let a = ed25519::PrivateKey::from_seed(22).public_key();
    let mut d = NetworkDescriptor {
        chain_id: "r#00000000".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(a.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    d.add_bootstrap(&a, "127.0.0.1:52200");
    let hints = d.reach_hints().expect("hints");
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0], ReachHint { expected_key: a, reach: Reach::Direct("127.0.0.1:52200".into()) });
}

#[test]
fn add_reach_dedups_by_expected_key_and_sorts() {
    let a = ed25519::PrivateKey::from_seed(23).public_key();
    let coord = ed25519::PrivateKey::from_seed(24).public_key();
    let mut d = NetworkDescriptor {
        chain_id: "r#00000000".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(a.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    d.add_reach(&ReachHint { expected_key: a.clone(), reach: Reach::Direct("1.1.1.1:1".into()) });
    // same expected_key, different reach — replaces, never duplicates.
    d.add_reach(&ReachHint {
        expected_key: a.clone(),
        reach: Reach::Coordinated(CoordRef { coord_addr: "c:2".into(), coord_key: coord }),
    });
    assert_eq!(d.reach.len(), 1);
    assert!(matches!(d.reach_hints().unwrap()[0].reach, Reach::Coordinated(_)));
    let mut sorted = d.reach.clone();
    sorted.sort();
    assert_eq!(d.reach, sorted);
}
```

**Green.** Add the field to the struct (after `bootstrap`):

```rust
    /// typed reach hints (v3), canonical strings like `direct:<hex>@host:port`.
    /// advisory and EXCLUDED from the genesis fingerprint, exactly like
    /// `bootstrap`. empty for v2/legacy descriptors — then [`reach_hints`]
    /// synthesises all-`Direct` hints from `bootstrap`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reach: Vec<String>,
```

Add methods in `impl NetworkDescriptor`:

```rust
    /// the reach hints, typed. if the descriptor carries explicit v3 `reach`
    /// entries they parse to those; otherwise every `bootstrap` entry is a
    /// `Direct` hint (so a v2/legacy descriptor yields all-`Direct` hints with
    /// no data duplicated and no double-dial).
    pub fn reach_hints(&self) -> Result<Vec<ReachHint>, String> {
        if !self.reach.is_empty() {
            return self.reach.iter().map(|s| ReachHint::parse(s)).collect();
        }
        self.bootstrap
            .iter()
            .map(|entry| {
                let (k, addr) = entry
                    .split_once('@')
                    .ok_or_else(|| format!("bootstrap entry {entry:?} is not pubkey@addr"))?;
                Ok(ReachHint { expected_key: decode_key(k)?, reach: Reach::Direct(addr.to_string()) })
            })
            .collect()
    }

    /// record a reach hint for a member, replacing any previous hint for the
    /// same expected key (a member's reach can move/upgrade). keeps the list
    /// sorted for stable file diffs — mirrors [`add_bootstrap`].
    pub fn add_reach(&mut self, hint: &ReachHint) {
        let ek = hex_bytes(hint.expected_key.as_ref());
        self.reach.retain(|s| {
            ReachHint::parse(s)
                .map(|h| hex_bytes(h.expected_key.as_ref()) != ek)
                .unwrap_or(true)
        });
        self.reach.push(hint.to_canonical());
        self.reach.sort();
    }
```

**Note — add `reach: Vec::new()` to every struct literal.** Adding a field
without a `#[derive(Default)]`-built default breaks every `NetworkDescriptor { .. }`
literal until it names `reach`. There are exactly these sites (grep
`NetworkDescriptor {` to confirm before committing — all outside tests are in this
list):
- **Production, `bin/node/src/main.rs:1765`** (`cmd_init`) — add `reach: Vec::new(),`
  after `bootstrap: Vec::new(),`. This is a second, tiny `main.rs` touch beyond
  Task 8; it is unavoidable (a new struct field). `cmd_init` seeds the founder as
  the sole validator, which is exactly why the inviter-∈-validators check holds
  pre-genesis.
- **`config.rs:466`** (`unpack_invite_v2`'s returned descriptor) — handled by
  Task 6's rewrite, but if you reach it here add `reach: Vec::new(),`.
- **`config.rs` tests** (lines ~861, 889, 910, 946, 978, 994, 1061, 1077, 1107,
  1123, 1130, 1158, 1176): `invite_blob_roundtrips_the_descriptor`,
  `admit_is_idempotent_and_sorted`,
  `network_shape_resolves_membership_and_bootstrap`,
  `a_non_member_identity_resolves_as_a_pending_joiner`,
  `duplicate_validators_are_a_config_error`, the `guard_join` pair, and the
  canonicalisation tests. Add `reach: vec![],` to each.

There are no other `NetworkDescriptor { .. }` literals in `crates/` or `app/`.

**Gate + commit.**

```
git commit -am "feat(invite-v3): additive NetworkDescriptor.reach + typed reach_hints/add_reach"
```

---

## Task 3 — Guard: reach hints are excluded from the genesis fingerprint

**Goal.** Lock requirement 4's invariant (consensus identity is unchanged by
reach) with an explicit test. This is a characterisation/guard test — it must be
**green on first run**; if it is red, `genesis_namespace` is wrongly consuming
`reach`/`bootstrap` and that is the bug to fix.

**Red/guard:**

```rust
#[test]
fn reach_hints_are_excluded_from_the_genesis_fingerprint() {
    let v = ed25519::PrivateKey::from_seed(31).public_key();
    let coord = ed25519::PrivateKey::from_seed(32).public_key();
    let base = NetworkDescriptor {
        chain_id: "fp#00000000".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(v.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    let ns0 = base.genesis_namespace();

    let mut with_reach = base.clone();
    with_reach.add_bootstrap(&v, "127.0.0.1:52200");
    with_reach.add_reach(&ReachHint {
        expected_key: v.clone(),
        reach: Reach::Coordinated(CoordRef { coord_addr: "p2p:7777".into(), coord_key: coord }),
    });

    // advisory reach + bootstrap NEVER move the consensus identity.
    assert_eq!(with_reach.genesis_namespace(), ns0);
    // two descriptors differing ONLY in reach fingerprint identically.
    let mut other_reach = base.clone();
    other_reach.add_reach(&ReachHint { expected_key: v, reach: Reach::Direct("9.9.9.9:9".into()) });
    assert_eq!(other_reach.genesis_namespace(), ns0);
}
```

**Green.** No production change expected — `genesis_namespace` already hashes only
`scheme` + sorted `validators`. If red, restore that exclusion.

**Gate + commit.**

```
git commit -am "test(invite-v3): lock reach out of the genesis fingerprint (advisory invariant)"
```

---

## Task 4 — v3 pack + signature (encoder side only)

**Goal.** `pack_invite_v3` and the v3 `encode_invite`. Verify the exact layout and
that the appended signature validates against the embedded inviter key over the
payload-minus-signature — without a decoder yet.

**Constants + version-byte rename.** Rename the existing v2 constants and add v3
ones (near the top of the invite section):

```rust
/// the v2 invite prefix (parse-only now). renamed from INVITE_PREFIX.
const INVITE_PREFIX_V2: &str = "ducktape-invite-v2:";
/// the v3 invite prefix; a v3 paste is visibly distinct from v2 and the two are
/// never confusable (prefix and payload version byte must agree on decode).
const INVITE_PREFIX_V3: &str = "ducktape-invite-v3:";

const INVITE_VERSION_V2: u8 = 2; // renamed from INVITE_VERSION
const INVITE_VERSION_V3: u8 = 3;

/// domain separator for the v3 invite signature (matches the wireguard-upgrade
/// namespace convention, e.g. ENDPOINT_NS = b"ducktape:wireguard-endpoint:v1").
const INVITE_SIG_NS: &[u8] = b"ducktape:invite:v3:";
```

Update the `INVITE_PREFIX` module/const doc comment and the invite-section banner
to describe v3 (signed, typed reach) with v2 as parse-only.

**Red:**

```rust
#[test]
fn pack_invite_v3_layout_and_signature_are_exact() {
    let inviter = ed25519::PrivateKey::from_seed(41);
    let ipk = inviter.public_key();
    let member = ed25519::PrivateKey::from_seed(42).public_key();
    let coord = ed25519::PrivateKey::from_seed(43).public_key();
    let mut d = NetworkDescriptor {
        chain_id: "pk#a1b2c3d4".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(ipk.as_ref()), hex_bytes(member.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    d.validators.sort();
    d.add_reach(&ReachHint { expected_key: member.clone(), reach: Reach::Direct("10.0.0.2:9000".into()) });
    d.add_reach(&ReachHint {
        expected_key: ipk.clone(),
        reach: Reach::Coordinated(CoordRef { coord_addr: "p2p:7777".into(), coord_key: coord }),
    });

    let bytes = pack_invite_v3(&d, &inviter, 5_000).expect("pack");
    // header
    assert_eq!(bytes[0], INVITE_VERSION_V3);
    let cid = d.chain_id.as_bytes();
    assert_eq!(bytes[1] as usize, cid.len());
    assert_eq!(&bytes[2..2 + cid.len()], cid);
    // last 64 bytes are the signature over everything before them, domain-separated.
    let split = bytes.len() - 64;
    let sig = ed25519::Signature::decode(&bytes[split..]).expect("sig decodes");
    assert!(ipk.verify(INVITE_SIG_NS, &bytes[..split], &sig), "signature verifies over payload-wo-sig");
    // and the wrong domain must NOT verify (domain separation is real).
    assert!(!ipk.verify(b"ducktape:invite:v2:", &bytes[..split], &sig));

    // the textual blob carries the v3 prefix.
    let blob = encode_invite(&d, &inviter, 5_000).expect("encode");
    assert!(blob.starts_with(INVITE_PREFIX_V3));
}
```

**Green.** Rewrite `encode_invite` to the v3 signature and add the packer + a
`put_str_u8` helper:

```rust
/// encode a v3 invite: the descriptor's reach hints + expiry, signed by the
/// inviter. the inviter must be a genesis validator (enforced on decode).
pub fn encode_invite(
    descriptor: &NetworkDescriptor,
    inviter: &ed25519::PrivateKey,
    expires_unix: u64,
) -> Result<String, String> {
    use base64::Engine as _;
    let payload = pack_invite_v3(descriptor, inviter, expires_unix)?;
    Ok(format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(payload)))
}

fn pack_invite_v3(
    d: &NetworkDescriptor,
    inviter: &ed25519::PrivateKey,
    expires_unix: u64,
) -> Result<Vec<u8>, String> {
    let mut out = vec![INVITE_VERSION_V3];

    let cid = d.chain_id.as_bytes();
    out.push(u8::try_from(cid.len()).map_err(|_| format!("chain_id too long ({} bytes)", cid.len()))?);
    out.extend_from_slice(cid);

    let vkeys = d.validator_keys()?; // hex -> raw, deduped, rejects malformed here
    out.push(u8::try_from(vkeys.len()).map_err(|_| format!("too many validators ({})", vkeys.len()))?);
    for k in &vkeys {
        out.extend_from_slice(k.as_ref());
    }

    let hints = d.reach_hints()?;
    out.push(u8::try_from(hints.len()).map_err(|_| format!("too many reach hints ({})", hints.len()))?);
    for h in &hints {
        out.extend_from_slice(h.expected_key.as_ref());
        match &h.reach {
            Reach::Direct(a) => { out.push(0); put_str_u8(&mut out, a)?; }
            Reach::Fronted(a) => { out.push(1); put_str_u8(&mut out, a)?; }
            Reach::Coordinated(c) => {
                out.push(2);
                put_str_u8(&mut out, &c.coord_addr)?;
                out.extend_from_slice(c.coord_key.as_ref());
            }
        }
    }

    out.extend_from_slice(&expires_unix.to_le_bytes());
    out.extend_from_slice(inviter.public_key().as_ref());

    // sign everything above; the 64-byte signature is appended and not itself signed.
    let sig = inviter.sign(INVITE_SIG_NS, &out);
    out.extend_from_slice(sig.as_ref());
    Ok(out)
}

/// length-prefix (u8) a short utf-8 string into the packed buffer.
fn put_str_u8(out: &mut Vec<u8>, s: &str) -> Result<(), String> {
    let b = s.as_bytes();
    out.push(u8::try_from(b.len()).map_err(|_| format!("string too long ({} bytes): {s:?}", b.len()))?);
    out.extend_from_slice(b);
    Ok(())
}
```

The old `pack_invite` (v2 packer) and the `INVITE_VERSION` references are removed
from prod here; the v2 *unpacker* stays and is renamed in Task 6. The crate will
not compile again until Task 5 wires `decode_invite` — that is expected within a
TDD slice; keep Tasks 4→5 tight (or stage them as one commit if you prefer a
green tree between commits — see note below).

> Green-tree note: if you require every commit to compile, fold Tasks 4, 5, 6 into
> a single commit (they are the v3/v2 codec as one unit). They are split here for
> reviewability; the recommended path is to implement 4→6 back-to-back, run the
> gate once, and commit as three messages via `git add -p` staging or as one
> squashed commit `feat(invite-v3): v3 pack/unpack + signature + v2 parse-only`.
> The plan keeps them numbered for clarity of intent.

**Gate + commit** (after 5–6 if you are keeping the tree green; otherwise commit
the encoder unit now with tests marked `#[ignore]` until decode lands — not
recommended). Recommended commit message:

```
git commit -am "feat(invite-v3): v3 pack + domain-separated inviter signature"
```

---

## Task 5 — v3 unpack + verify (signature, inviter∈validators, expiry) + trailing-byte rigor

**Goal.** `unpack_invite_v3` + the clock-injected `decode_invite_at` core, wired
into `decode_invite`. Fail closed on bad signature, non-validator inviter, expiry,
truncation, and trailing bytes.

**Red:**

```rust
fn v3_fixture(expires: u64) -> (ed25519::PrivateKey, NetworkDescriptor) {
    let inviter = ed25519::PrivateKey::from_seed(51);
    let ipk = inviter.public_key();
    let member = ed25519::PrivateKey::from_seed(52).public_key();
    let coord = ed25519::PrivateKey::from_seed(53).public_key();
    let mut d = NetworkDescriptor {
        chain_id: "v3#a1b2c3d4".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(ipk.as_ref()), hex_bytes(member.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    d.validators.sort();
    d.add_reach(&ReachHint { expected_key: member.clone(), reach: Reach::Direct("10.0.0.2:9000".into()) });
    d.add_reach(&ReachHint { expected_key: member, reach: Reach::Fronted("front:443".into()) }); // replaces
    d.add_reach(&ReachHint {
        expected_key: ipk,
        reach: Reach::Coordinated(CoordRef { coord_addr: "p2p:7777".into(), coord_key: coord }),
    });
    (inviter, d)
}

#[test]
fn v3_roundtrips_all_reach_kinds_and_verifies() {
    let (inviter, d) = v3_fixture(5_000);
    let blob = encode_invite(&d, &inviter, 5_000).expect("encode");
    let got = decode_invite_at(&blob, 4_000).expect("decode within ttl");
    assert_eq!(got.chain_id, d.chain_id);
    assert_eq!(got.validators, d.validators);
    assert_eq!(got.reach, d.reach);      // canonical reach round-trips exactly
    assert!(got.bootstrap.is_empty());   // v3 carries reach, not bootstrap
    // and the decoded descriptor fingerprints identically to the founder's.
    assert_eq!(got.genesis_namespace(), d.genesis_namespace());
}

#[test]
fn v3_rejects_a_tampered_expected_key() {
    let (inviter, d) = v3_fixture(5_000);
    use base64::Engine as _;
    let mut bytes = pack_invite_v3(&d, &inviter, 5_000).unwrap();
    // flip one byte inside the first reach hint's expected_key region.
    let cid = d.chain_id.as_bytes().len();
    let flip = 1 + 1 + cid + 1 + 32 * d.validators.len() + 1 + 1; // into expected_key[0]
    bytes[flip] ^= 0x01;
    let blob = format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(bytes));
    assert!(decode_invite_at(&blob, 4_000).is_err(), "tamper must break the signature");
}

#[test]
fn v3_rejects_expired() {
    let (inviter, d) = v3_fixture(5_000);
    let blob = encode_invite(&d, &inviter, 5_000).unwrap();
    assert!(decode_invite_at(&blob, 5_000).is_err(), "now == expires is expired");
    assert!(decode_invite_at(&blob, 6_000).is_err());
}

#[test]
fn v3_rejects_inviter_not_in_validators() {
    let outsider = ed25519::PrivateKey::from_seed(99); // not in the validator set
    let (_inviter, d) = v3_fixture(5_000);
    let blob = encode_invite(&d, &outsider, 5_000).unwrap();
    assert!(decode_invite_at(&blob, 4_000).is_err(), "inviter must be a genesis validator");
}

#[test]
fn v3_rejects_trailing_and_truncated() {
    let (inviter, d) = v3_fixture(5_000);
    use base64::Engine as _;
    let good = pack_invite_v3(&d, &inviter, 5_000).unwrap();
    let mut trailing = good.clone();
    trailing.push(0);
    let blob = format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(trailing));
    assert!(decode_invite_at(&blob, 4_000).is_err(), "trailing bytes rejected");
    let blob = format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(&good[..good.len() - 1]));
    assert!(decode_invite_at(&blob, 4_000).is_err(), "truncation rejected");
}
```

**Green.** Add reader helpers, `decode_invite`/`decode_invite_at`, and
`unpack_invite_v3`. Extend `InviteReader`:

```rust
    fn take_str_u8(&mut self) -> Result<String, String> {
        let len = self.u8()? as usize;
        Ok(std::str::from_utf8(self.take(len)?)
            .map_err(|e| format!("invite string: {e}"))?
            .to_string())
    }

    fn take_key(&mut self) -> Result<ed25519::PublicKey, String> {
        ed25519::PublicKey::decode(self.take(32)?)
            .map_err(|e| format!("invite public key: {e}"))
    }
```

Decode dispatcher (replaces the current `decode_invite`):

```rust
pub fn decode_invite(blob: &str) -> Result<NetworkDescriptor, String> {
    decode_invite_at(blob, unix_now_secs()?)
}

/// clock-injected core so expiry is deterministically testable.
fn decode_invite_at(blob: &str, now_unix: u64) -> Result<NetworkDescriptor, String> {
    use base64::Engine as _;
    let blob = blob.trim();
    // choose the codec by prefix; the payload version byte must AGREE (defence in
    // depth: a v2 payload can never ride under a v3 prefix, or vice-versa).
    let (body, prefix_version) = if let Some(b) = blob.strip_prefix(INVITE_PREFIX_V3) {
        (b, INVITE_VERSION_V3)
    } else if let Some(b) = blob.strip_prefix(INVITE_PREFIX_V2) {
        (b, INVITE_VERSION_V2)
    } else {
        return Err(format!(
            "not a ducktape invite (expected {INVITE_PREFIX_V3}... or {INVITE_PREFIX_V2}...)"
        ));
    };
    let bytes = INVITE_B64.decode(body).map_err(|e| format!("invite is not valid base64url: {e}"))?;
    let version = *bytes.first().ok_or("invite payload is empty")?;
    if version != prefix_version {
        return Err(format!("invite prefix is v{prefix_version} but payload is v{version}"));
    }
    match version {
        INVITE_VERSION_V2 => unpack_invite_v2(&bytes),
        INVITE_VERSION_V3 => unpack_invite_v3(&bytes, now_unix),
        other => Err(format!(
            "unsupported invite version {other} (this build reads v{INVITE_VERSION_V2}/v{INVITE_VERSION_V3})"
        )),
    }
}

fn unpack_invite_v3(bytes: &[u8], now_unix: u64) -> Result<NetworkDescriptor, String> {
    let mut r = InviteReader::new(bytes);
    let version = r.u8()?;
    debug_assert_eq!(version, INVITE_VERSION_V3);

    let cid_len = r.u8()? as usize;
    let chain_id = String::from_utf8(r.take(cid_len)?.to_vec()).map_err(|e| format!("chain_id: {e}"))?;

    let vcount = r.u8()? as usize;
    let mut validators = Vec::with_capacity(vcount);
    for _ in 0..vcount {
        validators.push(hex_bytes(r.take(32)?));
    }
    validators.sort();

    let hcount = r.u8()? as usize;
    let mut reach = Vec::with_capacity(hcount);
    for _ in 0..hcount {
        let expected_key = r.take_key()?;
        let reach_val = match r.u8()? {
            0 => Reach::Direct(r.take_str_u8()?),
            1 => Reach::Fronted(r.take_str_u8()?),
            2 => {
                let coord_addr = r.take_str_u8()?;
                let coord_key = r.take_key()?;
                Reach::Coordinated(CoordRef { coord_addr, coord_key })
            }
            other => return Err(format!("unknown reach tag {other} in v3 invite")),
        };
        reach.push(ReachHint { expected_key, reach: reach_val }.to_canonical());
    }
    reach.sort();

    let expires_unix = u64::from_le_bytes(r.take(8)?.try_into().expect("take(8) yields 8 bytes"));
    let inviter_key = r.take_key()?;

    let signed_len = r.pos; // everything up to (not incl.) the signature
    let sig_bytes = r.take(64)?;
    if !r.done() {
        return Err("invite payload has trailing bytes".into());
    }

    // fail closed, in order: signature integrity, then membership binding, then expiry.
    let signature = ed25519::Signature::decode(sig_bytes)
        .map_err(|e| format!("invite signature is malformed: {e}"))?;
    if !inviter_key.verify(INVITE_SIG_NS, &bytes[..signed_len], &signature) {
        return Err("invite signature does not verify".into());
    }
    if !validators.contains(&hex_bytes(inviter_key.as_ref())) {
        return Err("invite inviter is not a genesis validator".into());
    }
    if now_unix >= expires_unix {
        return Err(format!("invite expired (expires {expires_unix}, now {now_unix})"));
    }

    Ok(NetworkDescriptor {
        chain_id,
        scheme: SCHEME_ED25519.into(),
        validators,
        bootstrap: Vec::new(),
        reach,
    })
}
```

Add the clock helper near the identity helpers:

```rust
/// current unix time in whole seconds (invite expiry base).
pub fn unix_now_secs() -> Result<u64, String> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock is before the unix epoch".to_string())?
        .as_secs())
}
```

`r.pos` is a private field on `InviteReader` in the same module — accessible.
`ed25519::Signature::decode` / `ed25519::PublicKey::decode` use the already-imported
`DecodeExt`; `.verify` uses the `Verifier` trait added in Task 1.

**Gate + commit.**

```
git commit -am "feat(invite-v3): v3 unpack + fail-closed sig/inviter/expiry + trailing rigor"
```

---

## Task 6 — v2 parse-only + non-confusability

**Goal.** Keep decoding real v2 blobs (all-`Direct`, unsigned), prove v2/v3 are
never confusable, and provide a test-only v2 encoder to synthesise fixtures.

**Red:**

```rust
#[test]
fn v2_blob_decodes_to_all_direct_unsigned_hints() {
    let a = ed25519::PrivateKey::from_seed(61).public_key();
    let b = ed25519::PrivateKey::from_seed(62).public_key();
    let mut d = NetworkDescriptor {
        chain_id: "v2#a1b2c3d4".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(a.as_ref()), hex_bytes(b.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    d.validators.sort();
    d.add_bootstrap(&a, "127.0.0.1:52200");
    d.add_bootstrap(&b, "node.example.com:443");

    let blob = encode_invite_v2(&d).expect("v2 encode (test-only)");
    assert!(blob.starts_with(INVITE_PREFIX_V2));
    let got = decode_invite_at(&blob, 4_000).expect("v2 decodes, no signature/expiry");
    assert_eq!(got.bootstrap, d.bootstrap);
    assert!(got.reach.is_empty(), "v2 stores no explicit reach");
    // the TYPED view is all-Direct.
    let hints = got.reach_hints().unwrap();
    assert_eq!(hints.len(), 2);
    assert!(hints.iter().all(|h| matches!(h.reach, Reach::Direct(_))));
}

#[test]
fn v2_and_v3_are_never_confusable() {
    use base64::Engine as _;
    let inviter = ed25519::PrivateKey::from_seed(63);
    let ipk = inviter.public_key();
    let mut d = NetworkDescriptor {
        chain_id: "x#a1b2c3d4".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(ipk.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    d.validators.sort();

    let v3 = pack_invite_v3(&d, &inviter, 5_000).unwrap();
    let v2 = pack_invite_v2(&d).unwrap();
    // a v3 payload under a v2 prefix (and vice-versa) is rejected on the agreement check.
    let mislabelled_v3 = format!("{INVITE_PREFIX_V2}{}", INVITE_B64.encode(&v3));
    let mislabelled_v2 = format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(&v2));
    assert!(decode_invite_at(&mislabelled_v3, 4_000).is_err());
    assert!(decode_invite_at(&mislabelled_v2, 4_000).is_err());
    // an unknown version tag is rejected.
    let mut bogus = v2.clone();
    bogus[0] = 9;
    let blob = format!("{INVITE_PREFIX_V2}{}", INVITE_B64.encode(bogus));
    assert!(decode_invite_at(&blob, 4_000).is_err());
    // a garbage prefix is rejected.
    assert!(decode_invite_at("ducktape-invite-v1:AAAA", 4_000).is_err());
}
```

**Green.** Rename the current `unpack_invite` to `unpack_invite_v2`, drop its now-
redundant version guard to a `debug_assert`, and add `reach: Vec::new()` to its
returned descriptor. Add the test-only v2 encoder/packer (v2 is parse-only in
prod, so its packer is gated behind `#[cfg(test)]` to satisfy clippy `-D
warnings`):

```rust
fn unpack_invite_v2(bytes: &[u8]) -> Result<NetworkDescriptor, String> {
    let mut r = InviteReader::new(bytes);
    let version = r.u8()?;
    debug_assert_eq!(version, INVITE_VERSION_V2);
    // ... existing v2 body unchanged (chain_id, validators, bootstrap) ...
    Ok(NetworkDescriptor {
        chain_id,
        scheme: SCHEME_ED25519.into(),
        validators,
        bootstrap,
        reach: Vec::new(),
    })
}

/// test-only v2 encoder — v2 is parse-only in production, but tests must be able
/// to synthesise real v2 blobs to prove parse-compatibility and non-confusability.
#[cfg(test)]
fn encode_invite_v2(d: &NetworkDescriptor) -> Result<String, String> {
    use base64::Engine as _;
    Ok(format!("{INVITE_PREFIX_V2}{}", INVITE_B64.encode(pack_invite_v2(d)?)))
}

#[cfg(test)]
fn pack_invite_v2(d: &NetworkDescriptor) -> Result<Vec<u8>, String> {
    let mut out = vec![INVITE_VERSION_V2];
    let cid = d.chain_id.as_bytes();
    out.push(u8::try_from(cid.len()).map_err(|_| format!("chain_id too long ({} bytes)", cid.len()))?);
    out.extend_from_slice(cid);
    let vkeys = d.validator_keys()?;
    out.push(u8::try_from(vkeys.len()).map_err(|_| format!("too many validators ({})", vkeys.len()))?);
    for k in &vkeys {
        out.extend_from_slice(k.as_ref());
    }
    out.push(u8::try_from(d.bootstrap.len()).map_err(|_| format!("too many bootstrap hints ({})", d.bootstrap.len()))?);
    for entry in &d.bootstrap {
        let (key, host_port) = entry
            .split_once('@')
            .ok_or_else(|| format!("bootstrap entry {entry:?} is not pubkey@addr"))?;
        let key = decode_key(key)?;
        out.extend_from_slice(key.as_ref());
        put_str_u8(&mut out, host_port)?;
    }
    Ok(out)
}
```

**Gate + commit.**

```
git commit -am "feat(invite-v3): v2 parse-only decode + non-confusability guard"
```

---

## Task 7 — Resolution: `reach_entries()` dial targets + `Coordinated` dial target

**Goal.** Resolve reach hints to `(expected_key, dial_addr)` and route the joiner
dial through it. `Coordinated` yields the coordinator's socket as the dial target
while still expecting the target's key — the wire is now able to express "dial
`coord_addr`, expect `expected_key`."

**Red:**

```rust
#[test]
fn coordinated_hint_resolves_dial_target_to_coord_addr_with_expected_key() {
    let target = ed25519::PrivateKey::from_seed(71).public_key();
    let coord = ed25519::PrivateKey::from_seed(72).public_key();
    let mut d = NetworkDescriptor {
        chain_id: "co#00000000".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(target.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    // dial the coordinator's socket, but the identity we expect is the TARGET.
    d.add_reach(&ReachHint {
        expected_key: target.clone(),
        reach: Reach::Coordinated(CoordRef { coord_addr: "127.0.0.1:59999".into(), coord_key: coord }),
    });
    let entries = d.reach_entries().expect("resolve");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, target);                                   // expect target key
    assert_eq!(entries[0].1, "127.0.0.1:59999".parse().unwrap());       // dial coordinator
}

#[test]
fn reach_entries_falls_back_to_bootstrap_for_v2_and_skips_unresolvable() {
    let a = ed25519::PrivateKey::from_seed(73).public_key();
    let mut d = NetworkDescriptor {
        chain_id: "co#00000000".into(),
        scheme: SCHEME_ED25519.into(),
        validators: vec![hex_bytes(a.as_ref())],
        bootstrap: vec![],
        reach: vec![],
    };
    d.add_bootstrap(&a, "127.0.0.1:52200");           // resolvable
    d.add_reach(&ReachHint { expected_key: a.clone(), reach: Reach::Direct("127.0.0.1:52200".into()) });
    // reach present -> parsed from reach; the direct entry resolves.
    let entries = d.reach_entries().unwrap();
    assert_eq!(entries, vec![(a, "127.0.0.1:52200".parse().unwrap())]);
}
```

**Green.** Add `reach_entries()` and swap the one call in `resolve_network_shape`:

```rust
    /// reach hints resolved to `(expected_key, dial_addr)`: what a joiner dials
    /// and the identity it must end up authenticating end-to-end. `Direct`/
    /// `Fronted` dial the hint's own address; `Coordinated` dials the COORDINATOR
    /// while still expecting the target's key. advisory: an unresolvable or
    /// unspecified/port-0 hint is skipped, mirroring [`bootstrap_entries`].
    pub fn reach_entries(&self) -> Result<Vec<(ed25519::PublicKey, SocketAddr)>, String> {
        use std::net::ToSocketAddrs as _;
        let mut out = Vec::new();
        for hint in self.reach_hints()? {
            let dial = match &hint.reach {
                Reach::Direct(a) | Reach::Fronted(a) => a,
                Reach::Coordinated(c) => &c.coord_addr,
            };
            let Some(addr) = dial.to_socket_addrs().ok().and_then(|mut it| it.next()) else {
                continue; // unresolvable (stale DNS, offline) — advisory, skip.
            };
            if addr.ip().is_unspecified() || addr.port() == 0 {
                continue;
            }
            out.push((hint.expected_key.clone(), addr));
        }
        Ok(out)
    }
```

In `resolve_network_shape`, change the one line

```rust
    let bootstrap = descriptor.bootstrap_entries()?;
```

to

```rust
    // one dial source of truth: reach_entries() falls through to bootstrap
    // synthesis for v2/legacy descriptors, so existing behaviour is preserved
    // and Coordinated/Fronted hints route their dial target correctly.
    let bootstrap = descriptor.reach_entries()?;
```

Everything downstream (`mesh` union of `bootstrap` identities, `bootstrappers`
filtering self) is unchanged. The existing tests
`network_shape_resolves_membership_and_bootstrap` and
`a_non_member_identity_resolves_as_a_pending_joiner` must stay green (reach empty
→ synthesis from bootstrap → identical results). `bootstrap_entries()` remains a
public method (still used elsewhere / kept for API stability).

**Gate + commit.**

```
git commit -am "feat(invite-v3): resolve reach to dial targets (Coordinated dials coord_addr)"
```

---

## Task 8 — CLI: `cmd_invite` signs v3 with `--ttl-days`

**Goal.** Wire the founder/member mint path to the v3 signed encoder. This is the
only `main.rs` change. Keep the expiry math in `config.rs` so it is unit-testable.

**Red** (config.rs test):

```rust
#[test]
fn invite_expiry_adds_ttl_days_and_saturates_cleanly() {
    assert_eq!(invite_expiry(1_000, 7).unwrap(), 1_000 + 7 * 86_400);
    assert_eq!(invite_expiry(0, 1).unwrap(), 86_400);
    assert!(invite_expiry(0, u64::MAX).is_err(), "absurd ttl errors, never overflows");
    assert!(invite_expiry(u64::MAX, 1).is_err(), "expiry overflow errors");
}
```

**Green** (config.rs):

```rust
/// default invite lifetime if `--ttl-days` is not given.
pub const DEFAULT_INVITE_TTL_DAYS: u64 = 7;

/// invite expiry (unix secs) = now + ttl_days, erroring rather than overflowing.
pub fn invite_expiry(now_unix: u64, ttl_days: u64) -> Result<u64, String> {
    let secs = ttl_days.checked_mul(86_400).ok_or("--ttl-days too large")?;
    now_unix.checked_add(secs).ok_or_else(|| "invite expiry overflow".to_string())
}
```

**Green** (main.rs `cmd_invite`) — replace the final `encode_invite` call. The
inviter key is already loaded as `key`:

```rust
    descriptor.save(&descriptor_path)?;
    let ttl_days: u64 = flags
        .get("ttl-days")
        .map(|s| s.parse::<u64>().map_err(|e| format!("--ttl-days: {e}")))
        .transpose()?
        .unwrap_or(config::DEFAULT_INVITE_TTL_DAYS);
    let expires = config::invite_expiry(config::unix_now_secs()?, ttl_days)?;
    println!("{}", config::encode_invite(&descriptor, &key, expires)?);
    Ok(())
```

Confirm `cmd_invite` still adds the founder's own dial hint via `add_bootstrap`
(unchanged). Because the founder is in `validators` and `reach` is empty, the v3
invite carries that founder as an all-`Direct` synthesised hint AND signs with the
founder key (which is in `validators`) — so `decode` accepts it. (If a founder
later wants to advertise a `Coordinated` reach, they call `add_reach` before
`encode_invite`; wiring a `--coordinator` flag onto `cmd_invite` is a Slice 2/4
follow-up, out of scope here.)

**Note on `parse_flags`.** Verify `--ttl-days` is accepted by the existing
`parse_flags` (it collects `--k v` / `--k=v` into `flags`); it is a value flag, so
no allowlist change is expected — confirm by reading `parse_flags` before editing.

**Gate + commit.** This task recompiles the whole bin (main.rs changed) — slow,
accepted.

```
git commit -am "feat(invite-v3): cmd_invite signs v3 invites with --ttl-days"
```

---

## Task 9 — Docs, doc-comments, and final full gate

**Goal.** Update the in-file narrative so the code reads truthfully, and run the
full gate one last time.

- `config.rs`: update the invite-section banner comment (currently "v2 packs
  only what a joiner needs …") to describe v3 (typed reach + inviter signature +
  expiry) with v2 as parse-only, and update the `INVITE_PREFIX_V2` doc line.
- Add a short "v3 invite" note to
  `docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md`'s
  Component 2 marking Slice 1 landed.
- Out of scope, listed as follow-ups (do **not** do here): the desktop
  `OnboardingGate` placeholder text still says `ducktape-invite-v2:…`; `decode`
  accepts both prefixes so pastes keep working, and the UI copy update belongs to
  an app-tier task. The `--coordinator` mint flag and any live coordinator dial
  belong to Slices 2/4.

**Final gate** (run both, expect green):

```
cargo test -p node-bin --bin ducktape-node config::
cargo clippy -p node-bin --all-targets -- -D warnings
```

Optionally run the fuller bin unit suite to ensure nothing else regressed:

```
cargo test -p node-bin --bin ducktape-node
```

**Commit.**

```
git commit -am "docs(p3): document v3 signed invite + typed reach (Slice 1)"
```

---

## Risks / out of scope / follow-ups

- **Every commit compiling.** Tasks 4→6 are the codec as one unit; the tree is
  only guaranteed green after Task 6. Implement 4–6 back-to-back and either commit
  three staged messages or squash to one `feat(invite-v3): v3+v2 codec`. Tasks
  1–3, 7–9 each leave a green tree individually.
- **Inviter-∈-validators policy.** A deliberate fail-closed bind: only genesis
  validators (founder pre-genesis, members post-genesis) can mint a valid v3
  invite. This matches valset==membership; revisit only if a non-validator
  issuer requirement appears. Documented in the decode error and the doc-comment.
- **v3 does not carry `bootstrap`.** A decoded v3 descriptor has `bootstrap`
  empty and `reach` populated; `reach_hints()`/`reach_entries()` are the single
  dial source, so this is correct. Any code that reads `descriptor.bootstrap`
  directly (grep before finishing) must go through `reach_hints()`/`reach_entries()`
  instead — expected only inside `config.rs`.
- **Genesis fingerprint unchanged.** Reach and bootstrap are both advisory and
  excluded (Task 3 guard). Two members with different reach hints still connect;
  a stale *validator* set still fails loudly. No consensus change.
- **Live `Coordinated` dial** (the mesh actually completing a handshake through a
  coordinator relay) is Slice 2 — Slice 1 only makes the dial *target* resolvable.
- **App paste text** and a `--coordinator` mint flag are follow-ups (Slice 2/4).
