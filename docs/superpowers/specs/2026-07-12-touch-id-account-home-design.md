# Touch ID custody + account-centric Home

Date: 2026-07-12 · Target: `dev` · One combined redesign

Supersedes the deferred "Passkey/WebAuthn enrollment UI" non-goal of
`2026-07-10-account-console-onboarding-design.md` for the **macOS** case, and
promotes that spec's in-shell Account screen to a shell-level Home.

## Problem

Three gaps, delivered as one coherent redesign:

1. **Signup has one credential shape.** First-run Step 1 is always: set a
   password, memorize 24 words. There is no biometric option, even though
   macOS ships a Secure Enclave every user already trusts for everything else.
2. **Account management is welded to a workspace.** The Account screen is a
   route *inside* the workspace shell, and most of its cards read chain-scoped
   projections, so you must be connected to a network to see "your account."
   The person and the workspace are not physically separate.
3. **There is no account-centric front door.** Boot reconnects your last
   workspace and drops you into Chat; the workspace list is a first-run gate
   (`OnboardingGate`) reused as an ad-hoc switcher. There is no Keybase-style
   "here is you, here are your devices, here are your workspaces — enter one."

The passkey substrate is already built and unused: the consensus identity
module verifies WebAuthn assertions (`crates/system/identity/src/scheme.rs`),
the node CLI mints `p256` and `webauthn_p256` members
(`bin/node/src/userkey_cli.rs`), and `app/src/domain/passkey-enroll.ts` wraps
`navigator.credentials` — but nothing calls it, and browser WebAuthn is a dead
end in this app anyway (see Decisions). This redesign does **not** touch that
substrate; it stays as the future cross-platform lane.

## Decisions (resolved, load-bearing)

- **D1 — Touch ID gates the seed; it is NOT a chain member key.** The Secure
  Enclave holds the vault passphrase, not an on-chain key. Pure local custody,
  zero consensus change. (Rejected: a Secure-Enclave P-256 `AddMemberKey`
  member — stronger, but device-bound with no sync, more plumbing, and a harder
  lost-Mac story. It remains a clean future slice; the chain already supports
  it.)
- **D2 — Browser WebAuthn is not the mechanism; native Secure Enclave is.**
  Empirically established on the real CEF 148 runtime: `navigator.credentials`
  works only on the dev `http://localhost` origin and throws `SecurityError`
  on the release `tauri://` origin (Chromium's WebAuthn origin allowlist is
  stricter than secure-context). Serving the app from `http://localhost:<port>`
  to unblock it would force `rp.id="localhost"`, a **machine-global RP** shared
  by every loopback port on the box — an attacker-controlled local server could
  request an assertion over a chosen challenge (a valid identity-module
  preimage). Native Secure Enclave signing never touches a web origin and has
  neither problem. Gateway apps run on `duck://`, which cannot invoke WebAuthn
  at all, so no gateway-phishing path exists either way.
- **D3 — No human password on the Touch ID path.** "Use Touch ID" generates a
  random 32-byte passphrase the user never sees. The **24-word phrase is the
  sole recovery/fallback**. "No password to type, ever" is honest: the password
  exists but is machine-made and Keychain-held.
- **D4 — Home supersedes the in-shell Account screen.** The avatar opens Home,
  not a workspace-scoped route. Account cards move to Home; Settings stays
  thinned.
- **D5 — One combined redesign.** Onboarding + Touch ID + Home ship together.
  Accepted cost: the PR blocks on real-Mac verification of the Touch ID
  ceremony (this box is headless Linux, no Secure Enclave).

## Vocabulary

Unchanged from `2026-07-10-account-console-onboarding-design.md` (locked):
**Account** = the person (one 24-word phrase). **Device** = a machine holding a
member key. **Node** = a per-workspace daemon. **Workspace** = a network you
belong to. Touch ID adds no new noun — it is how a *device* unlocks its member
key on macOS.

## Design

### 1. Touch ID custody mechanism

The vault (`bin/node/src/userkey.rs`: argon2id + XChaCha20-Poly1305 over the
ed25519 seed; the mnemonic *is* the identity, the password is local-encryption
only) stays **byte-identical**. Touch ID only changes how the passphrase is
supplied.

