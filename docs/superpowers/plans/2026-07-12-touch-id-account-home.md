# Touch ID custody + account-centric Home — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add native macOS Touch ID as a biometric unlock over the existing ed25519 vault, offer it at signup as an alternative to the recovery-phrase path, and promote the in-shell Account screen to a shell-level account-centric Home with a workspace table and smart boot.

**Architecture:** Touch ID stores the vault passphrase in a biometric-ACL macOS Keychain item — no Secure-Enclave key generation, no chain member key, no consensus change. A `#[cfg(target_os="macos")]` Tauri shim (`touchid.rs`) with non-macOS stubs exposes four commands. The frontend gains a two-card onboarding chooser, a Touch ID unlock button, and a full-window `HomeView` layer (gated by `state.atHome`) that re-parents the existing Account cards plus a workspace table.

**Tech Stack:** Rust / Tauri (macOS `security-framework`), React + TypeScript, vitest. Reuses the existing `user-key` CLI vault (`bin/node/src/userkey.rs`) verbatim.

## Global Constraints

- **Zero consensus change.** No new chain op, no new member key, no genesis movement. Verbatim from spec §"Consensus / compatibility".
- **Vault format unchanged.** `bin/node/src/userkey.rs` (argon2id + XChaCha20-Poly1305 over the ed25519 seed) is not touched. Touch ID only changes how the passphrase is supplied.
- **No human password on the Touch ID path.** "Use Touch ID" generates a random 32-byte passphrase the user never sees; the 24-word phrase is the sole recovery/fallback (spec D3).
- **The 24-word grid + confirm ceremony is KEPT** on the Touch ID path, reframed as recovery (spec §Design.2).
- **Native dep is macOS-target-scoped.** `security-framework` under `[target.'cfg(target_os="macos")'.dependencies]` so the Linux/wasm build is untouched (spec §Design.1).
- **Non-macOS builds compile working stubs:** `touchid_available` → `false`, the other three → a clear "unsupported on this platform" error.
- **Per-crate lint gate** (from CLAUDE.md): `ops/build-with.sh cargo clippy -p <crate> --tests --no-deps`. Build Rust via `ops/build-with.sh cargo ...`. Frontend tests: `cd app && bun run test`.
- **Enroll runs AFTER the phrase is confirmed** so a failed enroll never strands the user (spec §Design.2, §Error handling).
- **Home does NOT tear down the node connection** (spec §Design.5): opening Home is a view toggle, not a disconnect.
- **PR delivery:** screenshots in the PR body, screenshot **files deleted from the branch before merge** (spec §Delivery convention).
- **Real-Mac verification is the delivery gate** for the Touch ID ceremony; headless Linux cannot exercise it (spec §Testing).

---

## File Structure

**Native (Rust):**
- Create `app/src-tauri/src/touchid.rs` — the four `touchid_*` commands (cfg-macos real impl + non-macos stubs). One responsibility: the Keychain passphrase item.
- Modify `app/src-tauri/src/main.rs` — `mod touchid;` + register 4 commands + the macOS CEF `--use-mock-keychain` interaction note.
- Modify `app/src-tauri/build.rs` — add 4 command names to the allowlist.
- Modify `app/src-tauri/Cargo.toml` — `security-framework` under the macOS target table.
- Modify `app/src-tauri/src/user_identity.rs` — extract `pub(crate) fn unlock_with_secret` so `touchid_unlock` reuses the exact unlock+cache path.

**Frontend (TS/React):**
- Create `app/src/domain/touchid-client.ts` — `touchidAvailable/touchidEnroll/touchidUnlock/touchidDisable` + `randomPassphrase`.
- Modify `app/src/console/views/onboarding/IdentityGate.tsx` — Touch ID card in `AbsentScreen`, Touch ID variant of `CreateFlow`, Touch ID button in `LockedScreen`.
- Create `app/src/console/views/home/HomeView.tsx` + `WorkspacesTable.tsx` — the Home layer + workspace table.
- Move `ProfileCard.tsx`, `CustodyCard.tsx`, `DevicesCard.tsx` from `views/account/` into `views/home/` (re-parent).
- Modify `app/src/console/views/home/DevicesCard.tsx` — Touch ID status row.
- Modify `app/src/console/store/{state.ts,actions.ts,DucktapeProvider.tsx}` — `atHome` flag, `goHome`, smart boot.
- Modify `app/src/console/DucktapeConsole.tsx` (ConsoleBody) — branch to `HomeView`.
- Modify `app/src/console/layout/Sidebar.tsx` — avatar → `goHome()`.
- Modify `app/src/console/layout/ConsoleShell.tsx` — drop the in-shell `"account"` route.
- Modify `app/src/console/views/settings/SettingsView.tsx` — "Open Account" → `goHome()`.

---

## Task 1: Native shim skeleton + `touchid_available`

**Files:**
- Create: `app/src-tauri/src/touchid.rs`
- Modify: `app/src-tauri/src/main.rs` (add `mod touchid;`, register command in the `generate_handler!` list next to the `user_sign_*` block ~line 155)
- Modify: `app/src-tauri/build.rs` (add `"touchid_available"` after `"user_sign_remove_member"` ~line 63)
- Modify: `app/src-tauri/Cargo.toml`

**Interfaces:**
- Produces: `#[tauri::command] pub async fn touchid_available() -> bool`

- [ ] **Step 1: Add the macOS-scoped dependency**

In `app/src-tauri/Cargo.toml`, add a target table (place near the other target-specific deps; create it if absent):

```toml
[target.'cfg(target_os = "macos")'.dependencies]
security-framework = "3"
```

- [ ] **Step 2: Write the stub-side unit test**

