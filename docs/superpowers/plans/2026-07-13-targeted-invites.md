# Mandatory Targeted Invites Implementation Plan (PR3)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every invite is minted against the invitee's public key (bearer invites removed), carries a `role` byte for the future thin-client plane, and its expiry moves into the token so consensus enforces it.

**Architecture:** `InviteToken` changes IN PLACE: `{ issuer, nonce, target, role, expires_unix_secs, sig }`, sig over `binding ‖ nonce ‖ target ‖ role ‖ expires`. Everything downstream follows mechanically: blob pack/unpack, lobby announce/intro, `GovMsg::Redeem`, `handle_redeem` (target match + block-time expiry + role gate). UX closes the key-exchange loop: a `keygen` verb / "join code" screen pre-generates the joiner identity that `cmd_join` already reuses (`load_or_generate_identity`, `cli.rs:1261`).

**Tech Stack:** Rust (`crates/system/governance`, `bin/node`), Tauri commands, React console.

**Spec:** `docs/superpowers/specs/2026-07-13-coordinator-invites-thin-client-design.md` (Design 3).

## Global Constraints

- Branch from `origin/dev` AFTER PR1 and PR2 land; worktree `<primary-checkout>/.worktree/targeted-invites`; PR against `dev`.
- **HARD CUTOVER (user mandate — no backward compatibility):** no namespace bumps, no format versioning, no legacy decode. Old invites and old binaries stop working; the whole valset updates together before the first post-cutover redeem. Say so in the PR body.
- Signature preimages: token = `binding ‖ nonce ‖ target ‖ [role:u8] ‖ expires_unix_secs.to_le_bytes()`. Join proof preimage is UNCHANGED (`binding ‖ nonce ‖ joiner`).
- `expires` unit is unix SECONDS end to end. `Env.consensus_time` units must be confirmed once (Task 3 Step 1) and converted at the comparison if the chain stamps millis.
- Gates per touched crate via `ops/build-with.sh cargo clippy -p <crate> --tests --no-deps`; no `cargo fmt --all`.
- Role is `Resident = 0` only in practice this PR; `Client = 1` exists on the wire and is REJECTED at redeem ("client invites are not redeemable yet") so the thin-client plan needs no second invite-format change.

---

### Task 1: Token core — target, role, expiry in `governance::invite`

**Files:**
- Modify: `crates/system/governance/src/invite.rs` (whole module — it is 115 lines)

**Interfaces:**
- Consumes: nothing new.
- Produces (every later task builds on these exact shapes):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum InviteRole { Resident = 0, Client = 1 }
impl InviteRole {
    pub fn from_u8(b: u8) -> Result<Self, String>;
    pub fn as_u8(self) -> u8;
}

pub struct InviteToken {
    pub issuer: ed25519::PublicKey,
    pub nonce: [u8; INVITE_NONCE_LEN],
    /// the ONE key this invite admits — mandatory, no bearer invites.
    pub target: ed25519::PublicKey,
    pub role: InviteRole,
    /// unix seconds; enforced at decode (joiner), announce (member), and
    /// in-consensus (redeem vs consensus_time).
    pub expires_unix_secs: u64,
    pub sig: ed25519::Signature,
}

