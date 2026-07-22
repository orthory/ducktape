# Account abstraction at the frame layer, and module view packages

Date: 2026-07-14
Status: design, not yet implemented
Citations: file:line references are anchored to `dev` as of this document's date and drift as the
tree moves — treat them as dated pointers, not live links.

> **UI substrate update — 2026-07-15.** The desktop console is being fully ported from the
> CEF/HTML webview to a native **iced** shell (`app/src-iced/`, branch `feat/iced-app`; ~55k lines,
> in progress in a separate session). This changes **Part A only.**
>
> - **Part B (account abstraction) stands, every line.** It lives at the frame/consensus layer and is
>   entirely UI-agnostic. `authorize()` at the drain, `KeyScope`, passkey-root + session-key, the frame
>   codec change — none of it knows or cares what renders the UI.
> - **Part A's *authority* half stands too** — the install grant, the per-module scoped session key, the
>   `submits` → `KeyScope.targets` mapping. That half was always pure Part B; the DOM was never
>   load-bearing to it.
> - **Part A's *rendering* half is retired.** A native shell has no DOM and no iframe to hand a third
>   party, so the iframe at `duck://<id>.mods.duck`, `origin_guard` as the view boundary, the
>   `postMessage` provider bridge, `provider.js`, and "the browser is the sandbox" are all gone. The
>   16 built-in screens are now hand-written native Rust (`app/src-iced/src/screens/*.rs`, Elm-style
>   `view(&State) -> Element` + `update`). A third-party *view runtime* is deferred, not designed-in;
>   Part A below records its forward shape (a wasm view emitting a host-rendered widget tree) so nobody
>   rebuilds it as an iframe. The pre-pivot Part A is preserved in git history.

## The ask

Today every console module's UI is a folder under `app/src/console/views/<id>/` plus a line in
`app/src/console/modules/registry.ts`, compiled into one Vite bundle and served over `tauri://`.
Adding or changing a module surface means rebuilding and reshipping the desktop app.

We want a module's view to ship *with the module*, as an installable package, so that a module's UI
can change without an app release or a node release, third-party packages can exist, and the view is
pinned in consensus so a node cannot serve a forged UI.

Pulling that thread ended somewhere else: **the blocker is not the view, it is the authorization
model.** A third-party page needs write authority, and ducktape has no way to grant a bounded one.
So the project is account abstraction at the frame layer; module view packages are what it enables.

## What already exists

Six findings drive the whole design. Each one killed a design that looked reasonable on paper.

### 1. `identity` is already account abstraction — everywhere except the frame layer

An account holds a **set** of member keys, cross-scheme:
`KeyKind::{Ed25519, P256, WebauthnP256}` (`crates/system/identity/src/scheme.rs:57-72`).
`verify_authority` (`scheme.rs:172-264`) dispatches per kind — P256 via commonware's namespaced ECDSA
(`secp256r1::standard`, `:243`), WebAuthn via an assertion-envelope verifier (`:264`), with the
`rp_id_hash` pinned in state. `MemberAuth { key, kind, proof }` is the authorization envelope, and
`AddMemberKey` enforces **global key uniqueness** — a key may not be a member of two accounts
(`identity/src/lib.rs:759`).

**This verifier is already deterministic and already runs in consensus.**

But frames are ed25519-only: `decode_frame` (`crates/kernel/node/src/lib.rs:216-228`) decodes the
origin as a raw `ed25519::PublicKey`. So **a passkey can join an account but cannot sign a
transaction.** The hard half of AA is built; it is simply not wired to the frame path.

### 2. `Origin` has no account, so a second key makes you a different person

`sdk::Origin` (`crates/kernel/sdk/src/lib.rs:206-215`) is `External(Vec<u8>) | Module(ModuleId) |
System`, and `actor_string()` returns `"ext:" + hex(raw pubkey)` (`:225-241`). There is **no
pubkey → account resolution anywhere outside identity's own five ops.** Identity keeps
`member_index: BTreeMap<member_pubkey, account_id>` (`identity/src/lib.rs:107`) and exposes
`IdentityQuery::OfMember` (`:567-578`) — and **nobody calls it.**

