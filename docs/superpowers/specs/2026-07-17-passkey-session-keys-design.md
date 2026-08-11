# Passkey Account Keys + Hyperliquid-Style Session Keys

Date: 2026-07-17
Status: **DRAFT** — design under review, not yet approved for implementation
Surfaces: identity consensus module, iced macOS app, macOS bundle/signing ops

## Goal

Make a macOS passkey a first-class **account member key** (primary credential),
and move all frequent signing onto **scoped, expiring, revocable session keys**
that a member key mints with one passkey sheet — Hyperliquid's master/agent
split, on Ducktape's existing Keybase-style multi-key account umbrella.

End state per device: no long-lived secret except on the founding Mac (cold
mnemonic vault). Daily UX: session valid → app just opens; session expired →
one passkey sheet (OS handles Touch ID → login-password fallback natively) →
new session → silence until expiry.

## What already exists (do not rebuild)

- `identity::KeyKind::WebauthnP256` with full assertion-envelope verification
  (challenge match, UP flag, RP-ID-hash enforcement, pure-Rust p256, raw R‖S).
  Consensus already accepts passkeys as member keys.
- The multi-key account umbrella: `AddMemberKey`/`RemoveMemberKey` with labels,
  possession proofs, mixed kinds, **any member may authorize** membership ops
  and any member may remove any key. This IS the Keybase requirement.
- `userkey_cli user-webauthn-challenge` — computes the exact challenge a
  passkey must sign for add-member possession.
- App-side member-key plumbing (`backend/signing.rs`, `screen_service/home.rs`):
  add/remove member ops, SEC1 33/65 validation, kind parsing.
- The Touch ID keychain shuttle (`backend/identity.rs::touch_id`) — retained
  ONLY for dev builds and passkey-less accounts; see Unlock repairs.

## Layer model

| Hyperliquid | Ducktape |
|---|---|
| master wallet | passkey member key (+ founding ed25519 = cold recovery) |
| approveAgent, one signature | one passkey assertion → `GrantSession` |
| agent key signs orders | session key signs content frames / everyday ops, no sheet |
| withdraw = master only | membership + session ops = member keys only |
| expiry, revoke, name | height expiry, `RevokeSession`, label |

**Member plane (durable, Keybase-style):** N keys of mixed kinds under one
account. Passkeys, hardware keys, founding ed25519 — equal standing.

**Session plane (ephemeral, NEW):** device-local ed25519 recorded on-chain as
"acts for account A until height H, content scope only". Not a member: cannot
authorize membership ops, cannot grant or revoke sessions.

## Consensus change (identity module — the one wire change)

New messages, flag-day wire update (no-backcompat doctrine):

- `GrantSession { session_key, expires_at, label, possession, authorizer }` —
  `authorizer` is any member (same `MemberAuth` shape as add-member;
  a passkey authorizer supplies the WebAuthn envelope). `possession` is the
  session key's own namespaced signature over a chain/account/nonce-scoped
  preimage (mirror the add-member preimage discipline, new namespace
  `IDENTITY_GRANT_SESSION_NS`). `expires_at` is a BLOCK HEIGHT
  (consensus time = height); reject grants already expired or beyond
  `MAX_SESSION_BLOCKS`.
- `RevokeSession { session_key, authorizer }` — any member. Sessions cannot
  revoke anything, including themselves.

State: sessions live beside the member set with the same pending-overlay /
commit / `root()` discipline the module already uses; `root()` commits to the
session set (sorted, length-prefixed) — session grant/expiry is consensus
state, so it must be deterministic and snapshot-committed.

Resolution: wherever origin→account attribution consults the member index,
an unexpired session resolves to its account for CONTENT attribution, but a
session key never satisfies "is a member" checks (membership ops, session
ops, custody/high-value ops). Expiry is checked against current height at
resolution time; expired sessions are inert (lazy cleanup — a sweep is not
required for correctness, `root()` may retain expired records until pruned).

Defaults: `MAX_SESSION_BLOCKS` and default grant length target ~30 days at
current block cadence; both are named constants set in the implementation
plan after measuring cadence.

**Prerequisite gate:** identity is a wasm module — this change is INERT until
the wasm is rebuilt (`include_bytes!` trap). The wasm-regen lane was broken on
dev (getrandom wbindgen poisoning, 2026-07-16). Fixing/verifying local regen
(wasi-sdk recipe) is a blocking precondition before any of this merges.
`wasm_identity_parity` must stay green. Governance-wire PRs gate on
`cargo clippy -p simnode` per standing doctrine.

## Native passkey transport (macOS, iced app)

New `app/src-iced/src/backend/passkey.rs`, `#[cfg(target_os = "macos")]`,
via `objc2-authentication-services` (fallback for any missing binding: raw
`msg_send!` in the same module — no Swift shim, no new toolchain):

- `register(challenge, account_label) -> (credential_id, sec1_pubkey)` —
  `ASAuthorizationPlatformPublicKeyCredentialProvider` create ceremony;
  extract the COSE key from the attestation and lift to raw SEC1 (33/65 B).
