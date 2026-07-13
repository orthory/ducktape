// Editor geometry and per-kind typography, in one place so the title input and
// the block rows cannot drift out of alignment again.

import type { BlockKind } from "../../../domain/pages-client";
import { font } from "../../theme/tokens";

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

/** Vertical padding on every row. The hanging marker offsets from the row's
 *  padding box, so it must add this back to sit on the same line as the text. */
export const ROW_PAD_Y = 2.5;

/** The hover affordances (BlockGutter) and the finalization/comment tray hang
 *  OUT of the row on either side, reserving no column width. */
export const GUTTER_WIDTH = 46;

/** The document column's horizontal padding. It has to house the left gutter
 *  (which sits beyond the hanging marker) or the scroll container clips it —
 *  so the two constants are derived, not guessed, and cannot drift apart. */
export const COLUMN_PAD_X = MARKER_HANG + GUTTER_WIDTH + 6;

/** Breathing room above a heading. Rows are otherwise uniformly spaced, which
 *  made an H1 sit as tight against the paragraph above it as another
 *  paragraph would. */
export const headingTopSpace = (kind: BlockKind): number =>
  kind === "heading1" ? 24 : kind === "heading2" ? 18 : kind === "heading3" ? 12 : 0;

/** Per-kind typography for the block textarea. The code block's line-height is
 *  load-bearing: its line-number gutter renders in the same font, so the two
 *  columns only stay in step while they share it. */
export const kindFont = (kind: BlockKind): string => {
  switch (kind) {
    case "heading1":
      return `700 26px/1.25 ${font.sans}`;
    case "heading2":
      return `700 20px/1.3 ${font.sans}`;
    case "heading3":
      return `650 17px/1.4 ${font.sans}`;
    case "code":
      return `400 13px/1.7 ${font.mono}`;
    default:
      return `400 15px/1.75 ${font.sans}`;
  }
};

/** What an empty block says it is. A block used to name itself only while
 *  FOCUSED, so an empty heading or list item at rest was an invisible row you
 *  could only find by clicking blind. */
export const kindPlaceholder = (kind: BlockKind): string => {
  switch (kind) {
    case "heading1":
      return "Heading 1";
    case "heading2":
      return "Heading 2";
    case "heading3":
      return "Heading 3";
    case "todo":
      return "To-do";
    case "bulleted":
    case "numbered":
      return "List item";
    case "toggle":
      return "Toggle";
    case "quote":
      return "Quote";
    case "code":
      return "Code";
    case "callout":
      return "Callout";
    default:
      return "Write, or press '/' for commands";
  }
};

/** The placeholder an UNFOCUSED empty block shows. Every kind names itself —
 *  except a paragraph, whose "write here" prompt belongs to the caret alone
 *  (a page of empty paragraphs all shouting it is noise, not a document). */
export const restPlaceholder = (kind: BlockKind): string =>
  kind === "paragraph" ? "" : kindPlaceholder(kind);