fn grant_preimage(binding: &[u8], nonce: &[u8], target: &ed25519::PublicKey, role: InviteRole, expires: u64) -> Vec<u8>; // private helper
pub fn verify_invite_token(token: &InviteToken, binding: &[u8]) -> bool; // same name, new preimage
// sign_join_proof / verify_join_proof: signatures unchanged.
```

- [ ] **Step 1: Update the module test to the new shape (failing first)**

```rust
#[test]
fn a_token_binds_network_target_role_and_expiry() {
    let issuer = ed25519::PrivateKey::from_seed(1);
    let target = ed25519::PrivateKey::from_seed(2);
    let token = mint(&issuer, BINDING, &target.public_key());
    assert!(verify_invite_token(&token, BINDING));
    assert!(!verify_invite_token(&token, b"other-net"));

    // tampering ANY covered field kills the signature.
    let mut t = token.clone();
    t.target = ed25519::PrivateKey::from_seed(3).public_key();
    assert!(!verify_invite_token(&t, BINDING));
    let mut t = token.clone();
    t.role = InviteRole::Client;
    assert!(!verify_invite_token(&t, BINDING));
    let mut t = token.clone();
    t.expires_unix_secs += 1;
    assert!(!verify_invite_token(&t, BINDING));
}
```

with the test-local `mint` updated to:

```rust
fn mint(issuer: &ed25519::PrivateKey, binding: &[u8], target: &ed25519::PublicKey) -> InviteToken {
    let nonce = [7u8; INVITE_NONCE_LEN];
    let (role, expires) = (InviteRole::Resident, 4_102_444_800); // 2100-01-01
    let msg = grant_preimage(binding, &nonce, target, role, expires);
    InviteToken {
        issuer: issuer.public_key(),
        nonce,
        target: target.clone(),
        role,
        expires_unix_secs: expires,
        sig: issuer.sign(INVITE_GRANT_NAMESPACE, &msg),
    }
}
```

- [ ] **Step 2: Run to verify failure** — `ops/build-with.sh cargo test -p governance invite::` → compile FAIL.

- [ ] **Step 3: Implement**

```rust
fn grant_preimage(
    binding: &[u8],
    nonce: &[u8],
    target: &ed25519::PublicKey,
    role: InviteRole,
    expires: u64,
) -> Vec<u8> {
    [
        binding,
        nonce,
        target.as_ref(),
        &[role.as_u8()],
        &expires.to_le_bytes(),
    ]
    .concat()
}
```

`verify_invite_token` verifies over `grant_preimage(binding, &token.nonce, &token.target, token.role, token.expires_unix_secs)`. The doc-comment at the top of the file updates: "a token is the issuer's signature over `binding ‖ nonce ‖ target ‖ role ‖ expiry` — minting IS the admission decision FOR THAT KEY." Namespaces stay exactly as they are (in-place mandate — no `-v2`).

- [ ] **Step 4: Run** — `ops/build-with.sh cargo test -p governance invite::` → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(governance)!: invite tokens bind target key, role, and expiry in place"`

---

### Task 2: Lobby announce + intro carry the full token

**Files:**
- Modify: `bin/node/src/lobby.rs` (JoinRequest :27-33, IntroRequest :162-172, `join_request` :70-85, `verify_join_request` :100-141, `intro_request` :198-218, `verify_intro` :234-266, tests)
- Modify: `bin/node/src/config/invite.rs` (`mint_invite_token` :44-53, `pack_invite_token`/`unpack_invite_token` :58-80, `INVITE_TOKEN_LEN` :56)

**Interfaces:**
- Consumes: Task 1 token.
- Produces:
  - `LobbyMsg::JoinRequest` / `IntroRequest` gain `target: Vec<u8>, role: u8, expires_unix_secs: u64` (raw-bytes idiom of the file).
  - `VerifiedJoinRequest` gains `role: InviteRole, expires_unix_secs: u64` (target is NOT carried separately — verification enforces `target == joiner`, so `joiner` IS the target downstream).
  - `verify_join_request` errs with `"invite is locked to a different key"` on mismatch (BEFORE the proof check, so the error names the real problem).
  - `mint_invite_token(signer, binding, target: &ed25519::PublicKey, role: InviteRole, expires_unix_secs: u64) -> InviteToken`.
  - `INVITE_TOKEN_LEN = 32 + 16 + 32 + 1 + 8 + 64` (= 153); pack order: `issuer ‖ nonce ‖ target ‖ role ‖ expires_le ‖ sig`.

- [ ] **Step 1: Failing tests** — update `a_join_request_roundtrips_and_verifies` (mint with a target = the joiner), and add:

```rust
#[test]
fn a_non_target_key_is_refused_by_name() {
    let issuer = ed25519::PrivateKey::from_seed(1);
    let target = ed25519::PrivateKey::from_seed(2);
    let thief = ed25519::PrivateKey::from_seed(3);
    let token = mint_invite_token(&issuer, BINDING, &target.public_key(), InviteRole::Resident, u64::MAX);
    // the thief holds the blob and announces under its OWN key with a VALID
    // self-proof — exactly the bearer hole this feature closes.
    let msg = join_request(&thief, BINDING, &token);
    let err = verify_join_request(&msg, BINDING).expect_err("refused");
    assert!(err.contains("locked to a different key"), "{err}");
    // the real target still verifies.
    let msg = join_request(&target, BINDING, &token);
    assert!(verify_join_request(&msg, BINDING).is_ok());
}
```

