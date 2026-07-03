// The front door (desktop): shown when there is no active workspace, or when
// the user asks to add/switch one. Two paths — found a new network, or join one
// from an invite blob — plus a list of existing workspaces to jump back into.
// On submit the store mints identity + workspace and connects; a joiner then
// falls through to JoinProgress while its node parks.

import { useState, type CSSProperties, type ReactNode } from "react";

import { accentVar, color, font, radius } from "../../theme/tokens";
import { useDucktape } from "../../store/use-ducktape";
import { LIVE_JOIN_SUPPORTED } from "../../../domain/workspace-client";

type Mode = "welcome" | "create" | "join";

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

const sectionLabelStyle: CSSProperties = {
  marginTop: 20,
  font: `600 10px ${font.mono}`,
  letterSpacing: ".1em",
  color: "#b7b7b7",
};

const inputFrameStyle: CSSProperties = {
  marginTop: 8,
  border: `1.5px solid ${color.dark}`,
  borderRadius: 10,
  padding: "12px 14px",
  display: "flex",
  alignItems: "center",
  gap: 7,
  background: color.paper,
};

const inputStyle: CSSProperties = {
  width: "100%",
  minWidth: 0,
  font: `500 14px ${font.mono}`,
  color: color.ink,
};

const mutedPanelStyle: CSSProperties = {
  marginTop: 18,
  background: "#f4f4f4",
  border: `1px solid ${color.borderSoft}`,
  borderRadius: 10,
  padding: 13,
};

function slug(name: string): string {
  return (
    (name || "network")
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9-]+/g, "-")
      .replace(/^-+|-+$/g, "") || "network"
  );
}

function useHover() {
  const [hover, setHover] = useState(false);
  return {
    hover,
    hoverProps: {
      onMouseEnter: () => setHover(true),
      onMouseLeave: () => setHover(false),
    },
  };
}

