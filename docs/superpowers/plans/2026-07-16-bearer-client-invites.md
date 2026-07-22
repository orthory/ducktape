# Bearer Client Invites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Single-use bearer invites, Client role only, redeemed over the existing `/v1/submit` lane by a new `user-redeem-invite` CLI verb.

**Architecture:** `InviteToken.target` becomes `Option<PublicKey>` with a signature-covered kind byte (one in-place format change, flag day). Consensus (`handle_redeem`) treats an empty `target` as bearer, requires `role == Client` for it, and skips the target lock — nonce exactly-once already gives single-use first-wins. No new gate, no new route: the resident lobby gate stays Resident-only, and a thin client POSTs its own `GovMsg::Redeem` to a member node's frameless `/v1/submit` (bin/node stamps its node key as the external origin; the token, not the submitter, authorizes admission).

**Tech Stack:** Rust (workspace), governance wasm module (adapter build via `make wasm-modules`), reqwest blocking (already a workspace dep with `blocking` feature).

**Spec:** `docs/superpowers/specs/2026-07-16-bearer-client-invites-design.md`

## Global Constraints

- Worktree: `/home/eddy/dev/ducktape/.worktree/bearer-client-invites` (branch `feat/bearer-client-invites`, forked from origin/dev `538fa38e8b`). ALL commands below run from this directory.
- NO backward compatibility (standing mandate): the preimage changes in place — no dual-decode, no version tags; one format exists. All outstanding invites die; that is accepted.
- Bearer ⇒ `role == Client`, enforced at mint (by construction), at redeem (consensus), and at node-join paste time. NEVER grant resident standing from a bearer token.
- Single-use stays the law: one invite = one redemption (operator directive). No multi-use anywhere.
- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`. Exception (pre-existing): `-p nat-traversal` needs `--features simnat` — not touched here.
- `cargo fmt` only on files you touched, never `--all`.
- `-p simnode` is a STANDING GATE for any governance-wire PR (this is one).
- Governance is a WASM module (`include_bytes!` of `crates/examples/governance-wasm/component.wasm`): after changing `crates/system/governance`, run `make wasm-modules` and commit the refreshed component bytes TOGETHER with the source, else the consensus change is inert and `make wasm-modules-check` fails.
- rustc SIGSEGV workaround on this box if it bites: `ulimit -s unlimited` in the shell; `CARGO_INCREMENTAL=0` for stubborn cases.
- Cached cargo emits no warnings: `touch` the touched `.rs` files before a clippy gate you want to be non-vacuous.
- Logging: `println!` is CORRECT in CLI verb code (stdout is the program's output contract); `tracing` for anything that runs inside a node. No URI/token/key material in tracing events.

---

### Task 1: governance — `InviteToken.target: Option`, the new preimage

**Files:**
- Modify: `crates/system/governance/src/invite.rs`

**Interfaces:**
- Produces: `pub struct InviteToken { target: Option<ed25519::PublicKey>, ... }` (other fields unchanged); `grant_preimage(binding, nonce, target: Option<&ed25519::PublicKey>, role, expires) -> Vec<u8>` stays private; `verify_invite_token(&InviteToken, binding) -> bool` signature unchanged; `sign_join_proof`/`verify_join_proof` unchanged (they never touch target).
- The preimage (every later task that re-states it MUST match byte-for-byte):
  `binding ‖ nonce ‖ 0x01 ‖ target(32) ‖ role ‖ expires_le` (targeted)
  `binding ‖ nonce ‖ 0x00 ‖ role ‖ expires_le` (bearer)

- [ ] **Step 1: change the struct and preimage**

In `crates/system/governance/src/invite.rs`:

```rust
pub struct InviteToken {
    /// the minting member — checked against CURRENT membership on redemption.
    pub issuer: ed25519::PublicKey,
    /// per-invite randomness: distinguishes tokens and is the single-use key.
    pub nonce: [u8; INVITE_NONCE_LEN],
    /// the ONE key this invite admits, or `None` for a BEARER invite —
    /// bearer is Client-role-only (a bearer token can never grant resident
    /// standing; redemption and every admission door enforce it).
    pub target: Option<ed25519::PublicKey>,
    pub role: InviteRole,
    pub expires_unix_secs: u64,
    pub sig: ed25519::Signature,
}

