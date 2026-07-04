// The automations surface over the node's `automations` module: a rule builder
// that pairs a Trigger (a chat post or a memory publish) with an Action (post a
// message, create a task, or deliver an inbox note), plus the committed rule
// list with per-rule enable + delete. Wired only through useDucktape().

import { useState } from "react";
import type { CSSProperties } from "react";

import type {
  Action,
  ActionKind,
  Rule,
  Trigger,
  TriggerKind,
} from "../../../domain/automations-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";
import { Toggle } from "../settings/Toggle";

const inputBase: CSSProperties = {
  width: "100%",
  minWidth: 0,
  height: 34,
  padding: "0 11px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12.5px ${font.sans}`,
  color: color.ink,
  outline: "none",
  boxSizing: "border-box",
};

const TRIGGER_KINDS: { value: TriggerKind; label: string }[] = [
  { value: "MessagePosted", label: "Message posted" },
  { value: "MemoryPublished", label: "Memory published" },
];

const ACTION_KINDS: { value: ActionKind; label: string }[] = [
  { value: "PostMessage", label: "Post message" },
  { value: "CreateTask", label: "Create task" },
  { value: "DeliverInbox", label: "Deliver inbox" },
];

// ── Pure helpers ────────────────────────────────────────

const shortId = (id: string): string =>
  id.length > 14 ? `${id.slice(0, 8)}…${id.slice(-4)}` : id || "—";

const chan = (id: string): string => (id.startsWith("#") ? id : `#${id}`);

function summarizeTrigger(trigger: Trigger): string {
  if ("MessagePosted" in trigger) {
    const t = trigger.MessagePosted;
    const parts: string[] = [];
    if (t.channel_id) parts.push(`in ${chan(t.channel_id)}`);
    if (t.mention) parts.push(`mentioning ${t.mention}`);
    if (t.text_contains) parts.push(`contains "${t.text_contains}"`);
    return parts.length
      ? `When a message ${parts.join(" ")}`
      : "When any message is posted";
  }
  const m = trigger.MemoryPublished;
  const parts: string[] = [];
  if (m.prefix) parts.push(`under ${m.prefix}`);
  if (m.meta_kind) parts.push(`of kind ${m.meta_kind}`);
  if (m.author_contains) parts.push(`by ${m.author_contains}`);
  return parts.length
    ? `When a memory ${parts.join(" ")} is published`
    : "When any memory is published";
}

function summarizeAction(action: Action): string {
  if ("PostMessage" in action) return `post to ${chan(action.PostMessage.channel_id)}`;
  if ("CreateTask" in action) return "create task";
  return `deliver inbox note (${action.DeliverInbox.kind})`;
}

/** A human, field-aware one-liner for a committed rule. */
export const summarize = (rule: Rule): string =>
  `${summarizeTrigger(rule.trigger)} → ${summarizeAction(rule.action)}`;

// ── Small building blocks ───────────────────────────────

