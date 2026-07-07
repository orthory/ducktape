// The identity gate (desktop only): renders BEFORE the workspace onboarding
// gate, driven by `user_identity_state()`. Identity is machine-scoped and
// orthogonal to any workspace/node — so unlike OnboardingGate (which reads
// `state.needsOnboarding` off the console store, gated on node/workspace
// resolution), this gate fetches its own boot state directly: a local
// `useEffect` + `useState`, self-contained, no store wiring. See
// IdentityGate.test.tsx / the task report for the full wiring rationale.
//
// State machine (spec: docs/superpowers/specs/2026-07-07-identity-onboarding-design.md):
//   absent    → Create | Restore chooser
//   plaintext → dismissable "Secure your identity" interstitial (dismiss = this
//               launch only, plain component state, never persisted)
//   locked    → unlock form, "skip for now" proceeds to the console
//   unlocked  → no gate
// A create that was interrupted before the mnemonic was confirmed resumes at
// the mnemonic/confirm step on relaunch (locked or unlocked, mnemonicConfirmed
// false) — password re-entry then `revealMnemonic`, since a fresh mount never
// still holds the mnemonic in component state.
//
// The card chrome, password form, mnemonic grid, and confirm-words step live
// in the sibling IdentityGateForms.tsx (this file passed the ~400-line split
// threshold); that file is also what Task 6's Settings view reuses.

import { useCallback, useEffect, useState } from "react";
import type { ReactNode } from "react";

import { isTauri } from "../../../domain/node-bootstrap";
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
import { font } from "../../theme/tokens";
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

// ── absent: create ──────────────────────────────────────────────────────

type CreateStep = "password" | "grid" | "confirm";

function CreateFlow({
  onDone,
  onSwitchToRestore,
}: {
  onDone: () => void;
  onSwitchToRestore: () => void;
}) {
  const [step, setStep] = useState<CreateStep>("password");
  const [mnemonic, setMnemonic] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (step === "password") {
    return (
      <GateCard
        title="Create your identity"
        subtitle="Set a password to encrypt this machine's identity key at rest. You'll get a 24-word recovery phrase next — write it down; it's the only backup."
      >
        <ModeTabs mode="create" onSelect={(m) => m === "restore" && onSwitchToRestore()} />
        <PasswordForm
          mode="set"
          busy={busy}
          error={error}
          submitLabel={busy ? "Creating…" : "Create identity"}
          onSubmit={(password) => {
            setBusy(true);
            setError(null);
            createIdentity(password)
              .then((created) => {
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
        subtitle="Write these 24 words down in order and keep them somewhere safe. Anyone with these words can restore your identity — this is shown only once."
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

// ── absent: restore ─────────────────────────────────────────────────────

function RestoreFlow({
  onDone,
  onSwitchToCreate,
}: {
  onDone: () => void;
  onSwitchToCreate: () => void;
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
      title="Restore your identity"
      subtitle="Enter your 24-word recovery phrase and set a new password for this device."
    >
      <ModeTabs mode="restore" onSelect={(m) => m === "create" && onSwitchToCreate()} />
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
          submitLabel={busy ? "Restoring…" : "Restore identity"}
          onSubmit={submit}
        />
      </div>
    </GateCard>
  );
}

function AbsentScreen({ onDone }: { onDone: () => void }) {
  const [mode, setMode] = useState<"create" | "restore">("create");
  return mode === "create" ? (
    <CreateFlow onDone={onDone} onSwitchToRestore={() => setMode("restore")} />
  ) : (
    <RestoreFlow onDone={onDone} onSwitchToCreate={() => setMode("create")} />
  );
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
        title="Secure your identity"
        subtitle="This identity isn't password-protected yet. Set a password so a stolen device can't be used to sign as you. You can do this later from Settings."
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
      <GateCard title="Set a password" subtitle="This encrypts your identity key at rest on this device.">
        <PasswordForm
          mode="set"
          busy={busy}
          error={error}
          submitLabel={busy ? "Securing…" : "Secure identity"}
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
      subtitle="You can write down your 24-word recovery phrase now, or do this later from Settings."
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

  return (
    <GateCard
      title="Unlock your identity"
      subtitle="Enter your password to unlock this device's identity for this session."
    >
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
    </GateCard>
  );
}

// ── create-flow resume (locked/unlocked, mnemonicConfirmed === false) ────

type ResumeStep = "password" | "grid" | "confirm";

function ResumeScreen({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState<ResumeStep>("password");
  const [mnemonic, setMnemonic] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (step === "password") {
    return (
      <GateCard
        title="Confirm your recovery phrase"
        subtitle="You created this identity but never confirmed its recovery phrase. Enter your password to view it and finish."
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
  const desktop = isTauri();
  const [report, setReport] = useState<IdentityStateReport | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);
  const [dismissedPlaintext, setDismissedPlaintext] = useState(false);
  const [skippedUnlock, setSkippedUnlock] = useState(false);

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
      <GateCard title="Couldn't read your identity" subtitle={bootError}>
        <button onClick={() => void refresh()} style={primaryButtonStyle(false)}>
          Retry
        </button>
      </GateCard>
    );
  }

  if (!report) return null; // resolving identity state — nothing to show yet

  if (report.state === "absent") {
    return <AbsentScreen onDone={refresh} />;
  }

  if (report.state === "plaintext") {
    if (dismissedPlaintext) return <>{children}</>;
    return <PlaintextScreen onDone={refresh} onDismiss={() => setDismissedPlaintext(true)} />;
  }

  // locked or unlocked from here — an interrupted create resumes first,
  // regardless of which of those two the session cache landed on.
  if (!report.mnemonicConfirmed) {
    return <ResumeScreen onDone={refresh} />;
  }

  if (report.state === "locked") {
    if (skippedUnlock) return <>{children}</>;
    return <LockedScreen onDone={refresh} onSkip={() => setSkippedUnlock(true)} />;
  }

  return <>{children}</>; // unlocked, confirmed
}