Create `app/src-tauri/src/touchid.rs` with a test that pins the non-macOS contract (this test runs on the Linux CI host):

```rust
#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn available_is_false_off_macos() {
        assert!(!touchid_available().await);
    }

    #[tokio::test]
    async fn enroll_errs_off_macos() {
        assert!(touchid_enroll("x".into()).await.is_err());
    }
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `ops/build-with.sh cargo test -p ducktape-desktop --lib touchid 2>&1 | tail -20`
Expected: FAIL — `touchid_available` / `touchid_enroll` not found. (Crate name: confirm with `grep '^name' app/src-tauri/Cargo.toml`; use that `-p` value everywhere below.)

- [ ] **Step 4: Write the skeleton with the four command signatures**

At the top of `app/src-tauri/src/touchid.rs`:

```rust
//! Native macOS Touch ID custody: a biometric-ACL Keychain item holding the
//! vault passphrase. No Secure-Enclave key generation, no chain member key —
//! this only changes how `user_identity_unlock`'s passphrase is supplied.
//! Non-macOS targets compile working stubs.

const KEYCHAIN_SERVICE: &str = "com.ducktape.app.userkey";
const KEYCHAIN_ACCOUNT: &str = "vault-passphrase";

/// True only on macOS with a usable biometric authenticator. Gates every
/// piece of Touch ID UI.
#[tauri::command]
pub async fn touchid_available() -> bool {
    imp::available()
}

/// Store the vault passphrase behind a biometry-current-set ACL. Called once,
/// right after the recovery phrase is confirmed.
#[tauri::command]
pub async fn touchid_enroll(passphrase: String) -> Result<(), String> {
    imp::enroll(passphrase)
}

/// Retrieve the passphrase (prompts Touch ID), unlock the vault, cache it.
/// Returns the pubkey, exactly like `user_identity_unlock`.
#[tauri::command]
pub async fn touchid_unlock(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, crate::user_identity::NodeControl>,
) -> Result<crate::user_identity::IdentityPubkey, String> {
    imp::unlock(app, window, control).await
}

/// Delete the Keychain item. The account (seed + phrase) is unaffected.
#[tauri::command]
pub async fn touchid_disable() -> Result<(), String> {
    imp::disable()
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn available() -> bool {
        false
    }
    pub fn enroll(_p: String) -> Result<(), String> {
        Err("Touch ID is only available on macOS".into())
    }
    pub fn disable() -> Result<(), String> {
        Err("Touch ID is only available on macOS".into())
    }
    pub async fn unlock(
        _app: crate::rt::AppHandle,
        _window: crate::rt::WebviewWindow,
        _control: tauri::State<'_, crate::user_identity::NodeControl>,
    ) -> Result<crate::user_identity::IdentityPubkey, String> {
        Err("Touch ID is only available on macOS".into())
    }
}

#[cfg(target_os = "macos")]
mod imp;
```

Create an empty macOS impl stub so it compiles on the Mac too — `app/src-tauri/src/touchid_macos.rs` is not used; instead put the macOS `imp` inline (filled in Task 2/3). For now, on macOS only, add a temporary `available() -> false` in the `#[cfg(target_os="macos")] mod imp { ... }` block matching the same four fns. (The Linux CI host compiles the `not(macos)` arm, so this task is green there; the macOS arm is completed in Tasks 2–3.)

Check `NodeControl` and `IdentityPubkey` are `pub` in `user_identity.rs` (make them `pub` if they are `pub(crate)` — grep first).

- [ ] **Step 5: Register the command + allowlist**

In `app/src-tauri/src/main.rs`: add `mod touchid;` with the other `mod` lines, and add to the `tauri::generate_handler![...]` list after `user_identity::user_sign_remove_member,`:

```rust
            touchid::touchid_available,
            touchid::touchid_enroll,
            touchid::touchid_unlock,
            touchid::touchid_disable,
```

In `app/src-tauri/build.rs`, after `"user_sign_remove_member",`:

```rust
        "touchid_available",
        "touchid_enroll",
        "touchid_unlock",
        "touchid_disable",
```

- [ ] **Step 6: Run tests + clippy**

Run: `ops/build-with.sh cargo test -p ducktape-desktop --lib touchid 2>&1 | tail -20`
Expected: PASS (both non-macOS tests).
Run: `ops/build-with.sh cargo clippy -p ducktape-desktop --tests --no-deps 2>&1 | tail -15`
Expected: no new warnings in touchid.rs.

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri/src/touchid.rs app/src-tauri/src/main.rs app/src-tauri/build.rs app/src-tauri/Cargo.toml
git commit -m "feat(touchid): native shim skeleton + touchid_available stub"
```

---

## Task 2: Keychain enroll + disable (macOS)

**Files:**
- Modify: `app/src-tauri/src/touchid.rs` (fill the `#[cfg(target_os="macos")] mod imp` — `available`, `enroll`, `disable`)

**Interfaces:**
- Consumes: `KEYCHAIN_SERVICE`, `KEYCHAIN_ACCOUNT` from Task 1.
- Produces: working `imp::enroll(String)`, `imp::disable()`, `imp::available()` on macOS.

> **macOS-only boundary (spec §Testing):** this code cannot compile-run on the Linux CI host. The `security-framework` calls below target the v3 safe API; **verify the access-control setter against the pinned crate version on the Mac** during live QA. If the safe `ItemAddOptions` lacks an access-control setter in the pinned version, drop to `security-framework-sys::SecItemAdd` with a `CFDictionary` carrying `kSecClass=kSecClassGenericPassword`, `kSecAttrService`, `kSecAttrAccount`, `kSecAttrAccessControl` (from `SecAccessControlCreateWithFlags(kSecAttrAccessibleWhenUnlockedThisDeviceOnly, kSecAccessControlBiometryCurrentSet)`), `kSecUseDataProtectionKeychain=true`, `kSecValueData=<passphrase bytes>`.

