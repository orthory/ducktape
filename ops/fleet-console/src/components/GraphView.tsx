import { useMemo } from "react";
import { ReactFlow, Background, Controls, type NodeTypes } from "@xyflow/react";
import type { FleetNode } from "../types";
import { toGraph } from "../layout";
import { WorktreeNode } from "./WorktreeNode";

const nodeTypes: NodeTypes = { worktree: WorktreeNode };

// The branch-tree layout: same live tiles, arranged by git relationship
// (base at the root, worktrees fanned beneath). Read-only canvas.
export function GraphView({
  nodes,
  base,
  onOpen,
}: {
  nodes: FleetNode[];
  base: string;
  onOpen: (n: FleetNode) => void;
}) {
  const { rfNodes, rfEdges } = useMemo(
    () => toGraph(nodes, base, onOpen),
    [nodes, base, onOpen],
  );

  if (nodes.length === 0) {
    return (
      <div className="empty">
        No worktrees in the fleet yet — run <code>ops/fleet.sh up</code>.
      </div>
    );
  }

  return (
    <div className="graph">
      <ReactFlow
        nodes={rfNodes}
        edges={rfEdges}
        nodeTypes={nodeTypes}
        fitView
        minZoom={0.2}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable={false}
        proOptions={{ hideAttribution: true }}
      >
        <Background gap={22} />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  );
}