So enrol your phone, sign a chat message with it, and `chat` sees a **completely different author**
(`crates/apps/chat/src/lib.rs:158`, `AuthorRef::User(raw_pubkey)`). `files` hands it a different
`/home/ext:<hex>` root. **Your phone is a stranger to every module.** This is a live bug independent
of anything else here.

**Consequence:** adding a `scope` field alone would not make session keys usable — a session key's
writes would be attributed to a stranger. Account resolution is the load-bearing half.

### 3. `MemberMeta` has no scope and no expiry. Today's session keys are bearer user-keys.

`MemberMeta { kind, label, rp_id_hash, added_at }` (`identity/src/lib.rs:70-77`). A member key is
all-or-nothing over the account.

Agent session keys already exist and are handed raw to child processes
(`bin/noded/src/agent_provision/session.rs`). Their own header says it:

> whoever holds it can sign ANY `Msg` to ANY module ... consensus gates the ACTION lane ... it does
> not gate the KEY ... a leaked one is a leaked user key, not a spent ticket.

The only scoping that exists is `CLIENT_SIGNABLE_TARGETS`
(`app/src-tauri/src/user_identity.rs:812-815`), a shell-side allowlist whose own doc says
*"Consensus is the real gate ... this list is defense-in-depth."* **There is no real gate.**

### 4. The real key hole is not `user.key` — it is the frameless `/v1/submit` lane

`user.key` is in better shape than its reputation: v2 is argon2id + XChaCha20-Poly1305 at rest,
`0600`, `create_new` (no chmod window) — `bin/node/src/userkey.rs:1-40`, `:375-440`. The private key
**never enters the Tauri app's memory**: signing spawns a short-lived `ducktape-node user-sign-frame`
child with the password on stdin (`app/src-tauri/src/user_identity.rs:832-869`,
`app/src-tauri/src/daemon.rs:320-360`). `bin/noded/src/origin_guard.rs:19-27` claims "a local process
can already read `user.key` off the disk" — **that comment is stale for v2 keys.**

The actual hole is `bin/node/src/validator/run/ingress.rs:532-537`:

```rust
NodeCommand::Submit { origin: _, .. } => node::encode_frame(&self.signer, seq, ...)
```

`/v1/submit` **discards the caller-supplied origin and re-signs with the node's `identity.key`** — the
key that signs validator votes. And the desktop's local workspace takes this lane for everything
except `files` (`app/src/domain/node-bootstrap.ts:94-99` passes no `signPayload`;
`app/src/domain/transport.ts:740-776` then posts to `/v1/submit`).

**So today, no user signature is involved in any non-`files` op on the local desktop path.** Nothing
in this design may use that lane.

### 5. There is exactly one consensus-binding authorization point

`decode_frame` is called in four places. Three verify and then **throw the origin away**
(`bin/noded/src/lib.rs:526` — *"the http layer never tells an actor who signed"*;
`bin/node/src/relay.rs:196`; `crates/kernel/node/src/lib.rs:1019`). Only the member loop of
`drain_delivered` — **`crates/kernel/node/src/lib.rs:1245-1258`** — produces the `Origin` that reaches
`execute` (stamped into `Env` at `crates/kernel/host/src/lib.rs:1435-1441`).

That point has everything a check needs: the verified key, `msg.target` (**bound by the signature** —
`frame_preimage` length-prefixes the target, `node/src/lib.rs:155-157`), and `height` (already computed
at `:1139`). A check anywhere else is advisory. There is precedent for reading committed module state
at ingress — `bin/node/src/host_reads.rs`, used by the relay door at
`bin/node/src/validator/run/ingress.rs:404-412`.

### 6. Modules are already wasm and already consensus-pinned — and that machinery is the wrong tool for views

`modreg` holds a 32-byte `active_code_hash` (`crates/system/modreg/src/lib.rs:58`);
`realize_module_swaps` (`crates/kernel/host/src/lib.rs:806`) verifies `sha256(bytes) == code_hash` and
hot-swaps every block (`crates/kernel/node/src/lib.rs:1201`), proven by
`crates/kernel/host/tests/module_swap.rs`.