/// the signed preimage of an invite grant: a kind byte distinguishes
/// targeted (0x01 ‖ target) from bearer (0x00, no target bytes) so neither
/// form can be replayed as the other. every covered field is authenticated.
fn grant_preimage(
    binding: &[u8],
    nonce: &[u8],
    target: Option<&ed25519::PublicKey>,
    role: InviteRole,
    expires: u64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(binding.len() + nonce.len() + 42);
    out.extend_from_slice(binding);
    out.extend_from_slice(nonce);
    match target {
        Some(t) => {
            out.push(1);
            out.extend_from_slice(t.as_ref());
        }
        None => out.push(0),
    }
    out.push(role.as_u8());
    out.extend_from_slice(&expires.to_le_bytes());
    out
}
```

Update `verify_invite_token` to pass `token.target.as_ref()`. Update the module doc header: replace "`target` is the ONE key the invite admits (no bearer invites)" with the bearer-is-client-only rule. Do NOT add a role invariant inside `verify_invite_token` — it stays pure signature math; the invariant is enforced at mint (by construction, Task 4), at redeem (Task 2), and at the admission doors (already role-gated).

- [ ] **Step 2: fix the in-file tests to the new preimage and add bearer coverage**

The `mint` helper and `a_client_role_invite...` test construct preimages inline — update both to the new shape (kind byte `1` + target). Add:

```rust
#[test]
fn a_bearer_token_verifies_and_kinds_are_not_interchangeable() {
    let issuer = ed25519::PrivateKey::from_seed(1);
    let nonce = [8u8; INVITE_NONCE_LEN];
    let (role, expires) = (InviteRole::Client, 4_102_444_800);
    let msg = grant_preimage(BINDING, &nonce, None, role, expires);
    let token = InviteToken {
        issuer: issuer.public_key(),
        nonce,
        target: None,
        role,
        expires_unix_secs: expires,
        sig: issuer.sign(INVITE_GRANT_NAMESPACE, &msg),
    };
    assert!(verify_invite_token(&token, BINDING));

    // grafting a target onto a bearer sig (or stripping one off a targeted
    // sig) breaks verification: the kind byte is signature-covered.
    let mut t = token.clone();
    t.target = Some(ed25519::PrivateKey::from_seed(3).public_key());
    assert!(!verify_invite_token(&t, BINDING));
}
```

Also update `the_join_proof_binds_the_key_not_just_the_token` only if it fails to compile (it uses `mint`, which now wraps `Some(target)`).

- [ ] **Step 3: run the crate tests — expect the rest of the workspace still broken**

Run: `cargo test -p governance --lib` (unit tests in invite.rs live in the lib)
Expected: PASS. (`cargo test -p governance` full will fail until Task 2 fixes lib.rs/tests — that is fine at this step.)

- [ ] **Step 4: commit**

```bash
git add crates/system/governance/src/invite.rs
git commit -m "governance: bearer invite tokens — optional target, kind-byte preimage"
```

---

### Task 2: governance — `handle_redeem` bearer branch + native tests

**Files:**
- Modify: `crates/system/governance/src/lib.rs` (`handle_redeem`, ~line 1222)
- Modify: `crates/system/governance/src/interface.rs` (Redeem doc comment only)
- Modify: `crates/system/governance/tests/invite_redemption.rs`

**Interfaces:**
- Consumes: `InviteToken { target: Option<...> }` from Task 1.
- Produces (wire, relied on by Tasks 4–8): `GovMsg::Redeem.target: Vec<u8>` — **empty bytes = bearer**. Deterministic reject strings (exact, matched by `lobby::redeem_reject_outcome` and tests): `"bearer invites are client-only"` (new); all existing strings unchanged.

- [ ] **Step 1: write the failing native tests**

In `crates/system/governance/tests/invite_redemption.rs`, the existing helpers mint targeted tokens with an inline preimage — update them to the new shape (kind byte, like Task 1's test helper). Add a bearer mint helper and three tests (adapt `Ctx`/module setup from the file's existing tests — reuse whatever fixture the sibling tests use verbatim):

```rust
fn mint_bearer(issuer: &ed25519::PrivateKey, binding: &[u8], nonce: [u8; INVITE_NONCE_LEN], role: InviteRole) -> InviteToken {
    // re-states the preimage deliberately: drift fails this suite.
    let mut msg = Vec::new();
    msg.extend_from_slice(binding);
    msg.extend_from_slice(&nonce);
    msg.push(0); // bearer kind
    msg.push(role.as_u8());
    msg.extend_from_slice(&4_102_444_800u64.to_le_bytes());
    InviteToken { issuer: issuer.public_key(), nonce, target: None, role,
                  expires_unix_secs: 4_102_444_800, sig: issuer.sign(INVITE_GRANT_NAMESPACE, &msg) }
}
```

1. `a_bearer_client_redeem_grants_client_standing_first_wins`: mint bearer Client; key A redeems (empty `target` bytes in the op) → Ok, clients set contains A, valset residents does NOT; key B redeems the SAME token with B's own valid proof → deterministic reject containing `"already redeemed"`.
2. `a_bearer_resident_token_is_rejected_as_client_only`: mint bearer with `InviteRole::Resident` (valid sig over new preimage) → reject containing `"bearer invites are client-only"`; joiner gains no standing of any tier.
3. `a_targeted_token_still_locks_to_its_target`: existing lock test keeps passing after the preimage update (adjust, don't delete).

- [ ] **Step 2: run to verify the new tests fail**

Run: `cargo test -p governance --test invite_redemption`
Expected: FAIL — compile errors on `target: Option` first; after mechanical fixes to the file, the two new tests fail on behavior (`handle_redeem` still requires `joiner == target`).

- [ ] **Step 3: implement the bearer branch**

In `handle_redeem` (lib.rs), `target: Vec<u8>` param:

```rust
// bearer = empty target bytes. bearer is CLIENT-ONLY: a bearer token can
// never grant resident standing (spec §1); reject before any decode work.
if target.is_empty() && role != invite::InviteRole::Client.as_u8() {
    return Err(Error::Module("bearer invites are client-only".into()));
}
let target_key = if target.is_empty() {
    None
} else {
    Some(
        ed25519::PublicKey::decode(target.as_slice())
            .map_err(|e| Error::Module(format!("target key: {e}")))?,
    )
};
```

(the role decode to `InviteRole` happens a few lines below already — compare raw `role` byte here, or move this check after `InviteRole::from_u8`; either is fine as long as the reject fires before the target lock.)

Token construction: `target: target_key`. Replace the lock:

```rust
// the invite admits exactly ONE key when targeted; a bearer invite has no
// lock — the join proof binds the redemption to whichever key redeems
// first, and the nonce set makes that exactly-once.
if !target.is_empty() && joiner != target {
    return Err(Error::Module("invite is locked to another key".into()));
}
```

Everything else (issuer membership, PoP, role grant arms, nonce exactly-once) unchanged. In `interface.rs`, update the `Redeem.target` doc: "empty = bearer (Client-role-only); otherwise the ONE key the token admits".

- [ ] **Step 4: run the governance gates**

Run: `touch crates/system/governance/src/*.rs && cargo test -p governance && cargo clippy -p governance --tests --no-deps`
Expected: all PASS, no new lints.

- [ ] **Step 5: commit**

```bash
git add crates/system/governance
git commit -m "governance: redeem bearer client invites — empty target, client-only, first-wins"
```

---

### Task 3: wasm regen + parity

**Files:**
- Regenerate: `crates/examples/governance-wasm/component.wasm` (+ any sibling components the deterministic rebuild refreshes — commit whatever `make wasm-modules` changes, TOGETHER)

- [ ] **Step 1: rebuild the wasm modules**

Run: `make wasm-modules`
Expected: succeeds; `git status` shows `crates/examples/governance-wasm/component.wasm` modified (others only if toolchain drift — if MANY change, stop and check `rustc --version` matches what CI pins before committing).

- [ ] **Step 2: parity + check gates**

Run: `cargo test -p host --test wasm_governance_parity && make wasm-modules-check`
Expected: PASS — the component and native crate agree on the new vectors; check confirms committed bytes == rebuilt bytes.

- [ ] **Step 3: commit**

```bash
git add crates/examples/*/component.wasm
git commit -m "governance-wasm: regenerate component for bearer invite redemption"
```

---

### Task 4: bin/node — token/blob codec, mint helpers, lobby Option handling

**Files:**
- Modify: `bin/node/src/config/invite.rs` (mint at ~46, pack/unpack token at 76–118, blob write at 556, blob read at 664, `DEFAULT_INVITE_TTL_DAYS` block at ~315)
- Modify: `bin/node/src/lobby.rs` (`gate_request` 121, `intro_request` 326, `verify_join_request` 186)

**Interfaces:**
- Consumes: Task 1's `InviteToken.target: Option`.
- Produces:
  - `pub fn mint_invite_token(signer, binding, target: &ed25519::PublicKey, role, expires) -> InviteToken` — signature UNCHANGED (wraps `Some(target)`); zero caller churn.
  - `pub fn mint_bearer_client_token(signer: &ed25519::PrivateKey, binding: &[u8], expires_unix_secs: u64) -> InviteToken` — the ONLY bearer constructor (Client role by construction).
  - `pub const DEFAULT_BEARER_INVITE_TTL_DAYS: u64 = 1;`
  - Packed token: `issuer(32) ‖ nonce(16) ‖ kind(1) ‖ [target(32) if kind==1] ‖ role(1) ‖ expires_le(8) ‖ sig(64)` → 122 bytes bearer / 154 targeted. Blob embeds it length-prefixed: `u8 len ‖ token bytes`.
  - Wire (`GateMsg`/`IntroRequest` field shapes untouched): `target: Vec<u8>` empty = bearer.

- [ ] **Step 1: codec + mint in config/invite.rs**

```rust
pub fn mint_invite_token(
    signer: &ed25519::PrivateKey,
    binding: &[u8],
    target: &ed25519::PublicKey,
    role: InviteRole,
    expires_unix_secs: u64,
) -> InviteToken {
    mint_token(signer, binding, Some(target), role, expires_unix_secs)
}

