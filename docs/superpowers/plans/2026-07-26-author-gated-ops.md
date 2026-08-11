# Author-gated ops: the sweep

- **Date:** 2026-07-26
- **Tree audited:** `origin/dev` @ `2eec1d307` (PR #835 merged). The primary
  checkout is 38 commits behind at `e0d773f68`; every module file in scope is
  byte-identical between the two except `saga/src/lib.rs` (a `lease_expiry`
  liveness fix, no authorization change) and `duckdns` + `blobstore`, which
  **dev deletes**. Findings below use dev.
- **Status:** read-only. Nothing was changed but this file. No cargo was run.
- **Scope:** all 20 module crates in `crates/modules/apps/*` and
  `crates/modules/system/*`, 130 write ops.

## Evidence labels

Same three as the assumption audit, same meanings.

| label | means |
|---|---|
| **EXECUTED** | a command was run, or production code was mutated and the guard watched. |
| **STATIC** | read from code where the fact is a single unambiguous expression — a match arm, a missing argument, a scan list. Not run, but not an inference. |
| **PLAUSIBLE** | reasoned across several files. Could be wrong. |

Phase 1 ran nothing, so there is no EXECUTED label below. Where a fact is
already asserted by a test that CI runs, that is noted — it is the strongest
claim available without touching the tree.

---

## The answer, first

**`SetMembership` is not an instance. It is not even the class — it is the
smaller of two classes, and the larger one is a network takeover.**

There are two independent ways an op ends up not knowing who asked:

- **Class 1 — the arm drops the author.** `execute` binds an author at the top,
  the arm does not pass it down, and it compiles because sibling arms use it.
  **12 ops in 6 modules.** This is the `SetMembership` shape.
- **Class 2 — the author reaches the arm, and the author is forgeable.** ~30 ops
  gate on `Origin::Module(_)`. The continuation lane lets any keyholder choose
  that module id. **The valset membership gate is one of them**, so any admitted
  member can join the validator set and evict every other validator, with no
  governance proposal. This is worse than every Class 1 defect combined.

Both classes have the same root: **`ctx.env().origin` is a LANE, not an author,
and every module in the tree authorizes against it.** The SDK ships
`Ctx::author_origin()` — the relay-aware accessor built precisely to prevent
this — and it has **zero callers among the 20 modules**. Its only caller
repo-wide is a host test.

Membership is not a confidentiality boundary today and cannot be one until the
private-messaging ADR is built, so `SetMembership` itself is a write-integrity
bug, not a disclosure. Section 4.

---

## 1. The author seam, and how an arm forgets it

### 1.1 Where the author comes from

```
signed frame bytes
  → node::decode_frame (crates/kernel/node/src/lib.rs:234-289)
      verifies ed25519 over (origin, seq, target, payload, continuation)
      → Origin::External(pubkey)                       [AUTHENTICATED]
  → host::BlockOp { origin, msg, continuation, frame }
  → Host::submit_block_ops                             (host/src/lib.rs:1085)
  → drain_queue                                        (host/src/lib.rs:1493)
  → HostCtx { env: Env { origin: trigger, .. }, relay } (host/src/lib.rs:1536)
  → Module::execute(ctx, msg) → ctx.env().origin
```

Three values, three lifetimes:

| value | is | set by |
|---|---|---|
| `env().origin` on a **root** dispatch | the authenticated submitter | `decode_frame` |
| `env().origin` on an **emitted follow-up** | `Origin::Module(emitter)` | `drain_queue`, from the emitting module |
| `env().origin` on a **released continuation** | `Origin::Module(msg.target)` — **the parent op's target, a caller-chosen string** | `host/src/lib.rs:1191` |
| `relay().author` | the authenticated submitter, always | `host/src/lib.rs:1185-1186` |
| `ctx.author_origin()` | `relay().author` if a continuation, else `env().origin` | `sdk/src/lib.rs:417-422` |

`author_origin()`'s own doc names the hazard:

> "one call, correct in both, so no module keys authz on the module-origin LANE
> of a continuation by accident (that would make `continue` privilege
> escalation: any external key could reach module-origin-gated arms by bouncing
> off an innocent parent op)."

**Zero modules call it.** (STATIC — `grep -rn author_origin crates/ bin/` returns
the definition at `sdk/src/lib.rs:417`, `host/tests/continuation_inline.rs:66`,
and `runs/src/module_impl.rs:55` — which merely *forwards* `relay()` through a
Ctx wrapper and never reads it.)

### 1.2 How an arm forgets it — the mechanism, exactly

The dominant module shape is:

```rust
async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
    let author = author_from_origin(&ctx.env().origin)?;   // bound ONCE
    match decode_msg(&msg.payload)? {
        Msg::A { .. } => self.stage_a(&author, ..).await,  // passes it
        Msg::B { .. } => self.stage_b(..).await,           // does not
    }
}
```

`Msg::B` compiles. `author` is used by `Msg::A`, so there is no unused-variable
warning, no clippy lint, and no type error — the handler simply has a different
arity. **Nothing in the language, the lint set, or the test suite distinguishes
"this op does not need an author" from "this op forgot one."** That is the whole
mechanism.

Two variants make it worse:

- **`_ctx`.** `kv::execute` is declared `async fn execute(&mut self, _ctx: &mut
  dyn Ctx, msg: &Msg)` (`kv/src/lib.rs:157`). The author is not dropped on one
  arm — it is structurally unavailable to the module.
- **Computed, then discarded.** `forge` calls `author_from_origin(&ctx.env().origin)?;`
  on `SetItemState` (`forge/src/lib.rs:621`) and `MergePr` (`:639`) and throws
  the value away. The `?` proves the origin is *authenticated*; nothing compares
  it to anything. It reads exactly like a gate and is not one.

### 1.3 The forgeable-author lane, in full

`crates/kernel/host/src/lib.rs:1131-1198`:

```rust
// the relay outcome the continuation (if any) will carry: ... THE CONTINUATION
// ALWAYS FIRES — dropping it on failure would strand the very reentry flow the
// envelope composed it for.
...
Err(reason) => { self.abort_all(..); self.replay_accepted(..); relay_outcome = Err(..); }
}
let Some(cont) = continuation else { continue };      // :1184  — after BOTH arms
let relay = Relay { author: origin, parent_target: msg.target.clone(), .. };
let corigin = Origin::Module(msg.target);             // :1191
```

Four facts, each a single expression:

1. `cont.target`, `cont.payload` and the parent `msg.target` are all
   attacker-chosen fields of a frame signed with the attacker's own key
   (`node/src/lib.rs:242-255`).
2. `corigin = Origin::Module(msg.target)` — the continuation's `env().origin` is
   a module id the attacker typed.
3. The continuation fires even when the parent rejects, including
   `Error::UnknownModule` (`host/src/lib.rs:1517`), so **the forged module id
   need not exist**.
4. It is remotely reachable. `verify_relay_submit` (`bin/node/src/relay.rs:204-224`)
   decodes the frame, binds `_cont` and **ignores it**, checking only that the
   origin holds committed resident-or-client standing. `POST /v1/submit/frame`
   (`bin/noded/src/lib.rs:535`) takes verbatim frame bytes; `bin/node` serves the
   same router (`bin/node/src/boot/surfaces.rs:278`).

The kernel's own tests assert (1)-(3) directly:
`continuation_inline.rs:148` — `assert_eq!(dispatches[0].origin, Origin::Module(PROBE.into()))`,
and `rejected_parent_still_releases_continuation_with_err`.

**The design doc specified the fix and the implementation shipped without it.**
`docs/superpowers/specs/2026-07-17-continuation-transactions-design.md:279-309`
states the authorization rule ("Attaching a continuation grants nothing"),
mandates `author_origin()`, and then:

> "the arm requires `Origin::Module` AND `ctx.relay().is_none()`; the sdk ships a
> `require_module_origin(ctx)` helper enforcing both so no module hand-rolls it
> wrong. Without this corollary, `continue` would be privilege escalation."

`require_module_origin` **is not in the sdk** (STATIC — absent from
`crates/kernel/sdk/src/lib.rs`). It exists exactly once, hand-rolled in
`identity/src/lib.rs:407-414`, without the relay half. Ten modules hand-roll
their own origin helper (`external_origin`, `acting_origin`, `acting_module`,
`admin_origin`, `origin_node`, `origin_key`, `require_module_or_system`,
`require_module_origin`); none checks `relay()`.

### 1.4 The seam does not cross the wasm boundary

This is the fact that decides the fix, and it is easy to miss:

- `crates/kernel/module-guest/wit/module.wit` — the WIT world exposes
  `record env { height, consensus-time, me, origin }` and `get-env()`. A grep of
  the whole file for `relay`, `author`, `continuation` returns **zero hits**.
- `crates/guests/guest-adapter/src/lib.rs:128` — `impl Ctx for WitCtx` implements
  `env()` and nothing else. `relay()` takes the trait default `None`.

Therefore **inside a wasm guest, `ctx.author_origin()` returns `ctx.env().origin`** —
the forged module lane. A change from `env().origin` to `author_origin()` in a
wasm module would compile, review as a fix, and do nothing.

**15 of the 20 production genesis modules are wasm** (`bin/node/src/host_state.rs`
`include_bytes!` sites): inbox, tasks, tagging, capability, identity, gateway,
governance, saga, agent, automations, dispatch, runs, pages, chat, files.
**Native, so `author_origin()` works today:** valset, forge, lifecycle (plus the
`hello`/`directory` examples).

The two most severe findings split across that line: **valset is native**
(fixable now), **dispatch, capability, identity and tagging are wasm** (need
relay in the WIT world first, or a host-side fix).

### 1.5 The continuation lane has no production producer

(STATIC.) Every `node::encode_frame` call in `crates/` and `bin/` passes `None`
for the continuation. The only `Continuation { .. }` construction outside
`decode_frame` is in tests (`node/tests/frame_envelope.rs`,
`host/tests/continuation_inline.rs`). No shipped CLI, service, or module composes
an envelope; `ducktape user sign-frame` hardcodes `None`
(`bin/node/src/userkey_cli.rs:735`).

So the lane is, today: a decoder that accepts attacker-supplied bytes, a host
that synthesizes a forgeable module origin from them, reachable by any admitted
member — **with zero consumers**. That materially changes what the cheapest fix
is (§6).

---

## 2. Per-op classification

130 write ops across 20 modules. `kv` and `vaults` are **not in the production
genesis set** (STATIC — `PRODUCTION` at `crates/kernel/host/src/topology.rs:123-147`
contains neither; `kv` appears only in `SIM_VALSET` and `DEMO`, `vaults` in no
topology at all). Everything else here ships.

Verdicts:

- **OK-gated** — consults the author and compares it to something (an owner, a
  roster, a stored author).
- **OK-self-scoped** — the author IS the subject of the write.
- **OK-no-principal** — the state has no owning principal, or the arm is
  `Origin::System`-only.
- **PARTIAL** — the author is consulted for its KIND only, while the *subject* of
  the write is a caller-supplied field.
- **DEFECT** — mutates state another principal owns without consulting who asked.

| module | ops | gated | self | no-principal | PARTIAL | DEFECT | wasm? |
|---|---:|---:|---:|---:|---:|---:|---|
| agent | 4 | 3 | 1 | | | | wasm |
| automations | 4 | 1 | | | | **3** | wasm |
| chat | 14 | 5 | 5 | | 1 | **3** | wasm |
| files | 6 | 4 | 2 | | | | wasm |
| forge | 7 | 1 | 3 | | 2 | **1** | native |
| inbox | 3 | | | | 1 | **2** | wasm |
| pages | 13 | 2 | 1 | 7 | 1 | **2** | wasm |
| runs | 9 | 5 | | | 4 | | wasm |
| tasks | 11 | 4 | 4 | 3 | | | wasm |
| vaults | 7 | 6 | 1 | | | | *not deployed* |
| capability | 2 | | 1 | | 1 | | wasm |
| dispatch | 9 | 2 | 1 | 2 | 4 | | wasm |
| gateway | 6 | 5 | 1 | | | | wasm |
| governance | 4 | 2 | | 1 | 1 | | wasm |
| identity | 9 | 4 | | | 5 | | wasm |
| kv | 1 | | | | | **1** | *not deployed* |
| lifecycle | 6 | 1 | | 1 | 4 | | native |
| saga | 8 | 5 | 1 | 1 | 1 | | wasm |
| tagging | 3 | | 2 | | 1 | | wasm |
| valset | 4 | | | | **4** | | native |
| **total** | **130** | **50** | **23** | **15** | **30** | **12** | |

### 2.1 Class 1 — the author never reaches the arm (12)

| module | op | the call | what it lets any member do |
|---|---|---|---|
| chat | `SetMembership` | `stage_membership(&channel_id, user, member)` `lib.rs:1206-1210` | add/remove anyone on any channel roster |
| chat | `RegisterHook` | `stage_register_hook(&channel_id, module_id)` `lib.rs:1200` | attach a hook module to any channel |
| chat | `UnregisterHook` | `stage_unregister_hook(&channel_id, &module_id)` `lib.rs:1202-1205` | **silently disable every automation on any channel** |
| inbox | `MarkRead` | `stage_mark_read(member, up_to_seq)` `lib.rs:318` | mark any member's notifications read |
| inbox | `Clear` | `stage_clear(member, up_to_seq)` `lib.rs:321` | **permanently delete any member's notifications** |
| automations | `CreateRule` | `stage_create_rule(rule_id, trigger, action, t)` `lib.rs:899` | mint a rule that posts/creates/delivers under `Origin::Module("automations")` |
| automations | `SetEnabled` | `stage_set_enabled(rule_id, enabled)` `lib.rs:903` | disable anyone's rule |
| automations | `DeleteRule` | `stage_delete_rule(rule_id)` `lib.rs:905` | delete anyone's rule |
| pages | `MoveCommentThread` | `comment_ops.rs:286-325`; `origin` is in scope at `:167` and never used | re-target any thread, overwrite its anchor |
| pages | `RemoveBlock` | `block_ops.rs:241` → `store.rs:110` `purge_comments_for_target` | **hard-delete author-owned comments** that `EditComment`/`DeleteComment` gate on the stored author |
| forge | `PushRefs` | `stage_push_refs(&name, updates, pack_digest)` `lib.rs:540` | move any branch of any repo to any oid; implicitly create repos |
| kv | `Set` | `execute(.., _ctx, ..)` `lib.rs:157`, `stage(key, value)` `:159` | overwrite any key (not in production genesis) |

`automations` is the sharpest of these because the module *has no principal at
all*: `Rule` records no owner (`interface.rs:58-67`), and the admin branch of
`execute` never binds an origin (`lib.rs:893-908`). Its header says "an operator
registers rules" and "Hook registration is a separate **operator** op"
(`lib.rs:3`, `:60-61`) — there is no operator concept in the code.

`chat::UnregisterHook` has a live consumer: `automations` requires a hook
registered on the channel (`automations/src/lib.rs:63`), so this arm is a
one-message off switch for another module's whole function.

### 2.2 Class 2 — the author is consulted and is forgeable (~30)

Every arm whose gate is `matches!(origin, Origin::Module(_))`, or that trusts a
module id string, is defeated by §1.3. By module:

| module | arms | what the forged origin buys |
|---|---|---|
| **valset** | `Join`, `Leave`, `Grant`, `Revoke` — `lib.rs:352-359` | **join the validator set; evict any validator; grant/revoke resident standing.** Handlers take `key: Vec<u8>` with no origin (`:192`, `:211`, `:234`, `:253`). |
| **lifecycle** | `RegisterModule`, `ScheduleSwap`, `ScheduleRegister`, `CancelSwap` — `require_module_or_system` `:279-287` | schedule an arbitrary module code swap |
| **identity** | `GrantClient`, `RevokeClient` — `require_module_origin` `:407-414` | mint/strip submit-door standing for any key |
| **dispatch** | the saga-callback route `:855-859`; `Dispatch` `:517`, `CancelDispatch` `:599`, `ReassignDispatch` `:628` | forge the *result* of any in-flight dispatch; write into or cancel another module's dispatch namespace. Correlation is public: `saga_id_for` is `dispatch\x1f{receiver}\x1f{id}` (`:59-67`) |
| **capability** | `ClaimClass` `:379-387` | permanently claim a routing class as another module — there is deliberately no unclaim (`interface.rs:19-21`) |
| **tagging** | `Subscribe`, `Unsubscribe`, `Tag` — `acting_module` `:150-159` | forge a subscription as any module; inject a `TagEvent` whose `author` is a caller-supplied payload field (`:232`) that gates the loop rule (`:242`) and rides into every recipient (`:283`) |
| **chat** | `check_channel_admin` `:417-428`, `check_post_policy` `:355-374`, `as_agent` `:1101-1114`, `validate_channel_namespace` `:395-414` | rename/archive any channel; post into any `MembersOnly` channel; **forge an `AuthorRef::Agent` author**, which the module's own doc says an external submitter cannot |
| **runs** | `execute_kind(&ctx.env().origin)` `module_impl.rs:182` routes to 5 privileged lanes | reach `ExecuteKind::Result` → `on_result_event` delivers an attacker-authored "agent result" with agent attribution |
| **files** | `is_module` `module.rs:396` → `watch_origin_gate` `fs.rs:1293-1301` | forge a watch registration for any module |
| **automations** | `HookEvent` `lib.rs:889-891` | inject a forged `ChatEvent` as chat |
| **saga** | `Cancel`, `Prune`, `Reassign` `:1134`, `:1155`, `:977` | cancel/prune/reassign any **module-triggered** saga (the compare is against `current.origin`, itself a module id) |
| **dispatch** | `RegisterRecipe` `:727` | register a recipe *owned by* another module |

`gateway` is the reference implementation and needs no change: `origin_node(ctx)`
is bound once at `module.rs:803` and threaded into every arm, and every subject —
`owner_account`, `RouteStatement.account_id` — is **re-derived from the origin**
and refused unless it matches (`:567`, `:464`). It also refuses a short origin
(`:348-353`), so the empty-external default cannot reach it. `vaults` is the
other clean one, and it is immune to §1.3 by construction: it refuses
`Origin::Module(_)` and `Origin::System` outright (`lib.rs:75-77`), which is what
"a continuation grants nothing" looks like when a module happens to get it right.

### 2.3 PARTIAL, non-forgeable, worth naming

- **forge `SetItemState` / `MergePr`** — author computed, `?`-checked, dropped
  (`lib.rs:621`, `:639`). Any authenticated member closes/reopens any item and
  merges any open PR. If that is intentional it should be `let _ =` with a
  comment; as written it reads like a gate.
- **runs `WatchChannel` / `UnwatchChannel` / `EnableJobWorker`** — module-global
  state, subject caller-supplied (`admin.rs:47`, `:77`, `:94`). Any member
  unwatches a channel another member set up.
- **runs `RequestRun`** — the requester is recorded, but `channel_id` is
  caller-supplied and `pin_context` (`dispatch_flow.rs:131-163`) pulls a
  `CONTEXT_WINDOW` slice of that channel's transcript into the dispatch envelope
  **with no membership check**. A non-member pumps a channel's history into an
  agent run. This is the one place where chat membership *would* have been a read
  barrier if it were one; see §4.
- **pages `ResolveThread`** — the author is stamped into `resolved_by` (`:397`)
  and never compared. Any origin resolves any thread.
- **chat `SweepHuddle`** — evicts an arbitrary `user`; gated only on author kind
  and post policy. Documented as deliberate ("cleanup is social", `lib.rs:955-960`).
- **identity `UnbindNode` / `AddMemberKey` / `RemoveMemberKey`** — origin
  kind-checked, value discarded; authority is the member certificate. Safe, but
  the origin is doing no work.
- **governance `Redeem`** — subject caller-supplied, bound by the invite token's
  proof-of-possession. Safe. `external_origin` (`:396-403`) is the one origin
  helper that does **not** reject `Origin::External(vec![])`, unlike its four
  siblings; not live-reachable, but inconsistent.
- **saga `Trigger`** — the author is recorded, but `saga_id` is caller-chosen,
  global, and a duplicate is a *silent* no-op (`:764`). Dispatch's saga ids are a
  pure function of public values, so pre-triggering one wedges that dispatch at
  `Pending` forever and makes the squatter its owner for `Cancel`/`Prune`.

### 2.4 Modules that reject the empty external origin

Yes: chat, agent, runs (admin family), vaults (with a regression test), forge
(tracker ops only), capability, dispatch (external family), identity, gateway,
lifecycle, valset, tagging, saga (`Accept` only).

**No:** automations (never inspects an origin), kv (structurally cannot), pages
(block/page ops and `MoveCommentThread`), tasks (task board), inbox, files
(`"ext:"` owns `/home/ext:/**`), forge `PushRefs`, governance `external_origin`,
saga `Trigger`/`Cancel`/`Prune`/`Reassign`.

---

## 3. Guards that claim what the code does not

Following the assumption audit's §B/§C convention. Each of these currently
teaches the next reader something untrue about authorization.

| # | location | the claim | the truth |
|---|---|---|---|
| G1 | `valset/src/lib.rs:4-8` | *"anyone holding a WELL-FORMED ed25519 key may `Join` … deliberately permissionless"* | `:354-358` rejects every `Origin::External`. The header describes a policy the module abandoned. |
| G2 | `valset/src/lib.rs:588` `permissionless_any_valid_key_joins` | permissionlessness | runs on `sys_ctx()` = `TestCtx::at_height`, whose origin is `Origin::System` (`sdk-testkit/src/lib.rs:63-69`). It never reaches the External arm. **A guard that guards nothing, whose name asserts the opposite of the module's current rule.** |
| G3 | `valset/src/lib.rs:349-350` | *"only a module origin (the governance module's follow-up after a passing proposal)"* | `:353` matches `Origin::Module(_)` — any id, and via §1.3 any external key. |
| G4 | `lifecycle/src/interface.rs:75`, `:78`, `:90`, `:97` | *"`Origin::Module(\"governance\") \| System` only"* | `lib.rs:281` matches `Origin::Module(_)`. |
| G5 | `identity/src/lib.rs:404-406`, `interface.rs:163-168` | *"client standing changes only via governance"*, *"an external key cannot self-grant"* | `:409` is `Origin::Module(_) \| Origin::System => Ok(())`. Both halves false. |
| G6 | `tagging/src/lib.rs:18-23` | *"every op is MODULE-ORIGIN ONLY, and the trusted party is always the dispatch origin, **never a payload field**"* | `TagEvent.author` is a payload field carrying trust (`:242`, `:283`). |
| G7 | `capability/src/lib.rs:37-38`, `interface.rs:206-208` | *"the claimant is the verified `Origin::Module` id, never payload data"* | True of `env().origin`; false of the author, once a continuation supplies that origin. |
| G8 | `dispatch/src/lib.rs:33-35` | *"the receiver of a delivery is always the module that dispatched — `Dispatch` is module-origin-only — so results route by construction, never by configuration"* | `host/src/lib.rs:1191` lets an external key author that module origin. |
| G9 | `sdk/src/lib.rs:169-170` | `Relay.author` is *"the identity a target module MUST authorize against"* | No module reads it. |
| G10 | `chat/src/interface.rs:207-212` | `as_agent`: *"an external or system submitter setting `as_agent` is REJECTED"* | Through a continuation an external submitter **is** a module origin, so the refinement succeeds and the author is forged. |
| G11 | `pages/src/interface.rs:165-167` | *"authorship is derived from the dispatch origin, never a payload (mirrors the chat module)"*, above the comment ops | `MoveCommentThread` derives no author at all. |
| G12 | `chat/src/lib.rs:419-421` | `check_channel_admin`: *"an unowned (module/system-minted) channel admits any user, **mirroring `SetMembership`'s trust posture**"* | The broken op is cited as the justification for a second op's laxity. This is how the class propagated. |
| G13 | `2026-07-17-continuation-transactions-design.md:305-307` | *"the sdk ships a `require_module_origin(ctx)` helper enforcing both so no module hand-rolls it wrong"* | The helper is not in the sdk. Ten modules hand-roll their own, none checking `relay()`. |
| G14 | `saga/src/lib.rs:110-112` | `LeasePolicy::Open` is *"the honest default until frames are signature-verified"* | Frames are verified (`node/src/lib.rs:267`). Production is `Strict` (`guest.rs:63`); the stale default is latent. |
| G15 | `automations/src/lib.rs:3`, `:60-61` | *"an **operator** registers rules"* | No operator concept exists in the module. |

---

## 4. What membership actually gates today

**Verdict: `SetMembership` is a write-integrity bug, not a live disclosure.** A
channel roster is not a confidentiality boundary today, and by design cannot be
one until the private-messaging ADR is built.

Evidence, in order of strength:

1. **The only consensus reader of membership is a write gate.** (STATIC.)
   `member_key` records are read in exactly one decision:
   `check_post_policy` (`chat/src/lib.rs:355-374`), reached from `PostMessage`,
   `AddReaction`, `RemoveReaction`, `JoinHuddle` and `SweepHuddle`. There is no
   other `is_member` call site in the module or the tree.
2. **The read surface takes no caller.** `Chat::query` is
   `async fn query(&self, req: &[u8])` (`chat/src/lib.rs:1219`) — no `ctx`, no
   origin, no filter. `ChatQuery::{Channel, MessagesRange, Message}` serve any
   channel to any caller. The index tier's `ChatViewQuery::Members` is likewise
   unfiltered.
3. **Every member node holds every byte.** Chat is a store-backed qmdb tenant;
   the module doc states a joiner "rebuilds the concrete store from a peer
   (`QmdbStore::sync_from`)" (`chat/src/lib.rs:23-27`). Message bodies are
   plaintext in that store. Replication hands every member the full history of
   every channel regardless of roster.
4. **The ADR says so, and said so seven weeks ago.**
   `docs/adr/2026-07-06-private-team-messaging.mdx:38-44`:

   > "The gap, confirmed in code (still true as of this ADR): chat bodies are
   > plaintext in qmdb and `ChatQuery` serves any channel to any caller;
   > `PostPolicy::MembersOnly` gates writes only; **`SetMembership`/`RegisterHook`
   > accept any origin**; no owners, no private channels, no DMs."

   And on the fix-governance-only option: *"Fails the requirement outright — any
   member node reads the plaintext replica; curtains, not locks."*

So the finding is not new information about the system's confidentiality — it is
a **known, recorded, deliberately deferred gap**. What is new is that it now has
a consumer: PR #835 needed membership to define a *set*, and could not use it.

**Do not soften it, either.** Within the write plane the defect is real and
unbounded: `MembersOnly` is chat's only admission rule, it is the rule the pty
gate wanted, and it is self-service. The one place membership would also have
been a *read* barrier — `runs::RequestRun` pulling a channel transcript into an
agent envelope (`dispatch_flow.rs:141`) — has no membership check either, so
fixing `SetMembership` alone does not close that path.

---

## 5. Ranked defects

Ranked by what an attacker gets × reachability today, not by fix cost.

| # | defect | attacker gets | reachable today | fix cost |
|---|---|---|---|---|
| **R1** | **valset `Join`/`Leave`/`Grant`/`Revoke` behind an `Origin::Module(_)` gate** (`valset/src/lib.rs:352-359`) + the continuation lane | **Network takeover.** `Join{self}` then `Leave{each other validator}` — the last-validator guard (`:211-222`) stops only the final removal, by which point the attacker is the remaining validator. Also a one-message liveness kill on a 2-node network. No governance proposal, no vote. | **Yes** — any resident or client standing. `verify_relay_submit` ignores the continuation (`relay.rs:212`). | valset is **native**: one line (`author_origin()`) + one refusal of `relay().is_some()`. Host-side fix is smaller still (§6). |
| **R2** | **identity `GrantClient`/`RevokeClient`** (`:407-414`) | Mint submit-door standing for arbitrary keys — i.e. manufacture the standing R1 requires — and strip it from others. | Yes, same lane. | wasm. Blocked on §6. |
| **R3** | **lifecycle `ScheduleSwap`/`ScheduleRegister`/`RegisterModule`/`CancelSwap`** (`:279-287`) | Schedule arbitrary module code onto every node. Consensus-code substitution. | Yes, same lane. Gated in practice by whatever `SwapReady` quorum the swap requires — worth confirming before ranking it below R2. | lifecycle is **native**. |
| **R4** | **dispatch saga-callback route** (`:855-859`) + `Dispatch`/`Cancel`/`Reassign` | Forge the *outcome* of any in-flight dispatch; correlation ids are publicly queryable (`saga_id_for`, `:59-67`). Cancel another module's work. | Yes, same lane. | wasm. |
| **R5** | **chat's module-author trust path** — `check_channel_admin` (`:417-428`), `check_post_policy` (`:369`), `as_agent` (`:1101-1114`) | Rename/archive any channel including another user's; post into any `MembersOnly` channel; **forge an `AuthorRef::Agent`**, which propagates into `ChatEvent::MessagePosted`, the tagging plane, and every hook module. | Yes, same lane. | wasm. |
| **R6** | **chat `SetMembership`** (`:1206-1210`) — the reported finding | Add anyone (including self) to any roster; defeat `MembersOnly` without any continuation trick. Write-integrity only (§4). | **Yes, trivially** — an ordinary signed op, no lane trick. | wasm, but Class 1: pass `&author` and call `check_channel_admin`. Genesis flag day. |
| **R7** | **chat `UnregisterHook`** (`:1202-1205`) | Silently disable every automation on any channel. Has a live consumer. | Yes, trivially. | Same commit as R6. |
| **R8** | **inbox `Clear`/`MarkRead`** (`:318`, `:321`) | Permanently delete any member's notification queue, unattributed. | Yes, trivially. | Class 1, but blocked: `member` is an opaque string, not `actor_string()` domain, so there is no canonical form to compare an origin against (`interface.rs:11-13` admits this). Needs an identity decision first. |
| **R9** | **automations `CreateRule`/`SetEnabled`/`DeleteRule`** (`:899`, `:903`, `:905`) | Mint state that posts to any channel / creates tasks / delivers to any inbox under `Origin::Module("automations")`; disable or delete anyone's rule. Accepts the unauthenticated empty origin. | Yes, trivially. | Class 1 + needs an `owner` field on `Rule` → record shape change, genesis flag day. |
| **R10** | **capability `ClaimClass`** (`:379-387`) | Permanently claim a routing class as another module. **No unclaim exists** (`interface.rs:19-21`), so it is unrecoverable consensus state. | Yes, same lane. Low value today — the class plane has zero non-test callers (assumption audit C6). | wasm. |
| **R11** | **tagging `Tag`'s payload `author`** (`:232`, `:242`, `:283`) | Forge the author of an engagement event into every subscriber, incl. runs. | Requires a module origin — i.e. the continuation lane. | wasm. |
| **R12** | **pages `MoveCommentThread`** (`:286-325`), **`RemoveBlock`** comment purge (`store.rs:110`) | Detach every thread from a block; hard-delete author-owned comments that sibling ops gate on the stored author. | Yes, trivially, incl. empty origin. | Class 1. |
| **R13** | **forge `PushRefs`** (`lib.rs:540`) | Move any branch of any repo to any oid; create repos. CAS prevents lost updates, not force-pushes. | Yes, trivially, incl. empty origin. | forge is **native**. Needs a repo-owner concept that does not exist. |
| **R14** | **forge `SetItemState`/`MergePr`** (`:621`, `:639`) | Close/reopen any item; merge any open PR. | Yes, any authenticated member. | Decide intent first — it may be deliberate. |
| **R15** | **runs `execute_kind`** (`module_impl.rs:182`) | Reach `ExecuteKind::Result`, `Engagement`, `Jobs`, `Agent`, `Saga` — deliver a forged agent result into a run's chat channel with agent attribution. | Yes, same lane. | wasm; and the fix is not a blind substitution — `author_origin()` on a continuation returns `External`, which reroutes to `Admin`. That is the right answer but it changes which arm legitimate traffic reaches. |
| **R16** | **saga `Trigger` id squatting** (`:764`, `:835`) | Wedge any predictable dispatch at `Pending` forever and own its `Cancel`/`Prune`. | Yes, trivially. | Namespace the id by origin, or make a duplicate an error. |
| **R17** | **kv `Set`** (`:157-159`) | Overwrite any key. | **No** — kv is not in `PRODUCTION`. Simnode `--with-valset` and `DEMO` only. | Trivial once someone decides whether kv is an unowned public scratch map (in which case: say so in the header). |
| **R18** | **governance `external_origin` accepts `External(vec![])`** (`:396-403`) | Nothing today — the empty origin is the host's pre-consensus default and never arrives from `decode_frame`. | No. | One line; do it when the file is next open. |

`vaults` (7 ops, all correct, immune to the continuation lane by refusing module
origins) and `gateway` (6 ops, subject always re-derived from the origin) are the
two reference implementations. Neither needs a change.

---

## 6. What the fix actually is

Three candidate shapes. They are not alternatives — (a) is a prerequisite for
(b) being meaningful, and (c) is the cheapest thing that closes R1-R5, R10, R11,
R15 at once.

**(a) Per-module: `env().origin` → `author_origin()`.** What the design doc
prescribes. It is correct for the three native modules (valset, lifecycle,
forge) and **a silent no-op for the other seventeen**, because the WIT world has
no relay slot (§1.4). Doing this to a wasm module without (b) would ship a change
that reviews as a fix and is not one — the exact defect this campaign exists to
find.

**(b) Plumb `relay` into the WIT world** (`module.wit` + `guest-adapter` +
`wasm-host`), then do (a) in 17 modules, then add the `require_module_origin`
helper the design doc already specified to the sdk. This is the doc's design,
fully built. It is a large, multi-module, genesis-moving change: 17 wasm
rebuilds, i.e. the 16-module flag day the task warns against.

**(c) Fix it at the host, where all callers route through.**
`crates/kernel/host/src/lib.rs:1191` is the single line that manufactures the
forgeable identity. Two sub-options:

- **c1 — the continuation dispatches under its real author.** Replace
  `let corigin = Origin::Module(msg.target)` with the parent's `origin`. Then
  `env().origin` is the authenticated submitter on every lane, every module's
  existing gate becomes correct with **zero module changes, zero WIT changes,
  zero genesis movement in `crates/modules/`**, and `author_origin()` becomes
  redundant rather than mandatory. The sending lane is still available to anyone
  who wants it, in `relay.parent_target`. Cost: it contradicts the design doc's
  "the dispatch Origin is `Module(parent_target)` — that is the LANE, useful for
  tracing and rate policy, and deliberately segregated." Nothing reads that lane
  today.
- **c2 — delete the continuation lane.** It has **zero production producers**
  (§1.5): no CLI, service, or module composes an envelope, and every shipped
  `encode_frame` passes `None`. What ships today is an attacker-reachable decoder
  and a forgeable-identity synthesizer with no consumer. Under the repo's own
  "No Legacy, No Compat" and YAGNI, an unused lane that grants network takeover
  is deleted, not hardened — and re-added with `author_origin()` wired when the
  reentry flow it was designed for actually lands.

**Recommendation: c1 or c2 first, as its own PR, before any module change.** It
is a `crates/kernel/` change with no `crates/modules/` diff, so no genesis
movement and no wasm rebuild. It closes R1-R5, R10, R11 and R15 in one place.
c2 is the lazier and more honest of the two; c1 is the one to take if the
envelope reentry flow is imminent. **This is a boundary change, so it is
ask-first** — I am not choosing between c1 and c2 unilaterally.

Class 1 (R6, R7, R9, R12) is unaffected by any of that and needs the ordinary
fix: pass the author down and compare it. `chat` already owns the right
comparison (`check_channel_admin`); `automations` and `inbox` do not have a
principal to compare against and need one designed first.

**The structural guard worth adding with the fix**, in the repo's own idiom (a
source-parsing lint test, not a comment): a test that fails when a module's
`execute` binds an author and any arm's handler does not receive it. The
mechanism in §1.2 is invisible to rustc and clippy by construction; only a lint
that reads the match can see it.

