// Read-only view of this node's genesis modules and authenticated roots. There
// is no runtime install/enable API, so every row is informational only.

import { useState } from "react";

import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";

const shortRoot = (hex: string): string =>
  hex.length > 20 ? `${hex.slice(0, 10)}…${hex.slice(-8)}` : hex || "—";

const monoOf = (id: string): string => id.slice(0, 2).toUpperCase();

// Human-facing name + one-line purpose per known genesis module. The node's
// /v1/status only carries {id, root}, so this static map turns cryptic ids into
// a legible module set. Unknown ids fall back to the raw id + a generic note.
const MODULE_INFO: Record<string, { label: string; desc: string }> = {
  chat: { label: "Chat", desc: "Channels, messages, threads, and reactions." },
  tasks: { label: "Tasks", desc: "A shared, ordered task list." },
  forge: { label: "Forge", desc: "A git-backed repository (one commit per block)." },
  document: { label: "Documents", desc: "Block-structured collaborative documents." },
  agent: { label: "Agents", desc: "The agent collaboration loop and run ledger." },
  governance: { label: "Governance", desc: "Validator-set proposals and quorum voting." },
  vaults: { label: "Vaults", desc: "Encrypted team secrets with an owner/reader ACL." },
  valset: { label: "Validator set", desc: "The active validator set backing consensus." },
  profiles: { label: "Profiles", desc: "Display names bound to member public keys." },
  inbox: { label: "Inbox", desc: "Per-member notification queues." },
  automations: { label: "Automations", desc: "Event-triggered rules over module events." },
  jobs: { label: "Jobs", desc: "A consensus-native job / claim board." },
  memory: { label: "Memory", desc: "A shared, filesystem-shaped agent workspace." },
  files: { label: "Files", desc: "Content-addressed file manifests + chunk sync." },
  saga: { label: "Saga", desc: "The deterministic async-RPC ledger behind agents." },
  kv: { label: "KV", desc: "A key-value store (internal scaffold)." },
  directory: { label: "Directory", desc: "An example / demo module (internal)." },
};

const infoOf = (id: string): { label: string; desc: string } =>
  MODULE_INFO[id] ?? { label: id, desc: "A registered genesis module." };

function CorePill() {
  return (
    <span
      style={{
        font: `700 9px ${font.mono}`,
        letterSpacing: ".04em",
        color: color.purple,
        background: "#f1edf5",
        border: "1px solid #ddd2e6",
        borderRadius: 999,
        padding: "3px 8px",
        flexShrink: 0,
      }}
    >
      CORE
    </span>
  );
}

function RootButton({
  root,
  copied,
  onCopy,
}: {
  root: string;
  copied: boolean;
  onCopy: () => void;
}) {
  return (
    <button
      type="button"
      title={root}
      onClick={onCopy}
      style={{
        all: "unset",
        cursor: "pointer",
        minWidth: 0,
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        padding: "4px 8px",
        borderRadius: radius.sm,
        border: `1px solid ${copied ? "#cfe3d7" : color.borderSoft}`,
        background: copied ? "#eef5f0" : color.sunken,
        font: `500 11px ${font.mono}`,
        color: copied ? "#5f9e74" : color.muted3,
      }}
    >
      <span
        style={{
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {copied ? "copied" : shortRoot(root)}
      </span>
    </button>
  );
}

function ModuleRow({
  id,
  root,
  copied,
  onCopy,
}: {
  id: string;
  root: string;
  copied: boolean;
  onCopy: () => void;
}) {
  const info = infoOf(id);
  return (
    <div
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: 13,
        padding: "12px 14px",
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
        boxShadow: shadow.card,
      }}
    >
      <div
        style={{
          width: 40,
          height: 40,
          borderRadius: 10,
          background: color.dark,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          font: `600 13px ${font.mono}`,
          color: color.onDark,
          flexShrink: 0,
        }}
      >
        {monoOf(id)}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
          <span
            style={{
              font: `600 13.5px ${font.sans}`,
              color: color.ink,
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
          >
            {info.label}
          </span>
          <span style={{ font: `400 11px ${font.mono}`, color: color.muted2, flexShrink: 0 }}>
            {id}
          </span>
          <CorePill />
        </div>
        <div
          style={{
            marginTop: 2,
            font: `400 12px ${font.sans}`,
            color: color.muted,
            lineHeight: 1.4,
          }}
        >
          {info.desc}
        </div>
        <div style={{ marginTop: 7, minWidth: 0 }}>
          <RootButton root={root} copied={copied} onCopy={onCopy} />
        </div>
      </div>
    </div>
  );
}

export function ModulesView() {
  const { state } = useDucktape();
  const modules = state.status?.modules ?? [];
  const [copied, setCopied] = useState<string | null>(null);

  const copyRoot = (id: string, root: string) => {
    void navigator.clipboard?.writeText(root).then(
      () => {
        setCopied(id);
        window.setTimeout(() => setCopied((current) => (current === id ? null : current)), 1200);
      },
      () => {},
    );
  };

  return (
    <div
      data-screen-label="Modules"
      style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}
    >
      <div
        style={{
          height: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "0 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Modules</span>
        <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
          {modules.length}
        </span>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "22px 26px" }}>
        <div style={{ display: "flex", alignItems: "flex-start", gap: 11 }}>
          <span
            style={{
              width: 36,
              height: 36,
              borderRadius: 10,
              background: color.sunken,
              border: `1px solid ${color.border}`,
              color: color.muted3,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
            }}
          >
            <Icon name="modules" size={18} />
          </span>
          <div style={{ minWidth: 0 }}>
            <div style={{ font: `600 19px ${font.sans}`, color: color.dark }}>
              Node module set
            </div>
            <div
              style={{
                font: `400 13px ${font.sans}`,
                color: color.muted,
                marginTop: 3,
                lineHeight: 1.45,
              }}
            >
              These are the genesis modules this node runs, with each module's
              committed Merkle root.
            </div>
          </div>
        </div>

        <div style={{ marginTop: 18, display: "flex", alignItems: "center", gap: 12 }}>
          <div
            style={{
              font: `600 10px ${font.mono}`,
              letterSpacing: ".13em",
              color: color.muted2,
            }}
          >
            INSTALLED
          </div>
          <div style={{ flex: 1, height: 1, background: color.borderSoft }} />
          <div style={{ font: `400 11px ${font.mono}`, color: color.muted2 }}>
            {modules.length} core
          </div>
        </div>

        <div
          style={{
            marginTop: 11,
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))",
            gap: 10,
          }}
        >
          {modules.map((mod) => (
            <ModuleRow
              key={mod.id}
              id={mod.id}
              root={mod.root}
              copied={copied === mod.id}
              onCopy={() => copyRoot(mod.id, mod.root)}
            />
          ))}
        </div>

        {modules.length === 0 && (
          <div
            style={{
              marginTop: 18,
              border: `1px dashed ${color.border}`,
              borderRadius: radius.lg,
              padding: 30,
              textAlign: "center",
              font: `400 13px ${font.sans}`,
              color: color.muted2,
            }}
          >
            Waiting for module roots from the node.
          </div>
        )}
      </div>
    </div>
  );
}
