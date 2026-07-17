# Bearer Client Invites — single-use, link-shaped onboarding for thin clients

Date: 2026-07-16. Status: Accepted (operator directive: single-use stays the
law — one invite admits exactly one redemption; this design ships to a PR).

## Problem

Every invite today is locked to one pre-exchanged key (`InviteToken.target`),
which forces a two-round-trip handshake: the invitee — the person who has
never run the app — must generate a key and deliver it to the inviter before
an invite can even be minted. That kills the product's front door for the
common case ("here, join our workspace") and makes pre-provisioning (mint
tonight, hand out tomorrow) impossible by construction.

Meanwhile the `Client` role is fully plumbed in consensus (`clients` module:
`Grant`/`Revoke`, submit door admits client standing, `handle_redeem` has a
Client arm) but **nothing can redeem it**: the lobby gate V8 refuses
`role != Resident` terminally ("the thin-client plane lands separately").

## Decision

Introduce **bearer invites**, constrained on three axes:

1. **Client role ONLY.** A bearer invite can never grant resident standing.
   A hijacked bearer invite caps out at submit authorization — no statesync,
   no mesh, no quorum seat (the `clients` module is read by the submit door
   and nothing else; see `relay.rs verify_relay_submit`).
2. **Single-use, first-wins.** The existing nonce exactly-once set
   (`pending_redeemed`/`redeemed`, shared across roles) already enforces
   this; bearer changes nothing about it. The first key to present a valid
   join proof takes the grant; the invite dies on commit.
3. **Short expiry.** Bearer invites default to a 1-day TTL (`--ttl-days`
   still overrides). Targeted invites keep their existing default.

Redemption rides the **existing `/v1/submit` lane** — no new gate, no new
route, no client-mode noded. `handle_redeem` already accepts any external
origin ("the token authorizes the admission, not the relaying node") and
noded's `/v1/submit` already settles-then-answers (the HTTP reply carries the
committed height or the deterministic reject reason). A thin client POSTs a
`GovMsg::Redeem` to any reachable member node and reads the verdict from the
HTTP response. The serving node needs no new code path at all.

## Alternatives rejected

- **A dedicated gate route (V1–V9 pre-filter over HTTP).** Duplicates checks
  consensus already decides authoritatively, adds surface, and the pre-filter
  exists only to protect the lobby's settle machinery — which HTTP does not
  use (the submit lane already blocks on consensus fate).
- **Full Design-4 client-mode noded (WG tunnel + proxy).** That is campaign
  PR9/PR10 scope: an entire connectivity plane. Bearer semantics do not
  depend on it; when PR9 lands it consumes the same tokens.
- **Multi-use bearer invites.** Operator ruling: one invite = one redemption,
  full stop. Group onboarding = mint N bearer invites.

## 1. Token: `target` becomes optional (wire flag day)

`governance::invite::InviteToken.target: ed25519::PublicKey` becomes
`Option<ed25519::PublicKey>`. `None` = bearer.

**The signed preimage** (changed IN PLACE — no dual-decode, no version tags, per the no-backcompat mandate; exactly ONE format exists and old invites simply fail to decode):

```
binding ‖ nonce ‖ kind ‖ [target] ‖ role ‖ expires_le
  kind = 0x01 ‖ target(32)   for targeted
       = 0x00                for bearer
```

The kind byte is signature-covered, so a targeted token cannot be replayed
as bearer or vice versa. All outstanding invites die with the preimage bump
(they live ≤7 days; flag-day precedent is standing law here).

**Invariant:** bearer ⇒ `role == Client`. Minting enforces it by
construction (`mint_bearer_client_token` is the only bearer constructor);
`handle_redeem` deterministically rejects a bearer Resident; the admission
doors (join paste, lobby V8, intro) role-gate everything else.
`verify_invite_token` stays pure signature math. The consensus copy is the
authority.

**Packed forms.** The 153-byte fixed token layout (`invite.token` file,
blob envelope) gains the kind byte and drops the target bytes when bearer.
`GovMsg::Redeem.target` (already `Vec<u8>`) carries empty bytes for bearer.
`GateMsg`/`IntroRequest` are untouched — see §3.

## 2. Consensus: `handle_redeem` bearer branch

In `crates/system/governance/src/lib.rs handle_redeem`:

- decode `target`: empty ⇒ bearer. Bearer with `role != Client` ⇒
  deterministic reject `"bearer invites are client-only"`.
- targeted: the existing `joiner != target` lock, unchanged.
- bearer: no target lock — the join proof (`binding ‖ nonce ‖ joiner`,
  `INVITE_JOIN_NAMESPACE`) binds the redemption to whichever key redeems
  first; the shared nonce set makes that exactly-once.
