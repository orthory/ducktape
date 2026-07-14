// The Auto-reply tab: per-channel watch rows (with finalization marks) and
// the composer that adds one. Watches key by channel — one policy per
// channel, enforced by the runs module.

import { useState } from "react";
import type { FormEvent } from "react";

import type { AgentRecord } from "../../../domain/agent-client";
import type { Channel } from "../../../domain/chat-client";
import type { TurnPolicy, WatchView } from "../../../domain/runs-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpLedger, OpRecord } from "../../store/finalization";
import { color, font, radius } from "../../theme/tokens";
import {
  channelLabel,
  EmptyState,
  FieldLabel,
  GroupCard,
  inputStyle,
  policyText,
  primaryButton,
  secondaryButton,
  SectionLabel,
} from "./parts";

const POLICY_KINDS = ["mention", "all", "round_robin", "assigned"] as const;
type PolicyKind = (typeof POLICY_KINDS)[number];

// "When to reply" options — plain language for the dispatch turn policy.
const POLICY_LABEL: Record<PolicyKind, string> = {
  mention: "When mentioned",
  all: "Every message",
  round_robin: "Take turns",
  assigned: "Only a chosen agent",
};

function WatchRow({
  watch,
  channels,
  agents,
  op,
  onUnwatch,
}: {
  watch: WatchView;
  channels: Channel[];
  agents: AgentRecord[];
  /** The watch's finalization record (watch/unwatch key by channel). */
  op: OpRecord | undefined;
  onUnwatch: (id: string) => void;
}) {
  const label = channelLabel(channels, watch.channel_id);
  const pending = op?.phase === "pending";
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 11,
        padding: "12px 14px",
        borderBottom: `1px solid ${color.borderSoft}`,
      }}
    >
      <span
        style={{
          width: 31,
          height: 31,
          borderRadius: radius.sm,
          border: `1px solid ${color.border}`,
          background: color.sunken,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: color.muted2,
          flexShrink: 0,
        }}
      >
        <Icon name="hash" size={15} />
      </span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          title={watch.channel_id}
          translate="no"
          style={{
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            font: `600 12px ${font.mono}`,
            color: color.ink,
          }}
        >
          {label}
        </div>
        <div
          style={{
            marginTop: 2,
            display: "flex",
            alignItems: "center",
            gap: 6,
            font: `400 11.5px ${font.sans}`,
            color: color.muted2,
          }}
        >
          {policyText(watch.policy, agents)}
          <FinalizationMark op={op} />
        </div>
      </div>
      <button
        type="button"
        disabled={pending}
        onClick={() => onUnwatch(watch.channel_id)}
        aria-label={`Stop watching ${label}`}
        style={{
          ...secondaryButton,
          minHeight: 30,
          color: color.red,
          cursor: pending ? "default" : "pointer",
          opacity: pending ? 0.6 : 1,
        }}
      >
        Turn off
      </button>
    </div>
  );
}

function WatchForm({
  channels,
  agents,
  ops,
  onWatch,
}: {
  channels: Channel[];
  agents: AgentRecord[];
  ops: OpLedger;
  onWatch: (params: { channelId: string; policy: TurnPolicy }) => void;
}) {
  const [channelId, setChannelId] = useState("");
  const [kind, setKind] = useState<PolicyKind>("mention");
  const [assigned, setAssigned] = useState("");

  const policy: TurnPolicy | null =
    kind === "assigned" ? (assigned ? { assigned: assigned } : null) : kind;
  const pending = channelId !== "" && ops[opKey.watch(channelId)]?.phase === "pending";
  const ready = channelId !== "" && policy !== null && !pending;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready || !policy) return;
    onWatch({ channelId, policy });
    setChannelId("");
    setKind("mention");
    setAssigned("");
  };

  return (
    <form
      onSubmit={submit}
      style={{
        padding: 14,
        borderTop: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
      }}
    >
      <div
        style={{
          display: "grid",
          gridTemplateColumns:
            kind === "assigned"
              ? "minmax(0, 1fr) minmax(0, 1fr) minmax(120px, .8fr) auto"
              : "minmax(0, 1.1fr) minmax(130px, .8fr) auto",
          gap: 8,
          alignItems: "end",
        }}
      >
        <div style={{ minWidth: 0 }}>
          <FieldLabel htmlFor="agent-watch-channel">Channel to watch</FieldLabel>
          <select
            id="agent-watch-channel"
            name="agent-watch-channel"
            value={channelId}
            onChange={(event) => setChannelId(event.target.value)}
            disabled={channels.length === 0}
            style={{ ...inputStyle, cursor: channels.length > 0 ? "pointer" : "default" }}
          >
            <option value="">{channels.length === 0 ? "No channels" : "Choose channel…"}</option>
            {channels.map((channel) => (
              <option key={channel.id} value={channel.id}>
                {channel.name}
              </option>
            ))}
          </select>
        </div>
        <div style={{ minWidth: 0 }}>
          <FieldLabel htmlFor="agent-watch-policy">When to reply</FieldLabel>
          <select
            id="agent-watch-policy"
            name="agent-watch-policy"
            value={kind}
            onChange={(event) => setKind(event.target.value as PolicyKind)}
            style={{ ...inputStyle, cursor: "pointer" }}
          >
            {POLICY_KINDS.map((option) => (
              <option key={option} value={option}>
                {POLICY_LABEL[option]}
              </option>
            ))}
          </select>
        </div>
        {kind === "assigned" && (
          <div style={{ minWidth: 0 }}>
            <FieldLabel htmlFor="agent-watch-assigned">Which agent</FieldLabel>
            <select
              id="agent-watch-assigned"
              name="agent-watch-assigned"
              value={assigned}
              onChange={(event) => setAssigned(event.target.value)}
              disabled={agents.length === 0}
              style={{ ...inputStyle, cursor: agents.length > 0 ? "pointer" : "default" }}
            >
              <option value="">{agents.length === 0 ? "No agents" : "Choose agent…"}</option>
              {agents.map((agent) => (
                <option key={agent.agent_id} value={agent.agent_id}>
                  {agent.display_name}
                </option>
              ))}
            </select>
          </div>
        )}
        <button type="submit" disabled={!ready} style={primaryButton(ready)}>
          Add auto-reply
        </button>
      </div>
    </form>
  );
}

export function WatchesPanel({
  channels,
  agents,
  watches,
  ops,
  onWatch,
  onUnwatch,
}: {
  channels: Channel[];
  agents: AgentRecord[];
  watches: WatchView[];
  /** The store's finalization ledger — watch rows draw their marks. */
  ops: OpLedger;
  onWatch: (params: { channelId: string; policy: TurnPolicy }) => void;
  onUnwatch: (id: string) => void;
}) {
  return (
    <section aria-label="Watches" style={{ minWidth: 0 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionLabel>AUTO-REPLY</SectionLabel>
        <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
          {watches.length}
        </span>
      </div>
      <GroupCard style={{ marginTop: 9 }}>
        {watches.length === 0 ? (
          <EmptyState
            icon="hash"
            title="No auto-reply set up"
            body="Pick a channel and your agents will answer there on the rule you choose."
          />
        ) : (
          watches.map((watch) => (
            <WatchRow
              key={watch.channel_id}
              watch={watch}
              channels={channels}
              agents={agents}
              op={ops[opKey.watch(watch.channel_id)]}
              onUnwatch={onUnwatch}
            />
          ))
        )}
        <WatchForm channels={channels} agents={agents} ops={ops} onWatch={onWatch} />
      </GroupCard>
    </section>
  );
}
