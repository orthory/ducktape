# Identity Onboarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The user key gets a 24-word recovery mnemonic and password encryption at rest, with create / restore / unlock / secure-legacy flows gated ahead of workspace onboarding in the desktop app.

**Architecture:** All crypto lands in `ducktape-node` (new `bin/node/src/userkey.rs` module + CLI verbs; secrets cross process boundaries via stdin only). The Tauri shell stays crypto-free: thin commands plus an in-memory session password cache. The app gains an identity gate (state machine over `absent | plaintext | locked | unlocked`) rendered before the existing `OnboardingGate`. Node keys, consensus, and wire formats are untouched — zero consensus impact.

**Tech Stack:** Rust (`bip39` for wordlist/entropy-checksum only, `argon2`, `chacha20poly1305`, `zeroize` — bin/node only), Tauri commands, TypeScript/React, vitest.

**Spec:** `docs/superpowers/specs/2026-07-07-identity-onboarding-design.md` (binding; read it before any task).

## Global Constraints

- Mnemonic = identity-preserving entropy encoding of the exact 32-byte seed (`bip39::Mnemonic::from_entropy` / `.to_entropy()`; the PBKDF2 `to_seed` path must NEVER be called). Restore must reproduce byte-identical seeds for keys created before this feature.
- Password encrypts locally only (argon2id m=64 MiB, t=3, p=1 defaults encoded in the file; XChaCha20-Poly1305; version-line prefix as AAD). It never enters key derivation.
- `user.key` v2 format: single line `ducktape-user-key-v2:<base64(salt(16) ‖ argon2-params ‖ nonce(24) ‖ pubkey(32) ‖ ciphertext(32+16))>`; pubkey rides in the clear. Legacy v1 = 64 hex chars, still readable everywhere.
- Secrets via stdin only — never argv, never env. Last stdout line = the value (`run_verb`/`last_line` contract).
- Node keys (`identity.key`), valset, consensus, frame format: DO NOT TOUCH.
- New Rust deps (`bip39`, `argon2`, `chacha20poly1305`, `zeroize`) go in bin/node's Cargo.toml (workspace-deps entries fine); NOTHING new in `app/src-tauri`.
- Password minimum 8 chars, enforced app-side AND in the `init`/`restore`/`encrypt` verbs.
- Web build (no Tauri): identity gate never renders; behavior identical to today.
- Commit per task; all work in worktree `<repo>/.claude/worktrees/feat+identity-onboarding` on branch `feat/identity-onboarding`. Run Rust tests `cargo test -p node-bin userkey` (module-scoped; the full node-bin suite only in the final sweep), app tests `cd app && bun run test -- --run`.

---

### Task 1: `userkey.rs` — v2 codec, crypto, mnemonic (bin/node)

**Files:**
- Create: `bin/node/src/userkey.rs` (module: `mod userkey;` in main.rs)
- Modify: `bin/node/Cargo.toml` (+ root `Cargo.toml` workspace-deps for bip39/argon2/chacha20poly1305/zeroize if not present)

**Interfaces (Produces):**
```rust
pub const USER_KEY_V2_PREFIX: &str = "ducktape-user-key-v2:";
pub enum UserKeyFile { Plaintext(ed25519::PrivateKey), Encrypted(EncryptedUserKey) }
pub struct EncryptedUserKey { pub pubkey: Vec<u8>, /* salt, params, nonce, ct held internally */ }
pub fn read_user_key_file(path: &Path) -> Result<UserKeyFile, String>       // sniffs v1 hex vs v2 prefix vs absent
pub fn seal_user_key(seed: &[u8; 32], password: &str) -> Result<String, String>   // -> full v2 line
pub fn open_user_key(line: &str, password: &str) -> Result<ed25519::PrivateKey, String> // AEAD failure -> "corrupt or wrong password"
pub fn mnemonic_of_seed(seed: &[u8; 32]) -> String                          // 24 words
pub fn seed_of_mnemonic(words: &str) -> Result<[u8; 32], String>            // checksum-validated
pub fn write_user_key_new(path: &Path, line: &str) -> Result<(), String>    // create_new, 0600
pub fn rewrite_user_key(path: &Path, line: &str) -> Result<(), String>      // temp + rename, 0600
```
- Consumes: `config::load_or_generate_identity`'s file discipline as the pattern; ed25519 types as used across bin/node.

