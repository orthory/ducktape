# Coordinator Authorization — Public/Private Coordination — Design of Record

Status: design of record for the coordinator-auth follow-on (the "§3.5
coordinator-auth" open item tracked in
`docs/deploy/private-cutover-integration-gap.md`). Builds directly on
`docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md` (the
rendezvous coordinator) and reuses the invite-token signing machinery in
`bin/node/src/config.rs`.

## Goal

Give a network a **choice of coordination privacy**, and make that choice
enforceable at a coordinator that holds no secret:

- **Public coordination** — open rendezvous. Anyone can use the coordinator;
  point at shared/community infrastructure (e.g. `p2p.ducktape.industries`)
  with zero credential management. The only requirement is a per-request
  **proof-of-possession**: you must sign for the key you register, so no user
  can poison another's mapping or register a key it does not hold.
- **Private coordination** — members-only rendezvous. Only nodes the network
  vouched for may register, look up, or use STUN. Topology and reflexive-address
  metadata stay within the membership. This is the `epic/p3-private-cutover`
  intent — a network can run (or pin) its own coordinator and keep non-members
  out entirely.

The mode is a **per-network policy** chosen by the founder, recorded in the
network's genesis artifact and echoed in invites so nodes and joiners behave
correctly without a separate flag.

## What authorization does and does not buy

Authorization here is an **anti-abuse / admission** boundary, not a
confidentiality or integrity boundary. The coordinator already cannot
impersonate, substitute keys, or MITM: downstream endpoint records are
owner-signed (`wireguard-upgrade::SignedEndpointRecord`), the invite's
`expected_key` is inviter-signed, and a wrong reflexive address merely makes the
hole-punch fail into a terminal `PeerFailed` (an honest failure, never a silent
degradation). See the private-cutover design's threat model.

So auth closes exactly these gaps in today's fully-open coordinator:

- **Non-members using the service** — an internet scanner registering keys,
  scraping topology via `Lookup`, or abusing STUN reflection (private mode).
- **Cross-node mapping poisoning** — one node registering or re-advertising
  *another* node's `NodeKey` to point rendezvous at an attacker-chosen source
  (both modes, via proof-of-possession). This reduces to a bounded
  denial-of-rendezvous even if it slips through, because the downstream punch
  fails safe.

It does **not** add impersonation resistance (already covered) and does **not**
turn the coordinator into a membership oracle over live consensus state (that
would break the keyless/stateless invariant — see Non-goals).

## The invariant this preserves

The coordinator stays **keyless, stateless, and disposable** — the property the
entire deploy recipe (`docs/deploy/coordinator.md`,
`ops/coordinator/*`) rests on: "if a reviewer can find a place a secret would
live on this host, the recipe is wrong." Authorization adds at most a **public**
pin (the network's genesis validator public keys, already public data in
`network.toml`) and per-request signature verification. No private key, no
shared secret, no session state, no database, no disk write. The box remains
replaceable at will.

## Identity — no new type needed

A node's rendezvous `NodeKey` **is** its raw ed25519 public key
(`reachability::binding::node_key` maps `ValidatorIdentity(pk_bytes)` straight to
`NodeKey(pk_bytes)`). So every existing request already carries the sender's
ed25519 verifying key in-band. Proof-of-possession is a signature verified
against those 32 bytes; no identity plumbing is added. The `nat-traversal`
crate already depends on `commonware-cryptography` (currently unused), so the
ed25519 primitives are available without a manifest change.

## The per-request authenticator (the "authorization header")

Every client→coordinator request (`Register`, `Readvertise`, `Lookup`,
`BindRequest`) carries an authenticator — the wire-level equivalent of an HTTP
`Authorization:` header, but a field on the UDP datagram rather than an HTTP
header, and self-validating rather than an opaque server-issued bearer string:

```text
Authenticator {
  timestamp : u64            # seconds since epoch; freshness
  pop_sig   : ed25519 sig    # sign(COORD_REQ_NS, inner_request_bytes ‖ timestamp)
  cap       : Option<CoordCap>   # present in private mode; absent in public mode
}

CoordCap {                        # the "auth token" — a signed capability
  issuer    : ed25519 pubkey  # a genesis validator (must be in the pinned set)
  not_after : u64             # expiry, seconds since epoch
  issuer_sig: ed25519 sig     # sign(COORD_CAP_NS, subject ‖ not_after)
}
```

