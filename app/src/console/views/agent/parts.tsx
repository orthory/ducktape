// Shared atoms and wire→display helpers for the Agents surface. Pure
// presentation and formatting — no store access; the section files compose
// these and own their own data flow.

import type { CSSProperties, ReactNode } from "react";

import type { AgentRecord, SagaOrigin } from "../../../domain/agent-client";
import type { Channel } from "../../../domain/chat-client";
import { sameKey } from "../../../domain/names";
import type { PendingRun, TurnPolicy } from "../../../domain/runs-client";
import { Icon } from "../../components/Icon";
import { accentVar, color, font, radius, shadow, tint } from "../../theme/tokens";

// ── Static labels ───────────────────────────────────────

export const ACTION_LABEL: Record<string, string> = {
  "chat.post": "Reply in chat",
  "chat.post_message": "Post to any channel",
  "tasks.create": "Create tasks",
  "tasks.update_status": "Update task status",
  "pages.comment": "Comment on pages",
  "pages.set_checked": "Check off page todos",
};

// Permission checkboxes read as plain abilities ("what this agent can do"),
// not as the wire action ids they map to.
//
// chat.post and chat.post_message are deliberately worded as the different
// powers they are: the first only lets an agent answer where it was spoken to,
// the second lets it speak, unprompted, anywhere. Granting the reply must never
// look like it grants the broadcast.
export const ACTION_HINT: Record<string, string> = {
  "chat.post": "Reply in the thread it was mentioned in",
  "chat.post_message": "Start messages in any channel, on its own initiative",
  "tasks.create": "Create tasks",
  "tasks.update_status": "Update task status",
  "pages.comment": "Comment on pages",
  "pages.set_checked": "Check off page todos",
};

/** Parse the pages_write caps field: whitespace/comma-separated page ids,
 *  the literal "*" allowed. The node canonicalizes (sort + dedup). */
export const parsePagesWrite = (text: string): string[] =>
  text.split(/[\s,]+/).filter(Boolean);

export type Tone = { text: string; bg: string; border: string };

export const statusTone = {
  success: tint(color.green),
  warning: tint(color.amber),
  danger: tint(color.red),
  neutral: tint(color.purple),
  blue: tint(color.blue),
  agent: tint(accentVar),
} satisfies Record<string, Tone>;

export const inputStyle: CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "9px 11px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12.5px ${font.sans}`,
  color: color.ink,
};

export const monoInputStyle: CSSProperties = {
  ...inputStyle,
  font: `400 12px ${font.mono}`,
};

export const secondaryButton: CSSProperties = {
  appearance: "none",
  border: `1px solid ${color.borderStrong}`,
  borderRadius: radius.sm,
  background: color.paper,
  color: color.inkSoft,
  cursor: "pointer",
  minHeight: 32,
  padding: "0 12px",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: 7,
  font: `600 12px ${font.sans}`,
  whiteSpace: "nowrap",
  touchAction: "manipulation",
};

// The module's decisive accent moment: primary actions carry the terracotta,
// not the flat dark, so "Add agent" / "Register" read as THE thing to do.
export const primaryButton = (enabled: boolean): CSSProperties => ({
  ...secondaryButton,
  borderColor: enabled ? accentVar : color.chip,
  background: enabled ? accentVar : color.chip,
  color: enabled ? "#fff" : color.muted2,
  cursor: enabled ? "pointer" : "default",
  boxShadow: enabled ? "0 1px 2px rgba(160,90,60,.30)" : undefined,
});

/** Mix the filled control's foreground and background so overlays follow the
 * filled surface when the theme flips its polarity. */
export const filledMix = (onFilledPercent: number): string =>
  `color-mix(in srgb, ${color.onDark} ${onFilledPercent}%, ${color.dark})`;

export const FILLED_IDENTITY_TEXT_PERCENT = 70;
export const FILLED_SEMANTIC_TEXT_PERCENT = 35;

/** Keep semantic foregrounds readable on the filled identity band. The
 * status hue stays visible, but the on-filled token supplies the contrast when
 * dark mode turns that band light. */
export const filledForeground = (base: string): string =>
  `color-mix(in srgb, ${base} ${FILLED_SEMANTIC_TEXT_PERCENT}%, ${color.onDark})`;

// A control styled to sit on the agent card's dark identity band.
export const onDarkButton: CSSProperties = {
  ...secondaryButton,
  minHeight: 30,
  border: `1px solid ${filledMix(22)}`,
  background: filledMix(7),
  color: color.onDark,
};