**Enroll** (`touchid_enroll`): store the vault passphrase as a macOS Keychain
generic-password item whose `kSecAttrAccessControl` is
`SecAccessControlCreateWithFlags(kSecAccessControlBiometryCurrentSet)`. No
Secure-Enclave key *generation* is needed — a biometric-ACL Keychain item is
SE-protected and prompts for Touch ID on read. `BiometryCurrentSet` (not
`BiometryAny`) means a change to the enrolled fingerprint set invalidates the
item — a deliberate security default whose fallback is the recovery phrase.

**Unlock** (`touchid_unlock`): `SecItemCopyMatching` triggers the OS Touch ID
prompt and releases the passphrase, which feeds the **existing**
`user_identity_unlock` path (passphrase over stdin, never argv/env) and caches
in `SESSION_PASSWORD` (process memory, zeroized on drop). App restart re-locks,
exactly as today.

**Available** (`touchid_available`): true only on macOS with a usable
biometric authenticator; gates every piece of Touch ID UI. **Disable**
(`touchid_disable`): delete the Keychain item; the account is unaffected (seed
+ phrase remain), only the biometric shortcut is removed.

Native layer, `#[cfg(target_os = "macos")]` (the established pattern —
`menu.rs`, `tray.rs`, `notify/present.rs` all do it). New dep
`security-framework` scoped under `[target.'cfg(target_os="macos")'.dependencies]`
so the Linux/wasm build is untouched. `LAContext` is **not** required for v1
(the Keychain ACL prompts on its own); a custom reason string / Face ID reuse
window is optional later polish. Non-macOS builds compile stub commands
(`touchid_available` → `false`, the rest → a clear "unsupported on this
platform" error).

### 2. Onboarding: two-card credential choice

Step 1's `absent` screen (`IdentityGate` → `AbsentScreen`) becomes a chooser:

- **Use Touch ID** (rendered only when `touchid_available`): runs today's
  `CreateFlow` with two edits — the password step is dropped (a random
  passphrase is generated), and an enroll step is appended. **The 24-word grid
  + confirm ceremony is KEPT**, reframed as "your recovery phrase — the only
  other way in." Order: generate seed → show + confirm phrase →
  `touchid_enroll(randomPassphrase)`. Enroll runs *after* the phrase is
  confirmed, so a failed enroll never strands the user: the account exists, the
  phrase works, and Touch ID can be enabled later from Home.
- **Recovery phrase**: today's `CreateFlow` verbatim (password + 24 words).

Non-macOS shows only Recovery phrase. Restore, Link-device, and the phone-QR
`p256` enrollment are untouched. The passkey/WebAuthn code is untouched.

The `absent` state is the only screen reworked; the 4-state custody machine
(`absent | plaintext | locked | unlocked`) and the resume screen are kept.

### 3. Unlock screen (returning user)

`LockedScreen` gains a **Unlock with Touch ID** primary button when a Keychain
item exists (`touchid_available` && item present). A recovery-phrase account
keeps its password field. A Touch ID account (no known password) replaces the
password field with the Touch ID button plus **"Use recovery phrase instead"**
→ the restore flow, which re-arms Touch ID after restoring. If the item is
invalidated (fingerprint set changed → `errSecAuthFailed`/`errSecItemNotFound`),
the screen states it and routes to the phrase.

### 4. Account Home (shell-level layer)

A layer in the same window — not a new Tauri window — rendered by `ConsoleBody`
ahead of the workspace shell, generalizing today's `OnboardingGate`-as-switcher
into a first-class front door. Renders with **no workspace connected**
(machine-scoped cards always; chain-scoped cards show an honest "connect a
workspace" banner, as the current `AccountView` already does).

Sections (re-parenting existing components, one file each):

- **ProfileCard** — avatar initials, editable display name, account id + copy.
  Moved from the in-shell Account view.
- **DevicesCard** — the account's member keys as *devices* with scheme labels
  (Seed key / Security key / Passkey) and a this-device marker; **Touch ID
  status** on this device (Enabled / Enable / unavailable) wired to
  `touchid_enroll` / `touchid_disable`. Link-another-device and remove-key
  ceremonies unchanged.
- **WorkspacesTable** — the workspace list as a **table** (not cards), one row
  per registered workspace: **Workspace · Network · Your standing**
  (Validator / Resident / No-seat — from the connected chain's projections,
  "—" when that workspace isn't the connected one) **· Active** marker **·
  Enter**. Enter → `selectWorkspace(id)` → `connectActive` → workspace shell.
  Create / Join / Remote actions sit above the table (reusing
  `OnboardingGate`'s tabbed forms). This is the account-centric "enter a joined
  workspace" surface.
- **CustodyCard** — lock state, reveal recovery phrase, set/change password
  (recovery-phrase accounts), Touch ID toggle. Moved from the in-shell view.

### 5. Boot & routing (smart boot)

In `DucktapeProvider` boot resolution:

- First run (needs onboarding) → the onboarding stepper (unchanged).
- Has workspaces, one active → **auto-enter it** (`connectActive`, today's
  behavior) and land in the workspace shell. Home is one click away via the
  sidebar avatar or ⌘⇧H.
- Has workspaces, none active, or the user explicitly opens Home → **Home**.

Home is a shell state, not a teardown: opening Home from a connected workspace
does **not** disconnect the node, so Enter-ing back is instant. `resolveScreen`
gains a `"home"` branch; the avatar button sets it. The old in-shell `"account"`
screen redirects to Home (D4).

### 6. Settings / Node

Unchanged from the account-console spec: Settings stays thinned (Preferences ·
Workspace · Danger zone); the Node page owns daemon facts. The Account link row
in Settings now opens Home.

## Error handling

- Touch ID unavailable (Linux, no hardware, biometrics off) → `touchid_available`
  false → all Touch ID UI hidden; nothing degrades.
- Enroll fails / user cancels → non-fatal; account already exists on the phrase,
  surfaced as "Touch ID couldn't be set up — enable it later in Home."
- Item invalidated (fingerprint change) → unlock routes to the recovery phrase,
  which re-arms Touch ID.
- Locked-account signing keeps the existing `"identity-locked"` contract.

## Consensus / compatibility

**Zero consensus change.** No new chain op, no new member key, no genesis
movement. New Tauri commands only (`touchid_*`), all macOS-gated. New native
dep is macOS-target-scoped. No new `localStorage` keys required (Touch ID
presence is read live from the Keychain, not cached in the web layer).

## File plan (~600-line cap, split by responsibility)

```
app/src-tauri/src/touchid.rs            native Keychain enroll/unlock/disable/available (cfg macos + stub)
app/src-tauri/src/main.rs               register the 4 commands
app/src-tauri/build.rs                  command allowlist
app/src-tauri/Cargo.toml                security-framework under [target.'cfg(target_os="macos")']
app/src/domain/touchid-client.ts        TS wrappers over the commands
app/src/console/views/onboarding/
  IdentityGate.tsx                       absent -> two-card chooser
  CreateFlow (within)                    Touch ID variant (drop password, append enroll)
  LockedScreen (within)                  Unlock-with-Touch-ID button + phrase fallback
app/src/console/views/home/
  HomeView.tsx                           composition root (the layer)
  ProfileCard.tsx  DevicesCard.tsx  WorkspacesTable.tsx  CustodyCard.tsx  (re-parented)
app/src/console/layout/ConsoleShell.tsx  resolveScreen "home" branch
app/src/console/layout/Sidebar.tsx       avatar -> "home"; ⌘⇧H
app/src/console/store/{state.ts,actions.ts,DucktapeProvider.tsx}  smart-boot + "home" screen
```

## Testing

- **Unit (Linux, runnable here):** onboarding card selection + Touch ID
  variant (password dropped, enroll appended, phrase still shown); unlock-button
  visibility across custody states; biometry-unavailable fallbacks; smart-boot
  routing (first-run / active / none → onboarding / shell / Home); Home
  rendering connected vs disconnected; WorkspacesTable rows + Enter →
  `selectWorkspace`; command wiring behind mocked `touchid-client`.
- **Native (macOS only):** a `#[cfg(target_os="macos")]` integration test
  round-trips a throwaway Keychain item (store → read → delete). The
  **biometric-prompt path is manual-only** — automated coverage stops at the
  non-biometric API-shape level; the Touch ID gesture cannot be driven in CI.
- **Live QA (real Mac, user-owned):** fresh Touch ID signup → quit → reopen →
  Touch ID unlock → disable → recovery-phrase fallback. Headless Linux cannot
  exercise this; it is the delivery gate.

## Non-goals

- **Secure-Enclave member key** (option B) — future slice; chain already
  supports it.
- **Windows Hello** — same native pattern, later.
- **Removing or wiring browser WebAuthn** — the passkey substrate stays as the
  cross-platform lane; not this slice.
- **Any consensus change.**

## Delivery convention

The PR carries Home / onboarding / unlock **screenshots** in its body for
review (captured via the fleet / tauri-debug), and those screenshot **files are
deleted from the branch before merge** — review scaffolding, not repo content.
