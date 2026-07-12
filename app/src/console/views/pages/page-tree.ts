import type { PageMeta } from "../../../domain/pages-client";

export interface TreeNode {
  id: string;
  title: string;
  depth: number;
  children: TreeNode[];
}

const label = (m: PageMeta) => m.title || "Untitled";

/** Build the folder forest from the flat enumeration. A page whose parent is
 *  missing (or points outside the set) surfaces at the root, so nothing is ever
 *  hidden by a dangling edge. Children are sorted by title (case-insensitive). */
export function buildForest(pages: PageMeta[]): TreeNode[] {
  const byId = new Map(pages.map((p) => [p.id, p]));
  const childrenOf = new Map<string | null, PageMeta[]>();
  for (const p of pages) {
    const parent = p.parent && byId.has(p.parent) ? p.parent : null;
    const list = childrenOf.get(parent) ?? [];
    list.push(p);
    childrenOf.set(parent, list);
  }
  const build = (parent: string | null, depth: number): TreeNode[] =>
    (childrenOf.get(parent) ?? [])
      .slice()
      .sort((a, b) => label(a).toLowerCase().localeCompare(label(b).toLowerCase()))
      .map((p) => ({ id: p.id, title: label(p), depth, children: build(p.id, depth + 1) }));
  return build(null, 0);
}

export interface VisibleRow {
  id: string;
  title: string;
  depth: number;
  hasChildren: boolean;
}

/** Preorder flatten, skipping the subtree of any collapsed node. */
export function flattenVisible(forest: TreeNode[], collapsed: ReadonlySet<string>): VisibleRow[] {
  const out: VisibleRow[] = [];
  const walk = (nodes: TreeNode[]) => {
    for (const n of nodes) {
      out.push({ id: n.id, title: n.title, depth: n.depth, hasChildren: n.children.length > 0 });
      if (n.children.length > 0 && !collapsed.has(n.id)) walk(n.children);
    }
  };
  walk(forest);
  return out;
}

/** A page's ancestry, root-first, INCLUDING the page itself — the breadcrumb.
 *  The header used to fake this with a literal "Pages / <title>", so a page
 *  nested three deep looked top-level. Cycle-safe (a self-parent or a loop from
 *  a torn enumeration ends the walk) and dangling-parent-safe (an unknown
 *  parent just ends the chain, the same way buildForest surfaces the page). */
export function ancestorChain(pages: PageMeta[], id: string): PageMeta[] {
  const byId = new Map(pages.map((p) => [p.id, p]));
  const chain: PageMeta[] = [];
  const seen = new Set<string>();
  let cursor = byId.get(id);
  while (cursor && !seen.has(cursor.id)) {
    seen.add(cursor.id);
    chain.unshift(cursor);
    cursor = cursor.parent ? byId.get(cursor.parent) : undefined;
  }
  return chain;
}

/** Every id in a node's subtree (itself included) — used to forbid moving a
 *  page under one of its own descendants. */
export function subtreeIds(forest: TreeNode[], id: string): Set<string> {
  const ids = new Set<string>();
  const find = (nodes: TreeNode[]): TreeNode | null => {
    for (const n of nodes) {
      if (n.id === id) return n;
      const hit = find(n.children);
      if (hit) return hit;
    }
    return null;
  };
  const collect = (n: TreeNode) => {
    ids.add(n.id);
    n.children.forEach(collect);
  };
  const node = find(forest);
  if (node) collect(node);
  return ids;
}