- [ ] **Step 1: Write the macOS integration test (non-biometric round-trip)**

Add to `touchid.rs`, gated to macOS. It uses a **non-biometric** service name so it runs unattended (the biometric-prompt path is manual-only per spec):

```rust
#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    // Round-trips a throwaway *non-biometric* generic-password item to prove
    // the add/copy/delete plumbing and CFString keys are correct. The
    // biometric-ACL path (real enroll) is manual-only — Touch ID cannot be
    // driven in CI.
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    #[test]
    fn generic_password_round_trips() {
        let svc = "com.ducktape.app.test-roundtrip";
        let acct = "t";
        let _ = delete_generic_password(svc, acct);
        set_generic_password(svc, acct, b"secret").unwrap();
        assert_eq!(get_generic_password(svc, acct).unwrap(), b"secret");
        delete_generic_password(svc, acct).unwrap();
        assert!(get_generic_password(svc, acct).is_err());
    }
}
```

- [ ] **Step 2: Implement `available`, `enroll`, `disable` on macOS**

In the `#[cfg(target_os = "macos")] mod imp` block:

```rust
use security_framework::access_control::{ProtectionMode, SecAccessControl};
use security_framework::item::{ItemAddOptions, ItemClass, ItemSearchOptions};

pub fn available() -> bool {
    // A biometric authenticator is usable iff LAContext can evaluate
    // .deviceOwnerAuthenticationWithBiometrics. Cheapest reliable probe:
    // attempt to build a biometry-current-set access control; it succeeds
    // only when the platform supports biometric ACLs.
    SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
        // BIOMETRY_CURRENT_SET flag bit (kSecAccessControlBiometryCurrentSet = 1<<3)
        1 << 3,
    )
    .is_ok()
}

pub fn enroll(passphrase: String) -> Result<(), String> {
    let _ = disable(); // idempotent re-enroll
    let ac = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
        1 << 3, // biometry-current-set
    )
    .map_err(|e| format!("access-control: {e}"))?;
    ItemAddOptions::new(security_framework::item::ItemAddValue::Data(
        passphrase.into_bytes(),
    ))
    .set_service(super::KEYCHAIN_SERVICE)
    .set_account(super::KEYCHAIN_ACCOUNT)
    .set_access_control(ac)
    .add()
    .map_err(|e| format!("keychain add: {e}"))?;
    Ok(())
}

pub fn disable() -> Result<(), String> {
    let mut opts = ItemSearchOptions::new();
    opts.class(ItemClass::generic_password())
        .service(super::KEYCHAIN_SERVICE)
        .account(super::KEYCHAIN_ACCOUNT);
    // delete via the passwords helper (matches on service+account)
    match security_framework::passwords::delete_generic_password(
        super::KEYCHAIN_SERVICE,
        super::KEYCHAIN_ACCOUNT,
    ) {
        Ok(()) => Ok(()),
        // not-found is success for a disable
        Err(_) => Ok(()),
    }
}
```

> The exact `ItemAddOptions` setter names (`set_access_control`, `ItemAddValue::Data`) must be confirmed against the pinned `security-framework` version on the Mac; adjust to the crate's actual API. The CFDictionary escape hatch in the boundary note above is the fallback.

- [ ] **Step 3: Verify (Linux host)**

Run: `ops/build-with.sh cargo test -p ducktape-desktop --lib touchid 2>&1 | tail -15`
Expected: the non-macOS tests still PASS (the macOS arm is not compiled here). Note in the commit that macOS compile + `generic_password_round_trips` are verified on the Mac.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/touchid.rs
git commit -m "feat(touchid): keychain enroll/disable behind a biometric ACL (macOS)"
```

---

## Task 3: `touchid_unlock` reuses the vault unlock path

**Files:**
- Modify: `app/src-tauri/src/user_identity.rs` (extract `pub(crate) fn unlock_with_secret`)
- Modify: `app/src-tauri/src/touchid.rs` (macOS `imp::unlock`)

**Interfaces:**
- Produces: `pub(crate) fn unlock_with_secret(app: &crate::rt::AppHandle, password: secrecy::SecretString) -> Result<IdentityPubkey, String>` in `user_identity`.
- Consumes: that helper from `touchid::imp::unlock`.

- [ ] **Step 1: Confirm the existing unlock test names**

Run: `grep -nE "fn .*unlock|SESSION_PASSWORD|mod tests" app/src-tauri/src/user_identity.rs | head`
Note the existing unlock test to keep green.

- [ ] **Step 2: Extract the reusable helper**

In `user_identity.rs`, refactor `user_identity_unlock_blocking` (currently ~line 355) so its body moves into a crate-visible helper and the blocking fn calls it:

```rust
pub(crate) fn unlock_with_secret(
    app: &crate::rt::AppHandle,
    password: SecretString,
) -> Result<IdentityPubkey, String> {
    let out = run_verb_with_stdin(
        &[
            "user-key",
            "unlock",
            "--key",
            &user_key_path(app)?.to_string_lossy(),
        ],
        &[&password],
    )?;
    let pubkey = last_line(&out);
    cache_store(&password);
    Ok(IdentityPubkey { pubkey })
}

