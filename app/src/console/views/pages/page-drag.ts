// Drag-to-reorder, as a pure function of the block tree.
//
// A drop is one MoveBlock — the wire op already reparents, reorders and rejects
// cycles — so the whole gesture costs exactly one consensus op, computed here
// from "which row am I over, and which half of it".

import type { PageBlock } from "../../../domain/pages-client";
import type { MoveTarget } from "./pages-model";

/** The drag's signature on the dataTransfer. A row only accepts a drag that
 *  carries it, so a file — or a text selection dragged in from another app —
 *  can never reorder the document. */
export const DRAG_MIME = "application/x-ducktape-block";

/** Which half of the hovered row the pointer is in: the block lands before or
 *  after it, as a sibling. */
export type DropEdge = "before" | "after";

/** A block and everything under it. A drop into your own subtree is the cycle
 *  the module would reject — catch it here so the indicator never invites it. */
export function blockSubtree(blocks: PageBlock[], id: string): Set<string> {
  const map = new Map(blocks.map((b) => [b.id, b]));
  const ids = new Set<string>();
  const walk = (blockId: string) => {
    if (ids.has(blockId)) return; // a torn snapshot must not hang the walk
    ids.add(blockId);
    for (const child of map.get(blockId)?.children ?? []) walk(child);
  };
  walk(id);
  return ids;
}

/** The MoveBlock a drop means, or null when it is illegal (into the dragged
 *  block's own subtree) or a no-op (it is already exactly there — a redundant
 *  consensus op is worth one map lookup to avoid).
 *
 *  ponytail: a drop always lands the block as a SIBLING of the hovered row, at
 *  that row's depth. Notion re-parents from the pointer's horizontal offset;
 *  Tab/Shift+Tab already do that here. */
export function dropTarget(
  blocks: PageBlock[],
  dragId: string,
  overId: string,
  edge: DropEdge,
): MoveTarget | null {
  const map = new Map(blocks.map((b) => [b.id, b]));
  const over = map.get(overId);
  const dragged = map.get(dragId);
  if (!over || !dragged || !over.parent) return null;

  const subtree = blockSubtree(blocks, dragId);
  if (subtree.has(overId) || subtree.has(over.parent)) return null;

  const parent = map.get(over.parent);
  if (!parent) return null;
  const pos = parent.children.indexOf(overId);
  if (pos === -1) return null;

  const after = edge === "after" ? overId : (parent.children[pos - 1] ?? null);
  if (after === dragId) return null; // already directly there
  // dropping below a row the block already follows is the same no-op.
  if (
    edge === "after" &&
    dragged.parent === parent.id &&
    parent.children[pos + 1] === dragId
  )
    return null;

  return { parent: parent.id, after };
}
