// Pasting a document into a block. Without this a markdown paste dumped its
// literal newlines into ONE textarea; now every line becomes the block it looks
// like, through the same `shortcutFor` grammar the editor already types with.
//
// Every block is one consensus op, so a paste is CAPPED: the finalization
// ledger keeps at most MAX_OPS (512) records, and a 500-line paste would be a
// 500-submit burst. The caller surfaces the drop count.

import type { BlockKind } from "../../../domain/pages-client";
import { shortcutFor } from "./pages-model";

/** The most blocks one paste may create. Past this the tail is dropped and the
 *  view says so. */
export const MAX_PASTE_BLOCKS = 60;

export interface PastedBlock {
  kind: BlockKind;
  text: string;
}

export interface PastePlan {
  blocks: PastedBlock[];
  /** Lines beyond the cap, dropped. Zero on an uncapped paste. */
  dropped: number;
}

/** Split pasted text into the blocks it should become. Blank lines are
 *  separators, not empty blocks (a markdown doc is mostly blank lines), and a
 *  line's markdown prefix picks its kind.
 *
 *  ponytail: line-at-a-time, so a fenced code block pastes as a divider-ish
 *  "```" code block followed by its lines as paragraphs, and leading
 *  indentation does not nest. A real markdown parser is the upgrade path. */
export function pastePlan(text: string, limit: number = MAX_PASTE_BLOCKS): PastePlan {
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.replace(/\s+$/, ""))
    .filter((line) => line.trim() !== "");
  const kept = lines.slice(0, Math.max(0, limit));
  return {
    blocks: kept.map((line) => {
      const shortcut = shortcutFor(line);
      return shortcut
        ? { kind: shortcut.kind, text: shortcut.rest }
        : { kind: "paragraph" as BlockKind, text: line };
    }),
    dropped: lines.length - kept.length,
  };
}