- [ ] **Step 1 (TDD): failing test matrix** in `userkey.rs`'s `#[cfg(test)]`: seal/open round-trip; wrong password fails with the exact "corrupt or wrong password" message; tampered ciphertext AND tampered prefix (AAD) fail; v2 line parses to `Encrypted` with correct clear pubkey; legacy 64-hex parses to `Plaintext` with matching pubkey; junk/absent → errors; `mnemonic_of_seed`→`seed_of_mnemonic` round-trips a known seed byte-identically (fixed vector: seed `[7u8;32]` → assert the words round-trip AND the pubkey equals `from_seed`-style construction from those bytes); one-word-flipped mnemonic fails checksum; argon2 params round-trip through the encoded line; `write_user_key_new` refuses existing files; `rewrite_user_key` replaces atomically (old content unreadable after).
- [ ] **Step 2:** `cargo test -p node-bin userkey` → FAIL (module absent).
- [ ] **Step 3:** implement per the format constraint. bip39: `Mnemonic::from_entropy(&seed)` / `mnemonic.to_entropy()` — 32 bytes ⇒ 24 words automatically.
- [ ] **Step 4:** tests green; `cargo clippy -p node-bin -- -D warnings 2>&1 | grep userkey` shows nothing.
- [ ] **Step 5:** commit `feat(node): user-key v2 codec — argon2id+XChaCha at rest, entropy mnemonic`

---

### Task 2: CLI verbs (init / restore / unlock / reveal / encrypt / status + stdin passwords on sign verbs)

