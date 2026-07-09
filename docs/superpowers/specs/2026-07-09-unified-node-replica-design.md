# Unified Node — Replica Pipeline — Design

Status: SHIPPED through phase 4 (2026-07-10; PRs #291, #293, #296 + the
phase-4 closeout). Residents fold finalized frames, restart by journal
replay, cross epochs by in-loop follower swap, and promote from their own
state; state sync is join-time bootstrap only. Two deltas against the plan
below, both forced by reality and recorded in the PRs: the promotion
collapse (phase 3 here) landed IN phase 2 — a quorum-widening cutover halts
the source awaiting the promoted node, so wait-for-the-source promotion
deadlocks — and the phase-3 replay gate unearthed a pre-existing recovery
hazard (the legacy trailing roll-forward sealed observed mixed roots),
fixed in `crates/kernel/recovery`. Residuals: V2/BLS verifier construction
awaits valset key registration; cutover behavior under adversarial timing
beyond the e2e suite is future hardening. Where this document and shipped
code disagree, the code is authoritative.

## The model in one paragraph

Every node runs the same state-advance pipeline: receive finalized frames and
their certificates, verify the certificate against the epoch's validator set,
fold the frame through the host, journal it, and serve reads from the result.
A **validator** is a node that additionally votes; a **resident** is a node
that doesn't (valset standing, per the ACL design's principal model — which
this document does not touch). State sync remains exactly one thing: the
join-time bootstrap that stands a brand-new node up at a recent boundary.
This replaces the current split where validators fold blocks while residents
loop the joiner bootstrap — re-installing whole boundaries and never
executing anything — with everything that split costs: no per-block
continuity on non-validators, derived indexes healed instead of folded,
read-your-writes gaps, and a promotion dance (app-hash equality wait,
fabricated checkpoint, re-exec, post-reboot catch-up) that exists only
because a resident's journal is a fiction.

## Why this is nearly free: the seams already exist

The fold path is trait-shaped today. `node::OrderedNode` — the component
that WAL-journals a finalized frame (`BlockSink::pre_apply`), folds it via
`host.submit_block`, seals it with roots + app-hash, and hands it to the
per-block index fold — is generic over `node::Orderer`. Validators plug in
`consensus::SimplexOrderer` (the live engine). The replica plugs in a
**follower orderer** behind the identical trait; the fold, the journal
records (`Pinned`/`Block`/`Seal`/`Cutover`), checkpoints (`Manifest`), floor
certs, and boot replay (`recover_with_sink`) are shared byte-for-byte.

Everything the follower consumes already reaches every mesh peer:

- **Payload bytes** ride the payload-gossip lane (`ConsensusRelay::broadcast`
  sends to `Recipients::All`); `spawn_payload_drain` is the existing
  store-only intake into the `ContentStore`, whose content addressing makes
  garbage unmatchable to a finalized digest.
- **Finalization certificates** ride the cert lane — the same lane the
  resident head-wake (merged f6e77827) already listens to, undecoded. The
  follower decodes (`decode_finalization`) and **cryptographically verifies**
  them (below).
