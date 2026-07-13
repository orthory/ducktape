# simnode scenario expansion — the candidate ledger

2026-07-13. Continuation doc: pick items off this list in future sessions.

**CAMPAIGN COMPLETE (2026-07-13)** — every enumerated item resolved across
nine PRs; ~80 scenarios now stand across 11 Rust suites
(`bin/simnode/tests/`) and 11 TS suites (`app/src/test/sim/`). PR map:
- **#539** C2–C7 (module_gaps.rs, 9 tests).
- **#540** E1+E6 → B1–B10 (--with-valset genesis, --invite-binding, hex:
  origins; governance_scenarios.rs, 12 tests). Targeted-invites (#545) later
  reshaped InviteToken and repaired the suite in #549, as this ledger
  prescribed.
- **#542** C1, D1–D3, A7, E5 (five TS suites + harness consolidation).
- **#543** A1, A3–A6 (reactor_seams.rs; A2 was satisfied by #539's
  task-id-collision abort test).
- **#546** E2/E3/E4 + C8 (signed-frame lane, multi-op peer blocks, --node-key;
  frame_and_batch.rs, 10 tests).
- **#547** follow-ups (gateway into the sweep → 14/16, session_lane.rs,
  share_governance.rs, upgrade slot reuse, kv smoke; 13 tests).
- **#552** TS round 2 (huddle membership, account writes, D4 networked
  persona, D6 supersede; 8 tests).
- **#556** layout refactor (user feedback): suites moved to
  `app/src/test/sim/<surface>.test.tsx`, short names (index→indexes). Old
  paths below are historical.
- **#557** round 3 (gateway_registry.rs, SetShares mid-mode, CancelUpgrade
  incl. the boundary-block two-lane pin, batch-vs-singles conformance;
  7 tests).