**Files:**
- Modify: `bin/node/src/main.rs` (verb dispatch beside the existing `user-key`/`user-sign-bind` verbs from #205)

**Interfaces:**
- Consumes: Task 1's `userkey.rs` API; existing `cmd_user_sign_bind`/`cmd_user_sign_unbind`/`user-key` verb structure (grep `"user-key"` / `"user-sign-bind"` in main.rs).
- Produces (stdin fields newline-delimited, in this order; last stdout line = value):
  - `user-key init --out <path>` — stdin `password` → prints mnemonic line then pubkey-hex line. `create_new` semantics.
  - `user-key restore --out <path>` — stdin `mnemonic`, `password` → prints pubkey.
  - `user-key unlock --key <path>` — stdin `password` → prints pubkey (verification only).
  - `user-key reveal --key <path>` — stdin `password` (line may be empty for legacy) → prints mnemonic.
  - `user-key encrypt --key <path>` — stdin `password` → migrates v1→v2 in place, prints pubkey; error if already v2.
  - `user-key status --key <path>` — no stdin → prints `absent` | `plaintext <pubkey>` | `encrypted <pubkey>`.
  - `user-sign-bind`/`user-sign-unbind`: when `--key` is v2, read `password` as the FIRST stdin line (legacy v1 = no stdin read, unchanged behavior). Output unchanged.
  - Legacy `user-key --out <path>` (bare generate) keeps working unchanged.
  - Password < 8 chars → error in init/restore/encrypt.
- [ ] **Step 1 (TDD where testable):** unit tests for the stdin-parsing helpers + an integration-style test module invoking the verb functions directly (the verb fns should take `&mut impl BufRead` for stdin so tests inject strings — mirror how existing verbs are structured; if they read `std::io::stdin()` inline, refactor the #205 sign verbs to the injectable shape as part of this task).
- [ ] **Step 2:** RED → implement → GREEN (`cargo test -p node-bin userkey_verbs` or the module name you choose).
- [ ] **Step 3: hand-verify the full lifecycle** in the scratchpad dir (`/tmp/claude-1000/-home-eddy-dev-ducktape/1be82b62-528e-4730-a8e3-aec6afacbcfc/scratchpad/`): init (capture mnemonic+pubkey) → status shows `encrypted <pubkey>` → unlock ok / wrong-password fails nonzero → reveal returns the same words → delete file → restore from words → status pubkey identical → user-sign-bind with v2 key + stdin password produces decodable JSON. Also: legacy flow — old `user-key --out` file → status `plaintext` → encrypt → status `encrypted`, reveal identical words before and after. Transcript in the report.
- [ ] **Step 4:** commit `feat(node): user-key lifecycle verbs — init, restore, unlock, reveal, encrypt, status`

---

### Task 3: Tauri commands + session password cache

**Files:**
- Modify: `app/src-tauri/src/user_identity.rs`, `app/src-tauri/src/main.rs` (register new commands), `app/src-tauri/src/workspaces.rs` (Registry gains `#[serde(default)] mnemonic_confirmed: bool` + a `user_identity_confirm_mnemonic` setter command — UX flag only)

**Interfaces:**
- Produces (camelCase wire like existing commands):
  - `user_identity_state() -> { state: "absent"|"plaintext"|"locked"|"unlocked", pubkey?: String, mnemonicConfirmed: bool }` (wraps `user-key status` + cache presence + registry flag)
  - `user_identity_create(password) -> { pubkey, mnemonic }` — runs init, caches password, leaves `mnemonic_confirmed=false`
  - `user_identity_restore(mnemonic, password) -> { pubkey }` — caches password, sets `mnemonic_confirmed=true`
  - `user_identity_unlock(password) -> { pubkey }` — verifies via unlock verb, caches
  - `user_identity_reveal(password) -> { mnemonic }` — ALWAYS uses the supplied password (never cache-only)
  - `user_identity_encrypt(password) -> { pubkey }` — legacy migration, caches
  - `user_identity_confirm_mnemonic() -> ()`
  - `user_identity_lock() -> ()` — drops the cache (Settings affordance)
  - Existing `user_sign_bind`/`user_sign_unbind`: if key file is v2 and cache empty → `Err("identity-locked")`; if cached → write password to child stdin. Existing `user_identity_status` kept as-is for compat.
- Cache: `static SESSION_PASSWORD: Mutex<Option<zeroize::Zeroizing<String>>>`… but NO new crypto deps in the shell — use a plain `Mutex<Option<String>>` with an explicit overwrite-with-empty on lock/drop and a comment (spec allows the std-only equivalent). Passwords go to child stdin via `Stdio::piped()` — extend `run_verb` with a `run_verb_with_stdin(bin, args, stdin_lines)` variant in workspaces.rs.
- [ ] **Step 1:** implement; `cargo check -p ducktape-desktop` + `cargo clippy -p ducktape-desktop -- -D warnings 2>&1 | tail -3` clean.
- [ ] **Step 2:** commit `feat(desktop): identity custody commands + session unlock cache`

---

### Task 4: TS client + auto-bind gating

**Files:**
- Create: `app/src/domain/user-identity-client.ts` + `.test.ts`
- Modify: `app/src/console/store/auto-bind.ts` + `auto-bind.test.ts`, `app/src/domain/workspace-client.ts` (only if the existing `userIdentityStatus` helper needs the new-state migration)

**Interfaces:**
- Produces: `export type IdentityState = "absent" | "plaintext" | "locked" | "unlocked"`; `identityState()`, `createIdentity(password)`, `restoreIdentity(mnemonic, password)`, `unlockIdentity(password)`, `revealMnemonic(password)`, `encryptLegacy(password)`, `confirmMnemonic()`, `lockIdentity()` — all `invoke`-wrappers guarded by `isTauri()` (non-Tauri: `identityState()` resolves `{ state: "absent", mnemonicConfirmed: true }` and mutators throw), mirroring `workspace-client.ts` style exactly.
- `autoBindUserIdentity`: first step now checks `identityState()`; proceeds only for `"unlocked" | "plaintext"`, returns new `"locked"` result otherwise (result union grows; existing tests updated, new locked-case test).
- [ ] **Step 1 (TDD):** client tests (invoke arg shapes, non-Tauri fallbacks) + auto-bind locked test → RED.
- [ ] **Step 2:** implement → GREEN; full app suite tail clean.
- [ ] **Step 3:** commit `feat(app): user-identity client + locked-gated auto-bind`

---

### Task 5: Identity gate UI

**Files:**
- Create: `app/src/console/views/onboarding/IdentityGate.tsx` (+ subcomponents in the same file unless it exceeds ~400 lines: password form, mnemonic grid, 3-word confirm, restore form, unlock form, secure-legacy interstitial), `IdentityGate.test.tsx`
- Modify: `app/src/console/DucktapeConsole.tsx` (render IdentityGate ahead of `OnboardingGate` when desktop && state ∉ {unlocked, plaintext-with-nudge-dismissed…} — read DucktapeConsole first; the gate ordering must reuse however OnboardingGate is currently gated), `app/src/console/store/state.ts` + `DucktapeProvider.tsx`/`actions.ts` (identity state slice: fetched at boot, refreshed after gate actions)

**Semantics (from spec — implement exactly):**
- absent → Create (password ×2 min 8 → create → mnemonic grid shown once → confirm 3 random indexed words → `confirmMnemonic()`) | Restore (24-word textarea with wordlist validation + password → restore).
- Create-flow resume: if state is locked/unlocked but `mnemonicConfirmed` is false → gate resumes at the mnemonic/confirm step (mnemonic re-fetched via `revealMnemonic` after password re-entry if the cache can't serve it — on fresh create the mnemonic is still in component state, no re-prompt).
- plaintext → dismissable "Secure your identity" interstitial (set password → encrypt; view mnemonic via reveal). Dismiss = continue to console this launch; reappears next launch.
- locked → unlock form with "skip for now" → console (auto-bind dormant).
- unlocked → no gate.
- Styling: reuse OnboardingGate's tokens/patterns (inputStyle etc.) — no new CSS files.
- [ ] **Step 1 (TDD):** gate state-machine tests (each state renders the right screen; create flow walks password→grid→confirm→done; confirm rejects a wrong word; restore validates checksum client-side via the wordlist; skip proceeds) with mocked client → RED.
- [ ] **Step 2:** implement → GREEN; full suite + `bun run build` clean.
- [ ] **Step 3:** commit `feat(app): identity gate — create, restore, unlock, secure-legacy flows`

---

### Task 6: Settings — lock state, unlock, reveal, set-password

**Files:**
- Modify: `app/src/console/views/settings/SettingsView.tsx` (Devices/identity section from #205) + its test file

**Semantics:** rows per spec — lock-state row ("Locked"/"Unlocked"/"Not password-protected"), Unlock button (password inline prompt), Lock button (drops cache), "Reveal recovery phrase" (password re-prompt → the same mnemonic grid in a modal/inline panel; never from cache alone), "Set password" for plaintext-legacy (→ encrypt). Reuse the gate's subcomponents where clean (export the mnemonic grid + password form from IdentityGate's file).
- [ ] **Step 1 (TDD):** settings tests: each state renders its rows; reveal requires password even when unlocked → RED → implement → GREEN; full suite clean.
- [ ] **Step 2:** commit `feat(app): identity custody controls in settings`

---

### Task 7: Docs

**Files:**
- Modify: `docs/src/content/docs/en/human/start/*` (find the getting-started/onboarding page; add the identity-creation step + recovery-phrase guidance); `docs/src/content/docs/en/human/modules/product-modules.mdx` identity section (one paragraph: custody model — mnemonic recovers identity, password is local-only, node keys deliberately stay plaintext/disposable for liveness + full-disk-encryption pointer); spec as-built amendments if any landed during implementation.
- [ ] **Step 1:** write; `cd docs && bun run build 2>&1 | tail -3` no new errors.
- [ ] **Step 2:** commit `docs(identity): onboarding + custody model documentation`

---

### Task 8: Verification sweep + finish

- [ ] `cargo test -p node-bin 2>&1 | tail -8` (full — verbs touched main.rs) and `cargo test -p identity 2>&1 | tail -3`; `cargo clippy -p node-bin -- -D warnings` no NEW warnings vs base.
- [ ] `cd app && bun run test -- --run 2>&1 | tail -5` + `bun run build 2>&1 | tail -3`.
- [ ] Scripted lifecycle re-run (Task 2's hand-verification transcript, fresh).
- [ ] Final whole-branch review → fix wave → PR to `dev` → merge (controller-driven).
