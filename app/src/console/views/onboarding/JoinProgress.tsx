// A joiner's waiting room: its node has parked on the mesh and cannot serve the
// app until a member admits it and the epoch cuts over. We surface this
// workspace's identity (to hand a member) and the live park→admitted→promoted
// phase the store polls off the node log. Once the promoted node's surface
// answers, the store swaps this out for the console.

import { useState } from "react";

import { color, font, radius, shadow } from "../../theme/tokens";
import { useDucktape } from "../../store/use-ducktape";
import type { OnboardingPhase } from "../../../domain/workspace-client";

// The ordered steps shown; the node's phase maps onto one of these.
const STEPS = ["Parked", "Admitted", "Synced", "Promoted"] as const;

const stepOf = (phase: OnboardingPhase): number => {
  switch (phase) {
    case "starting":
    case "parked":
      return 0;
    case "admitted":
      return 1;
    case "synced":
      return 2;
    case "promoted":
      return 3;
    case "fatal":
      return -1;
  }
};

export function JoinProgress() {
  const { state, actions } = useDucktape();
  const [copied, setCopied] = useState(false);
  const phase = state.onboardingPhase?.phase ?? "starting";
  const current = stepOf(phase);
  const fatal = phase === "fatal";
  const pubkey = state.workspace?.pubkey ?? "";

  const copy = () => {
    void navigator.clipboard?.writeText(pubkey).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      },
      () => undefined,
    );
  };

  return (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: color.paper,
        padding: 24,
      }}
    >
      <div
        style={{
          width: 460,
          maxWidth: "100%",
          background: color.sidebar,
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          boxShadow: shadow.pop,
          padding: 24,
          display: "flex",
          flexDirection: "column",
          gap: 18,
        }}
      >
        <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
          <span style={{ font: `600 16px ${font.sans}`, color: color.ink }}>
            Joining {state.workspace?.name ?? "workspace"}
          </span>
          <span style={{ font: `500 12px ${font.sans}`, color: color.muted }}>
            Your node is on the mesh. A member must admit you before it can sync.
          </span>
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
          <span style={{ font: `600 10.5px ${font.sans}`, color: color.muted2, letterSpacing: ".04em" }}>
            SEND YOUR IDENTITY TO A MEMBER
          </span>
          <button
            onClick={copy}
            title="Copy"
            style={{
              all: "unset",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: 10,
              padding: "9px 11px",
              borderRadius: radius.sm,
              border: `1px solid ${color.borderStrong}`,
              background: color.sunken,
            }}
          >
            <span
              style={{
                font: `500 10.5px ${font.mono}`,
                color: color.inkSoft,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {pubkey}
            </span>
            <span style={{ font: `600 10.5px ${font.sans}`, color: color.accent, flexShrink: 0 }}>
              {copied ? "copied" : "copy"}
            </span>
          </button>
        </div>

        {fatal ? (
          <span style={{ font: `500 11.5px ${font.mono}`, color: color.red }}>
            {state.onboardingPhase?.detail ?? "the node failed to join"}
          </span>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 9 }}>
            {STEPS.map((label, i) => {
              const done = i < current;
              const active = i === current;
              const dot = done ? color.green : active ? color.accent : color.chip;
              return (
                <div key={label} style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <span
                    style={{
                      width: 9,
                      height: 9,
                      borderRadius: "50%",
                      background: dot,
                      flexShrink: 0,
                      boxShadow: active ? `0 0 0 3px ${color.paper}, 0 0 0 5px ${color.accent}22` : "none",
                    }}
                  />
                  <span
                    style={{
                      font: `${active || done ? 600 : 500} 12px ${font.sans}`,
                      color: done ? color.muted : active ? color.ink : color.muted2,
                    }}
                  >
                    {label}
                  </span>
                </div>
              );
            })}
          </div>
        )}

        {state.onboardingPhase?.detail && !fatal && (
          <span style={{ font: `500 10.5px ${font.mono}`, color: color.muted2 }}>
            {state.onboardingPhase.detail}
          </span>
        )}

        <button
          onClick={actions.newWorkspace}
          style={{
            all: "unset",
            cursor: "pointer",
            textAlign: "center",
            font: `600 11px ${font.sans}`,
            color: color.muted,
          }}
        >
          ← workspaces
        </button>
      </div>
    </div>
  );
}
