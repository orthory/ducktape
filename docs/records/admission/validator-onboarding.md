# Validator Onboarding — Invites, Join Requests, Approval

> **SUPERSEDED (2026-07-07): the approval step is gone.** Minting an invite
> IS the admission decision: a joiner redeems its single-use token
> automatically (governance `Redeem`), comes up as a full node, and only the
> quorum seat (`promote`) remains a deliberate member act. Invites are also
> tunnel-first — the blob carries the inviter's WireGuard bootstrap, and the
> join rides the VPN before any p2p. The current recipe lives in
> [../../deploy/private-cutover-integration-gap.md](../../deploy/private-cutover-integration-gap.md);
> the approval mechanics below (`invite-accept`, join-request queues) survive
> only as the manual fallback for token-less joins.

How a new node joins a running Ducktape network, what the invite token does
(and deliberately does not do), and the consensus constraints every operator
should know before growing the validator set.

## The flow at a glance

```
member                          joiner                         network
──────                          ──────                         ───────
invite ──── one-line blob ────► join <blob>
                                start node (parks)
                                └─ announces {token, pubkey,
                                   proof} over the LOBBY lane ─► every member records a
                                                                 PENDING JOIN REQUEST
approve (app button, or
`invite-accept <pubkey>`) ───── governance AddValidator ──────► majority passes
                                                                 → valset Join
                                                                 → epoch cutover
                                joiner syncs state, reboots,
                                votes as a validator
```

Two things changed relative to the original routine:

- **The pubkey exchange is automatic.** The joiner no longer copies its key
  out of the CLI and nobody pastes it. Its parked node delivers the key to the
  members itself, and each member's Members view (or `ducktape-node
  join-requests`) shows it as a pending request naming the inviting member.
- **Approval is still a human act.** A join request admits nobody. Each
  member's "Approve" click (or `invite-accept`) casts that member's normal
  governance ballot; a strict majority (n/2 + 1) admits. One invitation never
  bypasses the membership vote.

## What the invite token is

`ducktape-node invite` embeds a token in the blob: the issuing member's
ed25519 signature over `genesis-namespace ‖ nonce`. When a joiner announces,
every receiving member verifies:

1. the token signature — so the announce provably comes from an invitation
   minted by a **current member of this network** (tokens die with a removed
   member, and a token for network X verifies nowhere else);
2. the joiner's proof-of-possession — a signature over
   `genesis-namespace ‖ nonce ‖ joiner-key`, so a blob holder cannot park a
   request under a key whose secret it does not hold.

Only then is the request recorded (in memory — the joiner re-announces every
few seconds, so member restarts lose nothing). The token is a **doorbell
credential, not an admission credential**: a leaked blob lets a stranger ring
(show up in the pending list), never enter.

`invite --manual` emits a token-less blob for the fully out-of-band flow; the
old copy/paste path still works end to end.

## How an un-admitted key can talk at all

The mesh transport refuses connections from keys outside the tracked peer
set — that is exactly what admission changes — so a fresh joiner could never
announce itself. The escape hatch is the **lobby identity**: a keypair
deterministically derived from the genesis namespace, which every member folds
into its tracked mesh and every invite holder can derive. The parked joiner
connects *as* the lobby identity (transport only — its real key still signs
the proof and, later, consensus) and speaks on the dedicated lobby channel.

The lobby identity authenticates nothing and is meant to be public to the
network's invitees. Anyone holding a descriptor can connect with it; without a
valid token their messages are dropped on receipt.

A key granted **resident standing** (a member's `invite-accept`) climbs one
rung further: its node follows finalized boundaries, serves reads locally, and
**writes through its own surface**. A submit against the resident's surface is
signed with the node's identity key and relayed over the mesh to a current
validator, which takes consensus custody and answers with the op's finalized
fate — authorship is the resident's key, never the relaying validator's.
Standing to write is not membership: member-gated modules (governance among
them) still reject a non-member origin deterministically.

## Consensus reality: no voting power, and what that means

The consensus engine is commonware `simplex` over ed25519. **It has no notion
of per-validator voting power** — the participant set is a flat list, the
leader rotates round-robin over it, and quorum is the standard `2f + 1` of the
flat count. There is no way to add a validator at zero weight and later dial
its power up; the moment the set changes at an epoch cutover, quorum
arithmetic changes with it.

Operators should internalize the consequences:

- **Admission immediately shifts quorum.** commonware's BFT arithmetic is
  `f = (n-1)/3` tolerated faults with quorum `n - f`: for n = 1, 2, 3 that is
  1, 2, 3 — **below four validators, every single node must be live to
  finalize**. Only at n = 4 does the network first tolerate one absent node
  (quorum 3). The newcomer is counted from the cutover on, whether or not it
  ever shows up.
- **An approved-but-absent joiner can halt the network.** If members approve a
  key whose node never syncs and promotes (crashed, deleted, lost its disk),
  the cutover still counts it. On a 1-member network this is fatal-until-fixed:
  the sole member admits a ghost, quorum becomes 2, and no further blocks
  finalize — including the governance ops that would remove the ghost.
  Recovery is then manual (restore the joiner's node) or a network restart.
- **The join-request queue is the liveness signal.** A pending request means
  the joiner's node is up and announcing *right now* (`last_seen` in
  `join-requests` output). Approving from the queue is therefore strictly
  safer than blind `invite-accept` of a pasted key: you are admitting a node
  you can see breathing. Prefer it, and approve promptly rather than hours
  later.
- **Remove before it bites.** If an admitted node is gone for good and quorum
  still holds without it, run the removal (`member-remove <key>`, or the app's
  remove control) before the set shrinks further for other reasons.

If a weighted-consensus variant ever lands (stake-weighted shares are an
explicitly deferred valset feature), the right shape is: admission adds the
validator at zero power (mesh + statesync, no quorum effect), and a separate,
manually gated action shifts power. Until then the approval click *is* the
power shift — treat it with that gravity.

## Advertised addresses: who actually needs one

Nodes are typically laptops behind NAT with some tunnel in front. The rules:

- **Only nodes that must accept inbound dials need `advertised`** — in
  practice the founder/bootstrap members whose dial hints ride the invite
  blob. A joiner dials out; it can leave `advertised` unset entirely (it
  falls back to `listen`, and nothing depends on it being reachable).
- **Advertise a hostname, not an IP, when you are behind a tunnel.**
  `advertised = "my-node.example.com:443"` stays a hostname end to end: it is
  carried verbatim in invites and in the signed peer record, and *dialing*
  peers re-resolve it on every attempt. When the tunnel's IP moves, nothing
  needs updating. (Previously the node resolved its own name once at boot —
  and refused to boot when resolution failed; both behaviors are gone.)
- Every `invite` re-folds the emitting member's *current* dialable address
  into the blob, so a fresh invite always carries a fresh hint. If a joiner
  parks with "mesh unreachable", the usual cause is a stale invite from
  before an address change — re-issue the invite.

## Addressing a node by network

Every verb (and the run path) accepts `-n <chain-id>` / `--network <chain-id>`
in place of `--config <path>`: it resolves through the desktop app's workspace
registry (`~/.ducktape/workspaces`, `DUCKTAPE_HOME` to override) by exact
chain-id or unique prefix:

```
ducktape-node -n ducktape join-requests
ducktape-node -n ducktape invite-accept <pubkey>
```

## Compatibility

The lobby lane is a flag-day change (same class as the invite v1 → v2 blob
break): new binaries fold the lobby key into the tracked mesh and register the
lobby channel, so a mixed old/new network refuses its own connections at the
transport layer. Upgrade all validators together (the height-gated upgrade
flow works); invite blobs are compatible both ways (v2 still decodes, v3 is
emitted).