fn user_identity_unlock_blocking(
    app: crate::rt::AppHandle,
    password: SecretString,
) -> Result<IdentityPubkey, String> {
    unlock_with_secret(&app, password)
}
```

Ensure `IdentityPubkey` and `NodeControl` are `pub` (Task 1 note). Ensure `secrecy::SecretString` is the type used (`SecretString::new`).

- [ ] **Step 3: Implement macOS `imp::unlock`**

In `touchid.rs` macOS `imp`:

```rust
pub async fn unlock(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, crate::user_identity::NodeControl>,
) -> Result<crate::user_identity::IdentityPubkey, String> {
    crate::user_identity::require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || {
            let bytes = security_framework::passwords::get_generic_password(
                super::KEYCHAIN_SERVICE,
                super::KEYCHAIN_ACCOUNT,
            )
            .map_err(|_| "touchid-unavailable".to_string())?;
            let pass = secrecy::SecretString::new(
                String::from_utf8(bytes).map_err(|_| "corrupt keychain item")?.into(),
            );
            crate::user_identity::unlock_with_secret(&app, pass)
        })
        .await
}
```

`get_generic_password` triggers the OS Touch ID prompt because the item carries a biometric ACL. `"touchid-unavailable"` is the sentinel the frontend maps to "use your recovery phrase." Confirm `require_main_window` is `pub(crate)`; make it so if needed.

- [ ] **Step 4: Verify existing unlock behavior is intact**

Run: `ops/build-with.sh cargo test -p ducktape-desktop --lib user_identity 2>&1 | tail -15`
Expected: the pre-existing unlock test PASSES (refactor is behavior-preserving).
Run: `ops/build-with.sh cargo clippy -p ducktape-desktop --tests --no-deps 2>&1 | tail -10`
Expected: no new warnings.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src/user_identity.rs app/src-tauri/src/touchid.rs
git commit -m "feat(touchid): unlock reuses the vault unlock+cache path (macOS)"
```

---

## Task 4: TS client `touchid-client.ts`

**Files:**
- Create: `app/src/domain/touchid-client.ts`
- Create: `app/src/domain/touchid-client.test.ts`

**Interfaces:**
- Produces: `touchidAvailable(): Promise<boolean>`, `touchidEnroll(passphrase: string): Promise<void>`, `touchidUnlock(): Promise<{pubkey: string}>`, `touchidDisable(): Promise<void>`, `randomPassphrase(): string`.

- [ ] **Step 1: Write the test**

`app/src/domain/touchid-client.test.ts` — follow the mock pattern in `user-identity-client` tests (grep for how `invoke`/`isTauri` are mocked there):

```ts
import { describe, expect, it, vi } from "vitest";
import { randomPassphrase } from "./touchid-client";

describe("randomPassphrase", () => {
  it("is 32 bytes of base64, unique per call", () => {
    const a = randomPassphrase();
    const b = randomPassphrase();
    expect(a).not.toEqual(b);
    // 32 bytes → 44 base64 chars incl. padding
    expect(a.length).toBeGreaterThanOrEqual(43);
  });
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd app && bun run test touchid-client 2>&1 | tail -15`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the client**

`app/src/domain/touchid-client.ts` (mirror the `isTauri()`-guarded invoke shape of `user-identity-client.ts`):

```ts
import { invoke, isTauri } from "./tauri"; // match the import used by user-identity-client.ts

/** A random 32-byte passphrase, base64. Never shown to the user, never
 *  persisted in JS — it encrypts the vault and is handed straight to the
 *  Keychain. Recovery is the 24-word phrase. */
export const randomPassphrase = (): string => {
  const b = new Uint8Array(32);
  crypto.getRandomValues(b);
  return btoa(String.fromCharCode(...b));
};

export const touchidAvailable = (): Promise<boolean> =>
  isTauri() ? invoke<boolean>("touchid_available") : Promise.resolve(false);

export const touchidEnroll = (passphrase: string): Promise<void> =>
  invoke<void>("touchid_enroll", { passphrase });

export const touchidUnlock = (): Promise<{ pubkey: string }> =>
  invoke<{ pubkey: string }>("touchid_unlock");

export const touchidDisable = (): Promise<void> =>
  invoke<void>("touchid_disable");
```

Confirm the exact import path/name for `invoke`/`isTauri` from a sibling client (e.g. `user-identity-client.ts` line 1) and match it.

- [ ] **Step 4: Run tests**

Run: `cd app && bun run test touchid-client 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/domain/touchid-client.ts app/src/domain/touchid-client.test.ts
git commit -m "feat(touchid): TS client + randomPassphrase"
```

---

## Task 5: Onboarding — Touch ID card + CreateFlow variant

**Files:**
- Modify: `app/src/console/views/onboarding/IdentityGate.tsx` (`AbsentScreen`, `CreateFlow`)
- Modify/Create: `app/src/console/views/onboarding/IdentityGate.test.tsx` (or the existing `onboarding.test.tsx`)

**Interfaces:**
- Consumes: `touchidAvailable`, `touchidEnroll`, `randomPassphrase` (Task 4); existing `createIdentity`, `confirmMnemonic`.

- [ ] **Step 1: Write the failing test**

Add to the onboarding test file (mock `touchid-client`):

```tsx
import { vi } from "vitest";
vi.mock("../../../domain/touchid-client", () => ({
  touchidAvailable: vi.fn().mockResolvedValue(true),
  touchidEnroll: vi.fn().mockResolvedValue(undefined),
  randomPassphrase: () => "RANDOMPASS",
}));

it("Touch ID card is hidden when unavailable", async () => {
  // touchidAvailable → false variant: re-mock, render AbsentScreen, assert no "Use Touch ID"
});

it("Touch ID create: no password step, phrase shown, enroll after confirm", async () => {
  // render, click "Use Touch ID", assert MnemonicGrid appears WITHOUT a password field,
  // walk grid → confirm, assert createIdentity called with "RANDOMPASS" and
  // touchidEnroll called AFTER confirmMnemonic resolves.
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd app && bun run test onboarding 2>&1 | tail -20`
Expected: FAIL — no "Use Touch ID" affordance.