/// mint a BEARER Client token: no target lock — the first key to present a
/// valid join proof takes the grant (single-use via the nonce set). Client
/// role by construction: no bearer path to resident standing exists.
pub fn mint_bearer_client_token(
    signer: &ed25519::PrivateKey,
    binding: &[u8],
    expires_unix_secs: u64,
) -> InviteToken {
    mint_token(signer, binding, None, InviteRole::Client, expires_unix_secs)
}

fn mint_token(
    signer: &ed25519::PrivateKey,
    binding: &[u8],
    target: Option<&ed25519::PublicKey>,
    role: InviteRole,
    expires_unix_secs: u64,
) -> InviteToken {
    let mut nonce = [0u8; INVITE_NONCE_LEN];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let mut msg = Vec::new();
    msg.extend_from_slice(binding);
    msg.extend_from_slice(&nonce);
    match target {
        Some(t) => { msg.push(1); msg.extend_from_slice(t.as_ref()); }
        None => msg.push(0),
    }
    msg.push(role.as_u8());
    msg.extend_from_slice(&expires_unix_secs.to_le_bytes());
    InviteToken {
        issuer: signer.public_key(),
        nonce,
        target: target.cloned(),
        role,
        expires_unix_secs,
        sig: signer.sign(INVITE_GRANT_NAMESPACE, &msg),
    }
}
```

(NOTE: the preimage is re-stated here because `grant_preimage` is private to the governance crate — same situation as today, where this file already re-states it via `mint_invite_token`. Keep it byte-identical to Task 1.)

Pack/unpack:

```rust
/// packed token: `issuer(32) ‖ nonce(16) ‖ kind(1) ‖ [target(32)] ‖
/// role(1) ‖ expires_le(8) ‖ sig(64)` — 154 targeted / 122 bearer.
const INVITE_TOKEN_TARGETED_LEN: usize = 32 + INVITE_NONCE_LEN + 1 + 32 + 1 + 8 + 64;
const INVITE_TOKEN_BEARER_LEN: usize = 32 + INVITE_NONCE_LEN + 1 + 1 + 8 + 64;

