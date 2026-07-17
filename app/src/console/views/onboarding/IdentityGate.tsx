// The account gate (desktop only): renders BEFORE the workspace onboarding
// gate, driven by `user_identity_state()`. The account key is machine-scoped
// and orthogonal to any workspace/node — so unlike OnboardingGate (which reads
// `state.needsOnboarding` off the console store, gated on node/workspace
// resolution), this gate fetches its own boot state directly: a local
// `useEffect` + `useState`, self-contained, no store wiring. See
// IdentityGate.test.tsx / the task report for the full wiring rationale.
//
// State machine (spec: docs/superpowers/specs/2026-07-07-identity-onboarding-design.md,
// vocabulary + link path: docs/superpowers/specs/2026-07-10-account-console-onboarding-design.md):
//   absent    → Create | Restore | Link-device chooser (first-run step 1 of 3)
//   plaintext → dismissable "Secure your account" interstitial (dismiss = this
//               launch only, plain component state, never persisted)
//   locked    → unlock form, "skip for now" proceeds to the console
//   unlocked  → no gate
// A create that was interrupted before the mnemonic was confirmed resumes at
// the mnemonic/confirm step on relaunch (locked or unlocked, mnemonicConfirmed
// false) — password re-entry then `revealMnemonic`, since a fresh mount never
// still holds the mnemonic in component state. Same "skip for now" escape as
// the locked screen: this resume step is still a hard requirement to type a
// password (possibly forgotten) into, so it must not be able to trap someone
// out of the console forever — skipping just means the gate re-offers next
// launch, same as an unconfirmed mnemonic always has.
//
// The card chrome, password form, mnemonic grid, and confirm-words step live
// in the sibling IdentityGateForms.tsx; the first-run step rail lives in
// OnboardingChrome.tsx; the new-device link wizard in LinkDeviceFlow.tsx.

import { useCallback, useEffect, useState } from "react";
import type { ReactNode } from "react";

import { hasNativeShell } from "../../../domain/node-bootstrap";
import { BIP39_ENGLISH_SET } from "../../../domain/bip39-wordlist";
import {
  confirmMnemonic,
  createIdentity,
  encryptLegacy,
  identityState,
  restoreIdentity,
  revealMnemonic,
  unlockIdentity,
} from "../../../domain/user-identity-client";
import type { IdentityStateReport } from "../../../domain/user-identity-client";
import {
  randomPassphrase,
  touchidAvailable,
  touchidEnroll,
  touchidUnlock,
} from "../../../domain/touchid-client";
import { saveLinkPending, savePendingDisplayName } from "../../store/state";
import { color, font } from "../../theme/tokens";
import {
  ConfirmWords,
  GateCard,
  MnemonicGrid,
  ModeTabs,
  PasswordForm,
  errMessage,
  errorTextStyle,
  inputStyle,
  linkButtonStyle,
  primaryButtonStyle,
} from "./IdentityGateForms";
import { LinkDeviceFlow } from "./LinkDeviceFlow";
import { OnboardingChrome } from "./OnboardingChrome";

// ── absent: the three entry paths ────────────────────────────────────────

const ABSENT_TABS = [
  { id: "create", label: "Create" },
  { id: "restore", label: "Restore" },
  { id: "link", label: "Link device" },
] as const;

// The Touch ID entry sits next to Create, but only on a Mac with a usable
// biometric authenticator — it's spliced into the Create/Touch-ID screens'
// tab rail when `touchidAvailable()` resolves true, never shown elsewhere.
const TOUCHID_TAB = { id: "touchid", label: "Use Touch ID" } as const;
const CREATE_TABS_WITH_TOUCHID = [ABSENT_TABS[0], TOUCHID_TAB, ...ABSENT_TABS.slice(1)] as const;

type AbsentMode = (typeof ABSENT_TABS)[number]["id"] | "touchid";

// ── absent: create ──────────────────────────────────────────────────────

type CreateStep = "password" | "grid" | "confirm";

