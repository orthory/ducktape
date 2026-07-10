// Read-only view of this node's genesis modules and authenticated roots. There
// is no runtime install/enable API, so every row is informational only.

import { useState } from "react";

import type { ModuleCategory, ModuleStatus } from "../../../domain/transport";
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
  agent: { label: "Agents", desc: "The agent collaboration loop and run ledger." },
  governance: { label: "Governance", desc: "Validator-set proposals and quorum voting." },
  vaults: { label: "Vaults", desc: "Encrypted team secrets with an owner/reader ACL." },
  valset: { label: "Validator set", desc: "The active validator set backing consensus." },
  inbox: { label: "Inbox", desc: "Per-member notification queues." },
  automations: { label: "Automations", desc: "Event-triggered rules over module events." },
  jobs: { label: "Jobs", desc: "A consensus-native job / claim board." },
  files: { label: "Files", desc: "A copy-on-write, content-addressed filesystem (duckfs)." },
  saga: { label: "Saga", desc: "The deterministic async-RPC ledger behind agents." },
  identity: { label: "Identity", desc: "Accounts, member keys, and node bindings." },
  duckdns: { label: "DuckDNS", desc: "Optional global .duck handles resolved to accounts." },
  gateway: { label: "Gateway", desc: "Signed account routes to DuckFS or local HTTP." },
  kv: { label: "KV", desc: "A key-value store (internal scaffold)." },
  directory: { label: "Directory", desc: "An example / demo module (internal)." },
};

const infoOf = (id: string): { label: string; desc: string } =>
  MODULE_INFO[id] ?? { label: id, desc: "A registered genesis module." };

// Presentation catalog for module categories: display order + the accent that
// colors each group's section header. The category itself is authored by the
// node (see ModuleCategory in domain/transport); this map only styles it.
const CATEGORIES: Record<ModuleCategory, { label: string; order: number; accent: string }> = {
  workspace: { label: "Workspace", order: 0, accent: color.blue },
  developer: { label: "Developer", order: 1, accent: color.purple },
  automation: { label: "Automation", order: 2, accent: color.amber },
  system: { label: "System", order: 3, accent: color.muted3 },
};

// An absent (older node) or unrecognized category groups under System, so the
// view always renders every module somewhere.
const categoryOf = (mod: ModuleStatus): ModuleCategory =>
  mod.category && mod.category in CATEGORIES ? mod.category : "system";

function CategoryDivider({ category, count }: { category: ModuleCategory; count: number }) {
  const cat = CATEGORIES[category];
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
      <div style={{ font: `600 10px ${font.mono}`, letterSpacing: ".13em", color: cat.accent }}>
        {cat.label.toUpperCase()}
      </div>
      <div style={{ flex: 1, height: 1, background: color.borderSoft }} />
      <div style={{ font: `400 11px ${font.mono}`, color: color.muted2 }}>{count}</div>
    </div>
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

  // Group the module set by category, in catalog order, dropping empty groups.
  const grouped = (Object.keys(CATEGORIES) as ModuleCategory[])
    .sort((a, b) => CATEGORIES[a].order - CATEGORIES[b].order)
    .map((category) => ({
      category,
      mods: modules.filter((mod) => categoryOf(mod) === category),
    }))
    .filter((group) => group.mods.length > 0);

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

        {grouped.map(({ category, mods }) => (
          <section key={category} style={{ marginTop: 18 }}>
            <CategoryDivider category={category} count={mods.length} />
            <div
              style={{
                marginTop: 11,
                display: "grid",
                // auto-fill (not auto-fit) keeps empty tracks so every category
                // section shares the same column count and card width; sparse
                // sections leave trailing space instead of stretching cards wide.
                gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))",
                gap: 10,
              }}
            >
              {mods.map((mod) => (
                <ModuleRow
                  key={mod.id}
                  id={mod.id}
                  root={mod.root}
                  copied={copied === mod.id}
                  onCopy={() => copyRoot(mod.id, mod.root)}
                />
              ))}
            </div>
          </section>
        ))}

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
