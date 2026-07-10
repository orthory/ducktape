// Shared UI toolkit for the identity gate (IdentityGate.tsx): the card chrome,
// mode tabs, password form, mnemonic grid, and confirm-3-words step. Split out
// of IdentityGate.tsx once that file passed ~400 lines (task-5-brief's split
// threshold) — these are also the pieces Task 6's Settings view reuses
// (PasswordForm, MnemonicGrid) for its lock/unlock/reveal/set-password rows.
// Styling is copied verbatim from OnboardingGate's tokens/patterns so the two
// gates read as one visual family.

import { useMemo, useState } from "react";
import type { ReactNode } from "react";

import { color, font, radius, shadow } from "../../theme/tokens";

export const errMessage = (err: unknown): string =>
  err instanceof Error ? err.message : String(err);

// ── Shared styling ───────────────────────────────────────────────────────

const cardStyle: React.CSSProperties = {
  width: 440,
  maxWidth: "100%",
  background: color.sidebar,
  border: `1px solid ${color.border}`,
  borderRadius: radius.lg,
  boxShadow: shadow.pop,
  padding: 24,
  display: "flex",
  flexDirection: "column",
  gap: 16,
};

const titleStyle: React.CSSProperties = { font: `600 16px ${font.sans}`, color: color.ink };
const subtitleStyle: React.CSSProperties = {
  font: `500 12px ${font.sans}`,
  color: color.muted,
  lineHeight: 1.5,
};

export const inputStyle: React.CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "9px 11px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.sunken,
  font: `500 12.5px ${font.sans}`,
  color: color.ink,
};

export const errorTextStyle: React.CSSProperties = {
  font: `500 11.5px ${font.mono}`,
  color: color.red,
};

export const primaryButtonStyle = (disabled: boolean): React.CSSProperties => ({
  all: "unset",
  textAlign: "center",
  cursor: disabled ? "default" : "pointer",
  padding: "10px 0",
  borderRadius: radius.md,
  background: disabled ? color.chip : color.dark,
  color: disabled ? color.muted3 : color.onDark,
  font: `600 12.5px ${font.sans}`,
});

export const secondaryButtonStyle: React.CSSProperties = {
  all: "unset",
  textAlign: "center",
  cursor: "pointer",
  padding: "9px 0",
  borderRadius: radius.md,
  border: `1px solid ${color.border}`,
  background: color.paper,
  color: color.ink,
  font: `600 12px ${font.sans}`,
};

export const linkButtonStyle: React.CSSProperties = {
  all: "unset",
  cursor: "pointer",
  textAlign: "center",
  font: `600 11px ${font.sans}`,
  color: color.muted,
};

const tabRowStyle: React.CSSProperties = {
  display: "flex",
  gap: 4,
  padding: 4,
  borderRadius: radius.md,
  background: color.panel,
};

function Tab({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        all: "unset",
        cursor: "pointer",
        flex: 1,
        textAlign: "center",
        padding: "8px 0",
        borderRadius: radius.sm,
        background: active ? color.paper : "transparent",
        boxShadow: active ? shadow.card : "none",
        font: `600 12px ${font.sans}`,
        color: active ? color.ink : color.muted,
      }}
    >
      {label}
    </button>
  );
}

/** The gate's mode switcher, generalized over the caller's tab set (the
 *  absent screen runs create/restore/link; other callers may run fewer). */
export function ModeTabs<M extends string>({
  tabs,
  mode,
  onSelect,
}: {
  tabs: ReadonlyArray<{ readonly id: M; readonly label: string }>;
  mode: M;
  onSelect: (mode: M) => void;
}) {
  return (
    <div style={tabRowStyle}>
      {tabs.map((tab) => (
        <Tab
          key={tab.id}
          label={tab.label}
          active={mode === tab.id}
          onClick={() => onSelect(tab.id)}
        />
      ))}
    </div>
  );
}

/** The gate's card chrome — title/subtitle + content. The centered outer
 *  wrapper lives in OnboardingChrome now (it also carries the first-run step
 *  rail); every GateCard renders inside one. */
export function GateCard({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
}) {
  return (
    <div style={cardStyle}>
      <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
        <span style={titleStyle}>{title}</span>
        {subtitle && <span style={subtitleStyle}>{subtitle}</span>}
      </div>
      {children}
    </div>
  );
}

// ── Exported building blocks (Task 6's Settings reuses these) ──────────────

export interface PasswordFormProps {
  /** "set" = password + confirm, inline min-length/mismatch validation — used
   *  by create/restore/secure-legacy. "confirm" = a single password field —
   *  unlock, and the resume flow's re-prompt before `revealMnemonic`. */
  mode: "set" | "confirm";
  submitLabel: string;
  onSubmit: (password: string) => void;
  busy?: boolean;
  /** Server-side error to show inline (wrong password, etc). */
  error?: string | null;
  minLength?: number;
  /** Override the password field's placeholder (e.g. restore's "New password"). */
  placeholder?: string;
  /** Override the confirm field's placeholder ("set" mode only). */
  confirmPlaceholder?: string;
}

/** A password entry form — single field ("confirm") or double with inline
 *  min-length/mismatch validation ("set"). Exported: Settings (Task 6) reuses
 *  this for "Set password" / unlock / reveal re-prompts. */
