# W6 — Account Profile Propagation (spec)

Date: 2026-07-14. Epic: account/workspace separation (ledger §W6, binding).
Branch: `feat/w6-profile-propagation` off `epic/account-workspace-separation`.

## Goal

One account-level profile — **display name, avatar, bio/status** — defined once
(app-local, global; no per-network overrides) and **reconciled to each joined
network on next connect** (dirty ⇒ auto-push, mirroring the idempotent
auto-bind-on-connect pass). No background fan-out (single-active premise).

## Decisions (micro, made here — user unreachable)

1. **Identity ops, tight + additive** (W5 also edits this crate): keep the
   existing origin-gated `SetAccountName` for the name; add exactly **one** new
   origin-gated op `SetProfile { avatar: Option<String>, bio: Option<String> }`
   and exactly **two** new `AccountRecord`/`AccountView` fields (`avatar`,
   `bio`). Same authorization shape as `SetAccountName` (bound-node origin gate,
   no member signature, no nonce bump; empty-trim clears; length-capped). Not
   two separate avatar/bio ops — the app always holds the full profile, so one
   combined setter is fewer submits and cannot half-apply.
2. **Avatar storage rides duckfs** (same plane as chat attachments, #541).
   Avatar path is **content-addressed**: `/shared/attachments/avatars/<sha16>.<ext>`
   where `sha16` is the first 16 hex of SHA-256 over the image bytes. This makes
   avatar reconciliation *idempotent by path comparison* — the on-chain avatar
   ref already encodes the source-image identity, so "already reconciled" is a
   string compare, no re-upload. The identity module stores the path string
   only; bytes live in the files module.
3. **Reconcile = idempotent compare-and-push**, no persisted dirty flag. The
   chain is the per-network state; the "dirty flag" is derived (local desired ≠
   on-chain). Exactly mirrors `autoBindUserIdentity` (re-derives from chain each
   connect, no-ops when converged). Runs on connect (adopt + identity-unlock
   retry) and on panel save. Best-effort: never throws, never blocks connect.
4. **Name is now part of the global profile too.** `setDisplayName` also
   persists into the app-local profile store, so the name propagates to networks
   joined later (not only the active one). Reconcile pushes it via the existing
   `SetAccountName`, guarded by `!= on-chain` (idempotent). First-run parked-name
   flush is left untouched (W1 territory); reconcile self-heals `profile.name`
   from on-chain when empty.
5. **UI = self-contained `AccountProfilePanel`** (avatar picker + bio editor;
   shows the name it mirrors). Mounted in `HomeView` behind a thin seam next to
   `ProfileCard` (W1 rebuilds the account home; final placement reconciles on the
   epic branch). A reusable `Avatar` component loads a duckfs path → `<img>`,
   initials fallback — reused in `ProfileCard` and `MembersView` member rows.
6. **Render scope kept sane** (ledger mandate): own avatar/bio in the panel +
   `ProfileCard`; member avatars in `MembersView`. Chat message-author avatars
   are **deferred** (hot path, needs a per-author fetch/cache) — noted, not built.

## Caps

- `MAX_BIO_LEN = 280` bytes (status-length). `MAX_AVATAR_REF_LEN = 512` bytes
  (a duckfs path). App-side avatar image capped at `MAX_INLINE_COMMIT_BYTES`
  (256 KiB) so it rides one inline commit and stays localStorage-safe.

## App-hash / re-seed impact

Adding fields + an op to the identity module **moves the genesis app-hash**
(accepted per ledger). Existing QA networks must be **re-seeded**.

The `identity-wasm` component is a **single-sourced adapter** over the native
crate, so the source change IS the wasm change. The committed binary artifacts
(`crates/examples/identity-wasm/component.wasm` +
`crates/kernel/host/tests/fixtures/identity.component.wasm`) still need a
regeneration, which **could not run on this box** — the wasm32 build of the
transitive `blst` C dependency needs `clang`, absent here with no root to
install it. Consequence: `cargo test -p host --test wasm_identity_parity` is
**red until the fixture is rebuilt** on a clang-equipped box (the 4
identity-specific Makefile commands, or `make wasm-modules`). This rebuild is
mechanical and coincides with **W5's identical need** (it also edits identity) —
one rebuild on the epic branch after both source diffs merge resolves the blob.
Native identity tests (24) are green, proving the logic.

## Out of scope

Per-network nickname overrides; background multi-network fan-out; chat
message-author avatar rendering; avatar cropping/resize UI.
