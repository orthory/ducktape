import type { Node, Edge } from "@xyflow/react";
import type { FleetNode } from "./types";

// Data carried by each graph node. Must be an index signature for xyflow's
// Node<data> constraint; functions are fine at runtime.
export interface WorktreeNodeData extends Record<string, unknown> {
  node: FleetNode;
  onOpen: (n: FleetNode) => void;
}

export type WorktreeGraphNode = Node<WorktreeNodeData, "worktree">;

// A worktree is "collapsed" in the graph unless its app is live — non-running
// nodes carry no screen, so they render short.
export const isCollapsed = (n: FleetNode): boolean => n.status !== "up";

const NODE_W = 340;
const COL_GAP = 110; // horizontal gap between the base column and children
const H_LIVE = 272; // header + pinned 190px screen + footer (matches css cap)
const H_COLLAPSED = 78; // header + activity only (matches css cap)
const V_GAP = 18;

const heightOf = (n: FleetNode): number =>
  isCollapsed(n) ? H_COLLAPSED : H_LIVE;

// Pure: turn the fleet into a HORIZONTAL branch tree — base on the left, every
// other worktree stacked to its right, edge base→child (animated when live).
// Children are packed by their actual heights so collapsed nodes sit tight.
export function toGraph(
  nodes: FleetNode[],
  base: string,
  onOpen: (n: FleetNode) => void,
): { rfNodes: WorktreeGraphNode[]; rfEdges: Edge[] } {
  const baseNode = nodes.find((n) => n.branch === base);
  const children = nodes.filter((n) => n.branch !== base);
  const rfNodes: WorktreeGraphNode[] = [];
  const rfEdges: Edge[] = [];

  // stack children vertically on the right, packed by height
  let y = 0;
  const placed = children.map((c) => {
    const at = y;
    y += heightOf(c) + V_GAP;
    return { c, y: at };
  });
  const childrenSpan = Math.max(y - V_GAP, 0);
  const childX = NODE_W + COL_GAP;

  if (baseNode) {
    const baseY = Math.max((childrenSpan - heightOf(baseNode)) / 2, 0);
    rfNodes.push({
      id: baseNode.id,
      type: "worktree",
      position: { x: 0, y: baseY },
      data: { node: baseNode, onOpen },
    });
  }
  placed.forEach(({ c, y: cy }) => {
    rfNodes.push({
      id: c.id,
      type: "worktree",
      position: { x: childX, y: cy },
      data: { node: c, onOpen },
    });
    if (baseNode) {
      rfEdges.push({
        id: `${baseNode.id}-${c.id}`,
        source: baseNode.id,
        target: c.id,
        animated: c.status === "up",
      });
    }
  });

  return { rfNodes, rfEdges };
}