fn pack_invite_token(t: &InviteToken) -> Vec<u8> {
    let mut out = Vec::with_capacity(INVITE_TOKEN_TARGETED_LEN);
    out.extend_from_slice(t.issuer.as_ref());
    out.extend_from_slice(&t.nonce);
    match &t.target {
        Some(target) => { out.push(1); out.extend_from_slice(target.as_ref()); }
        None => out.push(0),
    }
    out.push(t.role.as_u8());
    out.extend_from_slice(&t.expires_unix_secs.to_le_bytes());
    out.extend_from_slice(t.sig.encode().as_ref());
    out
}

fn unpack_invite_token(bytes: &[u8]) -> Result<InviteToken, String> {
    let issuer = ed25519::PublicKey::decode(bytes.get(..32).ok_or("invite token truncated")?)
        .map_err(|e| format!("invite token issuer: {e}"))?;
    let mut nonce = [0u8; INVITE_NONCE_LEN];
    nonce.copy_from_slice(bytes.get(32..32 + INVITE_NONCE_LEN).ok_or("invite token truncated")?);
    let mut pos = 32 + INVITE_NONCE_LEN;
    let kind = *bytes.get(pos).ok_or("invite token truncated")?;
    pos += 1;
    let (target, expect_len) = match kind {
        1 => {
            let t = ed25519::PublicKey::decode(bytes.get(pos..pos + 32).ok_or("invite token truncated")?)
                .map_err(|e| format!("invite token target: {e}"))?;
            pos += 32;
            (Some(t), INVITE_TOKEN_TARGETED_LEN)
        }
        0 => (None, INVITE_TOKEN_BEARER_LEN),
        other => return Err(format!("unknown invite token kind {other}")),
    };
    if bytes.len() != expect_len {
        return Err(format!("invite token must be {expect_len} bytes for its kind, got {}", bytes.len()));
    }
    let role = InviteRole::from_u8(bytes[pos])?;
    pos += 1;
    let expires_unix_secs = u64::from_le_bytes(bytes[pos..pos + 8].try_into().expect("8 bytes"));
    pos += 8;
    let sig = ed25519::Signature::decode(&bytes[pos..]).map_err(|e| format!("invite token signature: {e}"))?;
    Ok(InviteToken { issuer, nonce, target, role, expires_unix_secs, sig })
}
```

Blob embed (line 556 / 664): length-prefix it —

```rust
// pack_invite:
let tok = pack_invite_token(token);
out.push(u8::try_from(tok.len()).expect("token fits u8"));
out.extend_from_slice(&tok);
// unpack_invite:
let tok_len = r.u8()? as usize;
let token = unpack_invite_token(r.take(tok_len)?)?;
```

Add beside `DEFAULT_INVITE_TTL_DAYS`:

```rust
/// bearer invites default MUCH shorter: anyone holding the blob can redeem
/// it until first use, so the unredeemed-leak window stays hours, not a week.
pub const DEFAULT_BEARER_INVITE_TTL_DAYS: u64 = 1;
```

- [ ] **Step 2: lobby.rs Option handling**

`gate_request` (121) and `intro_request` (326):

```rust
target: token.target.as_ref().map(|t| t.as_ref().to_vec()).unwrap_or_default(),
```

`verify_join_request` (186): decode `target` only when non-empty, keep the check order (signature → lock → proof):

```rust
let target = if target.is_empty() {
    None
} else {
    Some(ed25519::PublicKey::decode(target.as_slice()).map_err(|e| format!("target key: {e}"))?)
};
...
let token = InviteToken { issuer: issuer.clone(), nonce: nonce_arr, target: target.clone(), role, expires_unix_secs: *expires_unix_secs, sig };
if !crate::config::verify_invite_token(&token, binding) {
    return Err("invite token signature does not verify for this network".into());
}
if let Some(t) = &target
    && *t != joiner
{
    return Err("invite is locked to a different key — this invite was minted for someone else".into());
}
```

A bearer token thus verifies crypto-clean, produces `VerifiedJoinRequest { role: Client, .. }`, and dies at the EXISTING role gates: ingress V8 (`RoleUnsupported`, terminal) and the intro doorbell's `msg.role != Resident` check. No resident-path behavior changes.

- [ ] **Step 3: tests (config/invite.rs + lobby.rs in-file suites)**

Fix compile fallout in both files' test modules (they construct `InviteToken` and call `mint_invite_token`). Add:

config/invite.rs tests:
```rust
#[test]
fn a_bearer_token_roundtrips_the_file_and_blob_codecs() {
    let signer = ed25519::PrivateKey::from_seed(1);
    let token = mint_bearer_client_token(&signer, b"net#0@f", 4_102_444_800);
    assert_eq!(token.role, InviteRole::Client);
    assert!(token.target.is_none());
    let packed = pack_invite_token(&token);
    assert_eq!(packed.len(), INVITE_TOKEN_BEARER_LEN);
    assert_eq!(unpack_invite_token(&packed).expect("roundtrip"), token);
}
```
plus a full `encode_invite`/`decode_invite_at` roundtrip of a bearer blob using whatever descriptor fixture the file's existing blob tests use (copy the neighboring test's setup).

lobby.rs tests:
```rust
#[test]
fn a_bearer_token_at_the_gate_verifies_as_client_and_hits_the_role_gate() {
    let issuer = ed25519::PrivateKey::from_seed(1);
    let joiner = ed25519::PrivateKey::from_seed(2);
    let token = crate::config::mint_bearer_client_token(&issuer, BINDING, u64::MAX);
    let msg = gate_request(&joiner, BINDING, &token);
    // crypto passes — ANY key may claim a bearer token —
    let verified = verify_join_request(&msg, BINDING).expect("bearer verifies");
    // — but it comes out role=Client, which V8 (ingress) and the intro
    // doorbell terminally refuse: no bearer path onto the resident plane.
    assert_eq!(verified.role, InviteRole::Client);
}
```

- [ ] **Step 4: run gates**

Run: `cargo test -p node-bin --lib && cargo clippy -p node-bin --tests --no-deps`
Expected: lib tests PASS (bins/e2e still broken until Task 5 — acceptable mid-task; if `--lib` doesn't cover these in-file test modules in this package layout, use `cargo test -p node-bin --bin ducktape-node` instead and let cli.rs errors drive Task 5's shape).

- [ ] **Step 5: commit**

```bash
git add bin/node/src/config/invite.rs bin/node/src/lobby.rs
git commit -m "node: bearer token codec + lobby optional-target handling"
```

---

### Task 5: CLI — `invite --client`, `join` refuses client blobs

**Files:**
- Modify: `bin/node/src/cli.rs` (`cmd_invite` ~541, `cmd_join` target check ~1573)
- Check-compile: `bin/node/tests/coordinated_invite_cli.rs`, `bin/node/tests/common/mod.rs` (they drive the CLI; fix any fallout, no behavior change)

**Interfaces:**
- Consumes: `mint_bearer_client_token`, `DEFAULT_BEARER_INVITE_TTL_DAYS` (Task 4).
- Produces: `ducktape-node invite --client [--ttl-days N]` → bearer Client blob on stdout (last line, same as today). `ducktape-node join <client-blob>` → hard error naming `user-redeem-invite`.

- [ ] **Step 1: cmd_invite**

Replace the unconditional `--target` requirement:

```rust
let bearer_client = flags.contains_key("client");
let ttl_days: u64 = match flags.get("ttl-days") {
    Some(v) => v.parse().map_err(|e| format!("--ttl-days {v:?}: {e}"))?,
    None if bearer_client => config::DEFAULT_BEARER_INVITE_TTL_DAYS,
    None => config::DEFAULT_INVITE_TTL_DAYS,
};
...
let token = if bearer_client {
    if flags.contains_key("target") {
        return Err("--client mints a bearer invite — it has no --target; drop one of the flags".into());
    }
    config::mint_bearer_client_token(&key, binding, expires)
} else {
    let target = flags.get("target").ok_or(
        "--target <invitee-pubkey-hex> is required: every resident invite is locked to \
         the person it admits (mint a bearer CLIENT invite with --client). the invitee \
         gets their code from the app's join screen or `ducktape-node keygen --dir <workspace>`",
    )?;
    let target = config::decode_key(target)?;
    config::mint_invite_token(&key, binding, &target, config::InviteRole::Resident, expires)
};
```

(`binding`/`expires` are whatever names the surrounding code already computes for the existing mint call at ~line 727 — fold in place, keep the descriptor/WG/fronts assembly identical for both forms.) After the blob print, when `bearer_client`, also `eprintln!` a one-line usage hint: redeem with `ducktape-node user-redeem-invite <blob> --node <member-http-url> --key <user.key>`.

- [ ] **Step 2: cmd_join refusal**

Before the existing target comparison (~1573):

```rust
if invite.token.role == config::InviteRole::Client {
    return Err("this is a CLIENT invite — it grants submit access, not a node. \
                redeem it with `ducktape-node user-redeem-invite <blob> --node <member-http-url> --key <user.key>`"
        .into());
}
let Some(invite_target) = invite.token.target.clone() else {
    unreachable!("bearer invites are client-role-only and were rejected above");
};
if invite_target != key.public_key() { ... existing error ... }
```

(mechanically: swap the `invite.token.target != key.public_key()` comparison to use `invite_target`.)

- [ ] **Step 3: compile-sweep the CLI-driving tests**

Run: `cargo test -p node-bin --no-run`
Expected: compiles clean (harness `invite()`/`join_friend` still mint targeted Resident — untouched behavior).

- [ ] **Step 4: run the cheap CLI e2e**

Run: `cargo test -p node-bin --test coordinated_invite_cli`
Expected: PASS (targeted flow unchanged end-to-end).

- [ ] **Step 5: commit**

```bash
git add bin/node/src/cli.rs bin/node/tests
git commit -m "cli: invite --client mints bearer client blobs; join refuses them by name"
```

---

### Task 6: `user-redeem-invite` verb

**Files:**
- Modify: `bin/node/src/userkey_cli.rs`

**Interfaces:**
- Consumes: `config::decode_invite`, `config::sign_join_proof`, `load_user_signer` (file-local), blob shapes from Task 4; noded `SubmitRequest { target, payload, origin? }` / `SubmitReceipt { height, .. }` JSON.
- Produces: `ducktape-node user-redeem-invite <blob> --node <http-base> --key <path>` — stdin: password line only when the key file is v2-encrypted. Stdout last line on success: `admitted: client standing committed at height <h>`.

- [ ] **Step 1: implement the verb**

Dispatch table entry:
```rust
"user-redeem-invite" => cmd_user_redeem_invite(args, &mut stdin),
```

Core (same core/wrapper split as the file's other verbs):

```rust
/// `user-redeem-invite <blob> --node <http-base> --key <path>` — stdin:
/// [password line when the key is v2-encrypted]. redeems a CLIENT invite as
/// this user key over the member node's frameless `/v1/submit` (the token,
/// not the submitter, authorizes admission — the node stamps its own key as
/// the frame origin and consensus verifies the token + proof inside the op).
/// prints the committed height; treats "already holds client standing" as
/// idempotent success.
fn user_redeem_invite(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    let [blob] = pos.as_slice() else {
        return Err("user-redeem-invite needs exactly one <invite blob>".into());
    };
    let node = flags
        .get("node")
        .ok_or("user-redeem-invite needs --node <http-base, e.g. http://host:port>")?
        .trim_end_matches('/')
        .to_string();
    let key_path = PathBuf::from(flags.get("key").ok_or("user-redeem-invite needs --key <path>")?);

    let invite = config::decode_invite(blob)?; // fail-closed expiry at decode
    if invite.token.role != config::InviteRole::Client {
        return Err("this is a node (resident) invite — use `ducktape-node join`".into());
    }
    let user = load_user_signer(&key_path, stdin)?;
    if let Some(target) = &invite.token.target
        && *target != user.public_key()
    {
        return Err(format!(
            "this invite is locked to a different key.\n  invite target: {}\n  this key: {}",
            hex_bytes(target.as_ref()),
            hex_bytes(user.public_key().as_ref()),
        )
        .into());
    }
    let binding = invite.descriptor.genesis_namespace();
    let proof = config::sign_join_proof(&user, binding.as_bytes(), &invite.token);

    let payload = serde_json::json!({ "redeem": {
        "issuer": invite.token.issuer.as_ref().to_vec(),
        "nonce": invite.token.nonce.to_vec(),
        "token_sig": invite.token.sig.encode().as_ref().to_vec(),
        "joiner": user.public_key().as_ref().to_vec(),
        "proof": proof.encode().as_ref().to_vec(),
        "target": invite.token.target.as_ref().map(|t| t.as_ref().to_vec()).unwrap_or_default(),
        "role": invite.token.role.as_u8(),
        "expires_unix_secs": invite.token.expires_unix_secs,
    }});
    let resp = reqwest::blocking::Client::new()
        .post(format!("{node}/v1/submit"))
        .json(&serde_json::json!({ "target": "governance", "payload": payload }))
        .send()
        .map_err(|e| format!("POST {node}/v1/submit: {e}"))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if status.is_success() {
        let height = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v["height"].as_u64())
            .ok_or_else(|| format!("unexpected submit receipt: {body}"))?;
        return Ok(format!("admitted: client standing committed at height {height}"));
    }
    if body.contains("already holds client standing") {
        return Ok("already admitted: this key holds client standing".into());
    }
    Err(format!("redemption rejected ({status}): {body}").into())
}
```

(Import `serde_json` if the file lacks it; `reqwest` is already a `node-bin` dependency. If the receipt shape differs, `SubmitReceipt` is defined in `bin/noded/src/lib.rs` ~446 — match its field names.)

- [ ] **Step 2: unit-test the offline refusal paths**

In the file's test module, mint blobs with the Task 4/5 helpers (the tests there already build configs/descriptors — copy a neighboring fixture): (a) resident blob → "use `ducktape-node join`" error; (b) targeted CLIENT blob for another key → "locked to a different key" error. The happy path is Task 8's e2e (needs a live node).

- [ ] **Step 3: gates**

Run: `cargo test -p node-bin --bin ducktape-node && cargo clippy -p node-bin --tests --no-deps`
Expected: PASS.

- [ ] **Step 4: commit**

```bash
git add bin/node/src/userkey_cli.rs
git commit -m "cli: user-redeem-invite — redeem a client invite over /v1/submit"
```

---

### Task 7: simnode pins (standing gate)

**Files:**
- Modify: `bin/simnode/tests/governance_scenarios.rs` (`mint_as` ~68, `redeem` ~100, new pins after B5d ~455)

**Interfaces:**
- Consumes: wire shapes only (the suite deliberately re-states the preimage).

- [ ] **Step 1: update the re-stated preimage (kind byte) and add the bearer helpers**

`mint_as`: insert the kind byte —
```rust
let msg = [
    binding,
    nonce.as_slice(),
    &[1u8],                    // targeted kind (the new preimage)
    target.as_ref(),
    &[role.as_u8()],
    &expires_unix_secs.to_le_bytes(),
]
.concat();
```

Add:
```rust
/// bearer mint — the new preimage kind 0x00, NO target bytes. re-stated on purpose.
fn mint_bearer(issuer: &Ed, binding: &[u8], nonce: [u8; INVITE_NONCE_LEN], role: InviteRole) -> InviteToken { ... }

