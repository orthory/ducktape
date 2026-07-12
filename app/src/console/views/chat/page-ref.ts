// The `[[page:<id>]]` reference grammar, mirrored from the module that already
// owns it: crates/apps/runs/src/inject.rs `parse_page_refs`. A run's injected
// page context is parsed there from the SAME message text a chat body renders
// here, so the two must accept exactly the same refs — a chip the console shows
// but the runs module skips (or vice-versa) is a lie about what the agent saw.
//
// The rule, verbatim: scan for `[[page:`, take everything up to the FIRST `]]`,
// and reject the ref when the id is empty or holds whitespace / `[` / `]`.
// A malformed ref is never an error — it just stays literal text. An
// unterminated `[[page:` ends the scan (nothing after it can close).

import type { PageMeta } from "../../../domain/pages-client";

const OPEN = "[[page:";
const CLOSE = "]]";

export type PageRefSegment = { text: string } | { pageId: string };

export const isPageRef = (segment: PageRefSegment): segment is { pageId: string } =>
  "pageId" in segment;

/** Split `text` into literal runs and accepted page refs, in order. The literal
 *  runs keep malformed refs verbatim, so concatenating every segment's source
 *  text reproduces the input. */
export const splitPageRefs = (text: string): PageRefSegment[] => {
  const out: PageRefSegment[] = [];
  let literalFrom = 0; // start of the pending literal run
  let scan = 0; // where the next `[[page:` search begins
  for (;;) {
    const open = text.indexOf(OPEN, scan);
    if (open === -1) break;
    const idFrom = open + OPEN.length;
    const close = text.indexOf(CLOSE, idFrom);
    if (close === -1) break; // unterminated — nothing after this can close it
    const pageId = text.slice(idFrom, close);
    scan = close + CLOSE.length;
    // Malformed: fall through WITHOUT moving `literalFrom`, so the raw text
    // (brackets and all) stays in the literal run — exactly what the runs
    // module does by skipping the ref and continuing past its close.
    if (pageId === "" || /[\s[\]]/.test(pageId)) continue;
    if (open > literalFrom) out.push({ text: text.slice(literalFrom, open) });
    out.push({ pageId });
    literalFrom = scan;
  }
  if (literalFrom < text.length) out.push({ text: text.slice(literalFrom) });
  return out;
};

/** The distinct page ids `text` references, in first-seen order — the same list
 *  the runs module builds for injection. */
export const parsePageRefs = (text: string): string[] => [
  ...new Set(splitPageRefs(text).filter(isPageRef).map((segment) => segment.pageId)),
];

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

/** Replace the typed fragment (`token.start` .. `caret`) with `[[page:<id>]] `
 *  and report where the caret lands afterwards — insertMention's contract. */
export const insertPageRef = (
  text: string,
  token: PageRefToken,
  caret: number,
  pageId: string,
): { text: string; caret: number } => {
  const inserted = `[[page:${pageId}]] `;
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