Mirror the same case for `verify_intro`.

- [ ] **Step 2: Run to verify failure**, then implement. `verify_join_request` order: decode fields → rebuild token → `verify_invite_token` (kills tampered target/role/expiry) → **`token.target != joiner` → Err("invite is locked to a different key — this invite was minted for someone else")** → proof-of-possession. NO expiry check here (that is the member's and consensus' job with their clocks; the lobby fn stays pure crypto — same division as membership checks).

- [ ] **Step 3: Run** — `ops/build-with.sh cargo test -p ducktape-node lobby invite` → PASS.
- [ ] **Step 4: Commit** — `git commit -m "feat(node)!: lobby announce and intro carry the targeted token verbatim"`

---

### Task 3: Consensus — `Redeem` enforces target, expiry, role

**Files:**
- Modify: `crates/system/governance/src/interface.rs` (`GovMsg::Redeem` :131-137 — add `target: Vec<u8>, role: u8, expires_unix_secs: u64`)
- Modify: `crates/system/governance/src/lib.rs` (`handle_redeem` :1056-1138, `execute` match :1178-1187)
- Modify: `crates/system/governance/tests/invite_redemption.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces: deterministic rejects `"invite is locked to another key"`, `"invite expired"`, `"client invites are not redeemable yet"`; unchanged `Grant` emission for a valid Resident redeem.

- [ ] **Step 1: Pin the `consensus_time` unit**

Run: `grep -rn "consensus_time" crates/kernel/ bin/node/src | grep -v test | head` and read the stamping site (where `BlockContext`/`Env.consensus_time` is filled from the proposal timestamp). If it is millis, compare as `ctx.env().consensus_time >= expires_unix_secs * 1000` and leave a one-line comment naming the unit at the comparison. Do not guess — read the stamp.

- [ ] **Step 2: Failing tests** in `invite_redemption.rs`. The rig is already there: `gov_host()` (members = keypairs 1 and 2), `submit_as(host, who, at, payload)` — where `at` lands verbatim in `BlockContext.consensus_time` — plus `residents()`/`redemptions()` readers. Update the file's `mint` helper to the new token and add the cases:

```rust
/// mint as `issuer`, locked to `target`, with explicit role and expiry
/// (fixed nonce — tests need determinism).
fn mint_for(
    issuer: &PrivateKey,
    nonce_byte: u8,
    target: &PrivateKey,
    role: InviteRole,
    expires: u64,
) -> InviteToken {
    let nonce = [nonce_byte; INVITE_NONCE_LEN];
    let msg = grant_preimage_for_tests(BINDING, &nonce, &target.public_key(), role, expires);
    InviteToken {
        issuer: issuer.public_key(),
        nonce,
        target: target.public_key(),
        role,
        expires_unix_secs: expires,
        sig: issuer.sign(INVITE_GRANT_NAMESPACE, &msg),
    }
}
// grant_preimage is private to governance::invite — either make it pub(crate)
// + expose a test constructor, or (lazier, honest) re-state the preimage here:
// [binding, nonce, target, &[role.as_u8()], &expires.to_le_bytes()].concat().
// re-stating in the test is the stronger pin: a preimage drift then FAILS here.

// redeem_msg gains the new fields verbatim from the token:
fn redeem_msg(token: &InviteToken, joiner: &PrivateKey) -> Vec<u8> {
    let proof = sign_join_proof(joiner, BINDING, token);
    gov_encode(&GovMsg::Redeem {
        issuer: token.issuer.as_ref().to_vec(),
        nonce: token.nonce.to_vec(),
        token_sig: token.sig.encode().as_ref().to_vec(),
        joiner: key_bytes(joiner),
        proof: proof.encode().as_ref().to_vec(),
        target: token.target.as_ref().to_vec(),
        role: token.role.as_u8(),
        expires_unix_secs: token.expires_unix_secs,
    })
}