### 6.1 Mutation-boundness

Phase 1 changed nothing, so nothing here is EXECUTED. Recording what each fix
must redden, so phase 2 can be held to it:

| rule | the test that must go red when the rule is broken | exists today? |
|---|---|---|
| a continuation cannot reach a module-origin-gated arm | — | **no.** `continuation_inline.rs` *echoes* both identities and asserts they differ; nothing asserts a module refuses. |
| valset membership requires governance | `permissionless_any_valid_key_joins` (`valset:588`) | **no, and worse** — it runs under `Origin::System`, so it never reaches the gate, and its name asserts the opposite rule (G2). |
| only the channel owner sets membership | — | none. `members_only_channels_gate_external_posts_and_reactions` (`chat/tests/channel_system.rs:846`) tests the *consumer* of membership, not who may write it. |
| a hook may only be (un)registered by the channel owner | — | none. |
| an inbox may only be cleared by its member | — | none. |
| a rule may only be disabled by its creator | — | none (there is no creator). |

Six rules, one guard, and that guard is inverted.

---

## 7. Phase-2 sequencing

**Split it. Three PRs, in this order.** The security fix ships first and the
capability restoration follows — the instinct in the task is right, and the
reason is stronger than "admission precedes delegation":

**PR-A — the host lane (c1 or c2).** `crates/kernel/` only. Zero
`crates/modules/` diff, so **no genesis flag day**, no wasm rebuild, no pin
change. Closes R1-R5, R10, R11, R15. It is independently reviewable, it is the
only fix on the list that is a network-takeover fix, and it is the one that
should not wait behind a chat change. **This must not be bundled with a chat
module change**, because the moment `crates/modules/` moves, the diff stops
being reviewable as "did this close the escalation" and becomes "did this move
the root hash correctly."

