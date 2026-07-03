import { Handle, Position, type NodeProps } from "@xyflow/react";
import { isCollapsed, type WorktreeGraphNode } from "../layout";
import { AppTile } from "./AppTile";

// xyflow custom node: the live AppTile with LEFT/RIGHT connectors for the
// horizontal tree. Non-running worktrees render collapsed + disabled (no
// screen). RFB keys off node.token, so re-layouts don't reconnect the stream.
export function WorktreeNode({ data }: NodeProps<WorktreeGraphNode>) {
  const collapsed = isCollapsed(data.node);
  return (
    <div className="graph-node" data-collapsed={collapsed || undefined}>
      <Handle type="target" position={Position.Left} className="gh" />
      <AppTile node={data.node} onOpen={data.onOpen} collapsed={collapsed} />
      <Handle type="source" position={Position.Right} className="gh" />
    </div>
  );
}
