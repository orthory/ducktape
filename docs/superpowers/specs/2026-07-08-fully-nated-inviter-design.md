# Unified all-paths invite (fully-NATed inviter)

Status: design of record (2026-07-08), **simplified**. Build ON PR #260. Branch
`feat/coordinated-first-contact` off `origin/dev @6095984`.

## The simple idea

One invitation bundles **every entry path the inviter chooses to offer**, and the
joiner tries them all and uses whichever works. That's the whole feature.

Paths bundled (all opt-in per the inviter):
- **direct** — the inviter's WG endpoint (only if it's willing to expose an IP);
- **coordinated** — reach the inviter by KEY via the coordinator (#260);
- **fronts** — a few *other* reachable members; a fully-NATed (even symmetric) inviter
  can't be reached directly OR punched, but the invite token is inviter-signed and
  `verify_intro` (lobby.rs:234) has no valset check, so ANY member installs the joiner
  and any member submits the in-consensus `Redeem`.

The joiner builds ONE candidate set = `{inviter} ∪ {fronts}`, where each candidate is
reached by its **direct endpoint if present, else by KEY via the coordinator**. It races
them, first `IntroAck.installed` wins, and it fails honestly only if *all* fail. A stale
direct endpoint transparently falls through to the coordinated path/fronts.

## The coordinator is ambient, NOT invite payload

Coordinator info does **not** belong in the invitation. The coordinator is network
rendezvous infrastructure — the invite carries *who* to reach (keys), and the joiner
uses **its own configured coordinator** (`config::primary_coordinator_or_default`,
defaulting to the deployed public `relay.ducktape.industries:3478`) to look any key up.

- The invite carries **no** `coord_addr`/`coord_key`. A candidate is "coordinated" iff it
  has no direct endpoint → the joiner rendezvous-looks-it-up by key.
- Concretely: the joiner's NAT resolver binds coordinators from `primary_coordinator_or_default`,
  NOT from the invite's reach hints (today #260 extracts them from the invite at
  `main.rs:5870-5871` — that round-trip is removed). `cmd_invite` stops embedding a
  coordinator address; the inviter still registers with its own coordinator (its own config),
  which is independent of what it puts in the invite.
- Benefits: smaller invite, non-stale when the coordinator moves, and `coord_key` (already
  vestigial/unused) disappears from the payload.

## Explicitly CUT (was over-engineered)

- **No rendezvous cap, no private-coordinator first contact.** #260 defaults to a public
  rendezvous coordinator; a fresh joiner authenticates with PoP over its own key, no cap
  needed. If a network ever wants private coordination for onboarding, the answer is
  "point onboarding at a public coordinator" — not a new genesis-signed cap type, wire
  migration, and coordinator op-narrowing. Dropped.
- **No new `reachable-members` RPC.** `cmd_invite` reads the already-persisted
  `mesh-state.json` to populate fronts — no live node round-trip.

## What #260 already gives us (reuse, don't rebuild)

- Joiner coordinated path: `ReachabilityCommand::BootstrapCoordinatedInvitePeer
  { peer, wireguard_public_key, intro, reply }` (resolve rendezvous → install peer →
  `send_datagram_and_recv(intro)` → ack). Direct path: the UDP announcer.
- Inviter receive: shared-socket `invite_intro_rx` receiver + `SendResolverDatagram` ack;
  and the legacy direct `intro_listen` UDP listener.
- Invite: `InviteWireGuard{ public_key, endpoint:Option, intro:Option, mesh_port }`,
  wire tag 1=direct / 2=coordinated. `genesis_namespace` (config.rs:263-286) hashes
  scheme + sorted validators ONLY — reach/bootstrap/coordination excluded, so new invite
  bytes are automatically off-fingerprint.

## Components

### A. Invite carries fronts (`config.rs`)

- `Front { member_key:[u8;32] (ed25519), wireguard_public_key:[u8;32], mesh_port:u16,
  endpoint:Option<String> }`. `endpoint Some` = host-capable (direct intro at wg_port+1);
  `endpoint None` = coordinated (punch via the coordinator). Distinct from `Reach::Fronted`
  (legacy direct-dial alias — do NOT overload).