#[test]
fn a_targeted_token_admits_only_its_target_and_only_before_expiry() {
    block_on(async {
        let mut host = gov_host();
        let issuer = keypair(1);
        let target = keypair(7);
        let thief = keypair(8);

        // the thief presents a valid self-proof for the target's token → reject.
        let token = mint_for(&issuer, 1, &target, InviteRole::Resident, 1_000);
        let err = submit_as(&mut host, &key_bytes(&thief), 10, redeem_msg(&token, &thief))
            .await
            .expect_err("locked");
        assert!(format!("{err:?}").contains("locked to another key"), "{err:?}");

        // the TARGET redeems before expiry → resident standing.
        submit_as(&mut host, &key_bytes(&target), 10, redeem_msg(&token, &target))
            .await
            .expect("target redeems");
        assert!(residents(&host).await.contains(&key_bytes(&target)));

        // a second targeted token already past consensus_time → expired.
        let stale = mint_for(&issuer, 2, &thief, InviteRole::Resident, 1_000);
        let err = submit_as(&mut host, &key_bytes(&thief), 1_000, redeem_msg(&stale, &thief))
            .await
            .expect_err("expired at consensus_time == expiry");
        assert!(format!("{err:?}").contains("expired"), "{err:?}");
    });
}

#[test]
fn a_client_role_token_is_not_redeemable_yet() {
    block_on(async {
        let mut host = gov_host();
        let issuer = keypair(1);
        let client = keypair(9);
        let token = mint_for(&issuer, 3, &client, InviteRole::Client, u64::MAX);
        let err = submit_as(&mut host, &key_bytes(&client), 10, redeem_msg(&token, &client))
            .await
            .expect_err("client role gated");
        assert!(format!("{err:?}").contains("not redeemable yet"), "{err:?}");
        assert!(residents(&host).await.is_empty());
    });
}
```

Existing tests in the file update mechanically (`mint(issuer, n)` → `mint_for(issuer, n, &joiner, InviteRole::Resident, u64::MAX)`); the single-use replay case must stay green with the TARGET replaying its own nonce.

- [ ] **Step 3: Implement `handle_redeem`**

After decoding issuer/joiner/nonce/sig (existing code), decode the new fields and rebuild the token:

```rust
let target_key = ed25519::PublicKey::decode(target.as_slice())
    .map_err(|e| Error::Module(format!("target key: {e}")))?;
let role = invite::InviteRole::from_u8(role).map_err(Error::Module)?;
let token = invite::InviteToken {
    issuer: issuer_key,
    nonce: nonce_arr,
    target: target_key,
    role,
    expires_unix_secs,
    sig,
};
if !invite::verify_invite_token(&token, binding) { /* existing reject */ }
if joiner != target {
    return Err(Error::Module("invite is locked to another key".into()));
}
// agreed block time, NOT wall clock — every validator settles identically.
if ctx.env().consensus_time >= token.expires_unix_secs {
    return Err(Error::Module("invite expired".into()));
}
if token.role == invite::InviteRole::Client {
    return Err(Error::Module(
        "client invites are not redeemable yet — the thin-client plane lands separately".into(),
    ));
}
```

(`joiner != target` compares the raw `Vec<u8>` args — cheap and exact.) Everything after (proof, membership, nonce single-use, grant) stays as is.

- [ ] **Step 4: Run** — `ops/build-with.sh cargo test -p governance` → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(governance)!: redeem enforces target key, block-time expiry, and role gate"`

---

### Task 4: Blob + CLI — `invite --target`, `keygen`, join-side self-check

**Files:**
- Modify: `bin/node/src/config/invite.rs` (`encode_invite` :362-378 — drop the separate `expires_unix_secs` param, the token carries it; `pack_invite` :440-530 — drop the expiry field write at :506, keep writing the (now 153-byte) token; `unpack_invite` :546-666 — drop the expiry read, enforce expiry from `token.expires_unix_secs` after unpacking the token; update every module test)
- Modify: `bin/node/src/cli.rs` (`cmd_invite` :198+ — required `--target`; new `cmd_keygen`; `cmd_join` :1174+ — self-check; verb dispatch table — find with `grep -n '"invite"\|"join"' bin/node/src/cli.rs`)
- Modify: usage/help text wherever the verb list is printed.

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces:
  - `encode_invite(descriptor, token, wireguard, fronts, signer)` (5 args — expiry comes from the token).
  - `ducktape-node keygen --dir <dir>` → prints the identity pubkey hex (creates `<dir>/identity.key` if absent, reuses otherwise — this IS the join code).
  - `ducktape-node invite --config … --target <hex-pubkey> [--ttl-days N] [--short]` — `--target` REQUIRED, with the error message telling the inviter where the invitee finds their code.
  - `cmd_join` refuses early when the local identity ≠ token target.

