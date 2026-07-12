// Sandbox onboarding tab of the Node view. Three parts, all over existing
// seams (spec §4/§6):
//   1. Serving state — the node.toml opt-in + backend mode, read by the host
//      preflight probe.
//   2. Detection checklist — backend binary / base image / cgroup delegation,
//      green / red / unknown following the view's status styling.
//   3. Opt-in switch — a confirmed apply that atomically updates node.toml and
//      restarts the managed node, rolling back on a failed boot.
// Red items offer "Set up with an agent": one canned run through the existing
// runs pipeline, anchored on the active channel.

import { useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";

import { sandboxApply, sandboxPreflight, type SandboxPreflight } from "../../../domain/sandbox-client";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow, tint } from "../../theme/tokens";
import {
  DEFAULT_SANDBOX_IMAGE,
  MODE_OPTIONS,
  currentSandboxMode,
  modeOptionsFor,
  preflightChecklist,
  setupPrompt,
  type CheckState,
  type SandboxMode,
} from "./sandbox";

const sectionLabelStyle = {
  font: `600 9.5px ${font.mono}`,
  letterSpacing: ".1em",
  color: color.muted2,
} as const;

function SectionLabel({ children, style }: { children: ReactNode; style?: CSSProperties }) {
  return <div style={{ ...sectionLabelStyle, ...style }}>{children}</div>;
}

const CHECK_TONE: Record<CheckState, { bg: string; text: string; glyph: string }> = {
  ok: { bg: tint(color.green).bg, text: tint(color.green).text, glyph: "✓" },
  fail: { bg: tint(color.red).bg, text: tint(color.red).text, glyph: "✕" },
  unknown: { bg: tint(color.amber).bg, text: tint(color.amber).text, glyph: "?" },
};

function ServingPill({ on }: { on: boolean }) {
  const t = on ? tint(color.green) : tint(color.amber);
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        background: t.bg,
        border: `1px solid ${t.border}`,
        borderRadius: radius.sm,
        padding: "3px 9px",
        font: `600 11px ${font.mono}`,
        color: t.text,
      }}
    >
      <span style={{ width: 6, height: 6, borderRadius: "50%", background: on ? color.green : color.amber }} />
      {on ? "Serving" : "Not serving"}
    </span>
  );
}

/** The active-agents picker popover — mirrors AskAgentButton, scoped to firing
 *  a setup run for `prompt`. */
function AgentPickerPopover({
  agents,
  onPick,
  onClose,
}: {
  agents: { agent_id: string; display_name: string }[];
  onPick: (agentId: string) => void;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", onKey);
    const timer = globalThis.setTimeout(() => document.addEventListener("click", onClose), 0);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("click", onClose);
      globalThis.clearTimeout(timer);
    };
  }, [onClose]);

  return (
    <div
      onClick={(e) => e.stopPropagation()}
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
      <div style={{ font: `600 9px ${font.mono}`, letterSpacing: ".08em", color: color.muted2, padding: "4px 6px 5px" }}>
        SET UP WITH
      </div>
      <div style={{ maxHeight: 208, overflowY: "auto" }}>
        {agents.map((agent) => (
          <button
            key={agent.agent_id}
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onPick(agent.agent_id);
            }}
            style={{
              all: "unset",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 8,
              width: "100%",
              boxSizing: "border-box",
              padding: "6px 8px",
              borderRadius: radius.sm,
            }}
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
            <span style={{ marginLeft: "auto", font: `400 10.5px ${font.mono}`, color: color.muted2, flexShrink: 0 }}>
              @{agent.agent_id}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

/** The small "Set up with an agent" control beside a red checklist item. */
function SetupWithAgent({ prompt }: { prompt: string }) {
  const { state, actions } = useDucktape();
  const [open, setOpen] = useState(false);
  const [fired, setFired] = useState(false);
  const active = useMemo(
    () => (state.agents ?? []).filter((a) => a.status === "active"),
    [state.agents],
  );
  const noChannel = !state.activeChannel;
  const disabled = active.length === 0 || noChannel;
  const hint =
    active.length === 0
      ? "register an active agent first"
      : noChannel
        ? "open a chat channel first"
        : "run a canned install/verify prompt on the active channel";

  if (fired) {
    return (
      <span style={{ font: `600 10.5px ${font.mono}`, color: tint(color.green).text, whiteSpace: "nowrap" }}>
        setup run requested →
      </span>
    );
  }

  return (
    <div style={{ position: "relative", flexShrink: 0 }}>
      <button
        type="button"
        title={hint}
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((o) => !o);
        }}
        style={{
          all: "unset",
          cursor: disabled ? "default" : "pointer",
          font: `600 10.5px ${font.sans}`,
          color: disabled ? color.muted2 : color.onDark,
          background: disabled ? color.sunken : color.dark,
          border: `1px solid ${disabled ? color.borderSoft : color.dark}`,
          borderRadius: radius.sm,
          padding: "5px 10px",
          whiteSpace: "nowrap",
        }}
      >
        Set up with an agent
      </button>
      {open && !disabled && (
        <AgentPickerPopover
          agents={active}
          onPick={(agentId) => {
            actions.startSetupRun({ agentId, prompt });
            setOpen(false);
            setFired(true);
          }}
          onClose={() => setOpen(false)}
        />
      )}
    </div>
  );
}

