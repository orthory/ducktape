// Menu-bar dropdown (macOS) — a compact master/detail popover rendered in its
// OWN frameless webview (index.html?view=tray), separate from the console. It
// can't share the console's in-memory store, so it self-fetches a snapshot
// from the active workspace's node (same domain clients the console uses) and
// drives the app through the tray_* Tauri commands: `tray_open_console` shows
// the main window and (optionally) emits `ducktape://navigate` so the console
// jumps to a screen; `tray_quit` exits.
//
// Shape: LEFT a ~150px nav column — a pinned "Node" entry (the default
// selection) on top, then the scrollable module rail below, with Settings +
// Quit pinned at the bottom. RIGHT a scrollable detail pane for whatever's
// selected: Node's own field list + a SOFTWARE section, or a small per-module
// card with its own "Open in console".
//
// Visual: a dark-vibrancy popover — a dark translucent scrim drawn OVER the
// native NSVisualEffectView "popover" material (see src-tauri/src/tray.rs),
// light text on top — matching the native macOS menu-bar look instead of the
// console's light paper surface. The tray never controls node lifecycle (no
// start/stop toggle) — that stays the console's job.

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

import { Icon } from "../../components/Icon";
import { MODULES } from "../../modules/registry";
import { accentVar, font } from "../../theme/tokens";
import * as profilesClient from "../../../domain/profiles-client";
import { remoteTransport, type NodeStatus } from "../../../domain/transport";
import { activeWorkspace, type Workspace } from "../../../domain/workspace-client";

// On the dark translucent material: light text, faint white for hover/hairlines.
const TEXT = "rgba(255,255,255,0.94)";
const DIM = "rgba(255,255,255,0.55)";
const FAINT = "rgba(255,255,255,0.40)";
const HOVER = "rgba(255,255,255,0.08)";
const SELBG = "rgba(255,255,255,0.13)";
const HAIRLINE = "rgba(255,255,255,0.12)";

interface Snap {
  workspace: Workspace | null;
  status: NodeStatus | null;
  memberCount: number;
}

const EMPTY: Snap = { workspace: null, status: null, memberCount: 0 };

// Read a snapshot straight from the active workspace's node. Read-only — it
// never spawns/selects (that's the console's job); if no workspace is active
// or the node is down, it degrades to a "Stopped" placeholder rather than
// throwing.
async function loadSnap(): Promise<Snap> {
  const workspace = await activeWorkspace().catch(() => null);
  if (!workspace) return EMPTY;
  const live = remoteTransport(`http://127.0.0.1:${workspace.ports.http}`);
  try {
    const [status, profiles] = await Promise.all([
      live.status(),
      profilesClient.allProfiles(live, { from: 0, limit: 256 }).catch(() => []),
    ]);
    return { workspace, status, memberCount: profiles.length };
  } catch {
    return { workspace, status: null, memberCount: 0 };
  }
}

const shortKey = (hex: string): string =>
  hex.length > 12 ? `${hex.slice(0, 6)}…${hex.slice(-4)}` : hex || "—";

const openConsole = (screen?: string): void =>
  void invoke("tray_open_console", screen ? { request: { screen } } : {}).catch(
    () => {},
  );
const quit = (): void => void invoke("tray_quit").catch(() => {});

type Sel = { kind: "node" } | { kind: "module"; id: string };