// ── Wire → display helpers ──────────────────────────────

/** Consensus admits only DNS-label agent ids (`validate_agent_id`,
 *  crates/apps/agent/src/lib.rs) — the id IS the local part of
 *  `<agent_id>@agents.duck`. The Rust const is the source of truth; the drift
 *  test in parts.test.ts reads it. */
export const MAX_AGENT_ID_LEN = 63;

/** Derive an agent id from free text. TOTAL: the result is always a legal
 *  label or the empty string (the caller's only failure case) — truncation to
 *  the length cap happens BEFORE the hyphen trim, so a cut mid-word can't
 *  leave a trailing hyphen. */
export const slug = (raw: string): string =>
  raw
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .slice(0, MAX_AGENT_ID_LEN)
    .replace(/(^-|-$)/g, "");

/** Present a lowercase executor tag ("codex") as a friendly label ("Codex").
 *  The raw tag stays the stored value — this is display only. */
export const titleCase = (tag: string): string =>
  tag ? tag.charAt(0).toUpperCase() + tag.slice(1) : tag;

export const initialsOf = (name: string): string => {
  const parts = name
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  if (parts.length === 0) return "AI";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return `${parts[0][0]}${parts[parts.length - 1][0]}`.toUpperCase();
};

const hexOf = (bytes: number[]): string =>
  bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("");

/** Whether a run was requested by the local user. On a networked node the
 *  requester's `external` bytes ARE the submitter's pubkey (== workspace
 *  pubkey), so this is a hex-key equality. Module/system requesters (chat,
 *  jobs) never match, and no local pubkey means "not mine". */
export const runIsMine = (
  run: PendingRun,
  workspacePubkey: string | null,
): boolean =>
  typeof run.requester === "object" &&
  "external" in run.requester &&
  sameKey(hexOf(run.requester.external), workspacePubkey);

export const shortHex = (bytes: number[]): string => {
  const hex = hexOf(bytes);
  return hex.length > 18 ? `${hex.slice(0, 10)}…${hex.slice(-6)}` : hex || "—";
};

export const ownerText = (origin: SagaOrigin): string => {
  if (origin === "system") return "system";
  if ("module" in origin) return `module:${origin.module}`;
  return `external:${shortHex(origin.external)}`;
};

export const channelLabel = (channels: Channel[], channelId: string): string =>
  channels.find((channel) => channel.id === channelId)?.name ?? channelId;

export const agentLabel = (agents: AgentRecord[], agentId: string): string =>
  agents.find((agent) => agent.agent_id === agentId)?.display_name ?? agentId;

export const policyText = (policy: TurnPolicy, agents: AgentRecord[]): string => {
  if (policy === "mention") return "When mentioned";
  if (policy === "all") return "Every message";
  if (policy === "round_robin") return "Take turns";
  return `Only ${agentLabel(agents, policy.assigned)}`;
};

/** Every listed entry is by definition awaiting its dispatch delivery — the
 *  node prunes entries the moment a result lands. */
export const runDetail = (run: PendingRun): string =>
  `dispatch ${run.dispatch_id.slice(0, 12)}…`;

// ── Shared UI atoms ─────────────────────────────────────

export function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        font: `600 9px ${font.mono}`,
        letterSpacing: ".11em",
        color: color.muted2,
      }}
    >
      {children}
    </div>
  );
}

export function GroupCard({
  children,
  style,
}: {
  children: ReactNode;
  style?: CSSProperties;
}) {
  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        boxShadow: shadow.card,
        overflow: "hidden",
        ...style,
      }}
    >
      {children}
    </div>
  );
}

export function StatusPill({ label, tone }: { label: string; tone: Tone }) {
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        borderRadius: 5,
        border: `1px solid ${tone.border}`,
        background: tone.bg,
        color: tone.text,
        padding: "3px 7px",
        font: `600 9px ${font.mono}`,
        letterSpacing: ".06em",
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </span>
  );
}

