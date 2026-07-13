// Pure editor model over the pages module's preorder snapshot. The view keeps
// keystroke handling; everything that can be computed from the flat block
// list lives here: visible rows (depth, collapse, list numbering), markdown
// shortcut detection, and the {parent, after} targets for indent/outdent and
// sibling moves — the exact shapes MoveBlock takes on the wire.

import type { BlockKind, PageBlock } from "../../../domain/pages-client";

/** A pause this long while typing is one edit boundary — one consensus op.
 *  Shared by the block rows and the title input; exported for the tests that
 *  drive the boundary timer. */
export const EDIT_BOUNDARY_MS = 700;

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
  ["## ", "heading2"],
  ["[] ", "todo"],
  ["# ", "heading1"],
  ["- ", "bulleted"],
  ["* ", "bulleted"],
  ["1. ", "numbered"],
  ["> ", "quote"],
];

// Tokens that convert the moment they are COMPLETE, with no trailing space —
// there is no text after them to keep. They used to be listed as prefixes
// ("--- ", "``` "), which meant the divider and the code block only appeared if
// you typed a space onto the end of a rule or a fence. Nobody does, so in
// practice neither shortcut existed.
const EXACT: [string, BlockKind][] = [
  ["---", "divider"],
  ["***", "divider"],
  ["```", "code"],
];

/** Detect a just-typed markdown prefix. The caller applies it only when the
 *  block is still a Paragraph (conversions never chain). */
export function shortcutFor(text: string): Shortcut | null {
  for (const [token, kind] of EXACT) {
    if (text === token) return { kind, rest: "" };
  }
  for (const [prefix, kind] of SHORTCUTS) {
    if (text.startsWith(prefix)) return { kind, rest: text.slice(prefix.length) };
  }
  return null;
}

/** The slash-menu catalogue: every insertable kind, plus Page. */
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
  // not a conversion: picking it creates a child page (see BlockRow.pickSlash).
  // Last on purpose — text/heading muscle memory owns the top of the menu.
  { kind: "page", label: "Page", hint: "new subpage" },
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
 *  there is no previous sibling to adopt it (already as deep as it can go), or
 *  when that sibling is a DIVIDER.
 *
 *  A divider is a horizontal rule, not a container: it renders no textarea, has
 *  no disclosure, and holds no text a child could belong to. The wire would take
 *  it — MoveBlock validates only page-match and cycles — and that was a hole with
 *  teeth. Tab under a divider re-parented the block you were typing in UNDER the
 *  rule; the caret's row then sat directly below it, so Backspace at offset 0 read
 *  as "remove the divider above" and RemoveBlock took the divider's whole subtree:
 *  the rule, your block, and everything nested under it. Refusing the indent shuts
 *  the door; removeDividerAbove's children guard (use-row-handlers) is the second
 *  lock, for the dividers that already adopted children before this landed. */
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
  if (!prevSibling || prevSibling.kind === "divider") return null;
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

/** One InsertBlock op plus the `checked` bit, which InsertBlock does not carry
 *  (the caller follows with SetChecked). Shared by duplicatePlan (fresh ids) and
 *  subtreePlan (original ids, for delete-undo). */
export interface DuplicateOp extends MoveTarget {
  blockId: string;
  kind: BlockKind;
  text: string;
  checked: boolean;
}

/** Preorder-emit a block and its subtree as InsertBlock ops: each block's id
 *  comes from `idFor`, the root anchors `after` `rootAfter`, and every child
 *  anchors after the previously-emitted sibling. Empty when the block is unknown
 *  or the page root. Capped, because every op is a consensus submit. */
function planSubtree(
  blocks: PageBlock[],
  blockId: string,
  idFor: (srcId: string) => string,
  rootAfter: string | null,
  limit: number,
): DuplicateOp[] {
  const map = byId(blocks);
  const source = map.get(blockId);
  if (!source || !source.parent) return [];

  const ops: DuplicateOp[] = [];
  const emit = (srcId: string, parent: string, after: string | null): string | null => {
    if (ops.length >= limit) return null;
    const block = map.get(srcId);
    if (!block) return null;
    const id = idFor(srcId);
    ops.push({
      blockId: id,
      parent,
      after,
      kind: block.kind,
      text: block.text,
      // `checked` only means anything on a to-do. SetKind does NOT reset the bit
      // in the module, so a to-do that was checked and later converted still
      // carries checked=true — and replaying that as SetChecked on a paragraph
      // is a NotTodo rejection (a spurious error toast on an otherwise clean
      // duplicate or undo). Drop it here, once, for both plans.
      checked: block.kind === "todo" && block.checked,
    });
    let prev: string | null = null;
    for (const child of block.children) prev = emit(child, id, prev) ?? prev;
    return id;
  };
  emit(blockId, source.parent, rootAfter);
  return ops;
}

/** Deep-copy a block and its subtree: preorder inserts with fresh ids, the copy
 *  landing directly after the original. Empty when the block is unknown or is
 *  the page root (a page duplicates through the page module, not here).
 *
 *  Capped, because every op is a consensus submit — same ceiling as a paste. */
export function duplicatePlan(
  blocks: PageBlock[],
  blockId: string,
  mintId: () => string,
  limit = 60,
): DuplicateOp[] {
  return planSubtree(blocks, blockId, () => mintId(), blockId, limit);
}

/** Snapshot a block and its subtree so a delete can be undone: preorder inserts
 *  that PRESERVE the original ids (client-minted and free to reuse once the
 *  remove commits) and the `checked` bit, re-anchoring the root at its original
 *  position (after its previous sibling). Empty when the block is unknown, is
 *  the page root, or the subtree exceeds `limit` — a partial restore is worse
 *  than none, so an over-cap subtree refuses outright rather than truncating.
 *  Same consensus-op ceiling duplicatePlan and paste honor. */
export function subtreePlan(
  blocks: PageBlock[],
  blockId: string,
  limit = 60,
): DuplicateOp[] {
  const map = byId(blocks);
  const siblings = map.get(map.get(blockId)?.parent ?? "")?.children ?? [];
  const pos = siblings.indexOf(blockId);
  const after = pos > 0 ? siblings[pos - 1] : null;
  // Ask for one past the ceiling: a plan that comes back longer than `limit`
  // means the subtree overflowed it, and gets refused (no partial restore).
  const plan = planSubtree(blocks, blockId, (id) => id, after, limit + 1);
  return plan.length > limit ? [] : plan;
}

/** List kinds continue on Enter (a fresh sibling keeps the kind); everything
 *  else splits into a plain paragraph. */
export const continuationKind = (kind: BlockKind): BlockKind =>
  kind === "bulleted" || kind === "numbered" || kind === "todo"
    ? kind
    : "paragraph";

/** Kinds you escape by pressing Enter on an empty one, rather than nesting
 *  another below. This used to be inferred from `continuationKind(k) === k`,
 *  which is only true of the three list kinds — so an empty quote or callout
 *  stayed put and grew a paragraph underneath it instead of becoming one. */
export const emptyEnterExits = (kind: BlockKind): boolean =>
  kind === "bulleted" ||
  kind === "numbered" ||
  kind === "todo" ||
  kind === "quote" ||
  kind === "code" ||
  kind === "callout" ||
  kind === "toggle";