- [ ] **Step 3: Add availability probe + Touch ID tab to `AbsentScreen`**

In `IdentityGate.tsx`, extend `AbsentScreen`:

```tsx
function AbsentScreen({ onDone }: { onDone: () => void }) {
  const [mode, setMode] = useState<AbsentMode>("create");
  const [touchid, setTouchid] = useState(false);
  useEffect(() => {
    touchidAvailable().then(setTouchid).catch(() => setTouchid(false));
  }, []);
  if (mode === "touchid") return <TouchIdCreateFlow onDone={onDone} onSwitchMode={setMode} />;
  if (mode === "create") return <CreateFlow onDone={onDone} onSwitchMode={setMode} touchidAvailable={touchid} />;
  if (mode === "restore") return <RestoreFlow onDone={onDone} onSwitchMode={setMode} />;
  return <LinkFlow onDone={onDone} onSwitchMode={setMode} />;
}
```

Extend `AbsentMode`/`ABSENT_TABS` to include `"touchid"` (label "Use Touch ID") **only when available** — filter the tab list by a passed `touchidAvailable` flag so it never shows on Linux/Windows.

- [ ] **Step 4: Add `TouchIdCreateFlow`**

A trimmed `CreateFlow`: no password step, generate the passphrase, enroll after confirm. Insert next to `CreateFlow`:

```tsx
function TouchIdCreateFlow({
  onDone,
  onSwitchMode,
}: {
  onDone: () => void;
  onSwitchMode: (mode: AbsentMode) => void;
}) {
  const [step, setStep] = useState<"intro" | "grid" | "confirm">("intro");
  const [name, setName] = useState("");
  const [mnemonic, setMnemonic] = useState("");
  const [pass] = useState(randomPassphrase); // stable for this flow
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (step === "intro") {
    return (
      <GateCard
        title="Use Touch ID"
        subtitle="Unlock this Mac with Touch ID — no password to remember. You'll still get a 24-word recovery phrase: it's the only other way back into your account, so save it."
      >
        <ModeTabs tabs={ABSENT_TABS} mode="touchid" onSelect={onSwitchMode} />
        <input
          aria-label="Display name"
          value={name}
          placeholder="Your name (optional)"
          onChange={(e) => setName(e.target.value)}
          style={inputStyle}
        />
        <button
          disabled={busy}
          style={primaryButtonStyle}
          onClick={() => {
            setBusy(true);
            setError(null);
            createIdentity(pass)
              .then((created) => {
                const trimmed = name.trim();
                if (trimmed) savePendingDisplayName(trimmed);
                setMnemonic(created.mnemonic);
                setStep("grid");
              })
              .catch((err) => setError(errMessage(err)))
              .finally(() => setBusy(false));
          }}
        >
          {busy ? "Creating…" : "Continue with Touch ID"}
        </button>
        {error && <ErrText error={error} />}
      </GateCard>
    );
  }

  if (step === "grid") {
    return (
      <GateCard
        title="Save your recovery phrase"
        subtitle="These 24 words are the ONLY way back into your account if you lose this Mac. Write them down in order; they're shown only once."
      >
        <MnemonicGrid mnemonic={mnemonic} onContinue={() => setStep("confirm")} continueLabel="I've saved it — continue" />
      </GateCard>
    );
  }

  return (
    <GateCard title="Confirm your recovery phrase" subtitle="Enter the requested words to prove you saved them.">
      <ConfirmWords
        mnemonic={mnemonic}
        busy={busy}
        error={error}
        onConfirmed={() => {
          setBusy(true);
          setError(null);
          confirmMnemonic()
            // Enroll AFTER confirm; a failed enroll is non-fatal — the account
            // and phrase already work, Touch ID can be enabled later in Home.
            .then(() => touchidEnroll(pass).catch(() => undefined))
            .then(onDone)
            .catch((err) => setError(errMessage(err)))
            .finally(() => setBusy(false));
        }}
      />
    </GateCard>
  );
}
```

Reuse the existing `primaryButtonStyle`/`inputStyle`/`ErrText` (or the inline error span pattern already in the file — match what `CreateFlow` uses). Add the `touchidAvailable?: boolean` prop to `CreateFlow` only to filter its `ModeTabs`.

- [ ] **Step 5: Run tests**

Run: `cd app && bun run test onboarding 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add app/src/console/views/onboarding/IdentityGate.tsx app/src/console/views/onboarding/*onboarding*.test.tsx app/src/console/views/onboarding/IdentityGate.test.tsx
git commit -m "feat(onboarding): Touch ID create path (macOS)"
```

---

## Task 6: Unlock screen — "Unlock with Touch ID"

**Files:**
- Modify: `app/src/console/views/onboarding/IdentityGate.tsx` (`LockedScreen`)
- Modify: the onboarding test file

**Interfaces:**
- Consumes: `touchidAvailable`, `touchidUnlock` (Task 4).

- [ ] **Step 1: Write the failing test**

```tsx
it("locked screen offers Touch ID when available and unlocks with it", async () => {
  // touchidAvailable → true; render LockedScreen; assert "Unlock with Touch ID" button;
  // click → touchidUnlock called → onDone called.
});
it("touchid-unavailable sentinel routes to the recovery phrase", async () => {
  // touchidUnlock rejects with "touchid-unavailable"; assert the phrase/restore hint shows.
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd app && bun run test onboarding 2>&1 | tail -20`
Expected: FAIL — no Touch ID button in `LockedScreen`.