export function PasswordForm({
  mode,
  submitLabel,
  onSubmit,
  busy = false,
  error = null,
  minLength = 8,
  placeholder,
  confirmPlaceholder = "Confirm password",
}: PasswordFormProps) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  const submit = () => {
    if (mode === "set") {
      if (password.length < minLength) {
        setLocalError(`password must be at least ${minLength} characters`);
        return;
      }
      if (password !== confirm) {
        setLocalError("passwords do not match");
        return;
      }
    }
    setLocalError(null);
    onSubmit(password);
  };

  const shown = localError ?? error;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      <input
        type="password"
        value={password}
        placeholder={placeholder ?? (mode === "set" ? "Password (min 8 characters)" : "Password")}
        onChange={(event) => {
          setPassword(event.target.value);
          setLocalError(null);
        }}
        onKeyDown={(event) => event.key === "Enter" && mode === "confirm" && submit()}
        autoComplete={mode === "set" ? "new-password" : "current-password"}
        style={inputStyle}
      />
      {mode === "set" && (
        <input
          type="password"
          value={confirm}
          placeholder={confirmPlaceholder}
          onChange={(event) => {
            setConfirm(event.target.value);
            setLocalError(null);
          }}
          autoComplete="new-password"
          style={inputStyle}
        />
      )}
      {shown && <span style={errorTextStyle}>{shown}</span>}
      <button onClick={submit} disabled={busy} style={primaryButtonStyle(busy)}>
        {submitLabel}
      </button>
    </div>
  );
}

const gridStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(3, 1fr)",
  gap: 6,
  maxHeight: 280,
  overflowY: "auto",
  padding: 2,
};

const wordCellStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: "6px 8px",
  borderRadius: radius.sm,
  border: `1px solid ${color.border}`,
  background: color.sunken,
};

const wordIndexStyle: React.CSSProperties = {
  font: `600 10px ${font.mono}`,
  color: color.muted2,
  width: 16,
  textAlign: "right",
};

const wordTextStyle: React.CSSProperties = { font: `600 12px ${font.mono}`, color: color.ink };

export interface MnemonicGridProps {
  mnemonic: string;
  /** Present unless this is a read-only reveal with no next step. */
  onContinue?: () => void;
  continueLabel?: string;
}

/** The 24-word recovery-phrase grid, numbered, with a copy button. Exported:
 *  Settings' "Reveal recovery phrase" (Task 6) reuses this verbatim. */
export function MnemonicGrid({ mnemonic, onContinue, continueLabel = "Continue" }: MnemonicGridProps) {
  const [copied, setCopied] = useState(false);
  const words = useMemo(() => mnemonic.trim().split(/\s+/), [mnemonic]);

  const copy = () => {
    const clipboard = typeof navigator !== "undefined" ? navigator.clipboard : undefined;
    if (!clipboard?.writeText) return;
    clipboard.writeText(mnemonic).then(
      () => setCopied(true),
      () => {},
    );
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={gridStyle}>
        {words.map((word, i) => (
          <div key={i} style={wordCellStyle}>
            <span style={wordIndexStyle}>{i + 1}</span>
            <span style={wordTextStyle}>{word}</span>
          </div>
        ))}
      </div>
      <button onClick={copy} style={secondaryButtonStyle}>
        {copied ? "Copied" : "Copy to clipboard"}
      </button>
      {onContinue && (
        <button onClick={onContinue} style={primaryButtonStyle(false)}>
          {continueLabel}
        </button>
      )}
    </div>
  );
}

/** Picks `count` distinct indices out of `[0, total)`, ascending — the
 *  confirm-3-words step's random sample. */
function pickConfirmIndices(total: number, count: number): number[] {
  const pool = Array.from({ length: total }, (_, i) => i);
  for (let i = pool.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [pool[i], pool[j]] = [pool[j], pool[i]];
  }
  return pool.slice(0, count).sort((a, b) => a - b);
}

/** The confirm-3-words step: rejects a wrong word inline and allows retry —
 *  the indices themselves stay fixed across retries (memoized on the
 *  mnemonic), only the offending word must be corrected. `onConfirmed` fires
 *  only once the words match; the caller owns the async terminal action
 *  (`confirmMnemonic`) and feeds its in-flight/failure state back via
 *  `busy`/`error`, the same contract as PasswordForm. */
export function ConfirmWords({
  mnemonic,
  onConfirmed,
  busy = false,
  error = null,
}: {
  mnemonic: string;
  onConfirmed: () => void;
  busy?: boolean;
  /** Server-side error to show inline (a failed confirmMnemonic, etc). */
  error?: string | null;
}) {
  const words = useMemo(() => mnemonic.trim().split(/\s+/), [mnemonic]);
  const indices = useMemo(() => pickConfirmIndices(words.length, 3), [words]);
  const [answers, setAnswers] = useState<Record<number, string>>({});
  const [localError, setLocalError] = useState<string | null>(null);

  const submit = () => {
    if (busy) return;
    const wrong = indices.find((i) => (answers[i] ?? "").trim().toLowerCase() !== words[i]);
    if (wrong !== undefined) {
      setLocalError(`word #${wrong + 1} doesn't match — try again`);
      return;
    }
    setLocalError(null);
    onConfirmed();
  };

  const shown = localError ?? error;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      {indices.map((i) => (
        <input
          key={i}
          aria-label={`Word #${i + 1}`}
          placeholder={`Word #${i + 1}`}
          value={answers[i] ?? ""}
          onChange={(event) => {
            const value = event.target.value;
            setAnswers((prev) => ({ ...prev, [i]: value }));
          }}
          autoCapitalize="off"
          spellCheck={false}
          style={inputStyle}
        />
      ))}
      {shown && <span style={errorTextStyle}>{shown}</span>}
      <button onClick={submit} disabled={busy} style={primaryButtonStyle(busy)}>
        {busy ? "Confirming…" : "Confirm"}
      </button>
    </div>
  );
}