That machinery is **R=n height-gated** (governance → all-validator readiness → activation height,
`crates/system/upgrade/src/lib.rs:253-296`) because a code swap moves app-hash. **A stylesheet does
not.** Routing a CSS fix through a governance vote and a full-validator roll would make UI iteration
*slower* than today.

**Rejected:** view bytes or a view hash in `modreg`; views welded into the node binary. Both turn a UI
change into a coordinated network event.

---

## Part B — Account abstraction at the frame layer

B is the project. A rides on it.

### B1. The frame carries a `MemberAuth`, not a raw signature

```
today:  frame = (pubkey, seq, target, payload, ed25519_sig)
AA:     frame = (MemberAuth { key, kind, proof }, seq, target, payload)
```

`MemberAuth` is an existing struct. Because keys are globally unique across accounts
(`identity/src/lib.rs:759`), **the key alone resolves the account** — the frame does not need to carry
an account id.

`decode_frame` becomes a pure decode with no signature verification. Verification moves to the one
place that has account state.

`frame_preimage` keeps its shape (`chain_id ‖ seq ‖ target ‖ payload`) under `FRAME_NS`. Keeping
`FRAME_NS` distinct from `IDENTITY_*_NS` is load-bearing: a `MemberAuth` signed to authorize an
identity op must never be replayable as a frame.

### B2. Authorization is a pure function at the drain

At `crates/kernel/node/src/lib.rs:1248`, before the member loop, snapshot identity's **committed**
member index and metadata via a `host_reads.rs`-style read. One read per block, not per frame.

Committed, not merged: identity's `query` reads pending-over-committed (`identity/src/lib.rs:227-239`),
so a mid-block read would let an `AddMemberKey` earlier in the same batch authorize a later frame in
that batch. Committed-only removes the coupling and is the right semantics anyway — a key added in
block N is usable from block N+1.

```rust
fn authorize(snap: &IdentitySnapshot, f: &Frame, height: u64) -> Result<Origin, Rejected> {
    let Some(account) = snap.account_of(&f.auth.key) else {
        // accountless key: legacy path, full authority, actor stays the raw key
        return Ok(Origin::External(f.auth.key.clone()));
    };
    let meta = snap.meta_of(&f.auth.key).ok_or(Rejected::UnknownKey)?;

    verify_authority(                              // crates/system/identity/src/scheme.rs:186
        meta.kind, &f.auth.key, meta.rp_id_hash.as_deref(),
        FRAME_NS, &frame_preimage(f), &f.auth.proof,
    )?;

    if let Some(s) = &meta.scope {
        if s.expires_at != 0 && height > s.expires_at { return Err(Rejected::Expired) }
        if !s.targets.contains(&f.target)            { return Err(Rejected::OutOfScope) }
    }

    Ok(Origin::External(account))                  // <-- the account, not the key
}
```

**Authorization reads only. It writes nothing.** That is what keeps it out of the host's state path
and lets it be a pure function of a committed snapshot — deterministic on every validator by
construction.

Rejected frames join the existing `decode_fail` disposition: dropped deterministically, recorded in
`op_meta`, no state change.

`verify_authority` is **already written and already runs in consensus.** We are calling it from a new
place, not writing a verifier.

This single function fixes three things at once:

1. **Passkeys can sign transactions** (B4).
2. **Multi-device is fixed** — a second member key resolves to the same actor. Every module keeps
   reading `env.origin.actor_string()`; it simply starts returning a stable account.
3. **Scope is enforced by consensus**, not by the shell.

### B3. `MemberMeta` gains a scope

```rust
// crates/system/identity/src/lib.rs
pub struct MemberMeta {
    pub kind: KeyKind,
    pub label: Option<String>,
    pub rp_id_hash: Option<Vec<u8>>,
    pub added_at: u64,
    pub scope: Option<KeyScope>,   // NEW. None = full authority (existing behavior)
}

pub struct KeyScope {
    pub targets: Vec<ModuleId>,    // may only sign Msgs to these modules
    pub expires_at: u64,           // consensus height; 0 = never
}
```

