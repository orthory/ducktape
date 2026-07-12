// Sandbox onboarding tab of the Node view. Three parts, all over existing
// seams (spec §4/§6):
//   1. Serving state — the node.toml opt-in + backend mode, read by the host
//      preflight probe.
//   2. Detection checklist — backend binary / base image / cgroup delegation,
//      green / red / unknown following the view's status styling.
//   3. Opt-in switch — a mode selector that emits the exact node.toml lines to
//      paste (the app has no config-write path; onboarding is guided copy).
// Red items offer "Set up with an agent": one canned run through the existing
// runs pipeline, anchored on the active channel.

import { useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";

import type { SandboxPreflight } from "../../../domain/sandbox-client";
import { sandboxPreflight } from "../../../domain/sandbox-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow, tint } from "../../theme/tokens";
import {
  DEFAULT_SANDBOX_IMAGE,
  preflightChecklist,
  SERVING_OFF_TOML,
  servingTomlLines,
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

/** A copy-to-clipboard button that flips its label for 1.2s (mirrors the
 *  StateCommitment copy affordance). */
function useCopy(): [string | null, (key: string, value: string) => void] {
  const [copied, setCopied] = useState<string | null>(null);
  const copy = (key: string, value: string) => {
    setCopied(key);
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(value).catch(() => {});
    }
    globalThis.setTimeout(() => setCopied((c) => (c === key ? null : c)), 1200);
  };
  return [copied, copy];
}

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

const MODE_OPTIONS: { id: "off" | SandboxMode; label: string; blurb: string }[] = [
  { id: "off", label: "Off", blurb: "Serve no agent work — leave the capability registry." },
  { id: "direct", label: "Direct", blurb: "Unsandboxed spawn. Tags only, no metered capacity." },
  { id: "podman", label: "Podman", blurb: "Rootless podman with per-run cpu/memory caps (Linux)." },
  { id: "tart", label: "Tart", blurb: "Apple-Silicon VM per run (macOS, phase 2)." },
];

function ModeGuidance({ mode, image }: { mode: "off" | SandboxMode; image: string }) {
  const [copied, copy] = useCopy();
  const toml = mode === "off" ? SERVING_OFF_TOML : servingTomlLines(mode, image);
  return (
    <div style={{ marginTop: 11 }}>
      <div style={{ font: `400 11px ${font.sans}`, color: color.muted3, lineHeight: 1.5, marginBottom: 8 }}>
        The app doesn't edit <code style={{ font: `500 10.5px ${font.mono}` }}>node.toml</code> directly. Add these
        lines to this workspace's <code style={{ font: `500 10.5px ${font.mono}` }}>node.toml</code> and restart the
        node — the switch takes effect on the next boot.
      </div>
      <div
        style={{
          border: `1px solid ${copied === "toml" ? tint(color.green).border : color.border}`,
          borderRadius: radius.md,
          background: copied === "toml" ? tint(color.green).bg : color.sunken,
          padding: "11px 13px",
          position: "relative",
        }}
      >
        <pre
          style={{
            margin: 0,
            font: `500 12px ${font.mono}`,
            color: color.inkSoft,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }}
        >
          {toml}
        </pre>
        <button
          type="button"
          onClick={() => copy("toml", toml)}
          style={{
            all: "unset",
            cursor: "pointer",
            position: "absolute",
            top: 8,
            right: 10,
            font: `600 9px ${font.mono}`,
            letterSpacing: ".05em",
            color: copied === "toml" ? color.accentAlt2 : color.muted2,
          }}
        >
          {copied === "toml" ? "COPIED" : "COPY"}
        </button>
      </div>
    </div>
  );
}

export function SandboxTab() {
  const { state } = useDucktape();
  const [pf, setPf] = useState<SandboxPreflight | null>(null);
  const [loading, setLoading] = useState(false);
  const [chosen, setChosen] = useState<"off" | SandboxMode | null>(null);
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

  const items = preflightChecklist(pf);
  const backendMode: SandboxMode = pf?.backend === "tart" ? "tart" : "podman";
  const image = pf?.image || DEFAULT_SANDBOX_IMAGE;
  const prompt = setupPrompt(backendMode, image);
  const serving = pf?.announceCapabilities ?? false;
  const configuredMode = pf?.mode ? pf.mode : "unset";
  const macos = pf?.os === "macos";
  const modeOptions = MODE_OPTIONS.filter((m) => m.id !== (macos ? "podman" : "tart"));

  return (
    <>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
        <div style={{ font: `600 15px ${font.sans}`, color: color.dark }}>Sandbox serving</div>
        <ServingPill on={serving} />
        <span style={{ font: `400 11px ${font.mono}`, color: color.muted2 }}>
          mode {configuredMode}
        </span>
        <button
          type="button"
          onClick={runPreflight}
          disabled={!canProbe || loading}
          title={canProbe ? "Re-run the host preflight probes" : "Only a locally managed node can be probed"}
          style={{
            marginLeft: "auto",
            all: "unset",
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

      <div style={{ font: `400 11px ${font.sans}`, color: color.muted3, marginTop: 8, lineHeight: 1.5, maxWidth: 640 }}>
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
            maxWidth: 640,
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
          maxWidth: 640,
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
          maxWidth: 640,
        }}
      >
        <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
          {modeOptions.map((opt) => {
            const active = chosen === opt.id;
            const current = configuredMode === opt.id || (opt.id === "off" && !serving);
            return (
              <button
                key={opt.id}
                type="button"
                onClick={() => setChosen(active ? null : opt.id)}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  font: `600 11.5px ${font.sans}`,
                  color: active ? color.onDark : color.inkSoft,
                  background: active ? color.dark : color.paper,
                  border: `1px solid ${active ? color.dark : color.borderStrong}`,
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

        {chosen ? (
          <>
            <div style={{ font: `400 11px ${font.sans}`, color: color.muted2, marginTop: 10, lineHeight: 1.5 }}>
              {MODE_OPTIONS.find((m) => m.id === chosen)?.blurb}
            </div>
            <ModeGuidance mode={chosen} image={image} />
          </>
        ) : (
          <div style={{ font: `400 11px ${font.sans}`, color: color.muted3, marginTop: 10 }}>
            Pick a mode to see the exact <code style={{ font: `500 10.5px ${font.mono}` }}>node.toml</code> lines to
            paste.
          </div>
        )}
      </div>
    </>
  );
}