function CreateFlow({
  onDone,
  onSwitchMode,
  touchidAvailable = false,
}: {
  onDone: () => void;
  onSwitchMode: (mode: AbsentMode) => void;
  /** Splices the "Use Touch ID" tab into this screen's rail on a capable Mac. */
  touchidAvailable?: boolean;
}) {
  const [step, setStep] = useState<CreateStep>("password");
  const [name, setName] = useState("");
  const [mnemonic, setMnemonic] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (step === "password") {
    return (
      <GateCard
        title="Create your account"
        subtitle="One account for all your devices and workspaces. Set a password to protect it on this device — your 24-word recovery phrase comes next."
      >
        <ModeTabs
          tabs={touchidAvailable ? CREATE_TABS_WITH_TOUCHID : ABSENT_TABS}
          mode="create"
          onSelect={onSwitchMode}
        />
        <input
          aria-label="Display name"
          value={name}
          placeholder="Your name (optional)"
          onChange={(event) => setName(event.target.value)}
          style={inputStyle}
        />
        <PasswordForm
          mode="set"
          busy={busy}
          error={error}
          submitLabel={busy ? "Creating…" : "Create account"}
          onSubmit={(password) => {
            setBusy(true);
            setError(null);
            createIdentity(password)
              .then((created) => {
                // The chosen name can only land on-chain after the first node
                // connects (names are chain-scoped) — park it until then.
                const trimmed = name.trim();
                if (trimmed) savePendingDisplayName(trimmed);
                setMnemonic(created.mnemonic);
                setStep("grid");
              })
              .catch((err) => setError(errMessage(err)))
              .finally(() => setBusy(false));
          }}
        />
      </GateCard>
    );
  }

  if (step === "grid") {
    return (
      <GateCard
        title="Save your recovery phrase"
        subtitle="These 24 words ARE your account — anyone holding them can restore it anywhere. Write them down in order; they're shown only once."
      >
        <MnemonicGrid
          mnemonic={mnemonic}
          onContinue={() => setStep("confirm")}
          continueLabel="I've saved it — continue"
        />
      </GateCard>
    );
  }

  return (
    <GateCard
      title="Confirm your recovery phrase"
      subtitle="Enter the requested words to prove you saved them."
    >
      <ConfirmWords
        mnemonic={mnemonic}
        busy={busy}
        error={error}
        onConfirmed={() => {
          setBusy(true);
          setError(null);
          confirmMnemonic()
            .then(onDone)
            .catch((err) => setError(errMessage(err)))
            .finally(() => setBusy(false));
        }}
      />
    </GateCard>
  );
}
// ── absent: create with Touch ID (macOS) ───────────────────────────────

type TouchIdStep = "intro" | "grid" | "confirm";

// Same ceremony as CreateFlow minus the human password: the vault is sealed
// with a random 32-byte passphrase the user never sees (stashed in the
// biometric Keychain after confirm), and the 24-word phrase is reframed as the
// sole recovery path. Enroll runs AFTER confirm and is non-fatal — the account
// already exists by then, so a failed enroll can never strand the user (they
// can enable Touch ID later from Home).
function TouchIdCreateFlow({
  onDone,
  onSwitchMode,
}: {
  onDone: () => void;
  onSwitchMode: (mode: AbsentMode) => void;
}) {
  const [step, setStep] = useState<TouchIdStep>("intro");
  const [name, setName] = useState("");
  const [mnemonic, setMnemonic] = useState("");
  const [pass] = useState(randomPassphrase); // generated once, stable for this flow
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (step === "intro") {
    return (
      <GateCard
        title="Use Touch ID"
        subtitle="Unlock this Mac with Touch ID — no password to remember. You'll still get a 24-word recovery phrase: it's the only other way back into your account, so save it."
      >
        <ModeTabs tabs={CREATE_TABS_WITH_TOUCHID} mode="touchid" onSelect={onSwitchMode} />
        <input
          aria-label="Display name"
          value={name}
          placeholder="Your name (optional)"
          onChange={(event) => setName(event.target.value)}
          style={inputStyle}
        />
        <button
          disabled={busy}
          style={primaryButtonStyle(busy)}
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
        {error && <span style={errorTextStyle}>{error}</span>}
      </GateCard>
    );
  }

  if (step === "grid") {
    return (
      <GateCard
        title="Save your recovery phrase"
        subtitle="These 24 words are the ONLY way back into your account if you lose this Mac. Write them down in order; they're shown only once."
      >
        <MnemonicGrid
          mnemonic={mnemonic}
          onContinue={() => setStep("confirm")}
          continueLabel="I've saved it — continue"
        />
      </GateCard>
    );
  }

  return (
    <GateCard
      title="Confirm your recovery phrase"
      subtitle="Enter the requested words to prove you saved them."
    >
      <ConfirmWords
        mnemonic={mnemonic}
        busy={busy}
        error={error}
        onConfirmed={() => {
          setBusy(true);
          setError(null);
          confirmMnemonic()
            // Enroll AFTER confirm; a failed enroll is swallowed on purpose —
            // the account + phrase already work, Touch ID can be enabled later.
            .then(() => touchidEnroll(pass).catch(() => undefined))
            .then(onDone)
            .catch((err) => setError(errMessage(err)))
            .finally(() => setBusy(false));
        }}
      />
    </GateCard>
  );
}