/// the bearer redeem op: target = EMPTY bytes.
fn redeem_bearer(token: &InviteToken, joiner: Vec<u8>, proof: Vec<u8>) -> Value {
    json!({ "redeem": { ..., "target": Vec::<u8>::new(), ... }})
}
```
(`InviteToken.target` is `Option` now — `mint_bearer` sets `None`; `mint_as` wraps `Some`. The existing `redeem()` helper reads `token.target` — change it to `token.target.as_ref().expect("targeted").as_ref().to_vec()`.)

- [ ] **Step 2: add pins B5e/B5f/B5g**

- **B5e** `a_bearer_client_invite_grants_client_standing_to_the_first_redeemer_only`: bearer Client mint; key A `submit_ok` → clients set has A, residents/validators don't; key B same-nonce redeem with B's own proof → `submit_rejected` contains `"already redeemed"`; clients set does NOT contain B.
- **B5f** `a_bearer_resident_invite_is_rejected_as_client_only`: bearer Resident (valid sig) → `submit_rejected` contains `"bearer invites are client-only"`; no standing of any tier.
- **B5g** existing B5b/B5c/B5d still pass with the updated helper (run, don't rewrite).

- [ ] **Step 3: run the standing gate**

Run: `cargo test -p simnode && cargo clippy -p simnode --tests --no-deps`
Expected: PASS (the suite exercises the WASM governance component — this is also the proof Task 3's regen actually took).

- [ ] **Step 4: commit**

```bash
git add bin/simnode/tests/governance_scenarios.rs
git commit -m "simnode: new-preimage mint helpers + bearer client invite pins"
```

---

### Task 8: e2e — mint `--client` → redeem over live HTTP

**Files:**
- Create: `bin/node/tests/bearer_client_e2e.rs`

**Interfaces:**
- Consumes: `common::{NetworkShapeCluster, poll_until, serial}` (`new/init_founder/spawn/config_file/query` — see `bin/node/tests/common/mod.rs:110-410`), `CARGO_BIN_EXE_ducktape-node`, clients query wire (`clients::{ClientsQuery, ClientsReply}`).

- [ ] **Step 1: write the e2e**

```rust
//! bearer client invite, end to end over a real HTTP surface: mint with
//! `invite --client`, redeem with `user-redeem-invite` as a fresh user key,
//! observe client standing in consensus, and pin single-use first-wins for a
//! second key on the same blob.

