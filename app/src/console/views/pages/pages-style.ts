// Editor geometry, in one place so the title input and the block rows cannot
// drift out of alignment again.

import type { BlockKind } from "../../../domain/pages-client";

/** One nesting level, in px. */
export const INDENT = 26;

/** How far a list marker hangs into the left margin.
 *
 *  Rows used to reserve this space *in flow* (a 20px box + an 8px gap) for
 *  every kind — including paragraphs and headings, which render no marker at
 *  all. The result was that all prose sat 28px right of the page title, so the
 *  document had no single left edge. The marker now hangs out of flow at
 *  `left: -MARKER_HANG` and the text column starts at offset 0, flush with the
 *  title. Bullets, numbers, checkboxes and chevrons land exactly where they
 *  did relative to their own text. */
export const MARKER_HANG = 28;

/** Breathing room above a heading. Rows are otherwise uniformly spaced, which
 *  made an H1 sit as tight against the paragraph above it as another
 *  paragraph would. */
export const headingTopSpace = (kind: BlockKind): number =>
  kind === "heading1" ? 24 : kind === "heading2" ? 18 : kind === "heading3" ? 12 : 0;