// ── absent: restore ─────────────────────────────────────────────────────

function RestoreFlow({
  onDone,
  onSwitchMode,
}: {
  onDone: () => void;
  onSwitchMode: (mode: AbsentMode) => void;
}) {
  const [words, setWords] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // PasswordForm owns the password ×2 policy (min length, mismatch), so this
  // only fires once the password is valid — it validates the words, then runs
  // the restore. Word errors surface through PasswordForm's `error` prop, the
  // same inline slot the server's checksum rejection lands in.
  const submit = (password: string) => {
    setError(null);
    const list = words
      .trim()
      .split(/\s+/)
      .filter(Boolean)
      .map((w) => w.toLowerCase());
    if (list.length !== 24) {
      setError(`enter all 24 words (got ${list.length})`);
      return;
    }
    const bad = list.find((w) => !BIP39_ENGLISH_SET.has(w));
    if (bad) {
      setError(`"${bad}" is not a recovery-phrase word`);
      return;
    }
    setBusy(true);
    restoreIdentity(list.join(" "), password)
      .then(onDone)
      .catch((err) => setError(errMessage(err)))
      .finally(() => setBusy(false));
  };

  return (
    <GateCard
      title="Restore your account"
      subtitle="Enter your 24-word recovery phrase and set a new password for this device."
    >
      <ModeTabs tabs={ABSENT_TABS} mode="restore" onSelect={onSwitchMode} />
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <textarea
          value={words}
          onChange={(event) => setWords(event.target.value)}
          placeholder="24-word recovery phrase, separated by spaces"
          rows={4}
          style={{ ...inputStyle, resize: "vertical", font: `500 11px ${font.mono}` }}
        />
        <PasswordForm
          mode="set"
          busy={busy}
          error={error}
          placeholder="New password"
          confirmPlaceholder="Confirm new password"
          submitLabel={busy ? "Restoring…" : "Restore account"}
          onSubmit={submit}
        />
      </div>
    </GateCard>
  );
}

// ── absent: link this device to an existing account ────────────────────

type LinkStep = "password" | "wizard";

function LinkFlow({
  onDone,
  onSwitchMode,
}: {
  onDone: () => void;
  onSwitchMode: (mode: AbsentMode) => void;
}) {
  const [step, setStep] = useState<LinkStep>("password");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (step === "password") {
    return (
      <GateCard
        title="Link this device"
        subtitle="Your account lives on another device. Set a password for this device's own key — you'll approve the link from your other device next."
      >
        <ModeTabs tabs={ABSENT_TABS} mode="link" onSelect={onSwitchMode} />
        <PasswordForm
          mode="set"
          busy={busy}
          error={error}
          submitLabel={busy ? "Creating…" : "Create this device's key"}
          onSubmit={(password) => {
            setBusy(true);
            setError(null);
            createIdentity(password)
              // A linked device's own phrase is a secondary backup — the
              // account's recovery lives on the other device — so the
              // shown-once confirm ceremony is skipped here (the flag is
              // UX-only); the phrase stays viewable from the Account view.
              .then(() => confirmMnemonic())
              .then(() => {
                // From here on, auto-bind must NOT found a fresh account for
                // this key — it waits for the other device's AddMemberKey.
                saveLinkPending();
                setStep("wizard");
              })
              .catch((err) => setError(errMessage(err)))
              .finally(() => setBusy(false));
          }}
        />
      </GateCard>
    );
  }

  return (
    <GateCard
      title="Approve from your other device"
      subtitle="On your other device, open Account → Link a device, then type the address under its QR here — or swap the two codes by hand. You can continue and finish the link later."
    >
      <LinkDeviceFlow onDone={onDone} doneLabel="Continue" />
    </GateCard>
  );
}

function AbsentScreen({ onDone }: { onDone: () => void }) {
  const [mode, setMode] = useState<AbsentMode>("create");
  const [touchid, setTouchid] = useState(false);
  useEffect(() => {
    touchidAvailable().then(setTouchid, () => setTouchid(false));
  }, []);
  if (mode === "touchid") return <TouchIdCreateFlow onDone={onDone} onSwitchMode={setMode} />;
  if (mode === "create")
    return <CreateFlow onDone={onDone} onSwitchMode={setMode} touchidAvailable={touchid} />;
  if (mode === "restore") return <RestoreFlow onDone={onDone} onSwitchMode={setMode} />;
  return <LinkFlow onDone={onDone} onSwitchMode={setMode} />;
}

