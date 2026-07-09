// Node-local operational facts — the operator-rail home for what Settings
// used to duplicate: this node's key and which account owns it, where the
// workspace lives on disk, which ports the node binds, and the quorum the
// validator set needs. Read-only projections of the active workspace and
// roster. (The node key lives HERE, on the node's page — it is not "your
// identity"; the person is the Account view's business.)

import { normalizeKey, shortKey } from "../../../domain/names";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { copyText } from "../settings/parts";

const workspaceDataDir = (id: string): string => `~/.ducktape/workspaces/${id}`;

const quorumText = (count: number): string => {
  if (count <= 0) return "not exposed";
  const threshold = Math.floor((count * 2) / 3) + 1;
  return `${threshold} of ${count} validator${count === 1 ? "" : "s"}`;
};

function FactRow({
  label,
  value,
  last,
}: {
  label: string;
  value: string;
  last?: boolean;
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
    </div>
  );
}

export function NodeFactsCard() {
  const { state } = useDucktape();
  const workspace = state.workspace;
  const validatorCount = state.members.length || (workspace?.member ? 1 : 0);
  const portLine = workspace
    ? `p2p ${workspace.ports.listen} · http ${workspace.ports.http} · rpc ${workspace.ports.rpc}`
    : "not available";
  const nodeKey = workspace?.pubkey ?? "";
  const owner = nodeKey ? state.nodeUsers[normalizeKey(nodeKey)] : undefined;
  // Prefer the account's display name, then the profiles overlay (a name set
  // before the bind landed) — never render the account id twice.
  const ownerName = owner
    ? (owner.name ?? state.authorNames[normalizeKey(nodeKey)] ?? null)
    : null;
  const ownerLine = owner
    ? ownerName
      ? `${ownerName} · ${shortKey(owner.accountId)}`
      : shortKey(owner.accountId)
    : "not linked to an account";

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
      <FactRow label="Owned by" value={ownerLine} />
      <FactRow
        label="Data dir"
        value={workspace ? workspaceDataDir(workspace.id) : "not available"}
      />
      <FactRow label="Ports" value={portLine} />
      <FactRow label="Quorum threshold" value={quorumText(validatorCount)} last />
    </div>
  );
}