- [ ] **Step 1: Failing blob tests** — update `encode_test_invite` helpers to mint with a target and thread the new `encode_invite` arity; the expiry test in `a_tampered_or_expired_or_stale_prefix_invite_is_refused` (:1024-1063) now mints a token with `expires = 1_000` instead of passing `1_000` to `encode_invite`, and must still refuse at `decode_invite_at(&blob, 1_000)`.

- [ ] **Step 2: Implement blob changes**, run `ops/build-with.sh cargo test -p ducktape-node invite` → PASS.

- [ ] **Step 3: CLI**

`cmd_keygen` (new, ~15 lines):

```rust
/// `keygen --dir <dir>` — mint (or reuse) the workspace identity and print
/// its public key: the JOIN CODE an invitee hands the inviter, so the
/// invite can be locked to this key before the workspace joins anything.
fn cmd_keygen(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let dir = PathBuf::from(flags.get("dir").map(String::as_str).unwrap_or("."));
    std::fs::create_dir_all(&dir)?;
    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    eprintln!("{} identity", if generated { "generated" } else { "reusing" });
    println!("{}", hex_bytes(key.public_key().as_ref()));
    Ok(())
}
```

`cmd_invite`: parse `--target` (required):

```rust
let target = flags
    .get("target")
    .ok_or(
        "--target <invitee-pubkey-hex> is required: every invite is locked to \
         the person it admits. the invitee gets their code from the app's \
         join screen or `ducktape-node keygen --dir <workspace>`",
    )?;
let target = config::decode_key(target)?;
```

mint: `config::mint_invite_token(&key, binding, &target, InviteRole::Resident, expires)` (expiry computed exactly as today at :363-367, now passed into the mint instead of the encode).

`cmd_join`: right after `load_or_generate_identity` (:1261), before writing anything else:

```rust
if invite.token.target != key.public_key() {
    return Err(format!(
        "this invite is locked to a different key.\n  invite target: {}\n  this workspace: {me_hex}\n\
         hand the inviter THIS key (the join code) and ask for a fresh invite.",
        hex_bytes(invite.token.target.as_ref()),
    )
    .into());
}
```

Note: `load_or_generate_identity` runs at :1261, after descriptor/config writes — MOVE the identity step up to just after `decode_invite` (:1179) so a target mismatch aborts before the join touches the directory shape. (`create_dir_all` + identity write are safe to keep.)

- [ ] **Step 4: e2e harness + tests ride the new flow**

`bin/node/tests/common/mod.rs`: `NetworkShapeCluster::invite()` (:191) becomes `invite(&self, target_hex: &str)`; add `pub fn keygen_friend(&self, idx: usize) -> String` that runs the `keygen` verb against the friend workspace dir (the dir `join_friend*` would use — mirror its path construction) and returns the printed hex. `join_friend`/`join_friend_manual` keep their signatures (join reuses the pre-generated identity).

Update every caller: `live_admission_e2e.rs`, `coordinated_invite_cli.rs`, and PR1's `invite_reuse_e2e.rs`. The reuse test's meaning shifts: B (a different key) now dies at VERIFY — assert the FATAL line contains `"locked to a different key"` instead of `"invite already redeemed"`; keep the exit assertion. Flow per test: `let code = cluster.keygen_friend(1); let invite = cluster.invite(&code); cluster.join_friend(&invite);`.

