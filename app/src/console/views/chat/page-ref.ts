// The composer's `[[` page typeahead. The `[[` is a shortcut trigger only —
// the INSERTED form is a markdown reference, `[title](duck://page/<id>)`, the
// unified duck:// grammar tokenized in `duck-ref.ts` (and, in a follow-up, the
// same grammar the runs module will parse for page-context injection). The old
// `[[page:<id>]]` token grammar is retired.

import type { PageMeta } from "../../../domain/pages-client";

import { pageRefMarkdown } from "./duck-ref";

// ── The composer's `[[` typeahead ───────────────────────

export interface PageRefToken {
  /** Index of the opening `[[` in the composer text. */
  start: number;
  /** What was typed after the `[[` so far (may be empty). */
  query: string;
}

/** The in-progress `[[` token ending at `caret`, or null. Unlike the @mention
 *  token (mention.ts), whitespace does NOT close this one — page titles contain
 *  spaces and the query matches on title. A `]` (the ref already closed) or a
 *  newline does close it, and so does a second `[[`, since `lastIndexOf` always
 *  takes the nearest opener. */
export const pageRefTokenAt = (text: string, caret: number): PageRefToken | null => {
  const upto = text.slice(0, caret);
  const start = upto.lastIndexOf("[[");
  if (start === -1) return null;
  const query = upto.slice(start + 2);
  if (/[[\]\n]/.test(query)) return null;
  return { start, query };
};

/** Replace the typed `[[…` fragment (`token.start` .. `caret`) with the
 *  markdown page ref `[title](duck://page/<id>)` and report where the caret
 *  lands afterwards — insertMention's contract. */
export const insertPageRef = (
  text: string,
  token: PageRefToken,
  caret: number,
  page: { id: string; title: string },
): { text: string; caret: number } => {
  const inserted = `${pageRefMarkdown(page.id, page.title)} `;
  return {
    text: text.slice(0, token.start) + inserted + text.slice(caret),
    caret: token.start + inserted.length,
  };
};

/** Pages matching `query` on title (case-insensitive), prefix matches first —
 *  the mention typeahead's ranking rule, on the title instead of the handle.
 *  An empty query lists everything, so a bare `[[` opens the full picker. */
export const pageRefCandidates = (pages: PageMeta[], query: string): PageMeta[] => {
  const q = query.trim().toLowerCase();
  const titleOf = (page: PageMeta) => (page.title || page.id).toLowerCase();
  return pages
    .filter((page) => titleOf(page).includes(q) || page.id.toLowerCase().includes(q))
    .sort((a, b) => {
      const rank = Number(titleOf(b).startsWith(q)) - Number(titleOf(a).startsWith(q));
      return rank !== 0 ? rank : titleOf(a).localeCompare(titleOf(b));
    });
};
