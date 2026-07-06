// A joiner's waiting room: its node is joining the network's VPN and cannot
// serve the app until its invite redeems and it syncs. We surface this
// workspace's identity and the live joining→redeemed→synced phase the store
// polls off the node log. Once the node's surface answers, the store swaps
// this out for the console.

import { useState, type CSSProperties } from "react";

import { color, font, radius } from "../../theme/tokens";
import { useDucktape } from "../../store/use-ducktape";
import type { OnboardingPhase } from "../../../domain/workspace-client";

// The ordered steps shown; the node's phase maps onto one of these. The
// invite is the admission: the node delivers this identity to the members
// automatically and a member node redeems it — no approval step. The
// identity card below stays as the manual fallback for token-less joins.
const STEPS = [
  {
    label: "Joining the network",
    detail: "Tunnel and announce are up — the invite redeems automatically",
  },
  {
    label: "Invite redeemed",
    detail: "Full-node standing is recorded in the network",
  },
  {
    label: "Finalized history synced",
    detail: "Projection catches up locally",
  },
  {
    label: "Running as a full node",
    detail: "The console opens automatically",
  },
] as const;

const rootStyle: CSSProperties = {
  flex: 1,
  minHeight: 0,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  overflowY: "auto",
  background: `radial-gradient(ellipse 90% 70% at 50% 0%, ${color.paper} 0%, ${color.sunken} 100%)`,
  padding: 30,
};

const columnStyle: CSSProperties = {
  width: 430,
  maxWidth: "100%",
  display: "flex",
  flexDirection: "column",
};

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

function StepIcon({ state }: { state: "done" | "running" | "pending" | "failed" }) {
  if (state === "done") {
    return (
      <span
        style={{
          width: 19,
          height: 19,
          borderRadius: "50%",
          background: "#eef5f0",
          border: "1px solid #cfe3d7",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          font: `600 10px ${font.mono}`,
          color: "#5f9e74",
          flexShrink: 0,
        }}
      >
        ✓
      </span>
    );
  }

  if (state === "failed") {
    return (
      <span
        style={{
          width: 19,
          height: 19,
          borderRadius: "50%",
          background: "#fbeeec",
          border: "1px solid #eccfc9",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          font: `700 11px ${font.mono}`,
          color: color.red,
          flexShrink: 0,
        }}
      >
        !
      </span>
    );
  }

  if (state === "running") {
    return (
      <span
        style={{
          width: 19,
          height: 19,
          borderRadius: "50%",
          borderWidth: 2,
          borderStyle: "solid",
          borderRightColor: "#e3b443",
          borderBottomColor: "#e3b443",
          borderLeftColor: "#e3b443",
          borderTopColor: "transparent",
          animation: "ik-pulse 1s ease-in-out infinite",
          flexShrink: 0,
        }}
      />
    );
  }

  return (
    <span
      style={{
        width: 19,
        height: 19,
        borderRadius: "50%",
        border: "1px dashed #d5d5d5",
        flexShrink: 0,
      }}
    />
  );
}

export function JoinProgress() {
  const { state, actions } = useDucktape();
  const [copied, setCopied] = useState(false);
  const phase = state.onboardingPhase?.phase ?? "starting";
  const current = stepOf(phase);
  const fatal = phase === "fatal";
  const pubkey = state.workspace?.pubkey ?? "";
  const progress = fatal
    ? "12%"
    : `${Math.round(((Math.max(current, 0) + 1) / STEPS.length) * 100)}%`;

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
    <div style={rootStyle}>
      <div style={columnStyle}>
        <div
          style={{
            font: `500 11px ${font.mono}`,
            color: color.muted2,
            letterSpacing: ".05em",
          }}
        >
          STEP 2 / 3
        </div>
        <div
          style={{
            font: `600 20px ${font.sans}`,
            color: color.dark,
            marginTop: 13,
          }}
        >
          {fatal ? "Join needs attention" : `Joining ${state.workspace?.name ?? "workspace"}`}
        </div>
        <div
          style={{
            font: `400 13px ${font.sans}`,
            color: color.muted,
            marginTop: 5,
            lineHeight: 1.5,
          }}
        >
          Parked nodes wait for admission, then sync finalized history and promote.
        </div>

        <div
          style={{
            height: 5,
            borderRadius: 3,
            background: "#e9e9e9",
            marginTop: 18,
            overflow: "hidden",
          }}
        >
          <div
            style={{
              height: "100%",
              width: progress,
              background: fatal ? color.red : color.dark,
              borderRadius: 3,
              transition: "width .5s ease",
            }}
          />
        </div>

        <div style={{ marginTop: 20 }}>
          <div
            style={{
              font: `600 10px ${font.mono}`,
              letterSpacing: ".1em",
              color: "#b7b7b7",
            }}
          >
            YOUR NODE IDENTITY
          </div>
          <button
            onClick={copy}
            title="Copy identity"
            style={{
              all: "unset",
              cursor: "pointer",
              marginTop: 8,
              display: "flex",
              alignItems: "center",
              gap: 10,
              width: "100%",
              border: `1px solid ${color.border}`,
              background: "#f4f4f4",
              borderRadius: radius.md,
              padding: "10px 12px",
            }}
          >
            <span
              style={{
                flex: 1,
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                font: `400 11px ${font.mono}`,
                color: color.muted3,
              }}
            >
              {pubkey || "waiting for identity"}
            </span>
            <span
              style={{
                font: `600 11px ${font.sans}`,
                color: copied ? "#5f9e74" : color.accent,
                flexShrink: 0,
              }}
            >
              {copied ? "copied" : "copy"}
            </span>
          </button>
        </div>

        <div style={{ marginTop: 22, display: "flex", flexDirection: "column", gap: 14 }}>
          {STEPS.map((item, i) => {
            const done = !fatal && i < current;
            const running = !fatal && i === current;
            const failed = fatal && i === 0;
            const visualState = failed
              ? "failed"
              : done
                ? "done"
                : running
                  ? "running"
                  : "pending";
            return (
              <div
                key={item.label}
                style={{ display: "flex", alignItems: "flex-start", gap: 12 }}
              >
                <StepIcon state={visualState} />
                <div style={{ minWidth: 0, display: "flex", flexDirection: "column", gap: 2 }}>
                  <span
                    style={{
                      font: `400 13.5px ${font.sans}`,
                      color: failed || done || running ? color.inkSoft : "#aeaeae",
                    }}
                  >
                    {item.label}
                  </span>
                  <span
                    style={{
                      font: `400 11px ${font.mono}`,
                      color: failed ? color.red : running ? color.muted3 : color.muted2,
                      lineHeight: 1.45,
                    }}
                  >
                    {failed
                      ? state.onboardingPhase?.detail ?? "the node failed to join"
                      : running && state.onboardingPhase?.detail
                        ? state.onboardingPhase.detail
                        : item.detail}
                  </span>
                </div>
              </div>
            );
          })}
        </div>

        {fatal && (
          <div
            style={{
              marginTop: 18,
              border: "1px solid #eccfc9",
              background: "#fbeeec",
              borderRadius: radius.sm,
              padding: "8px 10px",
              font: `500 11px ${font.mono}`,
              color: color.red,
              lineHeight: 1.45,
            }}
          >
            {state.onboardingPhase?.detail ?? "the node failed to join"}
          </div>
        )}

        <button
          onClick={actions.newWorkspace}
          style={{
            all: "unset",
            cursor: "pointer",
            textAlign: "center",
            marginTop: 28,
            font: `600 11px ${font.sans}`,
            color: color.muted,
          }}
        >
          workspaces
        </button>
      </div>
    </div>
  );
}
