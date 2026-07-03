import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { WorktreeGraphNode } from "../layout";
import { AppTile } from "./AppTile";

// xyflow custom node: the same live AppTile, with branch-tree connectors. RFB
// keys off node.token (stable), so graph re-layouts don't reconnect the stream.
export function WorktreeNode({ data }: NodeProps<WorktreeGraphNode>) {
  return (
    <div className="graph-node">
      <Handle type="target" position={Position.Top} className="gh" />
      <AppTile node={data.node} onOpen={data.onOpen} />
      <Handle type="source" position={Position.Bottom} className="gh" />
    </div>
  );
}