- everything else identical: token signature (over the new preimage), issuer
  still in members, role-specific grant (`ClientsMsg::Grant`), no expiry in
  consensus (block-height clock; joiner decode + serving-member wall clock
  remain the expiry authorities, single-use bounds the residual window).

`clients_min_version` is 0 on fresh networks — no version gating work.

**WASM regen is part of the change.** Governance runs as
`governance-wasm/component.wasm` via `include_bytes!` — without
`make wasm-modules` the consensus change is inert and `wasm-modules-check`
(in `make test`) pins the drift. Refreshed component bytes are committed
with the source. App-hash flag day: pre-existing networks must re-seed
(accepted, standard).

## 3. Node: mint + guards (the lobby gate does NOT change)

- `cmd_invite` gains `--role client`: with `--target` a targeted Client invite, without one a BEARER Client token,
  default TTL 1 day. The blob envelope is otherwise unchanged (descriptor +
  token + WG/fronts; the WG half is simply unused by a client redeemer).
- `cmd_join` (a NODE joining) refuses a client/bearer blob with a pointer at
  the client flow — a parked node redeeming a Client invite would gate-fail
  terminally anyway; fail at paste time instead.
- Lobby gate V8 and the intro doorbell already refuse `role != Resident`
  terminally; bearer never rides them (bearer ⇒ Client). A new lobby test
  pins that a bearer token verifies as `Client` — i.e. it lands exactly on
  those existing role gates, never on an admission path.
- `lobby::verify_join_request` keeps working for targeted tokens (its only
  callers are resident paths); it learns to DECODE the optional-target token
  but treats bearer as V8-refusable, not admittable.

## 4. Client side: `user-redeem-invite`

New `userkey_cli` verb (beside `user-sign-frame`):

```
ducktape-node user-redeem-invite <blob> --node <http-base> [--key <path>]
```

1. `decode_invite` (fail-closed expiry at decode, envelope + token verify).
2. Refuse a targeted blob whose target ≠ this key (same message as
   `cmd_join`); refuse a Resident blob ("this is a node invite — use join").
3. Sign the join proof with the **account key** (the user identity file the
   userkey plane already manages) — client standing attaches to the account
   key, which is the key `/v1/submit/frame` submissions are signed with
   (#579 lane).
4. POST `{target: "governance", payload: Redeem{...}}` to `<node>/v1/submit`.
5. Print the verdict: committed height = admitted; `"already holds client
   standing"` = idempotent success; any other reject verbatim, exit nonzero.

The serving node URL is handed out beside the invite (exactly the Remote
tab's current model). Embedding a serve URL in the blob is a deliberate
non-goal (revisit with PR9/PR10 UX).

## 5. What a redeemed client CAN do (unchanged, for the record)

Client standing = its key passes `verify_relay_submit` (submit door) and the
`clients` query set. Nothing else reads the set: no statesync, no mesh
session, no quorum, no gateway control. Revocation = `ClientsMsg::Revoke`
(already implemented) via normal governance submit.

## 6. Testing

- **governance unit** (native, runs pre-wasm): bearer mint/verify roundtrip;
  new-preimage pins (kind byte covered by sig — mutate target/kind ⇒ verify
  fails); bearer+Resident refused at mint, verify, and redeem; bearer redeem
  grants client standing; targeted lock still enforced; **the bearer race
  pin**: two keys, one nonce — first commit wins, second deterministically
  rejects `already redeemed`.
- **wasm parity**: `wasm_governance_parity` must stay green (same vectors
  through the component).
- **simnode** (standing gate for governance-wire PRs): scenario pins for
  bearer grant + single-use race + bearer-resident reject at consensus.
- **lobby/gate pins**: bearer token over `GateMsg` ⇒ `RoleUnsupported`
  terminal; codec roundtrip of the optional-target token file and blob.
- **e2e (bin/node tests)**: mint `--role client` → `user-redeem-invite` against a
  live single-node net over HTTP → clients query shows the key → second
  redeem of the same blob by another key rejects. (`invite_e2e::live_quorum`
  is known-broken on pristine dev — not this PR's gate.)

## 7. Out of scope

- App UX (Members mint button, Remote-tab redeem flow) — PR10 territory;
  the CSP remote-origin limit (#599) binds it anyway.
- Client-mode noded / WG tunnel (PR9).
- Multi-use or revocable-before-use invites.
- Any resident-path behavior change: the lobby gate, intro, park FSM, and
  statesync door are untouched by this PR.