- `assert(challenge, credential_ids) -> WebauthnEnvelope { authenticator_data,
  client_data_json, signature_raw_rs, credential_id }` — get ceremony;
  normalize the DER ECDSA signature to raw 64-byte R‖S before it leaves the
  module (consensus is DER-free by design).
- Presentation anchor: NSWindow via winit raw-window-handle (the
  `browser/platform.rs` path). Delegate wired with objc2 `define_class!`;
  keep the controller alive until the reply lands (LAContext lesson).
- Enrollment produces TWO sheets (create, then assert for possession) unless
  the verifier is confirmed to accept a `webauthn.create` clientData type —
  pin this during implementation; do not assume.

RP ID: `ducktape.industries`. Sheets appear only in provisioned builds; the
module reports `unavailable` cleanly otherwise (no fake success).

## Ops / infrastructure

- App ID `com.ducktape.app` gains Associated Domains capability;
  entitlement `com.apple.developer.associated-domains` =
  `["webcredentials:ducktape.industries"]` added to
  `app/src-iced/assets/macos/Entitlements.plist` (signed builds only).
- `ops/stage-macos-iced-app.sh` embeds the provisioning profile
  (`embedded.provisionprofile` in `Contents/`) when the identity is a real
  Developer ID; ad-hoc path unchanged.
- AASA: static JSON at
  `https://ducktape.industries/.well-known/apple-app-site-association`
  (`webcredentials.apps = ["<TEAMID>.com.ducktape.app"]`), content-type
  `application/json`, no redirects. Dev iteration uses
  `webcredentials:ducktape.industries?mode=developer` + associated-domains
  developer mode on the QA Mac to bypass Apple CDN caching.

## App flows

1. **Founding onboarding (signed build):** mnemonic ceremony unchanged
   (consensus rule: only Ed25519 can found) → account created → immediately
   offer "Register passkey" (add-member, WebauthnP256) → mint first session →
   founding vault goes cold. Skipping passkey leaves today's behavior.
2. **Unlock:** session key present + unexpired → open, no prompt. Expired or
   absent → passkey assertion sheet → `GrantSession` → open. The session key
   sits in the login keychain (ThisDeviceOnly, no presence ACL) — losing it
   is harmless by construction (scoped, expiring, revocable).
3. **New Mac:** workspace invite (existing join machinery) for transport →
   "Connect this Mac with a passkey" → assertion → `GrantSession` → working. No mnemonic,
   no vault, no add-member.
4. **Key management (settings):** Keybase-style list of member keys —
   kind, label, added-at; add (passkey / paste hardware pubkey+proof),
   remove (any member authorizes). Session list per account with label,
   expiry height, revoke button.
5. **High-value ops** (membership, session grant/revoke, gateway routes,
   custody): passkey assertion each time where a passkey is the authorizer;
   resident member key signs where not.
6. **Dev builds / passkey-less accounts:** same session machinery, authorizer
   = resident member key (vault). Touch ID shuttle survives only here.

## Unlock-surface repairs (folded in, fixes the reported bug)

- `touch_id::available()` stops lying: consult LAContext
  `canEvaluatePolicy(DeviceOwnerAuthentication)` and enrollment state, not
  `SecAccessControl` construction.
- No prompt-less dead ends: every unlock path either shows an OS sheet
  (assertion or LAContext, both of which fall back to the login password
  natively) or surfaces the in-app password/recovery field — never a bare
  error string with no next step.
- Touch-ID-created accounts (random passphrase, user knows no password):
  guidance names the recovery phrase, not a password the user never had.

## Security invariants

- No secret material in iCloud, ever. The synced artifact is the passkey
  credential itself (Apple's E2E plane), which is an on-chain public key.
- Session compromise ≤ content scope × remaining height window; revocable by
  any member the moment it's noticed.
- Passkey loss: any other member key (mnemonic founding key included) removes
  the passkey member key and enrolls a new one. All-members-lost = account
  unrecoverable, by design (no custodial back door).
- Frame signing preimages already bind chain + namespace; session possession
  and grant preimages must bind chain-id, account, nonce, expiry, and label.

## Verification plan (QA doctrine: node semantics → simnode, UI → live app)

- simnode scenarios: grant/expiry-at-height/revoke/scope-refusal (session key
  attempting membership op MUST be rejected), authorizer-kind matrix
  (ed25519-granted and webauthn-granted sessions), wasm parity.
- Ceremony + entitlement live pass on macmini-duke (tailnet — LAN is dead):
  provisioned dev build, developer-mode AASA, register/assert round-trip,
  raw R‖S accepted by a live chain via `user-webauthn-challenge` flow.
- Unlock-surface behavior matrix on the Mac: Touch ID on/off × session
  valid/expired × passkey-less account — every cell shows a prompt or a
  usable field.

## Out of scope (deliberate)

- PRF / largeBlob, mnemonic sync, any secret in iCloud — superseded by the
  session plane.
- Fine-grained session ACLs (per-module scopes) — two tiers only
  (member vs session). Revisit only with a concrete abuse case.
- Founding an account WITH a passkey (consensus forbids; unchanged).
- iOS app, cross-workspace key propagation, hardware-FIDO USB transport
  (the P256 kind already covers pasted hardware keys).
