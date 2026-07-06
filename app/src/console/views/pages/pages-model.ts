// Pure editor model over the pages module's preorder snapshot. The view keeps
// keystroke handling; everything that can be computed from the flat block
// list lives here: visible rows (depth, collapse, list numbering), markdown
// shortcut detection, and the {parent, after} targets for indent/outdent and
// sibling moves — the exact shapes MoveBlock takes on the wire.

import type { BlockKind, PageBlock } from "../../../domain/pages-client";

/** One visible editor row: a non-root block plus its render facts. */
export interface Row {
  block: PageBlock;
  /** Nesting depth below the page root (top-level blocks are 0). */
  depth: number;
  /** 1-based position within a run of consecutive Numbered siblings; only
   *  set for kind "Numbered" — it is the rendered list number. */
  listIndex?: number;
}

const byId = (blocks: PageBlock[]): Map<string, PageBlock> =>
  new Map(blocks.map((b) => [b.id, b]));

/** Derive the visible rows from the preorder snapshot: the root (the title
 *  surface) is skipped, depth comes from parent links, and any block below a
 *  collapsed Toggle is hidden. Order is the server's preorder, untouched. */
export function buildRows(
  blocks: PageBlock[],
  collapsed: ReadonlySet<string>,
): Row[] {
  const map = byId(blocks);
  const depths = new Map<string, number>();
  const rows: Row[] = [];
  // per-parent run of consecutive Numbered siblings -> the rendered number.
  const numberRuns = new Map<string, number>();

  for (const block of blocks) {
    if (block.parent === null) {
      depths.set(block.id, -1); // the root itself is not a row
      continue;
    }
    const parentDepth = depths.get(block.parent);
    if (parentDepth === undefined) continue; // orphan in a torn snapshot
    depths.set(block.id, parentDepth + 1);

    // hidden when any ancestor toggle is collapsed.
    let ancestor: string | null = block.parent;
    let hidden = false;
    while (ancestor !== null && !hidden) {
      hidden = collapsed.has(ancestor);
      ancestor = map.get(ancestor)?.parent ?? null;
    }
    if (hidden) continue;

    const row: Row = { block, depth: parentDepth + 1 };
    if (block.kind === "numbered") {
      // preorder emits siblings of one parent as a contiguous run only at the
      // same depth, so track the run per parent and reset on any other kind.
      const run = (numberRuns.get(block.parent) ?? 0) + 1;
      numberRuns.set(block.parent, run);
      row.listIndex = run;
    } else {
      numberRuns.set(block.parent, 0);
    }
    rows.push(row);
  }
  return rows;
}

/** A matched markdown prefix: the kind to convert to and the remainder the
 *  draft should keep. */
export interface Shortcut {
  kind: BlockKind;
  rest: string;
}

// longest-first so "### " wins over "## " wins over "# ".
const SHORTCUTS: [string, BlockKind][] = [
  ["[ ] ", "todo"],
  ["### ", "heading3"],
  ["--- ", "divider"],
  ["``` ", "code"],
  ["## ", "heading2"],
  ["[] ", "todo"],
  ["# ", "heading1"],
  ["- ", "bulleted"],
  ["* ", "bulleted"],
  ["1. ", "numbered"],
  ["> ", "quote"],
];

/** Detect a just-typed markdown prefix. The caller applies it only when the
 *  block is still a Paragraph (conversions never chain). */
export function shortcutFor(text: string): Shortcut | null {
  for (const [prefix, kind] of SHORTCUTS) {
    if (text.startsWith(prefix)) return { kind, rest: text.slice(prefix.length) };
  }
  return null;
}

