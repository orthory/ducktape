# `.quack` — the module package format

Date: 2026-07-15
Status: design, not yet implemented
Companion: `2026-07-14-scoped-session-keys-and-module-view-packages-design.md` (Part B is a
prerequisite; this document supersedes that spec's Part A as the packaging story)

## The ask

Third-party modules should be definable and shareable as a single package — the consensus module,
its view, and its authority grant in one file. This document specifies how the package is composed,
how it is served, how it is validated, and what its lifecycle is.

**One-line definition:** `.quack` is a signed transport envelope. The truth lives in network state —
modreg's hash pin, the gateway's route, content-addressed bytes. The file only carries; after
install it can be thrown away and reconstructed from the network.

**Standing principle (finding 6 of the companion spec):** the two wasm halves in one package ride
different rails. Module code moves app-hash, so it rides the R=n governance rail (vote → readiness
→ activation height). The view does not, so it rides the SetRoute-flip rail (no vote, minutes). One
package, two lifecycles.

## Context: the native-iced substrate

The console is a native iced shell (`app/src-iced/`). There is no DOM, no iframe, no origin to hand
a third party. A packaged view is a **wasm component exporting `view`/`update` over a serializable
widget vocabulary**; the shell instantiates it in wasmtime and interprets the returned widget tree
into iced `Element`s. The sandbox is wasmtime + WIT imports — a view can touch only what its world
imports. `origin_guard`, CSP, and `postMessage` are not load-bearing for views.

Honest regression, stated once: media modules (huddle) and webview modules (browser) cannot be
third-party packages — the widget vocabulary has no camera. Those stay first-party native. Smaller
third-party surface, stronger sandbox.

## What already exists (verified)

The registration and distribution machinery is three-quarters built:

- `ModregMsg::Register { module_id: String, code_hash }` — a post-genesis registration op **already
  exists** (`crates/system/modreg/src/interface.rs:54`), accepted from module or system origin
  (`require_module_or_system`), refusing duplicate ids. Nothing emits it yet.
- `WasmModule::from_bytes(id, component_bytes)` — instantiating a module from bytes **already
  exists** (`crates/kernel/wasm-host/src/lib.rs:327`).
- Byte distribution **already exists**: the code plane / blobstore, with mesh fetch by hash
  (`bin/node/src/blob_fetch.rs` `FetchingCodeSource`) and the readiness protocol ("full byte
  receipt, latched in committed state").
- `realize_module_swaps` explicitly skips ids absent from the host registry — *"its registry entry,
  if any, is a genesis concern"* (`crates/kernel/host/src/lib.rs:826`). That comment marks the
  exact spot where instantiate-on-register goes.
- View pinning **already exists**: DuckFS commit under `/home/<owner>/**` (no `check_authority`
  change) + `GatewayMsg::SetRoute` → `RouteTarget::DuckFs { manifest_sha256 }`
  (`crates/system/gateway/src/interface.rs:119`).
- Code-swap correctness **already tested**: `crates/kernel/host/tests/module_swap.rs` (state
  preservation, determinism, fail-closed on tampered bytes, statesync join, unready never arms,
  inert without modreg).

What does not exist: the widget WIT world and its host-side interpreter, the `quack` CLI, the
governance registration action, and reproducible guest builds (the Makefile says so itself:
*"`wasm-modules-check` guards mutual consistency, not reproducibility"*).

## 1. Composition

```
kanban-1.2.0.quack            (tar.zst)
├── quack.toml                ← the package manifest; the only signed object
├── module/component.wasm     ← optional: consensus module (ducktape:module world)
├── view/component.wasm       ← optional: view (ducktape:view world, widget-tree ABI)
├── source.tar                ← optional: the guest workspace, Cargo.lock included
├── assets/**                 ← optional: icons etc.
└── quack.sig                 ← publisher account-key signature
```

```toml
[package]
id        = "kanban"            # ModuleId = String, globally unique in modreg
version   = "1.2.0"
publisher = "<account_id>"      # Part B account (frozen founding-key bytes)
license   = "BSL-1.1"           # SPDX id or custom

[requires]
protocol  = ">=12"              # node protocol version range
view-api  = "1"                 # duck.* host API + widget vocabulary version

[module]                        # absent ⇒ view-only package
wasm    = "module/component.wasm"
sha256  = "…"                   # = the code_hash pinned in modreg

[view]                          # absent ⇒ headless module
wasm    = "view/component.wasm"
sha256  = "…"
submits = ["kanban", "chat"]    # install-time grant → session key KeyScope.targets

[source]                        # optional — see §5
tree   = "source.tar"
sha256 = "…"

[source.recipe]                 # everything a bit-exact rebuild needs, pinned exactly
rustc      = "1.96.1"
wasm-tools = "1.220.0"
target     = "wasm32-unknown-unknown"
flags      = ["--release", "--remap-path-prefix=…"]
```

**Signature chain:** every artifact's sha256 is inside the toml; the publisher signs
`quack_digest = sha256(canonical quack.toml)` — one signature covers the whole package. The
package's identity IS the digest.

At least one of `[module]` / `[view]` is required. All three combinations are valid: module+view
(a normal app), view-only (a view update for an existing module), headless (a background module).

Module builds follow the existing guest pattern verbatim — standalone guest workspace,
`guest-adapter`, `getrandom-stub`. Determinism is guaranteed by build structure, not by review.

`submits` is unchanged from the companion spec: declared by the package, approved by the user,
enforced by consensus at the drain — and deliberately **not** scoped to the serving module (chat
may invoke pages).

## 2. Serving — two planes

**File plane (sharing).** A `.quack` is just a file — share it as a chat attachment, through the
files module, or on the web. Zero new sharing infrastructure; the answer to "shareable" is that the
format is a file.

**Network plane (installed truth).**

- **Module bytes** → the existing code plane / blobstore. Validators fetch by hash over the mesh
  (`FetchingCodeSource`) — the same road swap bytes travel today. Readiness latches full byte
  receipt before activation can arm.
- **View bytes + assets** → DuckFS `/home/<publisher>/.duck/quack/<id>/<version>/` (inside the
  existing `/home/<owner>/**` rule) + `SetRoute` → `RouteTarget::DuckFs { manifest_sha256 }`. The
  shell's ViewHost reads the pinned bytes, verifies the hash, and instantiates.

After install the `.quack` file is disposable: all bytes are content-addressed on the network, all
pins are consensus state. This is why no package-registry server exists in this design.

## 3. Validation — six and a half layers

| # | check | enforcer · when |
|---|---|---|
| 1 | Envelope integrity: artifact hashes match toml; signature verifies against digest | anyone · offline |
| 2 | Publisher identity: account exists in `identity`; signing key is a **current** member (not revoked) | install tooling · against committed consensus state |
| 3 | Static wasm: component parses; exports the right world; imports ⊆ whitelist (module: no WASI/clock/random — determinism; view: `duck.*` only); size cap (existing components ≈ 1.6 MB; cap is a knob) | install tooling |
| 3.5 | Source reproduction (**optional**): `quack rebuild` reproduces the artifacts from `[source]` per the pinned recipe; output hashes match toml → **source-verified** | local rebuild · see §5 |
| 4 | Capability review: show `submits` + host imports to a human → approval → session-key mint. For new-module registration, the same sheet goes in the governance proposal | human |
| 5 | **Consensus — the only real gate**: modreg hash pin + fail-closed swap (existing, tested); gateway `view_publisher` check; drain `authorize()` scope (Part B) | network |
| 6 | Runtime containment: module = fuel-metered wasmtime (existing); view = ViewHost hash-gated reload + no-I/O world | node / shell |

Layers 1–4 are conveniences that fail early. Layer 5 is the gate — the same philosophy as "ingress
is advisory, the drain is authoritative." If layer 3 is bypassed entirely, layer 5 still holds.

## 4. Lifecycle

```
author ──quack build──▶ .quack ──share (chat/files/web)──▶ anyone
                                                              │
              ┌───────────── new module id ───────────────────┤ module already registered
              ▼                                               ▼
  publish: commit bytes to code plane + DuckFS        install (member, local):
  register (governance): proposal { module_id,          verify layers 1–4
    code_hash, view_publisher } → vote →                → mint session key
    readiness (all validators latch byte receipt)         (scope = submits, expiring)
    → at activation height H the host                   → ViewHost loads the pinned view
    instantiates via WasmModule::from_bytes
              │
              ▼
  publisher SetRoute(view manifest) → every member's shell hot-swaps
```

- **build** — compile both wasm halves (existing guest pattern), hash, sign with the publisher
  account key. Source is included by default; `--no-source` is an explicit opt-out (§5).
- **share** — the file, over any channel. Nothing on-chain yet.
- **publish** (publisher, once per version) — commit bytes to the code plane and DuckFS. Not yet
  active.
- **register** (governance; new module ids only) — the one missing machine, and it is small. Three
  pieces, each an extension of an existing pattern:
  1. a governance action that emits `ModregMsg::Register`;
  2. `Register` gains the pending/readiness gate `Schedule` already has — today it activates the
     hash immediately, which would make a byte-less validator fail closed at the boundary; the
     readiness latch guarantees every validator holds the bytes before activation arms;
  3. instantiate-on-register at the drain boundary: where `realize_module_swaps` currently skips a
     registry-absent id, fetch the bytes and `WasmModule::from_bytes` + `Host::register` at the
     activation height. Registering a module changes app-hash by construction (the registry set is
     what `app_hash()` composes over), which is exactly why it rides the R=n rail.
- **install** (member, local) — verify layers 1–4, mint the per-module session key
  (`scope = { targets: submits, expires_at: h + N }`, authorized by the passkey, once), ViewHost
  loads the pinned view. The module itself is already on the network: **install is an authority and
  UI act, not a code act.**
- **update** — two rails:
  - *view-only* (style, layout): publish + `SetRoute` flip. No vote. Shells hot-swap on the hash
    change. Minutes.
  - *module code*: the existing `Schedule` → readiness → activation path; `swap_code` preserves
    state.
  - *both*: the view must be wire-compatible across the activation boundary, or the view flip lands
    after activation. The CLI checks `[requires]` and orders the two submissions.
- **rollback** — view: `SetRoute` back to the previous manifest (bytes are content-addressed and
  still present — trivial). Module: `Schedule` back to the old hash (machinery exists; state-schema
  backward compatibility is the module's own responsibility).
- **revoke / uninstall** — member uninstall = `RemoveMemberKey` (session key) + local view drop.
  Network kill of a malicious package = swap to inert code (module *deletion* does not exist —
  its state is in app-hash; the tombstone swap is deactivation, using existing machinery).
  Publisher key compromise = identity `RemoveMemberKey`; subsequent installs fail layer 2, and
  view flips fail the `view_publisher` check.

## 5. Source: default, not mandatory

"Ship the source so it can be audited" is true **only if the build is reproducible.** Without
bit-exact reproduction, bundled source is decoration a malicious publisher trivially fakes (honest
source, dishonest binary). With it, the source becomes a statement about the binary, and audit —
human or LLM — becomes meaningful. This is proven practice (CosmWasm/NEAR-style pinned-builder
contract verification).

The larger win is not audit: reproducibility removes the publisher's build machine from the trust
boundary. A compromised build box cannot inject code that is not in the source — the hash diverges
and nobody can reproduce it.

**Why not mandatory:** layer 3 can only check *presence*; truth requires a rebuild, and a rebuild
(toolchain + minutes) cannot be an install gate. A mandatory-but-unverified source requirement is
theater — garbage source passes — and false confidence is worse than an honest "closed" label.
Instead:

- **Defaults shape culture.** `quack build` includes source by default; excluding it requires an
  explicit `--no-source`.
- **Governance is the enforcement point.** "This network only votes yes on source-verified
  packages" is a social policy, enforced by voters running their own rebuilds, at zero protocol
  change. No on-chain enforcement is built — validators will not run rebuilds, so enforcement is
  impossible there, and this design does not pretend otherwise.
- **The badge only renders from a local rebuild.** Trusting the manifest's claim would collapse the
  whole scheme; the manifest carries the *recipe*, never the verdict. Install UI shows three loud
  states: `source-verified` (local rebuild matched) / `source-included` (bundled, unverified) /
  `closed`.
- **LLM audit is advisory.** It catches legible malice — code doing more than the manifest declares
  — but a sophisticated attacker writes innocent-looking source. It raises the bar; it is not
  proof.

**Prerequisite, and independently valuable now:** the repo's own guest builds must become
reproducible first — pin rustc, pin wasm-tools, add `--remap-path-prefix`. Today
`wasm-modules-check` "guards mutual consistency, not reproducibility" (its own words). With
reproducibility, anyone can verify the committed `component.wasm` bytes, `.quack` or not.

### Commercial modules

Premise: a consensus system means **every validator receives and executes your binary**, and the
view wasm reaches every member's shell. Only the source can be hidden, wasm decompiles readily, and
LLMs read decompiled output well. Hiding is a speed bump, not a wall — commercial protection comes
from elsewhere:

1. **Source-available licenses.** Visible ≠ free. BSL / FSL / Elastic-style terms allow reading and
   audit while forbidding commercial reuse — the model MongoDB, Elastic, and Sentry run on. This is
   the primary answer, and why `[package]` carries `license`.
2. **The update channel is the product.** `view_publisher` and the code stream are account-gated
   consensus state. Anyone can copy the v1.2.0 file (it is a file; this design does not pretend to
   stop that), but only the vendor account can ship v1.3.0. A copying network cuts itself off from
   updates and security patches. Subscription without protocol change.
3. **Per-network B2B.** Installation is a governance act, so the natural unit of sale is the
   community — per-network licenses and support contracts. The Red Hat model, not the Photoshop
   model.
4. **Real secrets go off-chain.** A proprietary algorithm does not belong in a consensus module —
   it executes publicly by definition. Keep the secret logic in the vendor's off-chain service and
   submit results as transactions (the oracle pattern); the on-chain module is the verification and
   storage interface. Then 100% source disclosure leaks nothing.

Explicitly unsupported, because supporting them would be a lie: per-user DRM (file sharing cannot
be stopped), license checks inside module code (public + forkable = inert), secret algorithms in
consensus code (public execution).

## 6. The view runtime: ViewHost and hot-swap

The shell does not fetch views over HTTP. It reads pinned, hash-verified bytes from DuckFS and
instantiates them in wasmtime — the same sandbox the module half already runs in. The view exports
`view(state) -> WidgetTree`; the host walks the tree into iced `Element`s and routes interaction
messages back into the view's `update`. State is derived from `duck.query` / `duck.subscribe` host
imports: **the view is a function, not a cache.**

Hot-swap correctness lives in one small state machine, `ViewHost`, behind a runtime trait — so its
tests land before the widget vocabulary exists:

| scenario | expected |
|---|---|
| `reload_if_changed` with a new hash | `Ok(true)`; render output switches to v2 |
| same hash again | `Ok(false)`; no reload (thrash guard) |
| `sha256(bytes) != manifest_hash` | `Err(Integrity)`; **old view keeps rendering** — a tampered publish cannot blank the UI |
| intact bytes that fail instantiation | `Err(Instantiate)`; **old view keeps rendering** — a broken publish cannot blank the UI |

All four verify against a fake runtime — no wasmtime, no heavy deps. The real
`WasmtimeViewRuntime` (first shell-side wasmtime use in the tree) plugs in behind the same trait
later; a live-node SetRoute-flip E2E comes after that. Fail-closed symmetry with
`realize_module_swaps` is deliberate: on either rail, a bad publish leaves the previous artifact
running.

The write path is unchanged from the companion spec: `duck.submit` signs with the per-module
session key; writes are asynchronous (visible next block); reconciliation (optimistic pending
merged on event arrival) is implemented **in the host imports, not in views** — view authors never
manage pending state by hand.

## 7. New code, honestly, largest first

1. **Widget WIT world + tree→`Element` interpreter** — the largest piece; the browser used to
   provide this half for free. ViewHost is its seam.
2. **`quack` CLI** — build · rebuild · audit · publish · install · dev; mostly orchestration of
   existing parts.
3. **Registration, three pieces** — governance action; `Register` pending/readiness; the
   instantiate-on-register at the boundary. Each extends an existing pattern.
4. **Reproducible guest builds** — toolchain + wasm-tools pins, path remap. Independently valuable,
   do first.
5. **Install/approval UI** in the iced shell — capability sheet, three-state source badge, audit
   report.

Separate-repo question, answered now so it is not re-litigated: everything stays in this monorepo
until the view-api is frozen at v1 **and** external authors actually exist. At that trigger, the
author-facing SDK (widget WIT world, `quack` CLI, `guest-adapter`, sdk types) extracts to
**`byeongsu-hong/quack`** (destination decided 2026-07-15); ViewHost, modreg, gateway, and the
registration path cannot ever leave. Guest crates are already standalone workspaces inside the
repo — technical isolation is solved at the workspace level, not the repo level.

## Open items

1. **Id namespace.** `ModuleId` is a first-come String. Recommendation: no new mechanism — the
   governance vote is the namespace arbiter (approving "kanban" is approving the name). Enforced
   publisher prefixes only if squatting becomes real.
2. **view-api versioning.** A single integer in v1; the shell advertises supported versions and
   install tooling compares. Grows into a range when the vocabulary grows.
3. **View/module version skew.** The two rails deploy at different speeds, so a view-v1 /
   module-v2 window always exists. Pin a module version in the manifest and warn on skew, or
   declare the `Query` wire a public API with backward-compatibility discipline — probably both.
4. **devnet block time + event stream shape.** The inner loop needs submit→render inside ~200 ms,
   and reconciliation needs per-module subscription and block heights on events. Measure and
   confirm; if the WS stream lacks either, that is this project's first node-side task.
5. **Discovery.** v1 is file sharing. A catalog/directory module comes later — itself distributed
   as a `.quack`.

## Verification

- **ViewHost hot-swap:** the four-case matrix above, against a fake runtime (lands first).
- **Registration:** a governance-approved `Register` for a brand-new id activates at height H on
  every validator with app-hash continuity; a validator lacking bytes at H fails closed; the
  registered module executes from H+1. Template: `module_swap.rs`.
- **Envelope:** a package whose artifact bytes are swapped after signing fails layer 1; a package
  signed by a revoked member key fails layer 2.
- **Reproducibility:** `quack rebuild` of a source-carrying package on a second machine yields
  byte-identical components.
- **Scope:** an installed package's session key signing outside `submits` is dropped at the drain
  (this is Part B's test, exercised through the install flow).
- **Rails:** a view-only update reaches a running shell with no app or node restart; a module
  update preserves state across the activation boundary.
