// The per-message "ask an agent to respond" hover action: a HoverBar button
// that opens a small popover of Active agents; picking one submits
// RequestRun anchored on THIS message's chat seq (runs' anchor_seq IS the
// chat sequence — the same number the management form derived from
// head_seq). Renders nothing without a store or with no Active agents.

import { useContext, useEffect, useState } from "react";
import type { CSSProperties } from "react";

import type { AgentRecord } from "../../../domain/agent-client";
import { ConsoleContext } from "../../store/context";
import { HoverButton } from "./HoverButton";
import { color, font, radius, shadow } from "../../theme/tokens";

function BotGlyph({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <rect x="4.5" y="9" width="15" height="10" rx="2.5" />
      <path d="M12 9V5.5M12 5.5a1.4 1.4 0 1 0-.01 0z" />
      <path d="M9 13.6v.8M15 13.6v.8" strokeWidth={2} />
      <path d="M4.5 13H3M21 13h-1.5" />
    </svg>
  );
}

/** A resource-demand field's raw text → a positive integer, or null when it
 *  should be omitted (blank, zero, negative, or non-numeric — consensus
 *  rejects zero/empty, so those never go on the wire). */
const dimValue = (raw: string): number | null => {
  const n = Math.floor(Number(raw));
  return Number.isFinite(n) && n > 0 ? n : null;
};

function AskAgentPopover({
  agents,
  onPick,
  onClose,
}: {
  agents: AgentRecord[];
  onPick: (agentId: string, demands?: Record<string, number>) => void;
  onClose: () => void;
}) {
  const [cores, setCores] = useState("");
  const [memGb, setMemGb] = useState("");

  const buildDemands = (): Record<string, number> | undefined => {
    const demands: Record<string, number> = {};
    const c = dimValue(cores);
    const m = dimValue(memGb);
    if (c !== null) demands.cores = c;
    if (m !== null) demands.mem_gb = m;
    return Object.keys(demands).length > 0 ? demands : undefined;
  };

  // Escape + outside-click dismiss, attached one tick late so the click that
  // OPENED the popover doesn't immediately close it (mirrors EmojiPicker).
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    const timer = setTimeout(() => document.addEventListener("click", onClose), 0);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("click", onClose);
      clearTimeout(timer);
    };
  }, [onClose]);

  return (
    <div
      onClick={(event) => event.stopPropagation()}
      style={{
        position: "absolute",
        top: "calc(100% + 4px)",
        right: 0,
        width: 224,
        zIndex: 4,
        background: color.paper,
        border: `1px solid ${color.borderSoft}`,
        borderRadius: radius.md,
        boxShadow: shadow.pop,
        padding: 4,
      }}
    >
      <div
        style={{
          font: `600 9px ${font.mono}`,
          letterSpacing: ".08em",
          color: color.muted2,
          padding: "4px 6px 5px",
        }}
      >
        ASK TO RESPOND
      </div>
      <div style={{ display: "flex", gap: 6, padding: "0 6px 6px" }}>
        {[
          { label: "Cores", value: cores, set: setCores },
          { label: "Memory (GB)", value: memGb, set: setMemGb },
        ].map((field) => (
          <label key={field.label} style={{ flex: 1, minWidth: 0, display: "grid", gap: 3 }}>
            <span
              style={{
                font: `600 9px ${font.mono}`,
                letterSpacing: ".06em",
                color: color.muted2,
              }}
            >
              {field.label.toUpperCase()}
            </span>
            <input
              type="number"
              min={1}
              step={1}
              inputMode="numeric"
              placeholder="auto"
              value={field.value}
              onChange={(event) => field.set(event.target.value)}
              style={{
                width: "100%",
                boxSizing: "border-box",
                padding: "5px 7px",
                borderRadius: radius.sm,
                border: `1px solid ${color.borderSoft}`,
                background: color.paper,
                font: `400 11.5px ${font.sans}`,
                color: color.ink,
              }}
            />
          </label>
        ))}
      </div>
      <div style={{ maxHeight: 208, overflowY: "auto" }}>
        {agents.map((agent) => (
          <HoverButton
            key={agent.agent_id}
            onClick={(event) => {
              event.stopPropagation();
              onPick(agent.agent_id, buildDemands());
            }}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              width: "100%",
              boxSizing: "border-box",
              padding: "6px 8px",
              borderRadius: radius.sm,
            }}
            hoverStyle={{ background: color.hover }}
          >
            <span
              style={{
                font: `600 12px ${font.sans}`,
                color: color.ink,
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {agent.display_name || agent.agent_id}
            </span>
            <span
              style={{
                marginLeft: "auto",
                font: `400 10.5px ${font.mono}`,
                color: color.muted2,
                flexShrink: 0,
              }}
            >
              @{agent.agent_id}
            </span>
          </HoverButton>
        ))}
      </div>
    </div>
  );
}

export function AskAgentButton({
  channelId,
  seq,
  style,
}: {
  channelId: string;
  /** The message's chat sequence — becomes the run's anchor_seq. */
  seq: number;
  /** The HoverBar's shared button style, passed so the row reads as one bar. */
  style?: CSSProperties;
}) {
  const store = useContext(ConsoleContext);
  const [open, setOpen] = useState(false);
  const active = (store?.state.agents ?? []).filter((agent) => agent.status === "active");
  if (!store || active.length === 0) return null;
  return (
    <>
      <HoverButton
        title="Ask an agent to respond"
        onClick={(event) => {
          event.stopPropagation();
          setOpen((prev) => !prev);
        }}
        style={style}
        hoverStyle={{ background: color.hover }}
      >
        <BotGlyph size={14} />
      </HoverButton>
      {open && (
        <AskAgentPopover
          agents={active}
          onPick={(agentId, demands) => {
            store.actions.requestRun({ agentId, channelId, anchorSeq: seq, demands });
            setOpen(false);
          }}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
}