/** The slash-menu catalogue: every insertable kind (Page is CreatePage-only). */
export const SLASH_KINDS: { kind: BlockKind; label: string; hint: string }[] = [
  { kind: "paragraph", label: "Text", hint: "plain paragraph" },
  { kind: "heading1", label: "Heading 1", hint: "# " },
  { kind: "heading2", label: "Heading 2", hint: "## " },
  { kind: "heading3", label: "Heading 3", hint: "### " },
  { kind: "bulleted", label: "Bulleted list", hint: "- " },
  { kind: "numbered", label: "Numbered list", hint: "1. " },
  { kind: "todo", label: "To-do", hint: "[] " },
  { kind: "toggle", label: "Toggle", hint: "collapsible children" },
  { kind: "quote", label: "Quote", hint: "> " },
  { kind: "code", label: "Code", hint: "``` " },
  { kind: "callout", label: "Callout", hint: "highlighted box" },
  { kind: "divider", label: "Divider", hint: "--- " },
];

/** Filter the slash menu by the text typed after "/". */
export const filterSlashKinds = (query: string): typeof SLASH_KINDS =>
  SLASH_KINDS.filter(({ kind, label }) => {
    const q = query.trim().toLowerCase();
    return (
      q.length === 0 ||
      label.toLowerCase().includes(q) ||
      kind.toLowerCase().includes(q)
    );
  });

/** A MoveBlock destination, exactly as the wire wants it. */
export interface MoveTarget {
  parent: string;
  after: string | null;
}

/** Where Tab sends a block: last child of its PREVIOUS sibling. Null when
 *  there is no previous sibling to adopt it (already as deep as it can go). */
export function indentTarget(
  blocks: PageBlock[],
  blockId: string,
): MoveTarget | null {
  const map = byId(blocks);
  const block = map.get(blockId);
  const parent = block?.parent ? map.get(block.parent) : undefined;
  if (!block || !parent) return null;
  const pos = parent.children.indexOf(blockId);
  if (pos <= 0) return null;
  const prevSibling = map.get(parent.children[pos - 1]);
  if (!prevSibling) return null;
  const last = prevSibling.children[prevSibling.children.length - 1] ?? null;
  return { parent: prevSibling.id, after: last };
}

/** Where Shift+Tab sends a block: after its parent, under the grandparent.
 *  Null for top-level blocks (their parent is the page root). */
export function outdentTarget(
  blocks: PageBlock[],
  blockId: string,
): MoveTarget | null {
  const map = byId(blocks);
  const block = map.get(blockId);
  const parent = block?.parent ? map.get(block.parent) : undefined;
  if (!block || !parent || parent.parent === null) return null;
  return { parent: parent.parent, after: parent.id };
}

/** Alt+ArrowUp: swap with the previous sibling. Null when already first. */
export function moveUpTarget(
  blocks: PageBlock[],
  blockId: string,
): MoveTarget | null {
  const map = byId(blocks);
  const block = map.get(blockId);
  const parent = block?.parent ? map.get(block.parent) : undefined;
  if (!block || !parent) return null;
  const pos = parent.children.indexOf(blockId);
  if (pos <= 0) return null;
  // land AFTER the sibling two up, or at the front when becoming first.
  return { parent: parent.id, after: pos >= 2 ? parent.children[pos - 2] : null };
}

/** Alt+ArrowDown: swap with the next sibling. Null when already last. */
export function moveDownTarget(
  blocks: PageBlock[],
  blockId: string,
): MoveTarget | null {
  const map = byId(blocks);
  const block = map.get(blockId);
  const parent = block?.parent ? map.get(block.parent) : undefined;
  if (!block || !parent) return null;
  const pos = parent.children.indexOf(blockId);
  if (pos === -1 || pos === parent.children.length - 1) return null;
  return { parent: parent.id, after: parent.children[pos + 1] };
}

/** List kinds continue on Enter (a fresh sibling keeps the kind); everything
 *  else splits into a plain paragraph. */
export const continuationKind = (kind: BlockKind): BlockKind =>
  kind === "bulleted" || kind === "numbered" || kind === "todo"
    ? kind
    : "paragraph";