Targets and expiry only. No per-message-type scoping, no rate limits, no spend caps — each would need
its own consensus semantics and none is needed yet.

`AddMemberKey` (`crates/system/identity/src/interface.rs:99-105`) gains `scope: Option<KeyScope>`.

**`add_member_preimage` (`interface.rs:167-181`) MUST cover the scope.** It currently signs
`chain_id ‖ account_id ‖ new_key ‖ kind_tag ‖ nonce`. Without the scope in the preimage, an attacker
strips the scope from a signed `AddMemberKey` and registers the key with full authority. Append a
canonical scope encoding.

Revocation already exists: `RemoveMemberKey`.

### B4. Passkey as root, session key as everyday signer

This is the payoff, and it is the actual answer to "protect `user.key`". **You do not protect the key;
you take it out of the root position.**

```
passkey (Secure Enclave / TPM, unexfiltratable)
    │  AddMemberKey { scope }   ← user presence, at install / unlock
    ▼
session key (ed25519, in the shell, scoped + expiring)
    │  signs frames silently
    ▼
consensus checks scope at node/src/lib.rs:1248
```

WebAuthn requires user presence per signature — correct for minting authority, wrong for sending a
chat message. That asymmetry is exactly what session keys are for. `user.key` on disk becomes a
**legacy fallback member key, not the root.** Stealing it no longer takes the account, because it can
be revoked by the passkey and it need not exist at all.

`app/src-tauri/src/touchid.rs` today only guards the *vault passphrase* (its header says so, and the
unentitled fallback admits *"a debugger or another local process could read this item"*). Under this
design, Touch ID stops guarding a password and starts gating a real signing key.

### B5. The console gets a session key; the frameless lane stops being the desktop's default

On unlock, the shell mints an ed25519 session key and registers it with
`scope = { targets: CLIENT_SIGNABLE_TARGETS, expires_at: h + N }` — **one user-presence prompt, at
unlock, which the user is already doing.** Every console write afterwards is signed in-process and
posted to `/v1/submit/frame`. No password prompt, no confirm dialog, no validator key.

Strictly better than today on every axis: writes are authored by the user, scoped, expiring, revocable,
and the validator key never touches user traffic. `CLIENT_SIGNABLE_TARGETS` stops being a lie and
becomes the session key's scope.

The frameless `/v1/submit` node-re-signing lane can then be restricted to system callers. Doing so is
**not** in scope here, but nothing in this design may use it.

### B6. Ingress stays advisory; the drain stays authoritative

Moving verification out of `decode_frame` means the three ingress doors lose their check, so garbage
frames could reach consensus ordering. Ingress therefore does a **node-local advisory** verify (same
snapshot read, same `verify_authority`) purely to drop junk early — exactly the shape of the existing
relay door (`bin/node/src/relay.rs:196`, gating on valset/clients standing). It is a DoS filter, not a
gate. **The drain is the gate.**

### B7. What we deliberately do NOT take from account abstraction

