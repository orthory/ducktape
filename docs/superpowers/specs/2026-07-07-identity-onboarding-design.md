# Identity Onboarding — Mnemonic + Password Custody for the User Key

Status: design of record for `feat/identity-onboarding`. Builds directly on the
user/node identity split (`docs/superpowers/specs/2026-07-07-user-node-identity-split-design.md`,
merged as PR #205). Goal: a real onboarding flow for the user identity — the
key gets a **24-word recovery mnemonic** and a **password-encrypted file at
rest**, with create / restore / unlock / reveal flows in the desktop app.

## Scope decisions (resolved with the user, 2026-07-07)

1. **Node keys are NOT derived from the user key.** Deterministic node keys
   invite "restore my validator elsewhere", which is an equivocation /
   double-voting hazard (the consensus journal is the only double-sign guard).
   Node keys stay random and disposable; lost device = `UnbindNode` + rejoin.
2. **Node keys are NOT encrypted at rest either — liveness first.** The node
   key is a hot secret (consensus signs with it every view; it is read at every
   boot; the daemon is a detached orphan that must respawn without a human).
   Chaining node boot to a password prompt can halt small networks (quorum
   needs every validator below n=4). `identity.key` stays plaintext 0600;
   stolen-disk risk is handled by unbind + OS full-disk encryption (documented).
3. **The user key is the cold secret and gets the full treatment.** It is only
   touched at bind / unbind / rename / restore — interactive moments — so a
   password gate costs zero node liveness. A locked identity never blocks a
   node: an unbound or locked workspace still validates, syncs, and serves.

## The custody model

```text
24-word mnemonic  ⟷  32-byte seed  (identity-preserving encoding, no KDF)
password ── argon2id ──> KEK ── XChaCha20-Poly1305 ──> ~/.ducktape/user.key (v2)
```

- **Mnemonic = the identity.** The 24 words are the BIP39 wordlist encoding of
  the exact 32-byte ed25519 seed (entropy → words with checksum, i.e.
  `Mnemonic::from_entropy`; the BIP39 PBKDF2 seed-stretch is deliberately NOT
  used). This makes the encoding identity-preserving: every EXISTING plaintext
  `user.key` (a raw hex seed) can be revealed as a mnemonic retroactively, and
  restore-by-mnemonic reproduces byte-identical keys for old and new users.
- **Password = local encryption only.** It never enters key derivation, so a
  forgotten password is recoverable: restore from mnemonic, set a new password.
  Losing BOTH the file and the mnemonic loses the identity (unchanged from v1's
  lose-the-file semantics — but now there is something to write down).
- Rationale over the alternative (password-in-derivation / BIP39 passphrase):
  mnemonic-alone recovery is the property users actually need; a
  derivation-entangled password makes the mnemonic silently insufficient.

## File format — `user.key` v2

- **Legacy (v1)**: 64 lowercase hex chars = plaintext seed. Still readable
  everywhere (back compat); onboarding migrates it (see Flows).
- **v2 (encrypted)**: single line
  `ducktape-user-key-v2:<base64(salt ‖ argon2-params ‖ nonce ‖ pubkey ‖ ciphertext)>`
  - `salt` 16 B random per file; argon2id parameters encoded explicitly
    (m=64 MiB, t=3, p=1 defaults) so they can be raised later without breaking
    old files; `nonce` 24 B (XChaCha20-Poly1305); `ciphertext` = seed (32 B)
    + AEAD tag, with the version line's prefix bound as associated data.
  - **`pubkey` (32 B) rides in the clear** — it is public data, and it lets
    `status` report identity + bind state while locked, without a password.
  - Written 0600 via `create_new` (same discipline as v1); rewrite-in-place
    (migration) goes through a temp file + rename.

## CLI verbs (crypto stays in `ducktape-node`; the shell stays crypto-free)

All secrets cross process boundaries via **stdin only** (never argv/env).
Stdin protocol: newline-delimited fields in documented order per verb.
Last stdout line = the value (existing `run_verb`/`last_line` contract).

- `user-key init --out <path>` — stdin: password. Generates a fresh seed,
  writes v2, prints `<mnemonic-24-words>\n<pubkey-hex>` (pubkey is the last
  line; the shell reads both). Refuses to overwrite (`create_new`).
- `user-key restore --out <path>` — stdin: mnemonic line, then password line.
  Validates checksum, writes v2, prints pubkey. Refuses to overwrite.
- `user-key unlock --key <path>` — stdin: password. Verifies decryption,
  prints pubkey. (Pure verification; nothing persists.)
- `user-key reveal --key <path>` — stdin: password (ignored/absent for legacy
  plaintext). Prints the 24-word mnemonic.
- `user-key encrypt --key <path>` — stdin: password. Migrates legacy v1 → v2
  in place (temp + rename). No-op error if already v2.
- `user-key status --key <path>` — no stdin. Prints one of
  `absent`, `plaintext <pubkey>`, `encrypted <pubkey>`.
- `user-sign-bind` / `user-sign-unbind` (existing) — gain stdin password
  support when `--key` points at a v2 file (legacy plaintext continues to work
  with no stdin). Output contract unchanged.
- The v1 `user-key --out` generate verb keeps working (tests/dev), documented
  as the legacy/dev shape.

New dependencies (bin/node only): `bip39` (wordlist + entropy checksum; its
PBKDF2 path unused), `argon2`, `chacha20poly1305` (RustCrypto). No dependency
enters the Tauri shell.

## Tauri surface (thin wrappers, camelCase, password via IPC then stdin)

- `user_identity_state()` → `{ state: "absent" | "plaintext" | "locked" |
  "unlocked", pubkey?: string }`. "unlocked" = the shell holds the
  session-cached password (below) for an encrypted file.
- `user_identity_create(password)` → `{ pubkey, mnemonic }` (init verb).
- `user_identity_restore(mnemonic, password)` → `{ pubkey }`.
- `user_identity_unlock(password)` → `{ pubkey }` — verifies via the unlock
  verb, then caches.
- `user_identity_reveal(password)` → `{ mnemonic }` — always re-prompts;
  the cache is never sufficient for reveal.
- `user_identity_encrypt(password)` → `{ pubkey }` — legacy migration.
- **Session cache:** the shell keeps the verified **password** (not the seed)
  in process memory only (a `Mutex<Option<Zeroizing<String>>>`-style cell —
  zeroize on drop; std-only equivalent acceptable), and feeds it on stdin to
  `user-sign-bind`/`user-sign-unbind`/`reveal` invocations. App restart =
  locked again. Existing `user_identity_status` stays for compat but the app
  moves to `user_identity_state`.
- `user_sign_bind`/`user_sign_unbind` commands: when state is "locked", they
  fail with a distinct `identity-locked` error the app can react to.

## App flows (gate order: identity → workspace)

A new identity gate renders BEFORE the existing create/join workspace
onboarding, driven by `user_identity_state()`:

- **absent** → "Create your identity" / "Restore from recovery phrase".
  - Create: password (entered twice, min 8 chars) → `init` runs → mnemonic
    shown ONCE in a copy-safe grid → partial confirmation (re-enter 3 randomly
    indexed words) → done. Skipping the confirmation is not offered. Closing
    after the password step leaves a valid encrypted identity (the file exists
    once `init` ran); the words remain recoverable via Reveal with the
    password, and the gate resumes at the mnemonic/confirm step on next
    launch until confirmed once (a local "mnemonic confirmed" flag in the
    registry, UX-only, no security weight).
  - Restore: 24-word input (wordlist-validated, checksum-checked via the
    verb) + new password → done. After the first workspace connects,
    auto-bind re-links this machine and the on-chain display name comes back
    by itself (the identity module record persists — no extra UI needed).
- **plaintext (legacy)** → one-time "Secure your identity" interstitial:
  set a password (encrypt verb) and view the mnemonic (reveal). Dismissable
  ("later") — it reappears next launch; Settings carries the same action.
- **locked** → password prompt with "skip for now". Skipping proceeds to the
  console: workspaces connect and nodes run normally; bind/rename actions and
  auto-bind stay dormant; Settings' Devices strip shows "identity locked —
  unlock to link this device" with an unlock button.
- **unlocked / plaintext** → straight to the console (auto-bind as today).

Auto-bind gating: `autoBindUserIdentity` adds one early state check — runs
only in "unlocked" or "plaintext" states (its result vocabulary gains
`"locked"`). Settings display-name editing likewise degrades to the profiles
path with a hint when locked.

Settings additions (Devices section): lock state row, Unlock button,
"Reveal recovery phrase" (password re-prompt, then the same copy-safe grid),
"Set password" for legacy plaintext identities. Change-password is deferred
(restore-with-new-password covers it).

## What does NOT change

- Node key generation, storage, boot, valset, consensus, wire formats, the
  identity module itself — zero consensus impact this time (no module-set
  change, no root change). The only bin/node changes are new/extended CLI
  verbs and the v2 file codec.
- Web build: no Tauri → identity gate never renders (state helper returns
  "absent"-equivalent no-op and the app behaves exactly as today).
- Headless nodes and fleet/QA rigs: untouched (they never had user keys).

## Error handling

- Wrong password → verb exits non-zero with a clean one-line error; the app
  shows it inline (no state change, no lockout counter in v1).
- Corrupt v2 file / bad AEAD tag → explicit "corrupt or wrong password" error
  surfaced in the gate and Settings (file is never overwritten silently —
  restore-from-mnemonic is the recovery path, same posture as v1).
- Invalid mnemonic word / checksum → inline validation before the verb runs
  (wordlist ships to the app for typeahead) AND authoritative rejection in the
  verb.
- Mid-create abort → no file written (init is atomic: password first, then
  create_new).

## Testing

- **Verb matrix (bin/node)**: init→status→reveal→restore round-trips to a
  byte-identical seed/pubkey; wrong password fails decrypt; tampered
  ciphertext/AAD fails; encrypt migrates legacy and reveal works on both
  formats; checksum-invalid mnemonic rejected; sign verbs work with v2+stdin
  password and legacy without; status output shapes.
- **Format**: v2 encode/decode strict (trailing bytes, bad base64, unknown
  version prefix), argon2 params round-trip, pubkey-in-clear matches seed.
- **App**: gate state machine (absent/plaintext/locked/unlocked renders the
  right screen); create-flow confirmation logic; restore validation; auto-bind
  locked-gating; Settings lock/unlock/reveal rows. Mock the Tauri commands
  like the existing workspace tests do.
- **e2e floor**: scripted verb round-trip (shell out to the real binary), plus
  the existing live-daemon vitest lane staying green. No consensus e2e needed.

## Resolved decisions

1. Mnemonic is an identity-preserving entropy encoding (no PBKDF2 stretch) —
   retrofits existing keys; restore reproduces exact seeds.
2. Password encrypts locally only; never enters derivation. Forgotten password
   → restore from mnemonic + new password.
3. Node keys: plaintext, random, disposable (liveness-first; equivocation
   guard). OS full-disk encryption is the documented at-rest answer for them.
4. Password required for NEW identities; legacy plaintext keys keep working
   with a persistent-but-dismissable "secure your identity" nudge.
5. Session cache holds the password in shell memory (zeroized), not the seed;
   reveal always re-prompts.
6. Deferred: change-password verb, device labels/naming, QR/cross-device
   mnemonic transport, unlock rate-limiting, OS-keychain integration.

## As-built amendments

1. `PasswordForm` ships two modes, `"set"` and `"confirm"` — there is no
   `"enter"` mode. `RestoreFlow` validates the 24 words client-side (count,
   wordlist membership) only once `PasswordForm`'s own set/confirm policy
   passes, so no Tauri call fires until both the password and the words are
   locally well-formed; the no-client-call-until-valid invariant holds.
2. The identity gate (`IdentityGate.tsx`) self-fetches its boot state via a
   local `useEffect` + `useState` calling `user_identity_state()` directly —
   it does not live in the console store and does not re-fetch on Settings
   custody changes within the same session. Settings' own custody panel
   fetches and refreshes its own copy independently.
3. `autoBindUserIdentity` maps every non-"unlocked"/non-"plaintext" state —
   including "absent" — to its `"locked"` result. Auto-bind is fire-and-forget
   or a no-standing identity; the mapping is inert (no identity, nothing to
   bind) rather than a distinct code path.