`subject` (the key the cap authorizes) is **not** repeated in the cap — it is the
`NodeKey`/`from` of the inner request the authenticator wraps. This binds the cap
to the request's claimed identity and keeps it compact.

New signing namespaces (mirroring `INVITE_GRANT_NAMESPACE`):

- `COORD_REQ_NS = b"ducktape-coord-req-v1"` — proof-of-possession per request.
- `COORD_CAP_NS = b"ducktape-coord-cap-v1"` — the admission capability.

Namespace separation guarantees a coord signature can never cross-verify as an
invite or endpoint-record signature and vice versa.

### Verification (at the coordinator, stateless)

Given an authenticated request with inner claimed key `subject`:

1. **Freshness.** `|now − timestamp| ≤ WINDOW` (default 30 s). Else drop.
2. **Proof-of-possession.** `verify(subject, COORD_REQ_NS, inner_bytes ‖
   timestamp, pop_sig)`. Else drop.
3. **Admission (private mode only).** Authorize iff `subject ∈ pinned_genesis_set`
   **or** the cap verifies: `issuer ∈ pinned_genesis_set` **and**
   `not_after > now` **and** `verify(issuer, COORD_CAP_NS, subject ‖ not_after,
   issuer_sig)`. Else drop.
4. Proceed to the existing `AdvertBook` / `handle` logic unchanged.

Public mode runs steps 1–2 only (no admission gate). Every check is a clock read
plus one or two ed25519 verifications against public keys the coordinator already
holds — no state mutation, no secret.

### Rejection is a silent drop

A failed authenticator produces **no reply** (and increments an in-process
counter for observability). This avoids turning the coordinator into an
oracle ("is key K a member?") and avoids any amplification surface. It matches
the existing loop, which already `continue`s on decode failure.

### Replay: bounded, and why that is enough

Because the coordinator stores the *observed source* of the datagram (never a
self-reported address) and the client cannot know its own post-NAT source before
sending, `pop_sig` cannot bind the source. A captured request can therefore be
replayed from a different source **within the freshness window**. The blast
radius is deliberately small:

- `Readvertise` replays are already neutralized by the monotonic-nonce
  `AdvertBook` staleness guard (an equal-or-lower nonce is `Stale`).
- A `Register`/`BindRequest` replay within the window can at most re-point a
  mapping to the replayer's source or elicit a reflexive echo — a bounded
  denial-of-rendezvous that the downstream owner-signed records and fail-safe
  punch already tolerate.

Closing the window entirely would require a challenge-response round trip (the
coordinator issues a nonce the client signs), which adds latency and
per-client state — explicitly **out of scope** (YAGNI) given the anti-abuse
goal and the fail-safe downstream. The `WINDOW` is a tunable.

## Wire protocol changes

The existing tags 1–7 and 10 stay as the **unauthenticated** request/response
shapes (used by in-process tests and, transitionally, by a public coordinator
running fully-open for compatibility). Authenticated requests get a single new
**envelope** tag:

```text
TAG_AUTH_REQUEST = 11   # AuthRequest { inner: <one request Msg>, auth: Authenticator }
```

Tags 8/9 remain poisoned (retired relay). The envelope encodes the inner
request (its existing tag + body), then the authenticator fields, and `decode`
enforces the existing whole-buffer rule (`WireError::Trailing` on leftover
bytes). Decode rejects an envelope whose inner message is a *response* type
(`BindResponse`/`LookupResponse`/`PunchSync`/`Punch`) — only the four request
shapes are wrappable. A single envelope tag (rather than four new authenticated
tags) keeps the codec small and the authenticator logic in one place.

Responses (`BindResponse`, `LookupResponse`, `PunchSync`) are unchanged and
unsigned — the coordinator holds no key to sign with, by design. The client's
existing defense stands: it discards any response whose source is not the
coordinator it dialed.

## The coordinator: policy-driven, still stateless

`Coordinator` gains an authorization policy set at construction:

```text
AuthPolicy =
  | Open        { require_pop: bool }      # public coordination
  | Private     { genesis_set: Vec<ed25519 pubkey> }   # members-only
```

- `Coordinator::with_policy(AuthPolicy)`; `Coordinator::new()` stays as
  `Open { require_pop: false }` for existing tests (fully-open).
- `handle` calls a new `auth::verify_request(policy, now, inner, auth)` before
  touching the `AdvertBook`. On `Err`, it returns no datagrams and bumps a
  reject counter. On `Ok`, control flows into today's exact `handle` body.
- `Open { require_pop: true }` (the chosen public mode) accepts any subject but
  still enforces steps 1–2. `Open { require_pop: false }` is the legacy
  fully-open shape, retained only for tests and an explicit `--allow-anonymous`
  escape hatch.
- `Private` additionally enforces step 3 against the pinned genesis set.

The policy carries only public keys and a bool. No secret enters the process.

## Capability issuance and distribution

`CoordCap` mirrors `InviteToken` end to end (same crate, same primitives):

- `mint_coord_cap(issuer: &ed25519::PrivateKey, subject: &ed25519::PublicKey,
  not_after: u64) -> CoordCap` — `issuer_sig = issuer.sign(COORD_CAP_NS,
  subject ‖ not_after)`. The issuer must be a **genesis validator** for the cap
  to verify against a coordinator pinned to that network.
- `verify_coord_cap(cap, subject, genesis_set, now) -> bool`.
- `pack_coord_cap` / `unpack_coord_cap` — fixed `32 + 8 + 64` layout, hex-packed
  like `pack_invite_token`.
- `save_coord_cap` / `load_coord_cap` — a `coord.cap` artifact beside the
  descriptor, `0600` (it is a bearer-ish credential, though PoP-bound in use).

Who holds what:

- **Genesis validators** need **no cap** — they are in the pinned set and pass
  admission on `subject ∈ genesis_set`.
- **Joiners** carry a `coord.cap` minted by the founder **alongside the v3
  invite** (the invite flow already mints an `InviteToken`; the cap rides the
  same mint). The joiner presents it on every rendezvous request until it
  expires.
- **Expiry / rotation / revocation** — caps are expiry-bounded (a network policy
  knob, e.g. days); rotation is re-minting. There is no CRL — a stateless
  coordinator cannot hold one — so short expiry is the revocation lever. This is
  the same posture as `invite.token`.

**v1 limitation (documented, deferred):** a cap is only accepted if signed by a
**genesis** validator. If a *non-genesis* member invites a new joiner, that
joiner cannot use a private coordinator until a genesis key co-signs its cap; it
falls back to a `Fronted`/`Direct` reach hint for the join window. Full
**delegation chains** (a cap signed by any member that itself presents a valid
cap, verified up to a genesis root) are a clean future extension, out of scope
for v1. Chosen because current networks are founder-driven and small (see the
live-join rig); revisit if non-genesis onboarding through the coordinator
becomes routine.

## Network policy encoding + node behavior

The coordination mode is recorded once, by the founder, and consumed by nodes:

- **Genesis / `network.toml`** gains a `coordination = "public" | "private"`
  field (default `"private"` — the epic's intent and the safer default). It is
  parsed like the reach hints — **operational policy, NOT part of
  `genesis_namespace`** (which fingerprints validator identity only). Flipping
  the mode is not a key-substitution vector: a private network pointed at an
  open coordinator simply loses enforcement, and a node with the wrong mode
  fails closed at rendezvous — so it does not need fingerprint protection.
- **v3 invite** echoes the mode (and, for private, carries the joiner's
  `coord.cap`) so a joiner with only an invite knows whether to attach a cap.
  The `Reach::Coordinated { coord, coord_key }` hint already exists.
- **`coord_key` stays deferred.** The node currently drops the hint's
  `coord_key` (`bin/node/src/main.rs:4254`/`:5203`). Honoring it — pinning the
  coordinator's identity so the client can authenticate *the coordinator* —
  would require the coordinator to hold its own signing key and sign responses,
  a change to the "no key on the box" posture. Out of scope here; the client
  keeps its source-address discipline as the response defense. Noted for a
  future revision.
- **Node behavior:** in `public` mode the node signs each request (PoP) and
  attaches no cap; in `private` mode it additionally attaches its `coord.cap`
  (genesis validators attach none). A node with no configured coordination mode
  and no cap behaves as today (fully-open) for backward compatibility.

## Node configuration surface

A node must be configured with three things to use a coordinator — **path**,
**scheme**, **credential** — and each has an existing home so no new config
file is invented:

1. **Coordinator path** — *where* the coordinator is. Already carried by the
   `Reach::Coordinated { coord, coord_key }` hint in the descriptor/invite
   (`coordinated:<key>@<host:port>#<coord_key>`, `config.rs`). A `Vec` of hints
   gives multi-coordinator failover. Nothing new; `coord_key` stays parsed but
   unused (deferred, above).
2. **Auth scheme** — *how* to authenticate: the coordination mode
   (`public` → PoP only; `private` → PoP + cap). It comes from the network's
   `coordination` policy (genesis/`network.toml`), echoed in the v3 invite so a
   joiner holding only an invite knows the scheme without the full descriptor.
   The node does not choose the scheme; it obeys the network's recorded policy.
3. **Credential** — *what* to present:
   - the node's **identity key** (already loaded, `load_or_generate_identity`)
     signs the per-request PoP — needed in **both** modes;
   - in **private** mode, the **`coord.cap`** artifact (a genesis-issued
     capability, `0600` beside the descriptor, minted with the invite for
     joiners; genesis validators need none because the pinned set covers them).

Resolution of the triple at boot: mode + path come from the descriptor/invite;
the identity key is already in hand; the cap is loaded from `coord.cap` if
present. A node in `public` mode with no cap is fully configured. A node in
`private` mode that is neither a genesis validator nor holding a valid cap is
**mis-configured for private coordination** and fails closed (it gets no
rendezvous and falls back to other reach hints) — an explicit, surfaced state,
not a silent open.

## Client / node wiring

- `NatClient` gains the node's `signer: ed25519::PrivateKey` and an
  `Option<CoordCap>`; it builds an `Authenticator` for each `register` /
  `readvertise` / `lookup` / `discover_reflexive`, wrapping the inner `Msg` in
  the tag-11 envelope. The existing source-address discipline stays.
- `reachability::NatResolver::bind` already receives the node's key `me`; it now
  also receives the signer (already threaded into `reachability_plane` at
  `bin/node/src/main.rs:3551`) and the optional cap (newly loaded from
  `coord.cap`), and passes them to `NatClient`.
- `bin/coordinator` gains `--genesis-set <path-to-network.toml>` (private mode,
  builds `AuthPolicy::Private`) and defaults to public `Open { require_pop:
  true }`; `--allow-anonymous` selects the legacy fully-open shape. `--listen`
  is unchanged. The env/systemd/Docker recipe adds the (public) genesis-set path
  for private deployments — still no secret on the box.

## Components (isolation and boundaries)

- **`nat-traversal/src/auth.rs` (new)** — `CoordCap`, `Authenticator`,
  `AuthPolicy`, `AuthError`, pure `verify_request(policy, now, inner, auth)`.
  No I/O; fully unit-testable. *What it does:* decides whether one request is
  authorized. *Depends on:* `commonware-cryptography`, `wire::Msg`.
- **`nat-traversal/src/wire.rs`** — the tag-11 `AuthRequest` envelope
  encode/decode; the `Authenticator`/`CoordCap` byte layout; the response-inner
  rejection and the `Trailing` rule.
- **`nat-traversal/src/coordinator.rs`** — `AuthPolicy` field,
  `with_policy`, the `verify_request` call before `AdvertBook`, the reject
  counter.
- **`nat-traversal/src/client.rs`** — `NatClient` signer + cap; envelope
  construction; `run_coordinator` grows a policy parameter.
- **`bin/coordinator/src/main.rs`** — `--genesis-set` / `--allow-anonymous`
  parsing → `AuthPolicy`.
- **`bin/node/src/config.rs`** — `CoordCap` mint/verify/pack/save/load;
  `coordination` field; v3 invite echo of mode + cap; stop dropping `coord_key`.
- **`crates/system/reachability`** — `NatResolver::bind` signer+cap plumb.

## Error and fallback handling

- Unauthorized request → silent drop + reject counter (no reply, no oracle, no
  amplification).
- Expired cap → treated as unauthorized (drop). Node re-mints/rotates
  out-of-band; entry falls back to other reach hints.
- Clock skew beyond `WINDOW` → drop; document NTP as an operational requirement
  for coordinator hosts and nodes (already implicit for TLS-like freshness).
- Coordinator unreachable / rejects → the existing failover walks the other
  hints in the `Vec` (`discover_reflexive_failover`); established punched paths
  survive regardless (coordinator is not load-bearing).
- Public network pointed at a gated coordinator (or vice versa) → the network's
  recorded mode drives node behavior; a mismatch degrades to "coordinator does
  not enforce" (public) or "node's cap is ignored" — never a security break,
  since metadata exposure to the operator was always in the threat model.

## Testing / acceptance

- **Auth unit matrix (`auth.rs`)** — PoP valid/forged/wrong-key; timestamp
  fresh/skewed-past/skewed-future; genesis-subject-without-cap accepted;
  non-member-without-cap rejected; cap valid/expired/wrong-issuer(not in
  set)/wrong-subject; public-mode ignores cap; fully-open accepts all.
- **Wire (`wire.rs`)** — tag-11 envelope roundtrips for every inner request
  shape; envelope wrapping a response is rejected; `Trailing` bytes rejected;
  tags 8/9 still `BadTag`.
- **Coordinator (`coordinator.rs`)** — `Private` policy: authorized
  register→lookup→PunchSync succeeds; unauthorized register creates **no**
  `AdvertBook` entry and yields no datagrams; reject counter increments.
- **Integration (`simnat` / `orchestrator_e2e.rs`)** — extend the existing real-
  UDP `run_coordinator` e2e so both `NatResolver`s carry signer+cap and a
  private coordinator; add a negative case (a resolver with no/expired cap gets
  no rendezvous).
- **Node/config** — `coordination` parse + genesis-fingerprint coverage;
  `coord.cap` mint/pack/save/load roundtrip; v3 invite carries mode + cap.
- Scope `clippy` to the new/clean crates (`nat-traversal`) per the known
  toolchain-drift caveat; for `node-bin` gate on `cargo test` + a
  zero-new-clippy-errors baseline diff, not raw clippy exit.

## Migration and compatibility

- Existing in-process tests and any staged node path keep working: `new()` stays
  fully-open, unauthenticated tags 1–7/10 stay valid.
- A public deployment upgrades transparently — `require_pop` rejects only
  unsigned requests; the node attaches PoP once wired.
- A private network sets `coordination = "private"`, deploys a coordinator with
  `--genesis-set`, and mints joiner caps at invite time. Genesis validators need
  no artifact change.

## Non-goals

- No live valset / consensus feed on the coordinator (would break
  keyless/stateless/disposable). Admission is proven by the bearer's cap against
  a static public pin, not tracked by the coordinator.
- No secret, shared key, or HMAC on the coordinator host.
- No coordinator identity key / signed responses (would put a key on the box);
  `coord_key` pinning is therefore deferred and the client keeps source-address
  discipline for responses.
- No challenge-response anti-replay (bounded-window replay accepted; downstream
  fails safe).
- No CRL / online revocation (short cap expiry is the lever).
- No delegation chains in v1 (genesis-issued caps only).
- No change to consensus, admission, or the WireGuard data-plane "relay must be
  a validator" rule.

## Resolved decisions

- **Public vs private is a per-network founder policy** in genesis, echoed in
  invites — not a per-node or purely-ops choice.
- **Public = open + proof-of-possession** (not fully-open): PoP is near-free and
  stops cross-user mapping poisoning on shared infrastructure; fully-open stays
  available only behind `--allow-anonymous`.
- **The auth token is a self-validating signed capability bound by PoP**, carried
  as a per-request authenticator field (the wire's "authorization header") —
  not an opaque server-issued bearer string (the coordinator has no state to
  validate one against).
- **Caps are genesis-issued in v1**; delegation chains deferred.