- **Paymasters.** There are no fees. Nothing to sponsor.
- **Per-account nonces.** `seq` is signed into the preimage but never enforced
  (`node/src/lib.rs:220` binds it to `_seq`); the only replay guard is the content-digest set
  `FinalizedInner::seen` (`crates/kernel/consensus/src/lib.rs:864`), which its own doc calls a
  placeholder. It nonetheless stops byte-identical replay forever, and without the key an attacker
  cannot produce a different valid frame — so replay is already prevented. Adding a nonce would force
  authorization to **write state** (a system boundary op, modreg's `Advance` being the precedent) and
  buys nothing today. **Keep authorization read-only.** Track separately, along with the stale comment
  at `node/src/lib.rs:1236-1237` which claims modules dedup on `(origin, seq)` — they cannot; modules
  never see `seq`.
- **Multisig / social recovery.** `crates/labs/src/multisig` exists; we do not build it. But because
  verification dispatches on `KeyKind` against account state, a future threshold kind **slots in with
  no frame-layer change.** The extension point is the whole point.
- **Consensus votes stay ed25519.** `ConsensusScheme::V1Ed25519`
  (`crates/kernel/consensus/src/lib.rs:84-109`) is untouched. AA is for *user* frames only. This
  containment keeps the consensus core out of the blast radius.

---

## Part A — Module view packages (native-iced revision)

Under the iframe model A was "small and mostly existing code" — reuse `serve_duckfs`, zero new
rendering. **The native pivot inverts that:** the browser gave rich text, media, and a security origin
for free; a native shell gives none of them to a third party, so A's rendering half becomes real new
work. The *authority* half (A4) is unaffected — it was always Part B. What follows separates the two:
the authority half is landable now with B; the rendering half is the forward design, deferred until a
third-party view runtime is actually wanted.

### A0. First-party screens are not this

The 16 built-in screens (`app/src-iced/src/screens/*.rs`) stay hand-written native Rust — Elm-style
`view(&State) -> Element<Message>` + `update`, using the **full native toolbox**: `rich_text` spans
(chat/pages/forge), native media (huddle: `miniaudio` / `nokhwa` / `screencapturekit` / `oxideav-vp8`),
and CEF (browser, behind the `cef-browser` feature). **Those capabilities are host-privileged and are
not exposed to third-party views.**

Stated plainly, because it is a real regression from the iframe model: a media module (huddle) or a
browsing module (browser) **cannot be a third-party package** under a native shell. The iframe could
have granted a packaged huddle `getUserMedia` at its own origin; wasmtime + a widget vocabulary cannot
and will not. Third-party views get a bounded widget vocabulary, not a camera. That is a smaller
third-party surface and a stronger sandbox at once.

### A1. The package

Two wasm halves, published together — no `bun`, no web bundle, no `provider.js`:

- the **module** wasm (unchanged: `sdk::Module`, the consensus state machine), and
- a separate **view** wasm exporting `view`/`update` over a **serializable widget vocabulary**
  (Column / Row / Text / RichText / Button / TextInput / Table / Scrollable / … — the host's bounded
  set), plus `module.json { id, label, icon, section, submits: ["chat", "pages"] }`.

`submits` is unchanged: the install-time grant that becomes the session key's `KeyScope.targets` —
declared by the package, approved by the user, **enforced by consensus**, deliberately *not* scoped to
the serving module. The view stays a **separate artifact** from the module because finding 6 still
holds: a widget-tree tweak must not ride the R=n governance swap that moves app-hash.

### A2. Publish and pin — unchanged

Still a DuckFS commit + `GatewayMsg::SetRoute` → `RouteTarget::DuckFs { manifest_sha256 }` (existing
variant, `crates/system/gateway/src/interface.rs:119`), inside the existing `/home/<owner>/**` rule so
`check_authority` is untouched. Consensus still agrees "the canonical view of module X is manifest H"
with no governance vote per UI change. `modreg`'s module entry still gains a `view_publisher`, set once
at registration by governance, checked against the `SetRoute` signer — a registration-time field, not
part of the R=n swap path. **The only difference: the pinned bytes are a `.wasm` view component, not an
`index.html` tree.** Content-addressing is byte-agnostic; this half carries over verbatim.

### A3. Load and render (replaces "Serve") — the deferred, harder half

The shell does not fetch the view over HTTP; there is no `duck://` origin and no CEF for a view. It
reads the pinned, hash-verified view bytes from DuckFS and instantiates them in **wasmtime** — the same
sandbox the module half already runs in (fuel-metered, no I/O). The view exports
`view(state_snapshot) -> WidgetTree`; the host walks the tree, builds real iced `Element`s, and maps
each `on_press: MsgId` to a host `Message` that routes back into the view's `update`. State snapshots
are marshalled from `duck.query` / `duck.subscribe` results.

**The sandbox is now wasmtime + WIT capability imports, not a browser origin.** `origin_guard`,
`duck_scheme`, the `*.mods.duck` origin, `postMessage`, CSP, Permissions Policy — none of them are
load-bearing for views anymore. A view can touch only what its WIT world imports; there is no DOM, no
`fetch`, no `localStorage` to fence off. The footgun that section warned about does not exist.

**New code, and it is real:** a serializable widget WIT world, a host-side tree→`Element` interpreter,
and view↔host message routing. This is where the pivot makes A *more* work than the retired iframe
model — plan for it as a subsystem, not a serve route. It is deferred until third-party views are
actually on the roadmap; Part B and A0 do not wait on it.

### A4. Writes — unchanged in substance

No `provider.js`, no `postMessage`. The view's imported host functions *are* the provider:

```
duck.query({ target, query })    // → host → POST /v1/query
duck.submit({ target, msg })     // → host → sign with THIS view's per-module session key
                                 //        → POST /v1/submit/frame
duck.identity()
duck.subscribe(...)
```

**Install flow (identical to the iframe design — it is all Part B):**

1. Shell shows `module.json`'s `submits` list.
2. User approves → shell mints an ed25519 session key **for that module**.
3. Shell signs `AddMemberKey { new_key, scope: { targets: submits, expires_at: h + N }, possession,
   authorizer }` — authorized by the **passkey** (user presence), once, at install.
4. Afterwards `duck.submit` signs in-process with that key. No prompt, no popup.

Per-module session keys mean a compromised chat view **cannot touch `vaults`** — consensus refuses the
frame at `node/src/lib.rs:1248`. Revoking chat is one `RemoveMemberKey`. The on-chain frame records
which key signed, so authorship stays the account while attribution stays recoverable from `op_meta`.
**Media does not ride `duck.submit`** — but media modules are first-party now (A0), so the tokenized-ws
media plane is a first-party shell concern, not a package concern. The whole `permissions.rs` /
`gateway_window.rs` / iframe-media apparatus drops out of Part A.

### A5. Console integration

`app/src-iced/src/screens/mod.rs`'s static screen set gains a dynamic tail sourced from the gateway
route table. Each dynamic entry renders one generic native `ModuleView { module_id }` screen that hosts
the view-wasm interpreter (A3). Hand-written first-party screens keep working; migration is incremental.

---

## What does not change

`sdk::Module` · wasm module rebuilds · `modreg`'s swap path · `swap_code` · `check_authority` ·
`RouteTarget` · `serve_duckfs` (still serves *gateway* sites; it is no longer on the module-view path) ·
consensus vote signing · every module's `execute` (they keep reading `env.origin.actor_string()`; it
simply starts returning a stable account for account members).