- Add `fronts: Vec<Front>` to `Invite`, packed inside the issuer-signed envelope, OUTSIDE
  `genesis_namespace`. A pre-feature blob (no new bytes) still decodes; the blob is
  unversioned/re-mint-on-change by design. Keep `InviteWireGuard`'s tag 1/2 so an inviter
  can still be direct-only or coordinated-only — the union is assembled joiner-side.
- **Fingerprint-exclusion test:** two invites differing only in `fronts` yield the same
  `genesis_namespace`.

### B. Joiner races the union (`bin/node/first_contact_join.rs`, new module)

- Replace the exclusive `match (&wg.endpoint,&wg.intro)` (main.rs ~5920) with a union
  candidate builder; move BOTH #260 join branches (direct announcer 5921-6026,
  coordinated task 6027-6109) into `first_contact_join.rs` (don't grow main.rs).
- `candidates = {inviter} ∪ {each front}`. For each, if it has a **direct endpoint** →
  the direct announcer; else **coordinated** → `BootstrapCoordinatedInvitePeer{peer:key,…}`
  resolved through the node's **ambient coordinator** (`primary_coordinator_or_default`),
  not any invite-carried address.
- The resolver's coordinator list is bound from config/default, so first contact needs no
  coordinator address in the invite. (Removes #260's `main.rs:5870-5871` invite→coordinator
  extraction.)
- Race with bounded fan-out; first `IntroAck.installed` wins, cancel the rest; inject the
  winner's overlay ULA Direct hint at this run-time point (winner only known here).
- **Honest terminal:** all candidates fail within the window → distinct non-zero exit +
  an operator log naming the mode ("no reachable path: N candidates tried"). Never a silent
  success. Residual (all-symmetric, no reachable member) needs the removed relay — surfaced.
- Socket-mode: coordinated candidates need `wireguard_effect=socket` (the punched 5-tuple);
  a coordinated-only candidate on a TUN node is skipped/failed honestly, not hung.

### C. Mint bundles all paths (`bin/node/main.rs cmd_invite`)

- `cmd_invite` includes every path it can: its direct endpoint (when dialable) AND its
  coordinated reach hint (when a coordinator is set) — both coexist per #260 `223ca51` —
  plus `fronts` read from the inviter's persisted `mesh-state.json`
  (`reachability::store::load`), filtered to members with a concrete routable endpoint OR
  punchable (registered, default wg_port+1). No node round-trip; if no mesh state exists,
  emit empty `fronts` + a warning (invite still works, just no member fallback).
- A zero-exposure inviter simply omits its direct endpoint. Attack-surface note
  (documented): a fronts-bearing invite lists a few members' WG keys/endpoints, bounded by
  single-use token + expiry.

### D. Tests / e2e

- `config.rs` unit: fronts pack/unpack round-trip; pre-feature blob decodes;
  fingerprint-exclusion.
- `crates/system/reachability/tests/orchestrator_e2e.rs`: a `StaticResolver` test of
  `BootstrapCoordinatedInvitePeer` (none exists) — resolve→install→ack.
- `bin/node/tests/coordinated_invite_cli.rs`: extend — a fronts-carrying invite; a
  coordinated-only invite on a TUN node fails with the honest message.
- `ops/` leg (verifies #260's T1 too, closing #262): NATed socket-mode inviter + public
  coordinator; a symmetric inviter + one reachable front (T2); all-symmetric → honest fail.

## Key handling (trust)

Only **public** keys are ever transported. The WireGuard **private** key never leaves the
node (local `wireguard.key`, `0600`, used solely to configure the local interface) — it is
not in the invite, not in adverts, and never sent to the coordinator. The coordinator sees
only ed25519 **identity** keys + reflexive addresses (its wire protocol has no WG field at
all). A `Front` carries the member's WG **public** key under the inviter's envelope
signature; a wrong key can at most make that candidate fail the WireGuard handshake (fall
through), never hijack — the real gate is identity + the signed intro + in-consensus `Redeem`.

## Global constraints

No coordinator relay (PR #173); keyless coordinator; zero consensus (fronts off-fingerprint,
prove it); crate layering (seam already opaque in #260); no mono-files (new
`first_contact_join.rs`; don't grow main.rs); per-crate clippy `--no-deps`; no `cargo fmt
--all`.

## Non-goals

Rendezvous cap / private-coordinator onboarding; all-symmetric/no-reachable-member coverage
(needs the relay); overloading `Reach::Fronted`.