// ── plaintext (legacy) ──────────────────────────────────────────────────

type PlaintextStep = "interstitial" | "password" | "reveal";

function PlaintextScreen({ onDone, onDismiss }: { onDone: () => void; onDismiss: () => void }) {
  const [step, setStep] = useState<PlaintextStep>("interstitial");
  const [password, setPassword] = useState<string | null>(null);
  const [mnemonic, setMnemonic] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (step === "interstitial") {
    return (
      <GateCard
        title="Secure your account"
        subtitle="This account isn't password-protected yet. Set a password so a stolen device can't be used to sign as you. You can do this later from the Account view."
      >
        <button onClick={() => setStep("password")} style={primaryButtonStyle(false)}>
          Set a password
        </button>
        <button onClick={onDismiss} style={linkButtonStyle}>
          Not now
        </button>
      </GateCard>
    );
  }

  if (step === "password") {
    return (
      <GateCard title="Set a password" subtitle="This encrypts your account key at rest on this device.">
        <PasswordForm
          mode="set"
          busy={busy}
          error={error}
          submitLabel={busy ? "Securing…" : "Secure account"}
          onSubmit={(pw) => {
            setBusy(true);
            setError(null);
            encryptLegacy(pw)
              .then(() => {
                setPassword(pw);
                setStep("reveal");
              })
              .catch((err) => setError(errMessage(err)))
              .finally(() => setBusy(false));
          }}
        />
      </GateCard>
    );
  }

  // step === "reveal": the just-set password is still in component state, so
  // viewing the phrase now needs no extra prompt (revealMnemonic still always
  // re-verifies it fresh server-side — the session cache is never consulted).
  return (
    <GateCard
      title="View your recovery phrase"
      subtitle="You can write down your 24-word recovery phrase now, or do this later from the Account view."
    >
      {mnemonic ? (
        <MnemonicGrid mnemonic={mnemonic} onContinue={onDone} continueLabel="Done" />
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <button
            onClick={() => {
              if (!password) return;
              setBusy(true);
              setError(null);
              revealMnemonic(password)
                .then((revealed) => setMnemonic(revealed.mnemonic))
                .catch((err) => setError(errMessage(err)))
                .finally(() => setBusy(false));
            }}
            disabled={busy}
            style={primaryButtonStyle(busy)}
          >
            {busy ? "Loading…" : "View recovery phrase"}
          </button>
          <button onClick={onDone} style={linkButtonStyle}>
            Skip — I'll do this later
          </button>
          {error && <span style={errorTextStyle}>{error}</span>}
        </div>
      )}
    </GateCard>
  );
}

// ── locked ───────────────────────────────────────────────────────────────