function ChecklistRow({ item, prompt }: { item: ReturnType<typeof preflightChecklist>[number]; prompt: string }) {
  const tone = CHECK_TONE[item.state];
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 11,
        padding: "11px 13px",
        borderTop: `1px solid ${color.borderSoft}`,
      }}
    >
      <span
        style={{
          width: 19,
          height: 19,
          borderRadius: "50%",
          background: tone.bg,
          color: tone.text,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          font: `700 11px ${font.sans}`,
          flexShrink: 0,
        }}
      >
        {tone.glyph}
      </span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ font: `500 12px ${font.sans}`, color: color.inkSoft }}>{item.label}</div>
        <div style={{ font: `400 10.5px ${font.mono}`, color: color.muted2, marginTop: 2, wordBreak: "break-word" }}>
          {item.detail}
        </div>
      </div>
      {item.fixable && <SetupWithAgent prompt={prompt} />}
    </div>
  );
}

export function SandboxTab() {
  const { state } = useDucktape();
  const [pf, setPf] = useState<SandboxPreflight | null>(null);
  const [loading, setLoading] = useState(false);
  const [chosen, setChosen] = useState<"off" | SandboxMode | null>(null);
  const [applyState, setApplyState] = useState<"idle" | "applying" | "applied">("idle");
  const [applyError, setApplyError] = useState<string | null>(null);
  const reqId = useRef(0);

  // Only the app that owns the local managed node can truthfully probe the node
  // host; otherwise leave pf null → the checklist renders all-unknown.
  const workspaceId = state.workspace?.id ?? null;
  const canProbe = state.managed && Boolean(workspaceId);

  const runPreflight = useMemo(
    () => () => {
      if (!canProbe || !workspaceId) return;
      const id = ++reqId.current;
      setLoading(true);
      void sandboxPreflight(workspaceId)
        .then((result) => {
          if (id === reqId.current) setPf(result);
        })
        .catch(() => {
          if (id === reqId.current) setPf(null);
        })
        .finally(() => {
          if (id === reqId.current) setLoading(false);
        });
    },
    [canProbe, workspaceId],
  );

  useEffect(() => {
    runPreflight();
  }, [runPreflight]);

  const applyChosen = () => {
    if (!chosen || !workspaceId || applyState === "applying") return;
    const mode = chosen;
    setChosen(null);
    setApplyError(null);
    setApplyState("applying");
    void sandboxApply(workspaceId, mode)
      .then(() => {
        runPreflight();
        setApplyState("applied");
      })
      .catch((error) => {
        setApplyState("idle");
        setApplyError(error instanceof Error ? error.message : String(error));
      });
  };

  const items = preflightChecklist(pf);
  const backendMode: SandboxMode = pf?.backend === "tart" ? "tart" : "podman";
  const image = pf?.image || DEFAULT_SANDBOX_IMAGE;
  const prompt = setupPrompt(backendMode, image);
  const serving = pf?.announceCapabilities ?? false;
  const currentMode = currentSandboxMode(pf);
  const modeOptions = modeOptionsFor(pf?.os === "macos");
  const chosenOption = MODE_OPTIONS.find((option) => option.id === chosen);

  return (
    <>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
        <div style={{ font: `600 15px ${font.sans}`, color: color.dark }}>Sandbox serving</div>
        <ServingPill on={serving} />
        <span style={{ font: `400 11px ${font.mono}`, color: color.muted2 }}>
          mode {currentMode}
        </span>
        <button
          type="button"
          onClick={runPreflight}
          disabled={!canProbe || loading}
          title={canProbe ? "Re-run the host preflight probes" : "Only a locally managed node can be probed"}
          style={{
            all: "unset",
            marginLeft: "auto",
            cursor: canProbe && !loading ? "pointer" : "default",
            font: `600 11px ${font.sans}`,
            color: canProbe ? color.inkSoft : color.muted2,
            background: color.paper,
            border: `1px solid ${color.borderStrong}`,
            borderRadius: radius.sm,
            padding: "6px 12px",
          }}
        >
          {loading ? "Checking…" : "Re-check"}
        </button>
      </div>

      <div style={{ width: "100%", font: `400 11px ${font.sans}`, color: color.muted3, marginTop: 8, lineHeight: 1.5 }}>
        Nodes serve agent work only when opted in. Turning it on announces this node's executors (and, for sandboxed
        modes, its metered cpu/memory) into the capability registry so demand-carrying runs can land here.
      </div>

      {!canProbe && (
        <div
          style={{
            marginTop: 13,
            border: `1px solid ${tint(color.amber).border}`,
            background: tint(color.amber).bg,
            borderRadius: radius.md,
            padding: "10px 13px",
            font: `400 11.5px ${font.sans}`,
            color: tint(color.amber).text,
            width: "100%",
            boxSizing: "border-box",
          }}
        >
          This app isn't managing a local node, so the checks below can't reach the node host. Run the preflight on the
          machine that runs the node.
        </div>
      )}

      <SectionLabel style={{ marginTop: 22 }}>DETECTION</SectionLabel>
      <div
        style={{
          marginTop: 9,
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          background: color.paper,
          boxShadow: shadow.card,
          overflow: "hidden",
          width: "100%",
          boxSizing: "border-box",
        }}
      >
        <div style={{ padding: "10px 13px", font: `600 10.5px ${font.mono}`, color: color.muted2, letterSpacing: ".04em" }}>
          {pf ? `${pf.backend} · ${pf.os}` : "backend preflight"}
        </div>
        {items.map((item) => (
          <ChecklistRow key={item.id} item={item} prompt={prompt} />
        ))}
      </div>

      <SectionLabel style={{ marginTop: 22 }}>OPT-IN SWITCH</SectionLabel>
      <div
        style={{
          marginTop: 9,
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          background: color.paper,
          boxShadow: shadow.card,
          padding: "14px 16px",
          width: "100%",
          boxSizing: "border-box",
        }}
      >
        <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
          {modeOptions.map((opt) => {
            const active = chosen === opt.id;
            const current = currentMode === opt.id;
            const disabled = !canProbe || applyState === "applying" || current;
            return (
              <button
                key={opt.id}
                type="button"
                disabled={disabled}
                title={!canProbe ? "Only a locally managed node can be changed" : undefined}
                onClick={() => {
                  setApplyError(null);
                  setApplyState("idle");
                  setChosen(opt.id);
                }}
                style={{
                  all: "unset",
                  cursor: disabled ? "default" : "pointer",
                  font: `600 11.5px ${font.sans}`,
                  color: active ? color.onDark : disabled ? color.muted2 : color.inkSoft,
                  background: active ? color.dark : current ? color.sunken : color.paper,
                  border: `1px solid ${active ? color.dark : current ? color.border : color.borderStrong}`,
                  borderRadius: radius.sm,
                  padding: "6px 13px",
                }}
              >
                {opt.label}
                {current && (
                  <span style={{ marginLeft: 6, font: `600 8px ${font.mono}`, color: active ? color.onDark : color.muted2 }}>
                    CURRENT
                  </span>
                )}
              </button>
            );
          })}
        </div>
        <div
          style={{
            font: `400 11px ${font.sans}`,
            color: applyError
              ? tint(color.red).text
              : applyState === "applied"
                ? tint(color.green).text
                : color.muted3,
            marginTop: 10,
          }}
        >
          {applyError
            ? `Apply failed: ${applyError}`
            : applyState === "applying"
              ? "Applying config and restarting the node…"
              : applyState === "applied"
                ? "Applied. The node restarted with the selected mode."
                : "Choose a mode to review and apply it."}
        </div>
      </div>
      {chosen && chosenOption && (
        <ConfirmDialog
          title={`Apply ${chosenOption.label}?`}
          confirmLabel="Apply and restart"
          danger={false}
          onCancel={() => setChosen(null)}
          onConfirm={applyChosen}
        >
          {chosenOption.blurb} This updates this workspace's node config and restarts the local node. If the new node
          fails to start, the previous config is restored.
        </ConfirmDialog>
      )}
    </>
  );
}