**Discoveries the campaign banked** (beyond the per-item corrections below):
- **Injection-ordering seam (PR #557 review→test)**: at the activation
  boundary, the single-op drain queues the system `Advance` AHEAD of the
  root op's follow-ups (a governance cancel's follow-up finds the slot
  already cleared, "no matching pending", and the abort rolls the Advance
  back), while the batch engine drains a member's follow-ups BEFORE its
  step-4 injections (the same cancel hits "activation height already
  reached" and the block's own Advance settles the slot). Both pinned in
  one test.
- **Batch-invariance boundary**: "content-only AND block-coordinate-free →
  batch-invariant" — not "plain vs qmdb". kv/pages (qmdb Sequential) split
  roots on commit boundaries; duckdns (content-only sha256) is
  byte-identical (the standing-insurance assert); tasks/inbox differ because
  they stamp consensus_time.

**Still-open candidates for future sessions** (recorded, deliberately not
built): modreg code-swap ladder (needs modreg + a component blob in the sim
genesis); gateway RouteAudience sorted-uniqueness + manifest_sha256 hex
gates; a property pin that tasks/inbox stamp consensus_time deliberately;
/sim saga-lease helper (tooling); D5 muted/watch/mention decisions as an
app/src-tauri notify unit test (outside simnode); TS harness promotions
(authorAsNodeKey, bindAccount, boot({dialUrl})); huddle roster scenarios
naming the --node-key key in a --with-valset roster; whether consensus
should bound sweep_huddle staleness (product question).
- **TS layout refactor — PR #556 MERGED (user feedback)**: the eleven
  scenario suites moved from `app/src/console/store/simnode.<x>.scenario.test.tsx`
  to `app/src/test/sim/<x>.test.tsx` (index→indexes). Old paths in this doc
  are historical.
- **D2's #426 asymmetry is GONE (external PR #555)**: edit_comment gained a
  mentions plane ("refine collaboration and comments") — our pinned
  characterization test fired exactly as designed and was flipped by that PR
  to expect engagement. The asymmetry pin served its purpose; scenario now
  covers the fixed behavior.
- **Noted product observation (PR #552 review)**: `stage_sweep_huddle`
  enforces NO staleness in consensus — any member may evict any named
  identity in an open channel (post-policy gated only); "stale" is a
  client-level judgment in huddle-window.ts. Pinned as current behavior;
  whether consensus should enforce a staleness bound is a product question.
- **C8 ANSWERED (PR #546)**: the #536
  byte-identical-resubmit swallow lives in the ORDERED lane's exactly-once
  FrameId gate (`sha256(frame bytes)` in `crates/kernel/node`; seq is the
  tie-breaker because it changes the bytes) — machinery the sim's direct-host
  lane deliberately lacks; the sim honestly pins files' per-path CAS
  ("conflict: … changed since base") + frame-lane authorship
  (`ext:<signer hex>` via `env.origin.actor_string()`).
- Follow-ups (gateway sweep 14/16, session budgets, share governance,
  upgrade slot reuse, kv smoke): PR #547 open (review in flight).
  **Realities pinned**: AdoptShares AUTO-ENABLES share mode (doc implies a
  separate SetShareMode — doc nuance); share-mode ballot principal = the
  submitting node's BOUND ACCOUNT (`account_of_node`), power = shares;
  share-mode Signal freezes ParticipatingMajority{ceil(n/2)}, structural
  actions Threshold{ceil(2n/3)}; `MAX_ACTIONS_PER_SESSION=32`; KvMsg has NO
  delete op (empty Set ≠ absence).
- **NEW ideas (batch E/F)**: (1) a batch-vs-singles conformance test
  asserting the app-hash gap is entirely the qmdb commit-boundary merkle's —
  catches a module accidentally authenticating block structure; (2)
  `--node-key` + `--with-valset` combo naming the seeded key in the roster →
  huddle-membership consensus scenarios; (3) gateway registry over the wire:
  Get/List, per-name revision CAS ("revision must be N+1"), route=None
  authenticated tombstone, policy/audience rejections; (4) governance
  SetShares mid-mode re-weighting + CancelUpgrade freeing the slot
  pre-boundary (cancel path vs the pinned abort path).
- **E1+E6 → B1–B10 DONE — PR #540 MERGED** (`--with-valset` genesis,
  `--invite-binding`, `hex:` origins; 12 governance/invite/upgrade tests).
  NOTE for the targeted-invites work in flight elsewhere: reshaping
  `InviteToken` (target/role/expires) must update `governance_scenarios.rs`'s
  mint()/redeem() in the same change — the suite pins the 3-field token.

**A1 ANSWERED (PR #543)**: the self-retriggering rule is stopped by
automations' own loop-prevention guard (rules fire only on `AuthorRef::User`
posts; the follow-up post is module-authored) — NOT the host budget. The raw
`Error::BudgetExceeded` path stays unreachable from the wire through this
route; if a wire-reachable deep-emit chain ever appears, pinning the budget
abort becomes a new scenario.
**A5 partial by design (PR #543)**: 13/16 modules swept; `files` (binary
op-frame, not JSON-expressible over /v1/submit), `gateway` (SetRoute needs a
MemberAuthorization signature ceremony — FOLLOW-UP CANDIDATE: build the
ceremony like identity's and fold gateway into the sweep), `forge`
(libgit2-on-disk, repo-internal determinism) excluded with reasons.
Answered during recon: **B8's open question** — the host itself
(`crates/kernel/host/src/lib.rs` drain/apply_block) injects the once-per-block
`Origin::System` upgrade `Advance` / modreg `Advance` / `DeliverPending`,
"INERT until the module is registered" — so registering upgrade in the sim
genesis gets the boundary tick for free; E1 needs no injection plumbing.
Context: the QA doctrine is **UI/UX+integration → tauri-agent/fleet, node/module
semantics + races → simnode**, and simnode now has two scenario lanes:

- app lane (TS, PR #530 MERGED): `app/src/test/sim-scenario.tsx` harness +
  `app/src/console/store/simnode.{pages,comments,forge,index}.scenario.test.tsx`
- core lane (Rust, PR #534 MERGED): `bin/simnode/tests/harness/mod.rs` +
  `core_scenarios.rs` (jobs races, automations cascade, inbox, identity→duckdns)

**Why simnode and nothing else**: module unit tests fake the `Ctx`; noded's
`daemon_e2e` cannot control WHEN blocks commit; live QA can't either. simnode
is the ONLY harness that feeds blocks through the REAL host + full module
registry with scripted ordering (hold/step, peer-block, logical clock) — so
cross-module integration and the reactor itself are e2e-testable *only* here.
The host IS the reactor: `crates/kernel/host/src/lib.rs` — routes a Msg to its
module, drains emitted follow-ups FIFO under `MAX_DISPATCHES = 1024`, aborts
the WHOLE block on any module error (P2 atomicity), recomposes the app-hash.

Priority key: (a) past-incident trap, (b) unreachable by any other lane,
(c) security/data-loss semantics.

---

## A. Reactor / host integration seams (the "feed blocks e2e" tier)

These pin the HOST's cross-module machinery, not any one module. All core-lane
(Rust) unless noted.

1. **Dispatch-budget exhaustion surfaces as a clean rejection.** A cascade
   that exceeds `MAX_DISPATCHES` (1024) must reject the block
   (`Error::BudgetExceeded`), not wedge or partially commit. Needs a way to
   compose a deep emit chain from the wire — automations PostMessage → chat
   hook → automations is a 2-cycle building block; check whether a
   self-retriggering rule (rule posts into its own hooked channel) is the
   natural infinite loop, and pin WHAT stops it (automations' own guard? the
   budget?). If automations guards it, that guard is the scenario; if the
   budget fires, the abort atomicity is.
2. **Abort-block atomicity across modules (the P2 contract).** A follow-up
   failing DEEP in a cascade must erase the triggering op too — nothing from
   the block survives, no trace in any module root. #534's automations
   id-squatting note (item C3) is the concrete instance: pre-post the rule's
   composed message id, then the triggering post's ENTIRE block aborts.
   Assert: post rejected, message absent from chat, run-history absent, tasks
   absent, app-hash unchanged.
3. **Oracle drain discipline** (partially covered by the TS echo-oracle
   tests): multiple queued oracle follow-ups drain ONE per step as their own
   blocks (`kind: "oracle"`), never coalesce, and survive interleaved peer
   blocks. Rust-side version with 2+ queued follow-ups + a peer block wedged
   between drains.
4. **Saga wedge cap.** Batch-6 refactor added a "saga-wedge cap"
   (`crates/system/saga`) — find the cap, compose a wedged saga over the
   wire, pin the rejection. (Nobody has ever exercised it e2e.)
5. **Whole-registry determinism sweep.** Extend `same_script_same_app_hash`
   from chat+tasks to a script touching ALL 16 registered modules — the cheap
   standing insurance against nondeterminism (HashMap iteration, wall-clock
   reads) creeping into any module. Also run the same script through auto vs
   stepped (the existing two-path test only covers 3 modules).
6. **Sim restart/resume.** "the height resumes above the index watermark" is
   sim-only code with zero tests: run a script, kill, respawn on the SAME
   storage dir, assert height continuity + app-hash stability + module state
   survives (per-module `install`/statesync handles get exercised through the
   real boot path).
7. **Scoped hydration map correctness (TS).** `refreshScoped` refetches only
   slice groups whose module roots changed (`changedModules` → `scopeFor`).
   A wrong mapping = silent staleness. Scenario: peer-block a pages op while
   watching chat state, assert pages slices update; a module NOT in any scope
   group (e.g. a jobs op) must still advance status/blocks without clobbering
   held slices.

## B. Invitation + membership plane (the user's headline ask)

The invite scheme is **pure consensus-module semantics** —
`crates/system/governance/src/invite.rs`: minting IS the admission decision;
token = issuer ed25519 sig over `binding ‖ nonce`
(`INVITE_GRANT_NAMESPACE = b"ducktape-invite-grant-v1"`); joiner proves
possession over `binding ‖ nonce ‖ joiner`
(`INVITE_JOIN_NAMESPACE = b"ducktape-invite-join-v1"`); `GovMsg::Redeem`
re-verifies both sigs, requires the issuer to be a CURRENT member, enforces
SINGLE-USE via the redeemed-nonce set in consensus state, and emits
`ValsetMsg::Grant { joiner }` in the same block. Helper fns
(`verify_invite_token`, `sign_join_proof`, `verify_join_proof`) are exported —
the test ceremony is the same shape as #534's identity bind (ed25519
`from_seed` + namespace sign), already proven cheap.

**Precondition — sim genesis extension (see F1).** governance/valset/upgrade
are not in simnode's genesis (they're bin/node-only; registration reference:
`bin/node/src/host_state.rs:227-248` — `Valset::new("valset")`,
`Governance::new("governance", "valset", "upgrade", "identity")`,
`Upgrade::new("upgrade", "valset")`).

Scenarios once F1 lands:

1. **Redeem happy path**: mint token in-test, Redeem over `/v1/submit` →
   ValsetMsg::Grant emitted SAME block → joiner appears in Residents query;
   RedemptionView audit trail (nonce/joiner/issuer/height).
2. **Single-use**: second Redeem of the same nonce → rejected (the
   exactly-once key). THE invite-security property.
3. **Binding mismatch**: token minted for another network's binding
   (chain-id + genesis fingerprint) → rejected. Cross-network replay.
4. **Non-member issuer**: token from a key that is not a current member (or
   was removed after minting) → rejected. Revoked-member invites die.
5. **Proof-of-possession forgery**: valid token + wrong joiner proof →
   rejected ("a blob holder cannot redeem under a key that never asked to
   join").
6. **Staged admission ladder**: Grant (resident standing) → governance
   Propose/Vote/Execute admits validator (`ValsetMsg::Join` — a resident key
   is PROMOTED, removed from resident set in the same boundary) → removal
   ballot demotes. The whole admitMember/promoteMember/demoteMember console
   flow, deterministically.
7. **Governance lifecycle**: Propose (electorate-gated), Vote (change ballot
   while open), Execute (deadline via the LOGICAL clock — `voting_period` in
   consensus_time is deterministic in the sim!), early passage only when no
   remaining ballot can reverse; Threshold vs ParticipatingMajority rules;
   share-mode flip (AdoptShares/SetShares/SetShareMode).
8. **Upgrade module**: governance ScheduleUpgrade → SignalReady per member
   (R=n readiness) → the `Advance` boundary tick arms or aborts at
   activation_height. `Advance` is `Origin::System` injected at the height
   boundary — check whether the sim's block path injects it (bin/node's
   replica does); if not, that injection is part of F1. This would make the
   /upgrade skill's semantics testable without a live 3-validator roll.
9. **Leave/self-removal**: requestLeaveWorkspace's chain half — pending
   self-removal + remaining-member approval, the "node must stay up through
   its own removal" invariant at the module level.

## C. Highest-value module gaps (no sim changes needed)

1. **pages movePageBlock + merge compensation (TS)** — the remaining #457
   heart: CycleMove ("move target is inside the moved subtree"),
   CrossPageMove, and the merge gate (a failed move resolves false → the
   editor must NOT fire the RemoveBlock; store-level: rival deletes the
   adopter mid-merge → move genuinely rejected → subtree intact).
2. **identity nonce replay (core)** — AddMemberKey bumps the nonce; replay a
   pre-bump bind/unbind cert → "authorizer certificate does not verify".
   Plus RemoveMemberKey last-member refusal, UnbindNode + re-bind flow.
   Ceremony already built in #534 (`ed_bind_auth`).
3. **automations PostMessage id-squatting (core)** — composed id
   `auto-{rule}-{channel}-{seq}` pre-posted by a rival wedges the rule: the
   probe (`crates/apps/automations/src/lib.rs:474-480` "message id already
   taken") fails the cascade and ABORTS THE TRIGGERING POST's whole block.
   Pin it as intended (or surface it as a bug — a rival can block a hooked
   channel's posts). Doubles as the A2 atomicity scenario.
4. **agent session-key ACL (core)** — #423/#429 put per-run ed25519 session
   keys + allowed_actions into consensus; no wire-level scenario asserts an
   out-of-ACL action is rejected. Echo-oracle + a run whose agent lacks
   `chat.post`.
5. **jobs authorization matrix + MAX_CLAIMS (core)** — Finalize/Release by
   non-claimant → "only the current claimant may release", Cancel by
   non-submitter, Prune on non-terminal; MAX_CLAIMS boundary: after N claims
   an expired reclaim FAILS the job instead of requeuing (exact-boundary
   assert like the lease test).
6. **forge ref-CAS (core)** — PushRefs births branches; a merge/push with a
   stale `prevTargetOid` → CAS rejection. Repo bear via `commit` op is
   proven (#530 review fix). Also `submitForgeReview` pinned to a
   `commitOid`.
7. **tagging direct-owner gate (core)** — `TaggingModule::with_direct_owner("runs")`:
   a direct tagging op from a non-runs origin → rejected (same pattern as the
   automations hook spoof).
8. **duckfs/files frame-lane semantics (core, needs F3)** — #536 made files
   commits user-signed: frame seq = tie-breaker, byte-identical resubmits
   SWALLOWED on the validator lane. simnode currently refuses the signed-frame
   lane ("the simulator serves no signed-frame lane"), so this needs F3.

## D. TS lane extensions (app-observable semantics)

1. **chat gates**: members_only posting (non-member rejected; creator
   auto-membership on create), archived-channel post/reaction/huddle-join
   rejections, rename owner-gate, thread panel resync-on-failure (the
   "deleted-but-still-there message" trap), toggleReaction add/remove.
2. **comment @mention → agent engagement** (#426's bug): comment mention
   invokes the agent via echo oracle; `edit_comment` has NO mentions plane —
   pin that asymmetry.
3. **account flows now unblocked**: #534 proved BindNode works over the bare
   wire — seed identity via direct submits/peerBlock so the TS lane can
   exercise `setDuckHandle`, `accountBindNode`, auto-bind outcome vocabulary
   (locked/deferred/failed), nodeUsers→accountId mapping.
4. **networked persona pass**: the module suites all ran the local persona;
   one pages/forge pass under height-only receipts.
5. **notification decisions**: rival message via peerBlock → muted channel
   suppresses, watch/mention policies notify.
6. **pageThreadsToken / search-token supersede**: needs a delayed-transport
   wrapper to reorder responses deterministically — possible in vitest
   (wrap `remoteTransport` and hold one response), medium effort.

## E. Sim feature work (tooling before tests)

1. **Validator-genesis preset** (`--genesis validator` or `--with-valset`):
   register kv/valset/governance/upgrade (+vaults/directory if cheap) beside
   the noded set, AND inject the `Origin::System` `Advance` tick at height
   boundaries. CONSTRAINT: the default genesis must stay noded's exact set —
   `sim-parity.test.ts` pins the status module list against noded; the
   preset must be opt-in per spawn. Unblocks all of section B.
2. **Multi-op blocks**: sim is one-op-per-block; batch super-frames /
   aggregated-block rendering (N ops per block in the explorer) are
   unscriptable. `/sim/peer-block` accepting an ops array (or a
   `/sim/step?batch=n`) unblocks aggregation scenarios.
3. **Signed-frame lane**: accept `/v1/frame` like noded (verify, stamp the
   signer key as origin) so C8 (#536 frame-seq/dedup semantics) and
   frame-origin authorship tests can run against the sim.
4. **status.publicKey seeding** (`--node-key <hex>`): huddle consensus ops
   (join/leave/sweep membership, NOT media) are gated on `status.publicKey`
   in the app; seeding a key unblocks membership-semantics scenarios without
   any audio.
5. (housekeeping) Consolidate `simnode.scenario.test.tsx`'s inline harness
   onto `sim-scenario.tsx` (#530 review follow-up).

## New items discovered during the campaign (2026-07-13)

- **C3 premise CORRECTED (batch A, PR #539)**: the PostMessage id-squat does
  NOT abort — the composed id embeds the `rule_id`, so a rival pre-post is
  always visible to the probe and is *downgraded* to a run-history no-fire
  breadcrumb ("message id already taken"), protecting the poster's block.
  That defense is now pinned. The GENUINE P2 abort path is a post-probe
  follow-up collision: two rules composing the same task id emit past each
  other's probes and the second collides at execute → whole block aborts.
  Pinned too (this is A2 satisfied).
- **C4 premise CORRECTED (batch A, PR #539)**: the session-key ACL is
  reachable WITHOUT the signed-frame lane — the sim honors caller-named
  origins, so `SagaMsg::Accept` claims the run lease, `OpenAgentSession`
  binds a 32-byte ASCII session key, and the action origin is those bytes.
- **NEW: session-lane scenario class.** The origin-honoring /v1/submit is a
  general lever for signed-frame-lane invariants: session budget exhaustion
  (`MAX_ACTIONS_PER_SESSION`), one-session-per-run, "only the bound key may
  act", cross-node lease-holder gates — all reachable without a mesh.
- **NEW (tooling candidate): a /sim saga-lease helper.** C4-class setup
  hand-derives `saga_id` as `dispatch\x1f{receiver}\x1f{dispatch_id}`; a
  /sim control op (or an exported `saga_id_for_dispatch`) would make the
  run-lifecycle lane less brittle.
- **NEW (batch D leftovers): account-share governance lane.** The one
  governance surface still unexercised: bind Identity accounts, AdoptShares →
  SetShareMode → account-keyed ballots (ParticipatingMajority), then flip
  back to validator mode. Door checks (phantom account, pre-adoption
  SetShares/SetShareMode) are pinned in PR #540; the full lane needs the
  identity bind ceremony + a mode switch.
- **NEW: second-upgrade-after-arm.** After a v2 arm frees the at-most-one
  pending slot, schedule to_version=3 and cross a second boundary in one sim
  run — exercises slot reuse + version monotonicity.
- **B7 finding (PR #540)**: validator-mode proposals freeze
  `Threshold{required_yes = total/2+1}` (majority of the snapshot), with
  early passage once undecidable-to-reverse — NOT ParticipatingMajority;
  that rule is the share-mode lane's.
- **C1 premise CORRECTED (batch C, PR #542)**: cross-page `MoveBlock` is
  FORBIDDEN by design (`PageError::CrossPageMove` — "a cross-page relocation
  is a delete+insert, never a move"); pinned as the rejection, not a working
  move.
- **D5 RE-ROUTED out of the TS lane (batch C)**: the muted/watch/mention
  notification decision lives in the Tauri-shell Rust notifier
  (`app/src-tauri/src/notify/{engine,matchers}.rs`), reached only via
  `notifyClient.configure` which is a no-op outside `isTauri()` — no
  TS-observable seam exists. Belongs in a Rust notify unit/integration test,
  NOT simnode. Don't re-attempt in vitest.
- **NEW: comment-mention engagement lane.** Comment @mentions reach the
  agent WITHOUT a channel watch (entity mentions → tagging → agent owner) —
  distinct from chat's watch-gated path. Expand: multi-agent comment
  mentions, `as_agent` reply-comment posting, run/dispatch record shape.
- **NEW: store account-WRITE tier blocked on E4.** `setDuckHandle` /
  `accountBindNode` landing + the locked/deferred/failed vocabulary need a
  self identity — the sim must expose a non-empty `status.publicKey`
  (ledger E4). E4 also unlocks huddle-join consensus ops and every
  self-identity comparison in the store.
- **OPS trap (worktree reaper)**: a freshly-created commitless worktree
  (branch tip == origin/dev) is fair game for a concurrent
  `ops/worktree-clean.sh --yes` — commit a stub immediately after
  `git worktree add`, or accept the race.

- **E6 — origin hex escape (tooling, folded into the E1 branch).** Governance
  `Propose`/`Vote` key ballots by `Origin::External(pubkey)` — a raw 32-byte
  ed25519 pubkey, which is not valid UTF-8, so the sim's JSON-string origin
  lane cannot express member-authored ops. Fix: origin strings prefixed
  `hex:` decode to raw bytes on the sim's /v1/submit + /sim/peer-block lanes
  (malformed hex fails loud). Unblocks B6–B9 and any future
  authenticated-author scenario (upgrade SignalReady, valset-gated saga).
- **B10 — valset direct-author gate.** valset accepts membership ops only
  from `Origin::Module(_) | Origin::System` (governance is the sole external
  path); a direct external `ValsetMsg::Join`/`Grant` must be rejected. Same
  shape as the automations hook-spoof scenario. (Folded into batch D.)
- **Candidate: modreg code-swap ladder.** Once modreg is registered under a
  sim preset, governance `UpdateModule` → armed swap → boundary modreg
  `Advance` flips the committed active hash — the wasm live-update
  consensus half, deterministically. (Deliberately left OUT of the E1
  preset; needs modreg + a content-addressed component blob in-test.)
- **Candidate: kv module smoke under the valset preset** — kv registers in
  the preset; a set/get/delete + determinism pass is nearly free there.

## F. Explicitly OUT of simnode scope (don't chase here)

Mesh/overlay/NAT traversal/coordinator rendezvous, statesync-the-transport
(RangePruned backfill wedge #493), invite-TUNNEL endpoints + pre-warm (#487),
huddle MEDIA, App Nap / platform behavior — all network/effect-layer; simnode
has no networking by design. The invite/admission CHAIN SEMANTICS (section B)
are in scope; the tunnel that delivers the blob is not. Live-join, callbed,
and multi-validator quorum remain fleet/live-QA territory.

---

**Recommended next bites**: C1–C4 (no tooling, all past-incident or security),
then E1 → B1–B5 (the invitation scheme end-to-end — the headline ask), then
A5/A6 (cheap standing insurance).