export function TrayPopover() {
  const [snap, setSnap] = useState<Snap>(EMPTY);
  const [sel, setSel] = useState<Sel>({ kind: "node" });

  useEffect(() => {
    let alive = true;
    const tick = () =>
      void loadSnap().then((s) => {
        if (alive) setSnap(s);
      });
    tick();
    const id = window.setInterval(tick, 2500);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, []);

  // The pluggable module list minus Node (Node is the pinned identity-style
  // entry above it, not part of the scrollable rail).
  const rail = useMemo(
    () => [...MODULES].filter((m) => m.id !== "status").sort((a, b) => a.nav.order - b.nav.order),
    [],
  );
  const nodeMod = MODULES.find((m) => m.id === "status");
  const connected = snap.status !== null;

  return (
    <div
      style={{
        width: "100vw",
        height: "100vh",
        borderRadius: 12,
        overflow: "hidden",
        // A dark scrim OVER the native vibrancy: keeps a hint of the glass/blur but
        // stays consistently dark (and readable) regardless of what window sits
        // behind it — a translucent material over a white window washes out otherwise.
        background: "rgba(20,20,23,0.82)",
        boxShadow: "inset 0 0 0 0.5px rgba(255,255,255,0.12)",
        display: "flex",
        flexDirection: "column",
        font: `400 13px ${font.sans}`,
        color: TEXT,
        WebkitUserSelect: "none",
      }}
    >
      {/* Header — brand + connection/height. */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 9,
          padding: "10px 12px",
          background: "rgba(0,0,0,0.22)",
          borderBottom: `1px solid ${HAIRLINE}`,
        }}
      >
        <Chip text="D" />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ font: `600 12.5px ${font.sans}`, color: "#fff" }}>Ducktape</div>
          <div style={{ display: "flex", alignItems: "center", gap: 5, marginTop: 1 }}>
            <span
              style={{
                width: 6,
                height: 6,
                borderRadius: "50%",
                background: connected ? "#5cb45f" : "#cf6a5e",
              }}
            />
            <span style={{ font: `400 10px ${font.mono}`, color: DIM }}>
              {connected ? `Synced · h${(snap.status?.height ?? 0).toLocaleString()}` : "Stopped"}
            </span>
          </div>
        </div>
      </div>

      {/* Body — left nav / right detail. */}
      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        {/* LEFT: dense nav column. */}
        <div style={{ width: 150, flexShrink: 0, display: "flex", flexDirection: "column", borderRight: `1px solid ${HAIRLINE}` }}>
          {nodeMod && (
            <Nav selected={sel.kind === "node"} onClick={() => setSel({ kind: "node" })} pad="9px 11px">
              <Icon name={nodeMod.nav.icon} size={17} color={sel.kind === "node" ? "#fff" : DIM} />
              <span style={{ flex: 1, font: `600 12px ${font.sans}` }}>{nodeMod.nav.label}</span>
            </Nav>
          )}
          <Divider />
          {/* Scrollable module rail — grows by scroll, not by window height. */}
          <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
            {rail.map((m) => {
              const selected = sel.kind === "module" && sel.id === m.id;
              return (
                <Nav key={m.id} selected={selected} onClick={() => setSel({ kind: "module", id: m.id })}>
                  <Icon name={m.nav.icon} size={16} color={selected ? "#fff" : DIM} />
                  <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {m.nav.label}
                  </span>
                </Nav>
              );
            })}
          </div>
          <Divider />
          <Nav onClick={() => openConsole("settings")}>
            <Icon name="settings" size={16} color={DIM} />
            <span style={{ flex: 1 }}>Settings</span>
          </Nav>
          <Nav onClick={quit}>
            <Icon name="close" size={16} color={DIM} />
            <span style={{ flex: 1 }}>Quit</span>
          </Nav>
        </div>

        {/* RIGHT: scrollable detail. */}
        <div style={{ flex: 1, minWidth: 0, overflowY: "auto", padding: "12px 13px" }}>
          {sel.kind === "node" && <NodeDetail snap={snap} connected={connected} onOpen={() => openConsole()} />}
          {sel.kind === "module" && (
            <ModuleDetail
              mod={rail.find((m) => m.id === sel.id)}
              onOpen={(id) => openConsole(id)}
            />
          )}
        </div>
      </div>
    </div>
  );
}

// ---- right-pane detail views ----------------------------------------------

function NodeDetail({ snap, connected, onOpen }: { snap: Snap; connected: boolean; onOpen: () => void }) {
  const { workspace, status } = snap;
  const role = workspace
    ? workspace.founder
      ? "admin · validator"
      : workspace.member
        ? "member · validator"
        : "guest"
    : "—";
  return (
    <>
      <PaneTitle>Node</PaneTitle>
      <Field k="Network" v={workspace?.name ?? "—"} />
      <Field k="Key" v={workspace ? `0x${shortKey(workspace.pubkey)}` : "—"} mono />
      <Field k="Role" v={role} />
      <Field k="Status" v={connected ? "Synced" : "Stopped"} dot={connected ? "#5cb45f" : "#cf6a5e"} />
      <Field k="Height" v={`${(status?.height ?? 0).toLocaleString()}`} mono />
      <Field k="Members" v={`${snap.memberCount}`} />
      <Field k="Modules" v={`${status?.modules.length ?? 0} installed`} />
      <SoftwareBlock />
      <OpenButton onClick={onOpen} />
    </>
  );
}

