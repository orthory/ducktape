// The ONE reference grammar for chat/comment bodies: standard markdown link
// and image syntax over module-plane `duck://` URIs. One tokenizer renders
// them all; the module table (which URIs are real, what they mean) lives in
// the protocol core, `domain/duck-uri.ts`:
//
//   [label](duck://page/<id>)          → a page chip (opens the page)
//   [label](duck://files/<path>)       → a file chip (downloads on click)
//   ![label](duck://files/<path>)      → an attachment embed (image preview,
//                                        or a download chip if not an image)
//   [label](duck://forge/<repo>[/<n>]) → a forge chip (opens the repo/item)
//   [label](duck://channel/<id>[#seq]) → a channel chip (opens the channel)
//
// References ride as PLAIN markdown text on the wire (the composer inserts the
// literal `[..](..)` / `![..](..)`; the chat markdown parser leaves `duck://`
// links alone — it only marks `https?://` — so they reach the renderer as
// text). Keeping the URL in the text (not a mark) means the agent runtime can
// parse the same grammar from the message it sees.
//
// Security note carried over from the attachment work: FILE refs are confined
// to `/shared/attachments/<dir>/<name>` — classify is the only guard against a
// crafted ref steering a client read at another duckfs path (reads are not
// authority-gated on the node). A `duck://files` ref outside that root stays
// literal text and fetches nothing.

import {
  classifyDuckRef,
  displayName,
  type ChannelRef,
  type DuckRef,
  type FileRef,
  type ForgeRef,
  type PageRef,
} from "../../../domain/duck-uri";

export type { ChannelRef, DuckRef, FileRef, ForgeRef, PageRef };

export type DuckSegment = { text: string } | DuckRef;

export const isPageSeg = (s: DuckSegment): s is { page: PageRef } => "page" in s;
export const isFileSeg = (s: DuckSegment): s is { file: FileRef } => "file" in s;
export const isForgeSeg = (s: DuckSegment): s is { forge: ForgeRef } => "forge" in s;
export const isChannelSeg = (s: DuckSegment): s is { channel: ChannelRef } => "channel" in s;

/** Build the markdown a composer inserts for a page reference. */
export const pageRefMarkdown = (id: string, title: string): string =>
  `[${mdLabel(title || id)}](duck://page/${id})`;

/** Build the markdown for an attachment: `![name](…)` embeds (image preview),
 *  `[name](…)` is a plain download chip. */
export const fileRefMarkdown = (path: string, name: string, embed: boolean): string =>
  `${embed ? "!" : ""}[${mdLabel(name)}](duck://files${path})`;

/** A label safe to sit inside `[...]` AND to survive the chat markdown parser
 *  on the way back out: no `]`/newline (would end the link), no `*` (the
 *  parser's italic/bold marker — a `*` in the label would split the ref across
 *  spans and the render-time tokenizer would never see a whole ref), and no
 *  bidi/control spoofing. The label is decorative — chips render canonical
 *  store data — so neutralizing here costs nothing visible. */
const mdLabel = (raw: string): string =>
  displayName(raw).replace(/[\]\n*]/g, " ").replace(/\s+/g, " ").trim() || "file";

// `!?` embed flag, a label with no `]`/newline, then a module-plane duck://
// url — a single dotless lowercase label, so gateway hosts (`<name>.duck`)
// never match — with no whitespace or `)` (the closing paren is unambiguous).
// Matched left to right; each match is validated before it becomes a ref.
const DUCK_REF = /(!?)\[([^\]\n]*)\]\((duck:\/\/[a-z][a-z0-9-]*\/[^\s)]+)\)/g;

/** Split body text into literal runs and duck refs, in order and lossless
 *  (concatenating every segment's source reproduces the input). A ref that
 *  doesn't validate (bad page id, a file path outside the attachments root,
 *  an unknown module) stays in the literal run verbatim. */
export const splitDuckRefs = (text: string): DuckSegment[] => {
  const out: DuckSegment[] = [];
  let literalFrom = 0;
  for (const m of text.matchAll(DUCK_REF)) {
    const at = m.index ?? 0;
    const [whole, bang, label, url] = m;
    const seg = classifyDuckRef(url, label, bang === "!");
    if (!seg) continue; // malformed → leave in the literal run
    if (at > literalFrom) out.push({ text: text.slice(literalFrom, at) });
    out.push(seg);
    literalFrom = at + whole.length;
  }
  if (literalFrom < text.length) out.push({ text: text.slice(literalFrom) });
  return out;
};

/** Distinct page ids the body references, first-seen order — the list the
 *  agent runtime will mirror when it pulls page context. */
export const parsePageRefs = (text: string): string[] => [
  ...new Set(splitDuckRefs(text).filter(isPageSeg).map((s) => s.page.id)),
];