- [ ] **Step 3: Add the Touch ID branch to `LockedScreen`**

```tsx
function LockedScreen({ onDone, onSkip }: { onDone: () => void; onSkip: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [touchid, setTouchid] = useState(false);
  useEffect(() => {
    touchidAvailable().then(setTouchid).catch(() => setTouchid(false));
  }, []);

  const runTouchId = () => {
    setBusy(true);
    setError(null);
    touchidUnlock()
      .then(onDone)
      .catch((err) => {
        const msg = errMessage(err);
        setError(
          msg.includes("touchid-unavailable")
            ? "Touch ID is unavailable — unlock with your recovery phrase (Restore) instead."
            : msg,
        );
      })
      .finally(() => setBusy(false));
  };

  return (
    <GateCard title="Unlock your account" subtitle="Unlock this device for this session.">
      {touchid && (
        <button disabled={busy} style={primaryButtonStyle} onClick={runTouchId}>
          {busy ? "Unlocking…" : "Unlock with Touch ID"}
        </button>
      )}
      <PasswordForm
        mode="confirm"
        busy={busy}
        error={error}
        submitLabel={busy ? "Unlocking…" : "Unlock with password"}
        onSubmit={(password) => {
          setBusy(true);
          setError(null);
          unlockIdentity(password)
            .then(onDone)
            .catch((err) => setError(errMessage(err)))
            .finally(() => setBusy(false));
        }}
      />
      <button onClick={onSkip} style={linkButtonStyle}>
        Skip for now
      </button>
    </GateCard>
  );
}
```

The password field stays for recovery-phrase accounts; a Touch ID account's user simply won't know a password and uses the button (or Restore). This keeps one screen for both account types — no branching on account kind needed.

- [ ] **Step 4: Run tests**

Run: `cd app && bun run test onboarding 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/console/views/onboarding/IdentityGate.tsx app/src/console/views/onboarding/*.test.tsx
git commit -m "feat(onboarding): unlock with Touch ID + phrase fallback"
```

---

## Task 7: Smart boot — `atHome` state + `goHome`

**Files:**
- Modify: `app/src/console/store/state.ts` (add `atHome: boolean`, default false)
- Modify: `app/src/console/store/actions.ts` (add `goHome`, `enterWorkspace` reuses `selectWorkspace`)
- Modify: `app/src/console/store/DucktapeProvider.tsx` (boot: no active workspace → `atHome: true`)
- Modify: the store test (`workspace-management.test.tsx` or a new `home-routing.test.tsx`)

**Interfaces:**
- Produces: `state.atHome: boolean`; `actions.goHome(): void` (sets `atHome: true`, no disconnect); boot sets `atHome: true` when there is no active workspace and onboarding is not needed.

- [ ] **Step 1: Write the failing test**

```tsx
it("boot with no active workspace lands at Home, not onboarding", async () => {
  // mock listWorkspaces → [ws], activeWorkspace → null, identity unlocked
  // assert state.atHome === true and needsOnboarding === false
});
it("goHome does not disconnect", () => {
  // connected state → actions.goHome() → atHome true, nodeUrl unchanged
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd app && bun run test home-routing 2>&1 | tail -20` (or the store test file name)
Expected: FAIL — `atHome` undefined.

- [ ] **Step 3: Add `atHome` to state + `goHome` action**

In `state.ts` initial state add `atHome: false,` and to the `ConsoleState` type `atHome: boolean;` with a comment: `// The account-centric Home layer is showing (workspace shell is hidden). Not a disconnect.`

In `actions.ts` add near `setScreen`:

```ts
goHome: () => patch({ atHome: true }),
```

And ensure entering a workspace clears it — in `selectWorkspace`/`connectActive`, on successful adopt `patch({ atHome: false })` (add it where the shell becomes active).

- [ ] **Step 4: Wire smart boot**

In `DucktapeProvider.tsx` boot resolution (~line 417–474), where today "no active workspace → `needsOnboarding = true`": split the two cases — first run (no workspaces at all) stays onboarding; **has workspaces but none active → `patch({ atHome: true })`** instead. Leave the "active → connectActive" path as-is (it already lands in the shell; connectActive patches `atHome:false`).

- [ ] **Step 5: Run tests**

Run: `cd app && bun run test home-routing 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add app/src/console/store/state.ts app/src/console/store/actions.ts app/src/console/store/DucktapeProvider.tsx app/src/console/store/*home-routing*.test.tsx
git commit -m "feat(home): atHome state + goHome + smart boot"
```

---

## Task 8: HomeView layer + re-parent Account cards

**Files:**
- Create: `app/src/console/views/home/HomeView.tsx`
- Move: `views/account/ProfileCard.tsx` → `views/home/ProfileCard.tsx`; same for `CustodyCard.tsx`, `DevicesCard.tsx` (update imports)
- Modify: `app/src/console/DucktapeConsole.tsx` (ConsoleBody: branch to HomeView)
- Create: `app/src/console/views/home/HomeView.test.tsx`

**Interfaces:**
- Consumes: `state.atHome` (Task 7). Produces: `HomeView` rendered full-window when `atHome`.

- [ ] **Step 1: Write the failing test**

```tsx
it("HomeView shows profile + workspaces; custody always renders", () => {
  // render HomeView with a disconnected store; assert Profile + Custody present,
  // and a "connect a workspace" banner for chain-scoped cards.
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd app && bun run test HomeView 2>&1 | tail -15`
Expected: FAIL — module missing.

- [ ] **Step 3: Move the cards + build HomeView**