export function AgentAvatar({
  name,
  size = 34,
  tone = "dark",
}: {
  name: string;
  size?: number;
  /** "accent" pops the avatar on the card's dark identity band. */
  tone?: "dark" | "accent";
}) {
  const accent = tone === "accent";
  return (
    <span
      aria-hidden="true"
      style={{
        width: size,
        height: size,
        flexShrink: 0,
        borderRadius: Math.max(7, Math.round(size * 0.24)),
        background: accent ? accentVar : color.dark,
        color: accent ? "#fff" : color.onDark,
        boxShadow: accent ? "inset 0 0 0 1px rgba(255,255,255,.16)" : undefined,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 ${Math.max(10, Math.round(size * 0.31))}px ${font.mono}`,
        letterSpacing: 0,
      }}
    >
      {initialsOf(name)}
    </span>
  );
}

/** Split a stored capability tag into provider / model / effort (the picker's
 *  3-part `provider_model_effort` grammar; anything else is shown whole as the
 *  provider). */
const parseCap = (
  tag: string,
): { provider: string; model: string | null; effort: string | null } => {
  const parts = tag.split("_");
  if (parts.length === 3 && parts.every(Boolean)) {
    return { provider: parts[0], model: parts[1], effort: parts[2] };
  }
  return { provider: tag, model: null, effort: null };
};

/** Compact one-line executor label for dense rows: "Codex · gpt-5.5", or just
 *  "Codex" for a bare/opaque tag. */
export const capabilityShort = (tag: string): string => {
  const { provider, model } = parseCap(tag);
  return model ? `${titleCase(provider)} · ${model}` : titleCase(provider);
};

/** The agent's executor shown as a spec — PROVIDER › model · EFFORT — instead
 *  of a raw, truncation-prone tag like `Codex_gpt-5.5_h…`. */
export function CapabilityStrip({ capability }: { capability: string }) {
  const { provider, model, effort } = parseCap(capability);
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 8,
        minWidth: 0,
        flexWrap: "wrap",
      }}
    >
      <span style={{ font: `700 12.5px ${font.sans}`, color: accentVar, letterSpacing: ".01em" }}>
        {titleCase(provider)}
      </span>
      {model && (
        <>
          <span style={{ color: color.iconIdle, font: `400 12px ${font.mono}` }}>›</span>
          <span translate="no" style={{ font: `600 11.5px ${font.mono}`, color: color.ink }}>
            {model}
          </span>
        </>
      )}
      {effort && (
        <span
          translate="no"
          style={{
            padding: "1px 7px",
            borderRadius: 999,
            background: statusTone.agent.bg,
            border: `1px solid ${statusTone.agent.border}`,
            font: `700 9px ${font.mono}`,
            letterSpacing: ".06em",
            color: accentVar,
            textTransform: "uppercase",
          }}
        >
          {effort}
        </span>
      )}
    </span>
  );
}

export function EmptyState({
  icon,
  title,
  body,
}: {
  icon: "agent" | "hash";
  title: string;
  body: string;
}) {
  return (
    <div
      style={{
        minHeight: 170,
        padding: "30px 18px",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        textAlign: "center",
        gap: 8,
        color: color.muted2,
      }}
    >
      <span
        style={{
          width: 36,
          height: 36,
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: color.sunken,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: color.muted,
        }}
      >
        <Icon name={icon} size={17} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>{title}</div>
      <div style={{ maxWidth: 300, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
        {body}
      </div>
    </div>
  );
}

export function FieldLabel({ htmlFor, children }: { htmlFor: string; children: ReactNode }) {
  return (
    <label
      htmlFor={htmlFor}
      style={{
        display: "block",
        marginBottom: 5,
        font: `600 10px ${font.mono}`,
        letterSpacing: ".05em",
        color: color.muted2,
      }}
    >
      {children}
    </label>
  );
}

export function InfoRow({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        gap: 14,
        border: `1px solid ${color.border}`,
        borderRadius: radius.sm,
        padding: "9px 11px",
        font: `400 11px ${font.mono}`,
        color: color.muted3,
        minWidth: 0,
      }}
    >
      <span style={{ color: color.muted2, whiteSpace: "nowrap" }}>{label}</span>
      <span
        title={typeof value === "string" ? value : undefined}
        translate="no"
        style={{
          minWidth: 0,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          textAlign: "right",
        }}
      >
        {value}
      </span>
    </div>
  );
}

export function Chip({ text, tone = statusTone.neutral }: { text: string; tone?: Tone }) {
  return (
    <span
      title={text}
      translate="no"
      style={{
        minWidth: 0,
        maxWidth: 230,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
        padding: "3px 7px",
        borderRadius: 5,
        background: tone.bg,
        border: `1px solid ${tone.border}`,
        font: `500 10px ${font.mono}`,
        color: tone.text,
      }}
    >
      {text}
    </span>
  );
}
