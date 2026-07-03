# invite-after-genesis — slice 4 handoff

status: PR OPEN (feat/invite-after-genesis -> dev), 2026-07-03. both invite_e2e
scenarios green first-run (solo ~20s, live-quorum ~21s); full workspace suite
run pending at write time — see the PR checks.

the shipped flow (two humans, three commands, zero config edits, zero member
restarts): friend `join <blob>` + starts the node (it PARKS — the mesh refuses
un-admitted keys) → any member `ducktape-node invite-accept <pubkey>` → the
epoch cutover re-tracks the mesh → the parked node syncs the boundary,
fabricates its own recovery checkpoint, exec-reboots through the slice-3
restore path, and votes in the new epoch. promotion IS restart-recovery with
someone else's state.

## build findings (the load-bearing ones)

- (a) the PRE-EXISTING cutover scheduler was cross-node NONDETERMINISTIC:
  drain batches are arbitrary per node and the pump observed valset once per
  tick, so two nodes could observe the same membership change at different
  views → different cutover views → different epoch bases → app-hash fork.
  never tripped because no e2e ran a live cutover under concurrent traffic.
  root fix shipped as TWO deterministic rules: the ordered lane's OBSERVATION
  BARRIER (a drain batch ends AT a block that moves the watched valset root,
  so observation lands on the changing block's view everywhere), and the
  BOUNDARY READ (the next participant set is read from state at the cutover
  view, where the discard ceiling froze it — never pinned at observation
  time; a second change inside the window rides the same cutover).
- (b) engines must spawn over the EPOCH'S participant set, not the
  instantaneous valset projection: a restart inside a cutover window would
  otherwise run a different scheme than its peers. the recovery Manifest and
  the journal's Cutover record now both carry the participant set; recovery
  Manifest also records an armed-but-uncrossed cutover view, and recover()
  surfaces post-checkpoint per-block root vectors so boot re-derives a
  boundary armed above the checkpoint. RECOVERY FORMATS ARE NOT
  BACKWARD-COMPATIBLE — pre-slice-4 storage dirs fail loudly at decode
  (same posture as the pre-recovery migration story).
- (c) commonware discovery ground truth (verified against 2026.5.0 source):
  `oracle.track(index, set)` requires STRICTLY INCREASING indexes (same-index
  re-track is a warn-and-ignore no-op); unknown indexes in a peer's bit
  vector are ignored; a KNOWN index whose set length differs KILLS the peer.
  hence: mesh tracked at index = epoch, and the set at a given index must be
  identical on every node — epoch participants ∪ descriptor mesh is, live
  valset projections are not. `tracked_peer_sets = 4`, so demoted peers age
  out after 4 epochs.
- (d) finalized views only advance with ops, so an idle network parks AT an
  armed boundary forever. the pump pushes deterministically-rejected nops
  (target "consensus.nop", never a registered module) at 1/s while a cutover
  is pending — rejected frames advance the engine clock and leave no state.
- (e) the joiner learns its consensus coordinates from the statesync
  manifest, which now carries (epoch, view_base, participants, floor_cert).
  floor_cert is served only when it certifies exactly the boundary height;
  the joiner validates it against the epoch scheme BEFORE persisting. two
  promotion shapes: boundary == view_base → epoch genesis floor (the n≤3
  case — the epoch stalls until the joiner arrives); boundary past the base
  → Floor::Finalized (live quorum kept finalizing, proven at n=3→4).
- (f) promotion reboots via exec (unix `CommandExt::exec`, same argv):
  discovery channels can only be registered before network.start(), so a
  parked process cannot grow an engine in-place. listener binding on the
  NEXT boot is gated by a filesystem probe (storage_dir/recovery-manifest/
  non-empty); the probe only gates listeners — joiner-vs-validator is
  re-decided from the real recovery store.

## accepted edges (documented in code)

- a rejoining previously-removed key starts at next_seq=1 again; a
  byte-identical (origin, seq, payload) resubmission could be dropped by a
  peer's in-process digest gate. fix belongs to per-origin replay nonces in
  state (already on the open list).
- a parked joiner on a network at epoch ≥ EPOCH_CHANNEL_BANK (16) sees
  reconnect churn (engine lanes beyond its blackholed bank); promotion still
  works — the rebooted validator banks from the recovered epoch.
- statesync remains single-source (the first bootstrap validator ≠ self);
  failover when that validator is offline is a known deferred gap (workspace
  slice). a BUSY source moving its qmdb targets mid-sync is handled by
  refetch-and-retry (bounded, then fail-stop).
- demotion still halts the node (restart as observer) — degrading in-place
  to a sync-server stays on the open list.

## next slice pointer

slice 5 (onboarding-ui) is the front door over exactly these seams: identity
step, name-or-join workspace step, then the stack takes over — the park/
promote progress lines ("parked:", "admitted at epoch", "promoted:") are
already greppable markers an app can surface. the multi-workspace registry
(~/.ducktape/workspaces) also lands there.