function LockedScreen({ onDone, onSkip }: { onDone: () => void; onSkip: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [touchid, setTouchid] = useState(false);
  useEffect(() => {
    touchidAvailable().then(setTouchid, () => setTouchid(false));
  }, []);

  // The Keychain read prompts the OS user-presence sheet — Touch ID when the
  // sensor is usable, the Mac's login password when it isn't (lid closed). A
  // dismissed sheet rejects with "touchid-canceled" (not an error, stay
  // quiet); "touchid-unavailable" means the item itself is gone (never
  // enrolled, disabled, or unreadable), which we translate into the manual
  // paths rather than a raw error.
  const runTouchId = () => {
    setBusy(true);
    setError(null);
    touchidUnlock()
      .then(onDone)
      .catch((err) => {
        const msg = errMessage(err);
        if (msg.includes("touchid-canceled")) return;
        setError(
          msg.includes("touchid-unavailable")
            ? "Touch ID is unavailable — unlock with your password or recovery phrase (Restore) instead."
            : msg,
        );
      })
      .finally(() => setBusy(false));
  };

  return (
    <GateCard
      title="Unlock your account"
      subtitle="Enter your password to unlock your account on this device for this session."
    >
      {touchid && (
        <button disabled={busy} style={primaryButtonStyle(busy)} onClick={runTouchId}>
          {busy ? "Unlocking…" : "Unlock with Touch ID"}
        </button>
      )}
      <PasswordForm
        mode="confirm"
        busy={busy}
        error={error}
        submitLabel={busy ? "Unlocking…" : "Unlock"}
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
      <span
        style={{
          font: `400 10.5px ${font.sans}`,
          color: color.muted2,
          textAlign: "center",
          lineHeight: 1.4,
        }}
      >
        Until you unlock, nodes you start stay unlinked to your account.
      </span>
    </GateCard>
  );
}

// ── create-flow resume (locked/unlocked, mnemonicConfirmed === false) ────

type ResumeStep = "password" | "grid" | "confirm";

function ResumeScreen({ onDone, onSkip }: { onDone: () => void; onSkip: () => void }) {
  const [step, setStep] = useState<ResumeStep>("password");
  const [mnemonic, setMnemonic] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (step === "password") {
    return (
      <GateCard
        title="Confirm your recovery phrase"
        subtitle="You created this account but never confirmed its recovery phrase. Enter your password to view it and finish."
      >
        <PasswordForm
          mode="confirm"
          busy={busy}
          error={error}
          submitLabel={busy ? "Loading…" : "Continue"}
          onSubmit={(password) => {
            setBusy(true);
            setError(null);
            revealMnemonic(password)
              .then((revealed) => {
                setMnemonic(revealed.mnemonic);
                setStep("grid");
              })
              .catch((err) => setError(errMessage(err)))
              .finally(() => setBusy(false));
          }}
        />
        {/* Same escape hatch as LockedScreen: a forgotten password must not
            trap this device behind the gate forever — the unconfirmed flag
            just means the gate re-offers this resume step next launch. */}
        <button onClick={onSkip} style={linkButtonStyle}>
          Skip for now
        </button>
      </GateCard>
    );
  }

  if (step === "grid") {
    return (
      <GateCard
        title="Your recovery phrase"
        subtitle="Write these 24 words down in order and keep them somewhere safe."
      >
        <MnemonicGrid mnemonic={mnemonic} onContinue={() => setStep("confirm")} continueLabel="Continue" />
      </GateCard>
    );
  }

  return (
    <GateCard
      title="Confirm your recovery phrase"
      subtitle="Enter the requested words to prove you saved them."
    >
      <ConfirmWords
        mnemonic={mnemonic}
        busy={busy}
        error={error}
        onConfirmed={() => {
          setBusy(true);
          setError(null);
          confirmMnemonic()
            .then(onDone)
            .catch((err) => setError(errMessage(err)))
            .finally(() => setBusy(false));
        }}
      />
    </GateCard>
  );
}

// ── The gate itself ──────────────────────────────────────────────────────

export function IdentityGate({ children }: { children: ReactNode }) {
  const desktop = hasNativeShell();
  const [report, setReport] = useState<IdentityStateReport | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);
  const [dismissedPlaintext, setDismissedPlaintext] = useState(false);
  const [skippedUnlock, setSkippedUnlock] = useState(false);
  const [skippedResume, setSkippedResume] = useState(false);

  const refresh = useCallback(() => {
    if (!desktop) return Promise.resolve();
    return identityState().then(
      (r) => {
        setBootError(null);
        setReport(r);
      },
      (err) => setBootError(errMessage(err)),
    );
  }, [desktop]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  if (!desktop) return <>{children}</>;

  if (bootError) {
    return (
      <OnboardingChrome step={null}>
        <GateCard title="Couldn't read your account key" subtitle={bootError}>
          <button onClick={() => void refresh()} style={primaryButtonStyle(false)}>
            Retry
          </button>
        </GateCard>
      </OnboardingChrome>
    );
  }

  if (!report) return null; // resolving account state — nothing to show yet

  if (report.state === "absent") {
    // A true first run: the step rail numbers this Account stage 1 of 3.
    return (
      <OnboardingChrome step={1}>
        <AbsentScreen onDone={refresh} />
      </OnboardingChrome>
    );
  }

  if (report.state === "plaintext") {
    if (dismissedPlaintext) return <>{children}</>;
    return (
      <OnboardingChrome step={null}>
        <PlaintextScreen onDone={refresh} onDismiss={() => setDismissedPlaintext(true)} />
      </OnboardingChrome>
    );
  }

  // locked or unlocked from here — an interrupted create resumes first,
  // regardless of which of those two the session cache landed on.
  if (!report.mnemonicConfirmed) {
    if (skippedResume) return <>{children}</>;
    return (
      <OnboardingChrome step={null}>
        <ResumeScreen onDone={refresh} onSkip={() => setSkippedResume(true)} />
      </OnboardingChrome>
    );
  }

  if (report.state === "locked") {
    if (skippedUnlock) return <>{children}</>;
    return (
      <OnboardingChrome step={null}>
        <LockedScreen onDone={refresh} onSkip={() => setSkippedUnlock(true)} />
      </OnboardingChrome>
    );
  }

  return <>{children}</>; // unlocked, confirmed
}