function Segmented<T extends string>({
  options,
  value,
  disabled,
  onChange,
}: {
  options: { value: T; label: string }[];
  value: T;
  disabled: boolean;
  onChange: (value: T) => void;
}) {
  return (
    <div
      role="tablist"
      style={{
        display: "inline-flex",
        border: `1px solid ${color.borderStrong}`,
        borderRadius: radius.sm,
        overflow: "hidden",
        opacity: disabled ? 0.55 : 1,
      }}
    >
      {options.map((opt, i) => {
        const active = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            role="tab"
            aria-selected={active}
            disabled={disabled}
            onClick={() => onChange(opt.value)}
            style={{
              all: "unset",
              cursor: disabled ? "default" : "pointer",
              padding: "6px 13px",
              font: `600 11px ${font.sans}`,
              background: active ? accentVar : color.paper,
              color: active ? color.paper : color.muted3,
              borderLeft: i === 0 ? "none" : `1px solid ${color.borderStrong}`,
            }}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
  disabled,
  optional,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled: boolean;
  optional?: boolean;
}) {
  return (
    <label
      style={{
        display: "grid",
        gap: 5,
        minWidth: 0,
        font: `700 9px ${font.mono}`,
        letterSpacing: ".08em",
        color: disabled ? color.muted : color.muted2,
      }}
    >
      <span>
        {label}
        {optional ? (
          <span style={{ color: color.muted, letterSpacing: 0 }}> · blank = any</span>
        ) : null}
      </span>
      <input
        aria-label={label}
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        style={{
          ...inputBase,
          background: disabled ? color.sunken : color.paper,
          color: disabled ? color.muted2 : color.ink,
        }}
      />
    </label>
  );
}

function BuilderLabel({ children }: { children: string }) {
  return (
    <div
      style={{
        font: `700 9px ${font.mono}`,
        letterSpacing: ".11em",
        color: color.muted2,
      }}
    >
      {children}
    </div>
  );
}

// ── Rule builder card ───────────────────────────────────

function RuleBuilder({ backed }: { backed: boolean }) {
  const { actions } = useDucktape();

  const [triggerKind, setTriggerKind] = useState<TriggerKind>("MessagePosted");
  // MessagePosted fields
  const [mpChannel, setMpChannel] = useState("");
  const [mpMention, setMpMention] = useState("");
  const [mpText, setMpText] = useState("");
  // MemoryPublished fields
  const [mePrefix, setMePrefix] = useState("");
  const [meKind, setMeKind] = useState("");
  const [meAuthor, setMeAuthor] = useState("");

  const [actionKind, setActionKind] = useState<ActionKind>("PostMessage");
  // PostMessage fields
  const [pmChannel, setPmChannel] = useState("");
  const [pmTemplate, setPmTemplate] = useState("");
  // CreateTask fields
  const [ctPrefix, setCtPrefix] = useState("");
  const [ctTitle, setCtTitle] = useState("");
  // DeliverInbox fields
  const [diMember, setDiMember] = useState("");
  const [diKind, setDiKind] = useState("");
  const [diBody, setDiBody] = useState("");

  const [ruleId, setRuleId] = useState("");
  const [submitHover, setSubmitHover] = useState(false);

  const actionValid =
    actionKind === "PostMessage"
      ? Boolean(pmChannel.trim() && pmTemplate.trim())
      : actionKind === "CreateTask"
        ? Boolean(ctPrefix.trim() && ctTitle.trim())
        : Boolean(diMember.trim() && diKind.trim() && diBody.trim());

  const canCreate = backed && actionValid;

  const reset = () => {
    setMpChannel("");
    setMpMention("");
    setMpText("");
    setMePrefix("");
    setMeKind("");
    setMeAuthor("");
    setPmChannel("");
    setPmTemplate("");
    setCtPrefix("");
    setCtTitle("");
    setDiMember("");
    setDiKind("");
    setDiBody("");
    setRuleId("");
  };

  const create = () => {
    if (!canCreate) return;
    const trigger: Trigger =
      triggerKind === "MessagePosted"
        ? {
            MessagePosted: {
              channel_id: mpChannel.trim() || null,
              mention: mpMention.trim() || null,
              text_contains: mpText.trim() || null,
            },
          }
        : {
            MemoryPublished: {
              prefix: mePrefix.trim() || null,
              meta_kind: meKind.trim() || null,
              author_contains: meAuthor.trim() || null,
            },
          };
    const action: Action =
      actionKind === "PostMessage"
        ? { PostMessage: { channel_id: pmChannel.trim(), template: pmTemplate.trim() } }
        : actionKind === "CreateTask"
          ? {
              CreateTask: {
                task_id_prefix: ctPrefix.trim(),
                title_template: ctTitle.trim(),
              },
            }
          : {
              DeliverInbox: {
                member_template: diMember.trim(),
                kind: diKind.trim(),
                body_template: diBody.trim(),
              },
            };
    const id = ruleId.trim() || crypto.randomUUID();
    actions.createRule({ ruleId: id, trigger, action });
    reset();
  };

  return (
    <div
      style={{
        borderRadius: radius.lg,
        border: `1px solid ${color.border}`,
        background: color.paper,
        boxShadow: shadow.card,
        padding: 18,
        display: "grid",
        gap: 16,
      }}
    >
      {/* Trigger */}
      <div style={{ display: "grid", gap: 10 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
          <BuilderLabel>WHEN</BuilderLabel>
          <Segmented
            options={TRIGGER_KINDS}
            value={triggerKind}
            disabled={!backed}
            onChange={setTriggerKind}
          />
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 10 }}>
          {triggerKind === "MessagePosted" ? (
            <>
              <Field
                label="CHANNEL ID"
                value={mpChannel}
                onChange={setMpChannel}
                placeholder="general"
                disabled={!backed}
                optional
              />
              <Field
                label="MENTION"
                value={mpMention}
                onChange={setMpMention}
                placeholder="@alice"
                disabled={!backed}
                optional
              />
              <Field
                label="TEXT CONTAINS"
                value={mpText}
                onChange={setMpText}
                placeholder="deploy"
                disabled={!backed}
                optional
              />
            </>
          ) : (
            <>
              <Field
                label="PREFIX"
                value={mePrefix}
                onChange={setMePrefix}
                placeholder="/notes/"
                disabled={!backed}
                optional
              />
              <Field
                label="META KIND"
                value={meKind}
                onChange={setMeKind}
                placeholder="decision"
                disabled={!backed}
                optional
              />
              <Field
                label="AUTHOR CONTAINS"
                value={meAuthor}
                onChange={setMeAuthor}
                placeholder="operator"
                disabled={!backed}
                optional
              />
            </>
          )}
        </div>
      </div>

      {/* Action */}
      <div style={{ display: "grid", gap: 10 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
          <BuilderLabel>DO</BuilderLabel>
          <Segmented
            options={ACTION_KINDS}
            value={actionKind}
            disabled={!backed}
            onChange={setActionKind}
          />
        </div>
        <div
          style={{
            display: "grid",
            gridTemplateColumns:
              actionKind === "DeliverInbox" ? "1fr 1fr 1fr" : "1fr 1fr",
            gap: 10,
          }}
        >
          {actionKind === "PostMessage" ? (
            <>
              <Field
                label="CHANNEL ID"
                value={pmChannel}
                onChange={setPmChannel}
                placeholder="ops"
                disabled={!backed}
              />
              <Field
                label="TEMPLATE"
                value={pmTemplate}
                onChange={setPmTemplate}
                placeholder="Heads up: {text}"
                disabled={!backed}
              />
            </>
          ) : actionKind === "CreateTask" ? (
            <>
              <Field
                label="TASK ID PREFIX"
                value={ctPrefix}
                onChange={setCtPrefix}
                placeholder="auto-"
                disabled={!backed}
              />
              <Field
                label="TITLE TEMPLATE"
                value={ctTitle}
                onChange={setCtTitle}
                placeholder="Follow up: {text}"
                disabled={!backed}
              />
            </>
          ) : (
            <>
              <Field
                label="MEMBER TEMPLATE"
                value={diMember}
                onChange={setDiMember}
                placeholder="operator"
                disabled={!backed}
              />
              <Field
                label="KIND"
                value={diKind}
                onChange={setDiKind}
                placeholder="mention"
                disabled={!backed}
              />
              <Field
                label="BODY TEMPLATE"
                value={diBody}
                onChange={setDiBody}
                placeholder="{text}"
                disabled={!backed}
              />
            </>
          )}
        </div>
      </div>

      {/* Rule id + submit */}
      <div style={{ display: "flex", alignItems: "flex-end", gap: 10, flexWrap: "wrap" }}>
        <div style={{ flex: 1, minWidth: 180 }}>
          <Field
            label="RULE ID"
            value={ruleId}
            onChange={setRuleId}
            placeholder="auto (random uuid)"
            disabled={!backed}
            optional
          />
        </div>
        <button
          type="button"
          aria-label="Create rule"
          disabled={!canCreate}
          onClick={create}
          onMouseEnter={() => setSubmitHover(true)}
          onMouseLeave={() => setSubmitHover(false)}
          style={{
            all: "unset",
            boxSizing: "border-box",
            height: 34,
            padding: "0 16px",
            borderRadius: radius.sm,
            display: "inline-flex",
            alignItems: "center",
            gap: 6,
            font: `600 12px ${font.sans}`,
            background: canCreate ? (submitHover ? color.dark : accentVar) : color.chip,
            color: canCreate ? color.paper : color.muted2,
            border: `1px solid ${canCreate ? "transparent" : color.borderStrong}`,
            cursor: canCreate ? "pointer" : "default",
            whiteSpace: "nowrap",
          }}
        >
          <Icon name="plus" size={14} strokeWidth={1.9} />
          Create rule
        </button>
      </div>
    </div>
  );
}

// ── Rule list ───────────────────────────────────────────

function RuleRow({
  rule,
  disabled,
  op,
  onToggle,
  onDelete,
}: {
  rule: Rule;
  disabled: boolean;
  /** The rule's finalization record — the meta line draws the inline mark. */
  op: OpRecord | undefined;
  onToggle: (enabled: boolean) => void;
  onDelete: () => void;
}) {
  const [armed, setArmed] = useState(false);
  const [hover, setHover] = useState(false);
  const off = !rule.enabled;

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 13,
        padding: "13px 16px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: hover ? color.sidebar : "transparent",
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            font: `500 13px ${font.sans}`,
            color: off ? color.muted2 : color.ink,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
          title={summarize(rule)}
        >
          {summarize(rule)}
        </div>
        <div
          style={{
            marginTop: 5,
            font: `400 11px ${font.mono}`,
            color: color.muted2,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
            fired {rule.fire_count}× · {shortId(rule.rule_id)}
            <FinalizationMark op={op} />
          </span>
        </div>
      </div>

      <Toggle
        on={rule.enabled}
        disabled={disabled}
        label={`Enable rule ${shortId(rule.rule_id)}`}
        onToggle={() => onToggle(!rule.enabled)}
      />

      {armed ? (
        <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
          <button
            type="button"
            aria-label={`Confirm delete rule ${shortId(rule.rule_id)}`}
            onClick={() => {
              onDelete();
              setArmed(false);
            }}
            style={{
              all: "unset",
              cursor: "pointer",
              height: 28,
              padding: "0 11px",
              borderRadius: radius.sm,
              display: "inline-flex",
              alignItems: "center",
              font: `600 11px ${font.sans}`,
              background: color.danger,
              color: color.paper,
            }}
          >
            Confirm
          </button>
          <button
            type="button"
            aria-label="Cancel delete"
            onClick={() => setArmed(false)}
            style={{
              all: "unset",
              cursor: "pointer",
              height: 28,
              padding: "0 11px",
              borderRadius: radius.sm,
              display: "inline-flex",
              alignItems: "center",
              font: `600 11px ${font.sans}`,
              border: `1px solid ${color.borderStrong}`,
              color: color.muted3,
            }}
          >
            Cancel
          </button>
        </div>
      ) : (
        <button
          type="button"
          aria-label={`Delete rule ${shortId(rule.rule_id)}`}
          onClick={() => setArmed(true)}
          style={{
            all: "unset",
            cursor: "pointer",
            width: 28,
            height: 28,
            borderRadius: radius.sm,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            color: hover ? color.danger : color.muted2,
            border: `1px solid ${hover ? color.dangerBorder : color.borderSoft}`,
            background: hover ? color.dangerSoft : "transparent",
            flexShrink: 0,
          }}
        >
          <Icon name="close" size={14} strokeWidth={1.9} />
        </button>
      )}
    </div>
  );
}

function CenterState({ title, detail }: { title: string; detail: string }) {
  return (
    <div
      style={{
        minHeight: 200,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 9,
        padding: 24,
        textAlign: "center",
      }}
    >
      <span
        style={{
          width: 36,
          height: 36,
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: color.sunken,
          color: color.muted,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <Icon name="automations" size={17} strokeWidth={1.7} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>{title}</div>
      <div
        style={{
          maxWidth: 360,
          font: `400 11.5px ${font.sans}`,
          color: color.muted2,
          lineHeight: 1.55,
        }}
      >
        {detail}
      </div>
    </div>
  );
}

// ── View ────────────────────────────────────────────────

export function AutomationsView() {
  const { state, actions } = useDucktape();

  const loading = state.status === null;
  const backed = Boolean(state.status?.modules.some((m) => m.id === "automations"));
  const rules = state.rules;

  return (
    <div
      data-screen-label="Automations"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.paper,
      }}
    >
      <div
        style={{
          minHeight: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "0 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>
          Automations
        </span>
        <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
          {rules.length}
        </span>
      </div>

      <div
        style={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          padding: 18,
          background: color.sidebar,
          display: "grid",
          gap: 18,
          alignContent: "start",
        }}
      >
        <RuleBuilder backed={backed} />

        <div
          style={{
            borderRadius: radius.lg,
            border: `1px solid ${color.border}`,
            background: color.paper,
            boxShadow: shadow.card,
            overflow: "hidden",
          }}
        >
          {loading ? (
            <CenterState
              title="Loading rules…"
              detail="Waiting for this node's committed automation snapshot."
            />
          ) : !backed ? (
            <CenterState
              title="Automations module is not available"
              detail="This node did not report an automations module, so rules cannot be read or written."
            />
          ) : rules.length === 0 ? (
            <CenterState
              title="No automation rules yet"
              detail="No automation rules yet — build one above."
            />
          ) : (
            rules.map((rule) => (
              <RuleRow
                key={rule.rule_id}
                rule={rule}
                disabled={!backed}
                op={state.ops[opKey.rule(rule.rule_id)]}
                onToggle={(enabled) => actions.setRuleEnabled(rule.rule_id, enabled)}
                onDelete={() => actions.deleteRule(rule.rule_id)}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