**PR-B — the Class 1 author-gate, `chat` only.** `SetMembership`,
`RegisterHook`, `UnregisterHook` gated through `check_channel_admin`. This is the
genesis flag day: one module's `component.wasm` rebuilt (**not** `make
wasm-modules`), `GENESIS_ROOT_HASH` re-pinned in the same commit, the value taken
from `production_genesis_root_hash_is_pinned`'s own failure output and never
transcribed, `git diff origin/dev --name-only -- crates/modules/ crates/guests/
crates/examples/` against the base showing exactly `crates/modules/apps/chat/`.

One design decision PR-B must make, and it is not cosmetic: **`check_channel_admin`
lets any user administer an *unowned* channel** (`owner: None`, i.e. every
module- or system-minted channel, including forge's `forge:<repo>:<n>`) —
`lib.rs:417-428` — `Some(owner) if owner != user => Err` at `:420`, `_ => Ok(())`
at `:424`, so `owner: None` falls through the wildcard. Gating
`SetMembership` on `check_channel_admin` as-is therefore leaves module-minted
channels wide open, and PR-B would ship a flag day that does not close the hole
on the channels forge and runs actually create. Either close that arm in the same
PR or state explicitly why an unowned channel is public.

**PR-C — widen the pty gate.** Only after PR-B, because until then a participant
set is not expressible. The blocker is already named in the file
(`bin/noded/src/term_consensus.rs:57-66`), so PR-C is a doc-and-gate change in
one file with no consensus movement.

Two things PR-C should not assume:

- **`{owner}` is currently the *correct* answer, not merely the available one.**
  A `Shared` session spends the host's own env credential (PR #835's finding 1),
  which has no grant record and therefore no grantee. Widening to "channel
  members" widens who spends the operator's personal subscription. The right set
  may be narrower than "everyone the roster admits" even once the roster is
  trustworthy — that is a product question, not a security one, and it should be
  answered before the gate moves.
- **`Channel.owner` is `None` for module-minted channels.** `term-<id>` channels
  are node-minted under a `User` origin so they do have an owner, and #835 fails
  closed (`channel_unowned`) otherwise. Whatever set PR-C adopts must keep that
  fail-closed property.

**Not in any of the three:** R8 (inbox) and R9 (automations) need a principal
that does not exist — a canonical member-identity domain for inbox, an `owner`
field for `Rule`. Both are record-shape changes, i.e. their own flag days.
R13/R14 (forge) need an intent decision first. R17 (kv) needs someone to say
whether kv is an unowned scratch map. None of them should ride PR-B to save a
flag day; a flag day is free today and a bundled one is unreviewable.

---

## 8. What this sweep did NOT cover

- **`crates/labs/`** (multisig, evm) — out of the stated scope and not in any
  topology.
- **The `hello` and `directory` example modules**, which ARE in the production
  genesis set (`topology.rs`). They were not audited.
- **Whether `lifecycle`'s `SwapReady` quorum actually blocks R3 in practice.**
  The gate exists (`:499-504`); whether a forged `ScheduleSwap` can reach
  activation without honest validators signalling was not traced. R3's rank
  depends on it.
- **Nothing was run.** No mutation was applied, no test executed, no live node
  touched. R1 is reproducible on the dukenet pair with a ~30-line frame builder
  and should be, before PR-A is designed — a live confirmation of a network
  takeover is worth more than any amount of further reading.
- **The read plane.** This sweep asked "who may write". Section 4 touches reads
  only where membership was the question. `runs::RequestRun`'s unmembered
  transcript pull (R-list §2.3) suggests the read side deserves its own pass.

## Counts

| category | count |
|---|---|
| write ops classified | 130, in 20 modules |
| OK (gated / self-scoped / no-principal) | 88 |
| PARTIAL (author kind checked, subject caller-supplied) | 30 |
| DEFECT — Class 1, the author never reaches the arm | 12, in 6 modules |
| DEFECT — Class 2, the author reaches the arm and is forgeable | ~30 arms, in 12 modules |
| modules calling `ctx.author_origin()` | **0** |
| hand-rolled origin helpers, none checking `relay()` | 10 |
| false authorization claims in doc comments | 15 |
| guards that guard nothing (of those found) | 1 (`permissionless_any_valid_key_joins`) |
| rules with no guard at all | 6 (§6.1) |

Everything above is STATIC except §4's disclosure verdict and R3's ranking, which
are PLAUSIBLE, and §1.3's lane behaviour, which is asserted by tests CI runs
(`host/tests/continuation_inline.rs`) rather than by me.
