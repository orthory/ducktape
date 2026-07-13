# Coordinator-Managed Invites, Targeted Invitations, Fail-Loud Redemption, Thin Client

Date: 2026-07-13
Status: approved (brainstormed with user; all four sections approved)

## Goals

1. **Fail-loud redemption** — a joiner presenting a consumed (or otherwise
   unredeemable) invite must terminate with a clear reason, never wait
   forever. Reproduce the observed silent-wait first; fix root cause plus
   backstops.
2. **Coordinator-managed short invites** — `🦆://<chain-name>/<id>` instead of
   the full `🦆<base64>` blob. The coordinator stores blob-by-hash; the blob
   stays self-authenticating.
3. **Mandatory target binding** — every invite is minted against the invitee's
   public key. Bearer invites are removed. (User decision: "you always must
   know who you're inviting.")
4. **Thin client** — a non-dev uses the desktop app with **no local chain
   node**; a client-mode noded tunnels (coordinator rendezvous + WireGuard)
   to a dev's remote node and proxies the app's normal local surfaces.

## Current state (verified 2026-07-13)

- Invite = `🦆<base64url>` blob: `NetworkDescriptor` + `InviteToken{issuer,
  nonce, sig}` + optional WG bootstrap + fronts + expiry, envelope-signed by
  the issuer (`bin/node/src/config/invite.rs:346-530`, token crypto in
  `crates/system/governance/src/invite.rs`). Grant namespace
  `ducktape-invite-grant-v1`; token is **network-bound but bearer** — any
  holder redeems under their own key.
- Redemption is single-use **in consensus**: `Governance.redeemed` nonce map;
  duplicate → deterministic reject `"invite already redeemed"`
  (`crates/system/governance/src/lib.rs:1056-1138`). Member `on_lobby` replies
  `fatal:true` on a spent nonce (`bin/node/src/validator/run/ingress.rs:200-230`);
  joiner prints FATAL and `exit(1)` (`bin/node/src/replica/wiring.rs:370-395`);
  desktop surfaces it via daemon.log phase classification
  (`app/src-tauri/src/workspaces/phase.rs`).
- Known silent-wait hole: `JoinReply.fatal` is `#[serde(default)]` — a member
  binary without the spent-check never sets it, so the joiner re-announces
  every ~30s forever. The lobby phase has **no deadline** (first contact has a
  90s window; lobby has none).
- Coordinator (`bin/coordinator` + `crates/system/nat-traversal`) is a
  keyless, stateless, UDP rendezvous (STUN-ish `Msg` enum, per-request PoP
  auth, in-memory `AdvertBook` with 120s TTL / 4096 cap). It never relays
  traffic (DERP removed). It keeps no per-invite state today.
- A joiner cannot query the chain before joining, so any short-invite store
  must live on the coordinator (the only pre-membership contact point).
  duckdns/gateway registries are members-only planes.

## Design 1 — Fail-loud redemption

**Step 0: reproduce.** Two-node localnet; A redeems an invite; B presents the
same blob. Observe joiner logs, exit code, and desktop JoinProgress. Also
probe the **expiry path**: `expires_unix_secs` is enforced where? Consensus
cannot read wall-clock; determine whether expiry is checked at decode/join
time, member-side, or not at all, and fix so an expired invite is a loud
deterministic reject (block-time comparison in `handle_redeem` if in-consensus
enforcement is wanted).

**Fix A: joiner lobby-phase deadline.** If the announce loop has run
`JOIN_LOBBY_DEADLINE` (default 5 min) without standing landing and without a
fatal reply, exit FATAL carrying the last `JoinReply.detail` seen (or "no
reply from any member"). This kills every silent-wait variant — including
old members that never set `fatal` — regardless of root cause. Restart-with-
standing keeps its existing retry-forever behavior (that path is a restore,
not a join).

**Fix B: member reject audit.** Every deterministic reject in `on_lobby` /
`handle_redeem` surface (spent, expired, wrong network, already
member/resident, issuer no longer a member, target mismatch from Design 3)
must reply `fatal:true` with a human-readable `detail`. Audit and unify.
`JoinReply.fatal` also drops its `#[serde(default)]` back-compat shim —
replies decode strictly; pre-fix members are unsupported (no-back-compat
mandate).

**Testing:** e2e reuse-invite test asserting joiner exits nonzero with the
redeemed message within the deadline; unit tests per reject reason; manual
fleet QA that JoinProgress shows the detail.

## Design 2 — Coordinator-managed short invites

**Short form:** `🦆://<chain-name>/<id>`

- `id` = base32(first 16 bytes of sha256(blob)) — content-addressed,
  unguessable, ~26 chars.
- `<chain-name>` = the name part of `chain_id` (`"<name>#<hex4>"`), cosmetic
  and for user confirmation; after fetch the client verifies it matches the
  blob's `chain_id` prefix and rejects on mismatch. Trust lives only in the
  blob's envelope signature.
- Parser disambiguation: full blobs are `🦆<base64>`; short form contains
  `://`.

**Coordinator additions** (new `Msg` variants in
`crates/system/nat-traversal/src/wire.rs`, handled in `Coordinator`):

- `InvitePut { id, blob, expires_unix_secs }` — authenticated via the
  existing `AuthRequest` PoP path. Coordinator verifies `id == hash(blob)`,
  size ≤ 8 KiB, TTL ≤ 30 days, and stores in-memory.
- `InviteGet { id, chunk }` / `InviteChunk { id, chunk, total, bytes }` —
  unauthenticated read, one chunk per request/response datagram (stateless,
  no reply-cap change; client iterates chunks).

**QoS / DoS policy (user-required):**

- **Anti-amplification:** an `InviteGet` datagram must be padded to at least
  the maximum reply size (~1200 B); undersized requests are dropped. A
  spoofed-source reflection then amplifies ≤1x.
- **Put quota:** max 32 live invites per issuer key; token-bucket rate limit
  per key on `InvitePut`, plus a global bucket.
- **Get rate limit:** token bucket per source IP on `InviteGet`.
- **Store bounds:** global cap 4096 entries (evict soonest-expiry first),
  per-entry TTL = min(invite expiry, 30 d). Same backstop philosophy as
  `AdvertBook`.

**Statelessness preserved:** in-memory only; a coordinator restart drops
short links. Re-publishing (next "Reveal invite" or `invite --short`) is the
recovery. Persisting the store is explicitly out of scope.

**Node/CLI:** `ducktape-node invite --short` mints the blob, publishes via
`InvitePut` to its configured coordinator, prints the short URL (and the full
blob as fallback). Tauri `workspace_invite_blob` gains the same option.

**Joiner:** the app accepts either form. Short form → fetch chunks from the
default primary coordinator → reassemble → verify envelope signature +
chain-name match → hand to the existing join path unchanged. v1 assumes the
shared default coordinator; a `?c=<host>` override is deferred (YAGNI).

**Rejected alternatives:** consensus-module store (joiner can't read the
chain pre-join); short guessable slugs (enumeration); persisting coordinator
state (breaks the stateless invariant for no real need).

**Testing:** nat-traversal unit tests (put/get round-trip, hash mismatch,
quota, padding rule, eviction); e2e mint→publish→fetch→join on localnet;
fleet QA for the UI.

## Design 3 — Mandatory target binding (in-place, no back-compat)

Every invite is bound to the invitee's public key. Bearer invites are
removed.

**Token, changed in place:** `InviteToken { issuer, nonce, target:
PublicKey, role: Role, sig }`; sig covers `binding ‖ nonce ‖ target ‖ role`.
No namespace bump, no format version, no compat path (user mandate): the
existing structures gain the fields, the blob format changes in place, and
anything minted or built before the change simply stops decoding/verifying.
Invites live ≤7 days; that is the whole migration story.

**`role` is added now** so Design 4 needs no second invite-format change:
`Role::Resident` (today's semantics — full node standing) and `Role::Client`
(thin-client ACL entry only; redemption handling ships with Design 4, until
then Client redeems are rejected with a clear detail).

**Enforcement:** `handle_redeem` requires `joiner == target` — mismatch is a
deterministic reject `"invite locked to another key"`. Member `on_lobby`
pre-checks and replies `fatal:true` early. The join proof already binds the
announcing key; this closes the "any holder redeems" hole, and a fetched
short-invite blob becomes useless to anyone but the target.

**Hard cutover:** redemption verification runs in consensus, so the valset,
inviter, and joiner binaries update together, in place. No version
negotiation, no legacy-decode path — old binaries and old invites are simply
unsupported, consistent with this repo's flag-day precedents (pre-07-09
binaries already unsupported).

**UX:** the invitee's account console exposes a copyable "my key" (identity
exists pre-join thanks to account-first onboarding; add the surface if
missing). Invite creation (UI field + CLI `--target <pubkey>`) requires it.
Flow: invitee installs app → creates account → sends their key to the
inviter → receives a short invite locked to them.

**Testing:** governance unit tests (target mismatch reject, v2 namespace,
old-token reject, role byte round-trip); `invite_e2e` updated to the
targeted flow.

## Design 4 — Thin client (client-mode noded)

A non-dev runs the desktop app with no local chain node. Architecture keeps
the app's process model intact: the app still talks to a local process; that
process is `noded` in a new **client mode**.

**Client-mode noded:** no consensus, no state sync, no chain storage. It:

1. Establishes connectivity to the serving (dev's) node via coordinator
   rendezvous + hole-punch + WireGuard — the same first-contact machinery
   invites use today (`first_contact_join.rs` candidates from the invite
   blob).
2. Exposes the same local surfaces the app already consumes (gateway HTTP
   for duck:// browse, submit, queries) and proxies them over the tunnel to
   the serving node.

**Access grant = Design 3 invite with `role: Client`.** Redeeming it records
a client ACL entry (not resident standing). The serving node accepts tunnel
sessions only from keys with client standing. Client submits are signed by
the **client's own identity** — never impersonating the serving node's key
(same principle as the agent session-key plane, PR #423). Basic per-client
rate limiting on the serving node.

**App shell:** a workspace kind "remote" — join via a Client invite skips
the full daemon boot and chain sync; JoinProgress reflects the shorter
pipeline (tunnel up → ACL verified → ready).

**v1 limits (accepted):** no relay exists (DERP removed) — a hard-NAT pair
that cannot hole-punch cannot connect (mitigation: dev side is typically
punchable or port-forwarded). Serving node offline ⇒ service unavailable.
v1 connects only to the inviting dev's node; any-member serving is future
work.

**Rejected alternatives:** embedding the overlay stack in the Tauri shell
(duplicates the node's network stack in-app); exposing plain HTTPS publicly
(no NAT story, bespoke TLS+auth).

**Scope note:** this section is architecture-level. Implementation gets its
own plan (likely: client ACL plane → client-mode noded tunnel/proxy → app
remote-workspace UX), written after the invite-plane PRs land, since it
builds on Design 3's role field.

## Design 5 — Synchronous join gate (PR4, user-requested 2026-07-13)

The join flow today is eventually-consistent: the joiner fire-and-retries an
advisory announce and its authoritative signal is polled state (manifest
watching, parking bails). That indirection is the root of the fail-loud bug
class PR1 patched. Once Design 3 lands, every deterministic reject is
decidable by a single member at handshake time — so the join becomes a GATE:

- Member validates the announce fully (crypto, target == joiner, expiry,
  spent nonce, issuer in view) and REJECTS inline with a reason — no tunnel,
  no lobby residence for a doomed join. The intro path runs the same checks
  before installing a tunnel peer.
- On pass, the member submits `Redeem` and awaits the frame's consensus fate
  (the relay lane already reports settle), then replies authoritatively:
  `Admitted { height }` or `Rejected { reason }`.
- The joiner BLOCKS on that reply with a bounded wait (~30s, a few blocks),
  fails over across members a bounded number of rounds, then exits FATAL.
  It enters statesync already holding standing; parked-polling remains only
  for the restart/restore path.

**No statesync before admission (user mandate, 2026-07-13):** pre-admission,
a joiner runs ONLY the gate protocol — no manifest polling, no sync client,
no boundary fetches. Admission detection IS the gate's `Admitted` reply, not
polled chain state (today's park loop polls the manifest to notice its own
admission — that disappears). And the boundary is enforced on BOTH sides,
fail-closed: members refuse statesync/manifest service to any key without
committed standing (validator/resident), so a valid invite blob alone —
even targeted — leaks zero chain state before consensus grants standing.
The lobby identity carries lobby traffic only. Exception: a node with
PERSISTED standing syncing after a restart is the restore path, not a join —
it keeps its current behavior.

**One source of truth for join state (user-reported confusion, 2026-07-13):**
today admission state is scattered across five independent signals — app-side
log-marker classification (`phase.rs`), members' in-memory `join_requests`
queues, the workspace registry's join-time `member` flag, the joiner's own
manifest polling, and leftover on-disk invite files — and surfaces disagree
(observed: a joined network rendering as "admission not claimed"). The gate
therefore ships with a node-owned join state machine as the ONLY source:
`Unjoined → GateInProgress(step) → Admitted(height) → Synced → Promoted |
Rejected(reason)`, derived solely from gate progress + committed chain state,
exposed via one `join-state` RPC. The app's daemon.log phase classification
is RETIRED; JoinProgress, pending-join surfaces, and node-page banners all
render the RPC. The registry's cached `member` flag is dropped or refreshed
from the same RPC; a consumed invite token file is cleaned up at Admitted.
The reported joined-but-unclaimed symptom becomes a PR4 QA repro case.

The only irreducible asynchrony is block-commit latency (seconds), wrapped
inside the bounded handshake. PR1's fatal-reply machinery becomes the gate's
reject half verbatim. Detailed plan written after PR3 merges (the gate needs
target/role/expiry checks in the token).

## Delivery

Order, all targeting `dev`, each from its own worktree:

1. **PR1** — fail-loud: repro, root-cause fix, lobby deadline, reject audit
   (+ expiry enforcement finding).
2. **PR2** — coordinator invite store + short format + QoS + UI.
3. **PR3** — mandatory targeting + role byte (flag day; valset must upgrade
   before first v2 redeem).
4. **Thin client** — separate implementation plan, multiple PRs, after PR3.

Invite-format changes are in-place with no compat path, so PR2 and PR3 may
each change the blob freely; blobs live ≤7 days and are re-minted.

## Open questions / risks

- Expiry enforcement location is unknown until the PR1 investigation; if it
  turns out consensus-side block-time comparison is needed, that rides the
  PR3 cutover.
- PR3 is a hard cutover: the whole valset updates in place before the first
  targeted redeem (upgrade skill exists for coordinated rolls).
- Coordinator invite store is best-effort by design; users must understand a
  short link can die on coordinator restart (full blob remains the fallback).
- Thin client trust surface (which RPCs a client ACL may reach) needs its own
  review in the follow-up plan — loopback-only assumptions must be audited
  before any surface rides the tunnel.
