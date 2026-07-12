// Node-local operational facts — the operator-rail home for what Settings
// used to duplicate: this node's key and which account owns it, where the
// workspace lives on disk, which ports the node binds, and the quorum the
// validator set needs. Read-only projections of the active workspace and
// roster, plus ONE write: an unbound node's Owned by row offers Bind — the
// manual escape hatch when connect-time auto-bind returned locked/deferred/
// failed and nothing would ever retry. (The node key lives HERE, on the
// node's page — it is not "your identity"; the person is the Account view's
// business.)

import { useState, type ReactNode } from "react";

import { normalizeKey, shortKey } from "../../../domain/names";
import type { AutoBindResult } from "../../store/auto-bind";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { copyText, HoverButton, outlineButton } from "../settings/parts";

const workspaceDataDir = (id: string): string => `~/.ducktape/workspaces/${id}`;

const quorumText = (count: number): string => {
  if (count <= 0) return "not exposed";
  const threshold = Math.floor((count * 2) / 3) + 1;
  return `${threshold} of ${count} validator${count === 1 ? "" : "s"}`;
};

/** The reason a manual bind didn't land, in the operator's language. A landed
 *  (or already-landed) bind needs no message — the refreshed identity
 *  projection repaints the Owned by row. */
const bindOutcomeMessage = (outcome: AutoBindResult): string | null => {
  switch (outcome) {
    case "bound":
    case "already":
      return null;
    case "locked":
      return "your identity is locked — unlock it on the Home screen, then bind again";
    case "deferred":
      return "a device link is pending — approve this key from your other device first";
    case "skipped":
      return "binding needs the desktop shell (there is no machine key to sign with here)";
    case "failed":
      return "bind failed — check the node connection, then try again";
  }
};

function FactRow({
  label,
  value,
  last,
  action,
}: {
  label: string;
  value: string;
  last?: boolean;
  action?: ReactNode;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 16,
        padding: "11px 15px",
        borderBottom: last ? undefined : `1px solid ${color.borderSoft}`,
      }}
    >
      <span style={{ font: `500 12px ${font.sans}`, color: color.inkSoft }}>
        {label}
      </span>
      <span
        style={{
          marginLeft: "auto",
          minWidth: 0,
          font: `400 11.5px ${font.mono}`,
          color: color.muted,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title={value}
      >
        {value}
      </span>
      {action}
    </div>
  );
}

export function NodeFactsCard() {
  const { state, actions } = useDucktape();
  const [binding, setBinding] = useState(false);
  const [bindMessage, setBindMessage] = useState<string | null>(null);
  const workspace = state.workspace;
  const validatorCount = state.members.length || (workspace?.member ? 1 : 0);
  const portLine = workspace
    ? `p2p ${workspace.ports.listen} · http ${workspace.ports.http} · rpc ${workspace.ports.rpc}`
    : "not available";
  const nodeKey = workspace?.pubkey ?? "";
  const owner = nodeKey ? state.nodeUsers[normalizeKey(nodeKey)] : undefined;
  // Identity is the sole replicated account-name authority.
  const ownerName = owner?.name ?? null;
  const ownerLine = owner
    ? ownerName
      ? `${ownerName} · ${shortKey(owner.accountId)}`
      : shortKey(owner.accountId)
    : "not linked to an account";

  const bind = () => {
    setBinding(true);
    setBindMessage(null);
    actions
      .accountBindNode()
      .then((outcome) => setBindMessage(bindOutcomeMessage(outcome)))
      .catch((err: unknown) =>
        setBindMessage(err instanceof Error ? err.message : String(err)),
      )
      .finally(() => setBinding(false));
  };

  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        overflow: "hidden",
      }}
    >
      <button
        type="button"
        onClick={() => nodeKey && copyText(nodeKey)}
        title={nodeKey ? "Copy node key" : undefined}
        style={{ all: "unset", cursor: nodeKey ? "pointer" : "default", display: "block", width: "100%", boxSizing: "border-box" }}
      >
        <FactRow label="Node key" value={nodeKey || "not available"} />
      </button>
      <FactRow
        label="Owned by"
        value={ownerLine}
        action={
          !owner && workspace ? (
            <HoverButton
              ariaLabel="Bind this node"
              onClick={bind}
              disabled={binding}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              {binding ? "Binding…" : "Bind"}
            </HoverButton>
          ) : undefined
        }
      />
      {!owner && bindMessage && (
        <div
          role="alert"
          style={{
            padding: "0 15px 10px",
            font: `500 10.5px ${font.sans}`,
            color: color.danger,
          }}
        >
          {bindMessage}
        </div>
      )}
      <FactRow
        label="Data dir"
        value={workspace ? workspaceDataDir(workspace.id) : "not available"}
      />
      <FactRow label="Ports" value={portLine} />
      <FactRow label="Quorum threshold" value={quorumText(validatorCount)} last />
    </div>
  );
}