Run: `ops/build-with.sh cargo test -p ducktape-node --test live_admission_e2e --test invite_reuse_e2e --test coordinated_invite_cli -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit** — `git commit -m "feat(node)!: mandatory --target invites, keygen join codes, join-side self-check"`

---

### Task 5: App — join code screen + invitee-key field

**Files:**
- Modify: `app/src-tauri/src/workspaces/mod.rs` (new `workspace_join_code` command; `workspace_join_blocking` :386 — adopt staged identity; `workspace_invite_blob*` :486-505 — add `target: String` param, pass `--target`)
- Modify: `app/src-tauri/build.rs` + `app/src-tauri/capabilities/trusted.toml` (register `workspace_join_code` — new commands need both, per repo convention)
- Modify: `app/src/console/store/actions.ts` (`revealInvite` :2792, `joinWorkspace` :2701, new `joinCode` action), `app/src/console/views/MembersView.tsx` (invite section), the onboarding join view (`app/src/console/views/onboarding/` — the form that calls `joinWorkspace`)

**Interfaces:**
- Consumes: `keygen` verb (Task 4).
- Produces: `workspace_join_code() -> String` (pubkey hex from a staging dir); `workspace_invite_blob(id, target)`.

- [ ] **Step 1: Staged join identity (Tauri)**

```rust
/// the invitee's JOIN CODE: pre-mint the identity a future join will use, in
/// a staging dir, and return its pubkey. `workspace_join` adopts the staged
/// key so the code handed to the inviter IS the key the invite locks to.
/// one staging slot: repeat calls reuse the same identity (keygen semantics).
#[tauri::command]
pub async fn workspace_join_code(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
) -> Result<String, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || {
            let staging = workspaces_dir(&app)?.join(".pending-join");
            fs::create_dir_all(&staging).map_err(|e| format!("create {staging:?}: {e}"))?;
            run_verb(&["keygen", "--dir", &staging.to_string_lossy()]).map(|out| last_line(&out))
        })
        .await
}
```

In `workspace_join_blocking`, after `fs::create_dir_all(&dir)` (:406): if `workspaces_dir/.pending-join/identity.key` exists, MOVE it to `dir.join("identity.key")` (`fs::rename`, cross-dir on the same volume) so `cmd_join` reuses it; the staging dir is consumed exactly once.

- [ ] **Step 2: `--target` through the invite command** — `workspace_invite_blob(app, window, control, id, target: String)`: validate non-empty, append `"--target", &target` to the `run_verb` invite args (both the `--short` and fallback invocations from PR2).

- [ ] **Step 3: Console UI**

- Onboarding join view: a "Your join code" block ABOVE the invite-paste field — on mount call `joinCode()` (new action → `workspace_join_code`), render the hex with a copy button and the line "Send this code to whoever is inviting you — invites are locked to it." The paste-and-join flow below is unchanged.
- `MembersView.tsx` invite section: a required "Invitee join code" text input gating the Reveal button (disabled + hint until 64 hex chars); `actions.revealInvite(target)` threads it through.
- `JoinProgress.tsx`: no change — the fatal path (locked-to-different-key) already renders via the FATAL classifier.

- [ ] **Step 4: Live QA + commit**

Fleet QA (skills `qa`): instance B onboarding → copy join code → instance A Members → paste code → reveal short invite → B joins via the short URL → admitted. Negative: A mints an invite for a WRONG code (edit one hex char) → B's join fails fast with the locked-to-different-key message rendered in the join form (the `cmd_join` error surfaces through `workspace_join`'s Result, not the FATAL log path — verify which surface catches it and screenshot).

```bash
git add app/src-tauri app/src/console
git commit -m "feat(app): join codes — invites minted against the invitee's key"
```

---

### Task 6: Gates, cutover note, PR

- [ ] **Step 1: Gates**

```bash
ops/build-with.sh cargo clippy -p governance --tests --no-deps
ops/build-with.sh cargo clippy -p ducktape-node --tests --no-deps
ops/build-with.sh cargo test -p governance
ops/build-with.sh cargo test -p ducktape-node
ops/build-with.sh cargo check -p files --no-default-features   # standing wasm gate, untouched but cheap
cd app && npm run typecheck
```
Expected: all green (touch a `.rs` file first if cargo would serve a cached build).

- [ ] **Step 2: PR against dev**

Title: `feat!: mandatory targeted invites — target key, role byte, in-token expiry (hard cutover)`. Body: the cutover contract (whole valset + all inviter/joiner binaries update together; every pre-cutover invite is dead — re-mint; no compat path by user mandate), the role byte's purpose (thin-client plane, Client redeems rejected until then), the closed holes (bearer redemption by any blob holder; hand-crafted announce reusing a never-expiring token), and the UX loop (join code → targeted invite → self-check on join).