function BrandMark() {
  return (
    <div
      style={{
        width: 50,
        height: 50,
        borderRadius: 13,
        background: color.dark,
        color: color.onDark,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 22px ${font.mono}`,
        boxShadow: "0 6px 18px rgba(40,38,34,.22)",
      }}
    >
      D
    </div>
  );
}

function BackButton({ label, onClick }: { label: string; onClick: () => void }) {
  const back = useHover();
  return (
    <button
      onClick={onClick}
      {...back.hoverProps}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 8,
        font: `500 11px ${font.mono}`,
        color: back.hover ? color.muted3 : color.muted2,
      }}
    >
      <span style={{ fontSize: 14 }}>‹</span>
      {label}
    </button>
  );
}

function CtaButton({
  tone,
  title,
  subtitle,
  onClick,
  disabled = false,
}: {
  tone: "primary" | "secondary";
  title: ReactNode;
  subtitle?: ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  const button = useHover();
  const primary = tone === "primary";
  const background = primary
    ? button.hover && !disabled
      ? "#322f28"
      : color.dark
    : button.hover && !disabled
      ? "#f8f8f8"
      : color.paper;

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      {...button.hoverProps}
      style={{
        all: "unset",
        cursor: disabled ? "default" : "pointer",
        width: "100%",
        borderRadius: radius.lg,
        padding: "15px 17px",
        display: "block",
        background: disabled ? color.chip : background,
        border: primary ? "none" : `1px solid ${color.borderStrong}`,
        opacity: disabled ? 0.72 : 1,
      }}
    >
      <div
        style={{
          font: `600 14px ${font.sans}`,
          color: disabled ? color.muted3 : primary ? color.paper : color.inkSoft,
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        {title}
        <span style={{ marginLeft: "auto", color: primary ? color.muted : color.iconIdle }}>
          →
        </span>
      </div>
      {subtitle && (
        <div
          style={{
            font: `400 11.5px ${font.sans}`,
            color: primary ? "#afafaf" : color.muted2,
            marginTop: 3,
          }}
        >
          {subtitle}
        </div>
      )}
    </button>
  );
}

function BrandIntro() {
  return (
    <>
      <BrandMark />
      <div
        style={{
          font: `600 22px ${font.sans}`,
          color: color.dark,
          marginTop: 18,
        }}
      >
        Welcome to Ducktape
      </div>
      <div
        style={{
          font: `400 13.5px ${font.sans}`,
          color: color.muted,
          marginTop: 6,
          textAlign: "center",
          lineHeight: 1.55,
        }}
      >
        Invite-only networks for people and agents.
        <br />
        채팅, 코드, 작업을 한곳에서.
      </div>
    </>
  );
}

function ErrorLine({ message }: { message: string | null }) {
  if (!message) return null;
  return (
    <div
      style={{
        marginTop: 14,
        border: "1px solid #eccfc9",
        background: "#fbeeec",
        borderRadius: radius.sm,
        padding: "8px 10px",
        font: `500 11px ${font.mono}`,
        color: color.red,
        lineHeight: 1.45,
      }}
    >
      {message}
    </div>
  );
}

export function OnboardingGate() {
  const { state, actions } = useDucktape();
  const [mode, setMode] = useState<Mode>("welcome");
  const [name, setName] = useState("");
  const [blob, setBlob] = useState("");

  const busy = state.onboardingBusy;
  // live join is enabled (LIVE_JOIN_SUPPORTED); joinGated is the kill-switch
  // path that disables the join form should the flag ever be turned back off.
  const joinGated = mode === "join" && !LIVE_JOIN_SUPPORTED;
  const canSubmit =
    !joinGated &&
    (mode === "create"
      ? name.trim().length > 0
      : mode === "join"
        ? name.trim().length > 0 && blob.trim().length > 0
        : false);

  const submit = () => {
    if (busy || !canSubmit) return;
    if (mode === "create") actions.createWorkspace(name);
    else if (mode === "join") actions.joinWorkspace(name, blob);
  };

  if (busy) {
    const joining = mode === "join";
    const title = joining
      ? `Joining ${name.trim() || "network"}`
      : `Setting up ${name.trim() || "network"}`;
    const rows = joining
      ? ["Register local identity", "Park node on invite mesh", "Wait for admission"]
      : ["Create workspace", "Start local node", "Open console"];
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
            {title}
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
                width: joining ? "42%" : "58%",
                background: color.dark,
                borderRadius: 3,
                transition: "width .5s ease",
              }}
            />
          </div>
          <div style={{ marginTop: 22, display: "flex", flexDirection: "column", gap: 14 }}>
            {rows.map((label, index) => (
              <div key={label} style={{ display: "flex", alignItems: "center", gap: 12 }}>
                <span
                  style={{
                    width: 19,
                    height: 19,
                    borderRadius: "50%",
                    border: index === 0 ? "none" : "1px dashed #d5d5d5",
                    background: index === 0 ? "#eef5f0" : "transparent",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    font: `600 10px ${font.mono}`,
                    color: "#5f9e74",
                    flexShrink: 0,
                    animation: index === 1 ? "ik-pulse 1s ease-in-out infinite" : undefined,
                  }}
                >
                  {index === 0 ? "✓" : ""}
                </span>
                <span
                  style={{
                    font: `400 13.5px ${font.sans}`,
                    color: index <= 1 ? color.inkSoft : "#aeaeae",
                  }}
                >
                  {label}
                </span>
              </div>
            ))}
          </div>
          <div
            style={{
              font: `400 11px ${font.mono}`,
              color: "#b7b7b7",
              textAlign: "center",
              marginTop: 28,
            }}
          >
            {joining
              ? "parked nodes continue in the admission checklist"
              : "the console opens automatically"}
          </div>
        </div>
      </div>
    );
  }

  const workspaceList = state.workspaces.length > 0 && (
    <div
      style={{
        marginTop: 24,
        paddingTop: 18,
        borderTop: `1px solid ${color.border}`,
      }}
    >
      <div
        style={{
          font: `600 10px ${font.mono}`,
          letterSpacing: ".1em",
          color: "#b7b7b7",
        }}
      >
        YOUR WORKSPACES
      </div>
      <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 7 }}>
        {state.workspaces.map((w) => (
          <button
            key={w.id}
            onClick={() => actions.selectWorkspace(w.id)}
            style={{
              all: "unset",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: 12,
              border: `1px solid ${color.border}`,
              borderRadius: radius.md,
              padding: "10px 13px",
              background: color.paper,
            }}
          >
            <span style={{ font: `600 12.5px ${font.sans}`, color: color.ink }}>
              {w.name}
            </span>
            <span
              style={{
                font: `500 10.5px ${font.mono}`,
                color: color.muted2,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {w.chainId}
            </span>
          </button>
        ))}
      </div>
    </div>
  );

  const currentWorkspaceBack = state.workspace && (
    <button
      onClick={actions.dismissOnboarding}
      style={{
        all: "unset",
        cursor: "pointer",
        textAlign: "center",
        marginTop: 18,
        font: `600 11px ${font.sans}`,
        color: color.muted,
      }}
    >
      back to {state.workspace.name}
    </button>
  );

  if (mode === "welcome") {
    return (
      <div style={rootStyle}>
        <div style={{ ...columnStyle, alignItems: "center" }}>
          <BrandIntro />
          <div style={{ width: "100%", marginTop: 28 }}>
            <CtaButton
              tone="primary"
              title="Create a network"
              subtitle="Start a local node on this machine"
              onClick={() => setMode("create")}
            />
          </div>
          <div style={{ width: "100%", marginTop: 11 }}>
            <CtaButton
              tone="secondary"
              title="Join"
              subtitle="with an invite from an existing network"
              onClick={() => setMode("join")}
              disabled={!LIVE_JOIN_SUPPORTED}
            />
          </div>
          <div
            style={{
              font: `400 10.5px ${font.mono}`,
              color: color.iconIdle,
              marginTop: 24,
              textAlign: "center",
              lineHeight: 1.7,
            }}
          >
            keys generated on-device · local node lifecycle
            <br />
            nothing leaves this machine without your signature
          </div>
          <div style={{ width: "100%" }}>
            <ErrorLine message={state.error} />
            {workspaceList}
            {currentWorkspaceBack}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div style={rootStyle}>
      <div style={columnStyle}>
        <BackButton
          label={mode === "create" ? "STEP 1 / 3" : "BACK"}
          onClick={() => setMode("welcome")}
        />

        <div
          style={{
            font: `600 20px ${font.sans}`,
            color: color.dark,
            marginTop: 16,
          }}
        >
          {mode === "create" ? "Create a network" : "Join a network"}
        </div>
        <div
          style={{
            font: `400 13px ${font.sans}`,
            color: color.muted,
            marginTop: 5,
            lineHeight: 1.5,
          }}
        >
          {mode === "create"
            ? "Create the first node and founder identity on this machine."
            : joinGated
              ? "Live joining is temporarily unavailable."
              : "Paste an invite blob, then wait for a member to admit this node."}
        </div>

        {joinGated ? (
          <div
            style={{
              ...mutedPanelStyle,
              padding: "12px 13px",
              font: `500 11.5px ${font.sans}`,
              color: color.muted3,
              lineHeight: 1.5,
            }}
          >
            Joining a running network is temporarily unavailable. Found a new
            network to get started, and invite others from Settings.
          </div>
        ) : (
          <>
            <div style={sectionLabelStyle}>NETWORK NAME</div>
            <div style={inputFrameStyle}>
              <span style={{ font: `500 14px ${font.mono}`, color: "#b7b7b7" }}>#</span>
              <input
                aria-label="Workspace name"
                value={name}
                placeholder="Workspace name"
                onChange={(event) => setName(event.target.value)}
                onKeyDown={(event) => event.key === "Enter" && mode === "create" && submit()}
                style={inputStyle}
              />
            </div>

            {mode === "create" && (
              <>
                <div style={sectionLabelStyle}>LOCAL SETUP</div>
                <div
                  style={{
                    marginTop: 8,
                    display: "flex",
                    flexDirection: "column",
                    gap: 7,
                  }}
                >
                  {[
                    ["network id", slug(name)],
                    ["node", "founder validator"],
                    ["identity", "generated on-device"],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        gap: 12,
                        border: `1px solid ${color.border}`,
                        borderRadius: radius.md,
                        padding: "10px 13px",
                        font: `400 12px ${font.mono}`,
                        color: color.muted3,
                        background: color.paper,
                      }}
                    >
                      <span style={{ color: color.muted2 }}>{label}</span>
                      <span
                        style={{
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {value}
                      </span>
                    </div>
                  ))}
                </div>
              </>
            )}

            {mode === "join" && (
              <>
                <div style={sectionLabelStyle}>INVITE BLOB</div>
                <div
                  style={{
                    ...inputFrameStyle,
                    alignItems: "flex-start",
                    minHeight: 90,
                  }}
                >
                  <textarea
                    aria-label="Invite blob"
                    value={blob}
                    placeholder="Paste invite blob (ducktape-invite-v1:…)"
                    onChange={(event) => setBlob(event.target.value)}
                    rows={4}
                    style={{
                      ...inputStyle,
                      resize: "vertical",
                      font: `400 11.5px ${font.mono}`,
                      lineHeight: 1.45,
                    }}
                  />
                </div>
                <div
                  style={{
                    ...mutedPanelStyle,
                    display: "flex",
                    flexDirection: "column",
                    gap: 10,
                  }}
                >
                  {[
                    ["#5cb45f", color.muted3, "invite parsed locally"],
                    ["#e3b443", color.muted3, "parks until a member admits you"],
                    ["#d5d5d5", "#aeaeae", "then syncs and promotes automatically"],
                  ].map(([dot, textColor, text]) => (
                    <div key={text} style={{ display: "flex", alignItems: "center", gap: 9 }}>
                      <span
                        style={{ width: 6, height: 6, borderRadius: "50%", background: dot }}
                      />
                      <span style={{ font: `400 11.5px ${font.mono}`, color: textColor }}>
                        {text}
                      </span>
                    </div>
                  ))}
                </div>
              </>
            )}

            <ErrorLine message={state.error} />

            <button
              onClick={submit}
              disabled={busy || !canSubmit}
              style={{
                all: "unset",
                textAlign: "center",
                cursor: busy || !canSubmit ? "default" : "pointer",
                marginTop: 22,
                padding: "13px 0",
                borderRadius: 10,
                background: busy || !canSubmit ? color.chip : color.dark,
                color: busy || !canSubmit ? color.muted3 : color.paper,
                font: `600 13.5px ${font.sans}`,
              }}
            >
              {busy
                ? "Setting up…"
                : mode === "create"
                  ? "Create workspace"
                  : "Join workspace"}
            </button>
          </>
        )}

        {mode === "join" && (
          <button
            onClick={() => setMode("create")}
            style={{
              all: "unset",
              cursor: "pointer",
              marginTop: 14,
              textAlign: "center",
              font: `600 11px ${font.sans}`,
              color: accentVar,
            }}
          >
            Create a network instead
          </button>
        )}

        {workspaceList}
        {currentWorkspaceBack}
      </div>
    </div>
  );
}