// Software / version readout. The version comes straight from the Tauri app —
// real, not a placeholder. There is no updater wired up yet, so "Check for
// update" stays purely cosmetic and never fabricates a check result.
function SoftwareBlock() {
  const [version, setVersion] = useState("");
  const [status, setStatus] = useState("");
  const [checkLabel, setCheckLabel] = useState("Check for update");

  useEffect(() => {
    void import("@tauri-apps/api/app").then((m) => m.getVersion()).then(setVersion).catch(() => {});
  }, []);

  const check = () => {
    setCheckLabel("Checking…");
    setStatus("");
    window.setTimeout(() => {
      setCheckLabel("Check for update");
      setStatus("No update channel is configured yet");
    }, 600);
  };

  return (
    <>
      <div style={{ height: 1, background: HAIRLINE, margin: "11px 0 8px" }} />
      <div style={{ font: `600 10.5px ${font.sans}`, color: DIM, marginBottom: 6, letterSpacing: 0.2 }}>SOFTWARE</div>
      <Field k="Version" v={version ? `v${version}` : "—"} mono />
      <GhostButton label={checkLabel} onClick={check} />
      {status && <div style={{ font: `400 10.5px ${font.mono}`, color: DIM, marginTop: 7 }}>{status}</div>}
    </>
  );
}

function ModuleDetail({ mod, onOpen }: { mod: (typeof MODULES)[number] | undefined; onOpen: (id: string) => void }) {
  if (!mod) return <Empty>Module not found</Empty>;
  return (
    <>
      <div style={{ display: "flex", alignItems: "center", gap: 9, marginBottom: 12 }}>
        <span style={{ width: 26, height: 26, borderRadius: 7, flexShrink: 0, background: "rgba(255,255,255,0.10)", display: "flex", alignItems: "center", justifyContent: "center" }}>
          <Icon name={mod.nav.icon} size={16} color={TEXT} />
        </span>
        <div style={{ font: `600 13px ${font.sans}`, color: TEXT }}>{mod.nav.label}</div>
      </div>
      <OpenButton onClick={() => onOpen(mod.id)} />
    </>
  );
}

// ---- small building blocks ------------------------------------------------

function Nav({ children, selected, onClick, pad = "7px 11px" }: { children: ReactNode; selected?: boolean; onClick?: () => void; pad?: string }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        cursor: onClick ? "pointer" : "default",
        boxSizing: "border-box",
        width: "100%",
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: pad,
        font: `500 12px ${font.sans}`,
        color: selected ? "#fff" : TEXT,
        background: selected ? SELBG : hover ? HOVER : "transparent",
        boxShadow: selected ? `inset 2px 0 0 ${accentVar}` : "none",
      }}
    >
      {children}
    </button>
  );
}

function Chip({ text }: { text: string }) {
  return (
    <span
      style={{
        width: 24,
        height: 24,
        borderRadius: 7,
        background: "rgba(255,255,255,0.16)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 12px ${font.mono}`,
        color: "#fff",
        flexShrink: 0,
      }}
    >
      {text}
    </span>
  );
}
function Divider() {
  return <div style={{ height: 1, background: HAIRLINE }} />;
}
function PaneTitle({ children }: { children: ReactNode }) {
  return <div style={{ font: `600 12px ${font.sans}`, color: "#fff", marginBottom: 9 }}>{children}</div>;
}
function Field({ k, v, mono, dot }: { k: string; v: string; mono?: boolean; dot?: string }) {
  return (
    <div style={{ display: "flex", alignItems: "baseline", gap: 8, padding: "3px 0" }}>
      <span style={{ width: 66, flexShrink: 0, font: `400 10.5px ${font.sans}`, color: FAINT }}>{k}</span>
      <span style={{ flex: 1, minWidth: 0, font: mono ? `400 10.5px ${font.mono}` : `400 11.5px ${font.sans}`, color: TEXT, wordBreak: "break-all" }}>
        {dot && <span style={{ display: "inline-block", width: 6, height: 6, borderRadius: "50%", background: dot, marginRight: 5 }} />}
        {v}
      </span>
    </div>
  );
}
function Empty({ children }: { children: ReactNode }) {
  return <div style={{ font: `400 11px ${font.sans}`, color: FAINT, padding: "8px 2px" }}>{children}</div>;
}
// Primary CTA — accent-filled, full width.
function OpenButton({ onClick }: { onClick: () => void }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "block",
        boxSizing: "border-box",
        width: "100%",
        textAlign: "center",
        marginTop: 16,
        padding: "8px 0",
        borderRadius: 8,
        font: `600 12px ${font.sans}`,
        color: "#fff",
        background: accentVar,
        filter: hover ? "brightness(1.1)" : "none",
        transition: "filter .12s",
      }}
    >
      Open in console
    </button>
  );
}

// Secondary — compact outlined ghost.
function GhostButton({ label, onClick }: { label: string; onClick: () => void }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "inline-block",
        marginTop: 9,
        padding: "5px 12px",
        borderRadius: 7,
        font: `600 11px ${font.sans}`,
        color: TEXT,
        background: hover ? "rgba(255,255,255,0.12)" : "rgba(255,255,255,0.05)",
        border: "1px solid rgba(255,255,255,0.16)",
        transition: "background .12s",
      }}
    >
      {label}
    </button>
  );
}