- **Missed gossip** is covered by the existing resolver fetch lane
  (`PayloadProducer`/`PayloadConsumer` + mailbox); **deep gaps** by the
  statesync `Frames` lane (paginated `fetch_frames`, `RangePruned` when below
  the server's retained suffix).
- **Epoch coordinates** come from the shared, engine-free
  `ValsetOrchestrator` (observe/respawn-if-due state machine) plus the
  journal's `Cutover` records — a follower tracks `(epoch, view_base,
  participants)` exactly as recovery already does; it just never starts an
  engine with them.

Two small seams are missing in the consensus crate, and they are the whole
Phase 1: `FinalizedInbox`'s mutating methods are crate-private, and every
`SimplexOrderer` constructor starts an engine — a follower needs one
engine-free constructor wiring reporter + inbox + content store + resolver.

## Trust model — verify every height (and close today's gap)

The follower verifies each finalization certificate with the commonware
surface that is public and ready but currently unwired in ducktape:
`Scheme::verifier(namespace, participants)` + `Finalization::verify` (quorum
`N3f1` baked in), for both schemes — V1 ed25519 collections and V2 BLS
aggregates. Stated honestly: **today no production path outside the engine
verifies a certificate cryptographically** — `verify_manifest_floor`
structurally decodes the cert and binds its coordinates only, and the V2
joiner path is `unimplemented!`. Phase 1 wires real verification for the
follower AND retrofits it into the floor check. A replica therefore trusts
its frame source for nothing: payload bytes are content-addressed, the
certificate proves finalization by the epoch's quorum, and the fold's own
seal verification (disposition, module roots, app-hash) catches divergence
at the first block, exactly as `apply_verified_suffix_frame` does today.

The rejected alternative — generalizing the post-reboot `fetch_frames` +
`apply_and_journal_verified_frame` catch-up into a steady wake-driven pull
loop — was cheaper (no consensus-crate changes) but keeps pull semantics and
trusts the serving node between floor-cert checks instead of verifying every
height. Decided against 2026-07-09; the follower orderer is the design.

## Join, follow, promote

**Join (bootstrap, unchanged surface):** statesync stands the node up at a
verified boundary B — manifest, module snapshots, qmdb op-ranges, exactly
today's lanes and trust model.

**Journal continuity:** immediately after bootstrap the node fetches the
Frames suffix (B, tip] and folds it through the shared pipeline, journaling
as it goes — so its recovery journal is what a validator restarting at B
would have written, with no fabricated checkpoint. From then on the follower
orderer folds from gossip; a `RangePruned` on a deep gap (node offline past
the servers' retained suffix) degrades to a fresh bootstrap, same as today.

**Follow (steady state):** every node — validator or resident — advances
per block: WAL, fold, seal, index fold, ws block event, periodic checkpoint
+ floor cert. The resident-tier pumps (serve window, submit relay, announce,
dispatch) ride the same loop they ride today; only the state-advance
mechanism beneath them changes. `heal_index`-per-boundary retires in favor
of the per-block `IndexFold` validators use; a resident's explorer row
becomes a real block row.

**Promote (the collapse):** a folding resident is already at head with a
live journal. Promotion reduces to: at the next epoch cutover that seats the
key, construct the engine over the current `(scheme, participants)` and swap
the follower orderer for `SimplexOrderer` — the same swap `respawn_if_due`
already performs between epochs on validators. The process re-exec survives
only if the per-epoch channel-bank registration genuinely requires it (to be
settled in Phase 3 — the follower already registers the full bank). Deleted
outright: `choose_promotion_boundary`'s app-hash equality wait, checkpoint
fabrication (`next_seq = 1`), the post-reboot frame catch-up and its
full-sync fallback, and the boot-time `heal_index` at the converged tip.

## Phases

1. **consensus: follower seam + real cert verification.** Engine-free
   follower-orderer constructor (reporter + inbox + store + resolver, no
   engine, no automaton); wire `Finalization::verify` for V1 and V2 and
   retrofit it into `verify_manifest_floor`. Gate: unit tests verifying and
   refusing certs across both schemes and across a cutover; existing engine
   tests untouched.
2. **node: replica mode.** Joiner path becomes bootstrap → Frames suffix →
   gossip-fed fold; cert/payload lanes consumed instead of black-holed (the
   head-wake's nudge is subsumed — the wake becomes the fold trigger);
   resident pumps ride the fold loop. Gate: `resident_follow_e2e` (already
   merged — behavior-level, guards freshness through the swap), plus a new
   e2e asserting per-block ws events and a restart that replays the
   follower's own journal.
3. **promotion collapse.** Engine swap at cutover; delete the fabrication /
   catch-up machinery; settle the re-exec question. Gate: live-admission and
   upgrade e2es green; promotion under load (chain advancing during
   promotion) e2e.
4. **statesync demoted to bootstrap-only.** Remove the boundary-reinstall
   follow loop; captures/`MAX_CAPTURES` sizing revisited for join-only load.
   Downstream unblocked: the console-hydration redesign (see
   `.claude/handoffs/follow-the-head.md`) gains per-block continuity on
   every node — its constraint #3 dissolves.

## What this deliberately does not do

- No voting-path or engine-internals changes — the simplex engine, automaton,
  and quorum arithmetic are untouched; the follower only consumes outputs.
- No wire-format changes — payload gossip, cert lane, statesync protocol,
  and journal record shapes all stay as shipped. (Residents already receive
  the consensus lanes; consuming is a local decision.)
- No ACL/standing semantics — who may submit what is the ACL module design's
  concern; this changes how any node stays current, not what it may do.
- No relay removal — a resident still relays writes to a validator for
  consensus custody; folding locally grants no proposal rights.
- The 2s `BLOCK_TIME`, cutover-delay, and channel-bank constants are
  untouched; `EPOCH_CHANNEL_BANK` sizing is Phase 3's re-exec question, not
  assumed away here.