`git mv` the three card files into `views/home/`, fix their relative imports. Then create `HomeView.tsx` (composition root modeled on the existing `AccountView.tsx` — reuse its connected/disconnected banner logic verbatim):

```tsx
export function HomeView() {
  const { state } = useDucktape();
  return (
    <div style={homeShellStyle}>
      <ProfileCard />
      <WorkspacesTable />
      <DevicesCard />
      <CustodyCard />
    </div>
  );
}
```

`homeShellStyle`: full-window scroll container (max-width column, centered), matching the onboarding gate's full-window feel — no workspace rail.

- [ ] **Step 4: Branch ConsoleBody to HomeView**

In `DucktapeConsole.tsx` `ConsoleBody`, add the `atHome` branch (after `needsOnboarding`, before the shell):

```tsx
if (state.needsOnboarding) return <OnboardingGate />;
if (state.atHome) return <HomeView />;
if (state.bootError) return <NodeFailed />;
if (state.onboardingPhase) return <JoinProgress />;
return <ConsoleShell />;
```

- [ ] **Step 5: Run tests**

Run: `cd app && bun run test HomeView 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A app/src/console/views/home app/src/console/views/account app/src/console/DucktapeConsole.tsx
git commit -m "feat(home): HomeView layer, re-parent Account cards"
```

---

## Task 9: WorkspacesTable

**Files:**
- Create: `app/src/console/views/home/WorkspacesTable.tsx`
- Create: `app/src/console/views/home/WorkspacesTable.test.tsx`

**Interfaces:**
- Consumes: `state.workspaces`, `state.workspace` (active), `actions.selectWorkspace(id)`, `actions.newWorkspace()`. Standing per row from the connected chain projection ("—" for non-active rows).

- [ ] **Step 1: Write the failing test**

```tsx
it("renders a row per workspace and Enter selects it", async () => {
  // two workspaces, one active; assert 2 rows, active marker on one,
  // click Enter on the other → selectWorkspace(id) called.
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd app && bun run test WorkspacesTable 2>&1 | tail -15`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement the table**

```tsx
export function WorkspacesTable() {
  const { state, actions } = useDucktape();
  return (
    <section>
      <header style={sectionHeaderStyle}>
        <span>YOUR WORKSPACES</span>
        <div>
          {/* reuse OnboardingGate's Create/Join/Remote entry — open the gate */}
          <button style={ghostBtn} onClick={() => actions.newWorkspace()}>+ Add workspace</button>
        </div>
      </header>
      <table style={tableStyle}>
        <thead>
          <tr><th>Workspace</th><th>Network</th><th>Your standing</th><th></th><th></th></tr>
        </thead>
        <tbody>
          {state.workspaces.map((w) => {
            const active = state.workspace?.id === w.id;
            return (
              <tr key={w.id}>
                <td>{w.name}</td>
                <td style={mono}>{w.chainId}</td>
                <td>{active ? standingOf(state) : "—"}</td>
                <td>{active ? <span style={activeDot} title="Active" /> : null}</td>
                <td><button style={enterBtn} onClick={() => actions.selectWorkspace(w.id)}>Enter</button></td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}
```

`standingOf(state)` derives Validator / Resident / No-seat from the connected chain projections the same way `NodesCard`/`MembersView` already do (reuse that helper if one exists; otherwise a small local map over `state.members`/`state.residents`). Table styling via existing theme tokens (`color`, `font`).

- [ ] **Step 4: Run tests**

Run: `cd app && bun run test WorkspacesTable 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/console/views/home/WorkspacesTable.tsx app/src/console/views/home/WorkspacesTable.test.tsx
git commit -m "feat(home): workspace table with Enter"
```

---

## Task 10: DevicesCard — Touch ID status row

**Files:**
- Modify: `app/src/console/views/home/DevicesCard.tsx`
- Modify: `app/src/console/views/home/DevicesCard.test.tsx` (or add one)

**Interfaces:**
- Consumes: `touchidAvailable`, `touchidEnroll`, `touchidDisable` (Task 4).

- [ ] **Step 1: Write the failing test**

```tsx
it("shows an Enable Touch ID control on this device when available", async () => {
  // touchidAvailable → true; render; assert a Touch ID status row with Enable.
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd app && bun run test DevicesCard 2>&1 | tail -15`
Expected: FAIL.

- [ ] **Step 3: Add the Touch ID status row**

On the this-device member row, when `touchidAvailable()` is true, render a status/toggle. **Enable** requires the passphrase — but a Touch ID account has none to hand and a recovery-phrase account would need to type its password. Keep it honest and lazy: Enable is offered **only when the session is unlocked and a password is cached is not knowable here**, so gate Enable behind a small prompt: "Enable Touch ID" opens a password confirm (reusing `PasswordForm mode="confirm"`), and on submit calls `touchidEnroll(password)`. **Disable** calls `touchidDisable()` with a ConfirmDialog. (For accounts created via the Touch ID path, Touch ID is already enrolled at signup, so this row shows "Enabled · Disable".)

```tsx
// within DevicesCard, this-device row:
{touchid && (enabled
  ? <button style={ghostBtn} onClick={confirmDisable}>Touch ID: Enabled · Disable</button>
  : <button style={ghostBtn} onClick={() => setPromptOpen(true)}>Enable Touch ID</button>)}
```

Track `enabled` by attempting a lightweight presence read — since there is no "is-enrolled" command, add one **or** derive it: simplest is to add `touchid_enrolled(): bool` to the shim (a non-prompting `SecItemCopyMatching` with `kSecReturnData=false`). Add that command (mirrors Task 1 registration) if the reviewer prefers an explicit signal over inferring.

- [ ] **Step 4: Run tests**