mod common;

use std::process::Command;
use std::time::Duration;

use common::{NetworkShapeCluster, poll_until, serial};

const FINALIZE: Duration = Duration::from_secs(60);

fn clients_contains(cluster: &NetworkShapeCluster, key_hex: &str) -> bool {
    use clients::{ClientsQuery, ClientsReply};
    let req = clients::encode_query(&ClientsQuery::Clients);
    let Some(raw) = cluster.query(0, "clients", &req) else { return false };
    let Ok(ClientsReply::Clients { clients }) = clients::decode_reply(&raw) else { return false };
    clients.iter().any(|c| hex::encode(c) == key_hex)
}

#[test]
fn a_bearer_client_invite_redeems_over_http_once() {
    let _guard = serial();
    let mut cluster = NetworkShapeCluster::new();
    cluster.init_founder("bearer-e2e");
    cluster.spawn(0);

    // founder serves /v1/submit on its --http port once the node is up.
    let http = format!("http://127.0.0.1:{}", cluster.http_ports[0]);
    poll_until("founder http up", FINALIZE, || {
        reqwest::blocking::get(format!("{http}/v1/status")).ok().map(|_| ())
    });

    // mint a bearer client blob — no --target.
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["invite", "--client", "--config"])
        .arg(cluster.config_file(0))
        .output()
        .expect("run invite --client");
    assert!(out.status.success(), "invite --client: {}", String::from_utf8_lossy(&out.stderr));
    let blob = String::from_utf8_lossy(&out.stdout).trim().lines().last().expect("blob").to_string();

    // key A redeems: load_user_signer auto-generates a plain identity at a
    // fresh --key path, so no stdin dance is needed.
    let key_a = cluster.dir.path().join("client-a.key");
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["user-redeem-invite", &blob, "--node", &http, "--key"])
        .arg(&key_a)
        .output()
        .expect("run user-redeem-invite");
    assert!(out.status.success(), "redeem A: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("admitted: client standing committed at height"), "{stdout}");

    // standing is queryable in the clients module.
    let pub_a = {
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
            .args(["keygen", "--out"]).arg(&key_a).output().expect("keygen reuse");
        String::from_utf8_lossy(&out.stdout).trim().to_string() // reuses, prints pubkey
    };
    poll_until("client standing committed", FINALIZE, || clients_contains(&cluster, &pub_a).then_some(()));

    // single-use first-wins: key B on the SAME blob is refused.
    let key_b = cluster.dir.path().join("client-b.key");
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["user-redeem-invite", &blob, "--node", &http, "--key"])
        .arg(&key_b)
        .output()
        .expect("run second redeem");
    assert!(!out.status.success(), "second redeem must fail");
    let err = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(err.contains("already redeemed"), "single-use: {err}");
}
```

(Adjust to the harness's real field visibility: if `http_ports`/`dir` are private, add `pub fn http_url(&self, idx: usize) -> String` and `pub fn scratch(&self) -> &std::path::Path` accessors to `common/mod.rs` rather than making fields public. `hex` and `reqwest` are already dev-deps of node-bin — check `[dev-dependencies]`, add if missing.)

- [ ] **Step 2: run it**

Run: `cargo test -p node-bin --test bearer_client_e2e -- --nocapture`
Expected: PASS in ~1-2 min (single node, no mesh formation).

- [ ] **Step 3: commit**

```bash
git add bin/node/tests
git commit -m "e2e: bearer client invite over live http — admit once, refuse the second key"
```

---

### Task 9: gates sweep + PR

- [ ] **Step 1: full touched-crate gate sweep (non-vacuous)**

```bash
find crates/system/governance/src bin/node/src bin/simnode -name '*.rs' -newer Cargo.toml -exec touch {} + 2>/dev/null
cargo clippy -p governance -p node-bin -p simnode --tests --no-deps
cargo test -p governance -p simnode
cargo test -p node-bin --lib --bins
cargo test -p node-bin --test bearer_client_e2e --test coordinated_invite_cli --test invite_e2e -- --skip live_quorum_admits_a_fourth_validator
make wasm-modules-check
cargo test -p host --test wasm_governance_parity
```
Expected: all green. (`live_quorum_admits_a_fourth_validator` is broken on pristine dev — pre-existing, skip by name and say so in the PR.)

- [ ] **Step 2: fmt touched files only**

```bash
cargo fmt -p governance -- crates/system/governance/src/invite.rs  # or rustfmt the touched files directly
git diff --stat  # confirm no unrelated reformat
```

- [ ] **Step 3: push + PR against dev**

```bash
git push -u origin feat/bearer-client-invites
gh pr create --base dev --title "Bearer client invites: single-use, /v1/submit redemption" --body "<summary: spec link, wire flag day (the new preimage + blob codec), wasm regen, what's out of scope (PR9/PR10 app UX), gates run + the pre-existing live_quorum skip>

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```

- [ ] **Step 4: clean-context review**

Dispatch a fresh-context review of the PR diff (scope creep, missing verification, the redeem reject-string contract, wasm bytes match source) before declaring done; leave the PR open with findings if confidence is not high.

---

## Self-Review Notes

- Spec coverage: §1 tokens → Task 1/4; §2 consensus + wasm → Tasks 2/3; §3 mint/guards → Tasks 4/5; §4 client verb → Task 6; §6 testing → Tasks 1,2,4,6,7,8; §7 out-of-scope honored (no app changes, no lobby gate change).
- Wire strings are contracts: `"bearer invites are client-only"`, `"already redeemed"`, `"already holds client standing"`, `"invite is locked to another key"` — Tasks 2/6/7/8 all match on them; do not rephrase one without the others.
- the new preimage is re-stated in four places BY DESIGN (governance/invite.rs authoritative; bin/node mint; simnode helper; governance native-test helper) — drift anywhere fails a suite.