`origin_guard`, `duck_scheme.rs`, and `tauri-runtime-cef` are untouched **by this design** — but they
are no longer part of Part A (the native view path uses none of them), and the iced port reworks the
webview stack on its own track. Do not treat their names here as a claim the port leaves them alone.

## New code

**B:** frame codec carries `MemberAuth`; `decode_frame` demoted to pure decode; `KeyScope` +
`MemberMeta.scope` + `AddMemberKey.scope` + scope bound into `add_member_preimage`; identity snapshot
read in `host_reads.rs`; `authorize()` at `crates/kernel/node/src/lib.rs:1248`; advisory ingress
verify; shell passkey-root + session-key mint; desktop switched to `/v1/submit/frame`.

**A — authority half (lands with B):** per-module scoped session-key mint under passkey presence;
`submits` → `KeyScope.targets`; install-flow UI; `modreg.view_publisher` + gateway publisher check;
packaging CLI (view-wasm build → manifest → DuckFS commit → `SetRoute`).

**A — rendering half (deferred):** the serializable widget WIT world; the host-side widget-tree →
iced `Element` interpreter with view↔host message routing; the `duck.*` host imports; native
`ModuleView` screen + `screens/mod.rs` dynamic tail. This is the subsystem A3 flags — not started, not
blocking B.

## Risks and open items