Run: `cd app && bun run test DevicesCard 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/console/views/home/DevicesCard.tsx app/src/console/views/home/DevicesCard.test.tsx app/src-tauri/src/touchid.rs app/src-tauri/build.rs app/src-tauri/src/main.rs
git commit -m "feat(home): manage Touch ID from DevicesCard"
```

---

## Task 11: Repoint avatar + Settings to Home; drop in-shell account route

**Files:**
- Modify: `app/src/console/layout/Sidebar.tsx` (avatar `onClick`)
- Modify: `app/src/console/layout/ConsoleShell.tsx` (remove `"account"` branch + `AccountView` import)
- Modify: `app/src/console/views/settings/SettingsView.tsx` ("Open Account" → `goHome`)
- Delete: `app/src/console/views/account/AccountView.tsx` and its now-orphaned view dir (cards already moved in Task 8; keep `NodesCard` only if still referenced — check)

**Interfaces:**
- Consumes: `actions.goHome` (Task 7).

- [ ] **Step 1: Write/adjust the failing test**

```tsx
it("sidebar avatar opens Home", async () => {
  // render shell; click the avatar (aria-label "Account"); assert actions.goHome called / atHome true.
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd app && bun run test Sidebar 2>&1 | tail -15`
Expected: FAIL — avatar still calls `setScreen("account")`.

- [ ] **Step 3: Repoint the entry points**

In `Sidebar.tsx`, the avatar button (~line 167): `onClick={() => actions.goHome()}`, and drop the `state.screen === "account"` active styling (Home isn't a rail screen — the avatar can highlight on `state.atHome`).

In `ConsoleShell.tsx` `resolveScreen`: remove the `if (screen === "account") return AccountView;` line and the `AccountView` import.

In `SettingsView.tsx`, the "Open Account" row: `onClick={() => actions.goHome()}`.

- [ ] **Step 4: Delete the orphaned AccountView**

Check remaining references: `grep -rn "AccountView\|views/account" app/src`. Move any still-needed card (e.g. `NodesCard`) into `views/home/` or leave it if the workspace table subsumes it; delete `AccountView.tsx`. Update `AccountView.test.tsx` (delete or convert to `HomeView.test.tsx`).

- [ ] **Step 5: Run the full frontend suite**

Run: `cd app && bun run test 2>&1 | tail -25`
Expected: PASS (no dangling `account` route references).

- [ ] **Step 6: Commit**

```bash
git add -A app/src/console
git commit -m "feat(home): avatar + settings open Home; drop in-shell account route"
```

---

## Task 12: Full gates + live-QA checklist

**Files:** none (verification task)

- [ ] **Step 1: Frontend suite + typecheck**

Run: `cd app && bun run test 2>&1 | tail -20` — Expected: all PASS.
Run: `cd app && bun run build 2>&1 | tail -20` (or the tsc check the repo uses) — Expected: no type errors (dropped `account` route, moved cards).

- [ ] **Step 2: Rust gates**

Run: `ops/build-with.sh cargo test -p ducktape-desktop --lib 2>&1 | tail -20` — Expected: PASS.
Run: `ops/build-with.sh cargo clippy -p ducktape-desktop --tests --no-deps 2>&1 | tail -15` — Expected: no new warnings.

- [ ] **Step 3: Record the macOS-only gate in the PR**

The Touch ID ceremony is **not** verifiable on this headless Linux box. In the PR body, list the required real-Mac QA (run via remote-tauri):
  1. Fresh onboarding → "Use Touch ID" → phrase shown once → app usable.
  2. Quit, reopen → "Unlock with Touch ID" succeeds (biometric prompt).
  3. Home → DevicesCard → disable Touch ID → reopen → password/Restore path.
  4. `security-framework` compile + `generic_password_round_trips` green on macOS.

- [ ] **Step 4: PR screenshots**

Attach Home / two-card onboarding / unlock screenshots (fleet or tauri-debug) to the PR body. **Delete the screenshot files from the branch before merge** (spec §Delivery convention).

---

## Self-Review

**Spec coverage:** §Design.1 Touch ID mechanism → Tasks 1–3. §Design.2 onboarding two-card → Task 5. §Design.3 unlock → Task 6. §Design.4 HomeView → Tasks 8–9. §Design.5 smart boot → Task 7. §Design.6 Settings/Node → Task 11. §Error handling (unavailable/enroll-fail/invalidated) → Tasks 5 (non-fatal enroll), 6 (sentinel→phrase). §Consensus (zero change) → global constraint, no chain task. §Testing (Linux unit + macOS integration + live QA) → per-task tests + Task 12. §Delivery (screenshots then delete) → Task 12.

**Placeholder scan:** native FFI in Task 2 carries an explicit macOS-boundary caveat with a concrete CFDictionary fallback — this is a real, spec-acknowledged "can't compile on Linux" boundary, not a hand-wave. All TS steps show real code.

**Type consistency:** `randomPassphrase`/`touchidEnroll`/`touchidUnlock`/`touchidAvailable`/`touchidDisable` used identically in Tasks 4/5/6/10. `unlock_with_secret(&AppHandle, SecretString)` defined Task 3, consumed Task 3. `atHome`/`goHome` defined Task 7, consumed Tasks 8/11. `selectWorkspace(id)` used Task 9 matches the existing action. `IdentityPubkey`/`NodeControl` made `pub` in Task 1, used Task 3.

**Open confirmations for the implementer (grep-first, don't guess):** the desktop crate `-p` name; the `invoke`/`isTauri` import path in a sibling client; the exact `security-framework` v3 item-add access-control setter (macOS); whether a `standingOf` helper already exists to reuse.