1. **`account_id` is the founding member pubkey — and that is fine. Resolved: zero migration.**

   `identity/src/lib.rs:3` says *"an ACCOUNT is keyed by its FOUNDING key (`account_id` = the first
   member key)"*, which reads like the Keybase anti-pattern (identifier == key ⇒ rotation kills the
   account). It is not, because `account_id` is **frozen bytes used as a map key**, not a live key
   reference:

   - The founding key **can be revoked** — `remove_member_key` (`identity/src/lib.rs:835`) refuses only
     the *last* member, not the founding one. The account survives with its remaining members and keeps
     its id.
   - Re-founding an account on a stale `account_id` is **explicitly refused**
     (`identity/src/lib.rs:643-647`: *"account id already exists but its founding key is not a
     member"*), so a revoked founding key cannot be used to clobber the record.

   That is exactly Keybase's model — an eldest key *seeds* a stable identifier that then outlives it —
   minus the cosmetic separation. **`account_id` is already rotation-safe.**

   Consequence: `Origin::External(account_id)` is byte-identical to what modules already store for a
   primary key (`ext:<founding-pubkey-hex>`). Existing file homes, chat authorship, pages comments,
   forge, vaults, jobs, inbox and runs data are **untouched**; only *additional* keys (phone, session
   keys) remap onto the primary. **Zero migration.**

   The one real cost of not having an opaque UID: `account_id` leaks the founding device's pubkey
   forever. A privacy nit, not a blocker.

   **And this change is a prerequisite for ever minting opaque UIDs.** Today eight modules key user
   state on *N device pubkeys per user*. After this change they key on *one `account_id` per user*. A
   future UID migration then becomes a 1→1 rename instead of an N→1 merge — which matters because
   **there is no state migration primitive at all**: `swap_code`
   (`crates/kernel/wasm-host/src/lib.rs:682`) re-instantiates the component and deliberately leaves the
   store untouched (that *is* the live-update primitive). An N→1 merge across eight modules is not
   merely hard today; it has no mechanism. Do this first, whatever the long-term identifier is.
2. **Protocol version bump.** The frame codec and `add_member_preimage` both change shape. B rides the
   existing `upgrade` path (`crates/system/upgrade`), R=n gated. Old frames must be rejected, not
   reinterpreted.
3. **WebAuthn determinism in the frame path.** Identity already verifies WebAuthn assertions in
   consensus, so the envelope parse is presumed canonical — but it has never been on the frame hot path.
   Confirm the parse has no non-determinism and bound the proof size (assertions are large).
4. **Accountless keys keep full authority** (the `account_of() == None` branch). That is the status quo
   and is how a brand-new user with no account works. Tightening it is a separate decision.
5. **Session key at rest in the shell.** Low-value (scoped + expiring) but still a key. Store it under
   the existing session-password seal, not plaintext.
6. **Third-party packages still need a new module id** — post-genesis module *registration*
   **landed 2026-07-16 (PR #630)**: `GovAction::RegisterModule` → modreg `ScheduleRegister` →
   host `ModuleFactory` instantiation at the activation boundary, riding the same R=n readiness +
   height gate as code swaps. It ships version-gated dormant (`modreg::ADMISSION_ACTIVATION_VERSION
   = 4`, above every shipping `MAX_PROTOCOL_VERSION`, with a compile-time assert in
   `bin/node/src/constants.rs`): activating it requires first landing the admitted-module
   restore/statesync path — today's composers enumerate a fixed module set, so a post-admission
   checkpoint would brick restart/join. That restore half is the remaining project.

## Verification

- `crates/kernel/host/tests/module_swap.rs` is the template for B's tests.
- **Scope:** a key scoped to `chat` signing to `vaults` is dropped at the drain, no state change,
  app-hash continuity across the boundary.
- **Expiry:** a key past `expires_at` is refused at height `h+1`.
- **Preimage binding:** an `AddMemberKey` whose scope is stripped after signing fails verification.
- **Account resolution:** a second member key signing to `chat` produces the *same* `AuthorRef` as the
  primary key. (This is the multi-device bug fix, and it is the regression test that matters most —
  every module's actor identity depends on it.)
- **Passkey frame:** a `WebauthnP256` member key signs a frame end to end and `chat` attributes it to
  the account.
- **Namespace separation:** a `MemberAuth` signed under `IDENTITY_ADD_MEMBER_NS` is rejected as a frame.
- **A (authority half):** install a package, confirm the minted session key is scoped to `submits` and
  a write outside that set is dropped at the drain.
- **A (rendering half, deferred):** publish a view-wasm, flip the manifest, confirm the native shell
  re-instantiates it and picks up the new widget tree with no app or node restart.
