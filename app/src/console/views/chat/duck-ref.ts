// The ONE reference grammar for chat/comment bodies: standard markdown link
// and image syntax over `duck://` URIs. This replaces the two split grammars
// that came before — the custom `[[page:<id>]]` tokens and the bare
// `duck://files/...` attachment URIs — so a body has a single, markdown-native
// way to reference anything, and one tokenizer renders them all:
//
//   [label](duck://page/<id>)          → a page chip (opens the page)
//   [label](duck://files/<path>)       → a file chip (downloads on click)
//   ![label](duck://files/<path>)      → an attachment embed (image preview,
//                                        or a download chip if not an image)
//
// References ride as PLAIN markdown text on the wire (the composer inserts the
// literal `[..](..)` / `![..](..)`; the chat markdown parser leaves `duck://`
// links alone — it only marks `https?://` — so they reach the renderer as
// text). Keeping the URL in the text (not a mark) means the agent runtime can
// parse the same grammar from the message it sees.
//
// Security note carried over from the attachment work: FILE refs are confined
// to `/shared/attachments/<dir>/<name>` — the tokenizer is the only guard
// against a crafted ref steering a client read at another duckfs path (reads
// are not authority-gated on the node). A `duck://files` ref outside that root
// stays literal text and fetches nothing.

import { displayName } from "./attachments";

const ATTACHMENTS_ROOT = "/shared/attachments";

export interface PageRef {
  id: string;
  /** the markdown label — decorative; the chip prefers the live store title. */
  label: string;
}

export interface FileRef {
  /** absolute duckfs path, `/shared/attachments/<dir>/<name>`. */
  path: string;
  /** display/download name — the markdown label, spoof-stripped. */
  name: string;
  /** `![..]` embed form: an image previews inline; a non-image still downloads. */
  embed: boolean;
}

export type DuckSegment =
  | { text: string }
  | { page: PageRef }
  | { file: FileRef };

export const isPageSeg = (s: DuckSegment): s is { page: PageRef } => "page" in s;
export const isFileSeg = (s: DuckSegment): s is { file: FileRef } => "file" in s;

/** Build the markdown a composer inserts for a page reference. */
export const pageRefMarkdown = (id: string, title: string): string =>
  `[${mdLabel(title || id)}](duck://page/${id})`;

/** Build the markdown for an attachment: `![name](…)` embeds (image preview),
 *  `[name](…)` is a plain download chip. */
export const fileRefMarkdown = (path: string, name: string, embed: boolean): string =>
  `${embed ? "!" : ""}[${mdLabel(name)}](duck://files${path})`;

/** A label safe to sit inside `[...]`: no `]`, no newline (would break the
 *  link), and no bidi/control spoofing. */
const mdLabel = (raw: string): string =>
  displayName(raw).replace(/[\]\n]/g, " ").trim() || "file";

// `!?` embed flag, a label with no `]`/newline, then a duck:// url with no
// whitespace or `)` (so the closing paren is unambiguous). Matched left to
// right; each match is validated before it becomes a ref.
const DUCK_REF = /(!?)\[([^\]\n]*)\]\((duck:\/\/(?:page|files)\/[^\s)]+)\)/g;

/** Split body text into literal runs and duck refs, in order and lossless
 *  (concatenating every segment's source reproduces the input). A ref that
 *  doesn't validate (bad page id, a file path outside the attachments root)
 *  stays in the literal run verbatim. */
export const splitDuckRefs = (text: string): DuckSegment[] => {
  const out: DuckSegment[] = [];
  let literalFrom = 0;
  for (const m of text.matchAll(DUCK_REF)) {
    const at = m.index ?? 0;
    const [whole, bang, label, url] = m;
    const seg = classify(url, label, bang === "!");
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

function classify(url: string, label: string, embed: boolean): DuckSegment | null {
  const page = url.match(/^duck:\/\/page\/([^/\s)]+)$/);
  if (page) return { page: { id: page[1], label } };

  // duck://files<absolute-path>; the path already carries its leading slash.
  const filePath = url.slice("duck://files".length);
  if (!filePath.startsWith(`${ATTACHMENTS_ROOT}/`)) return null;
  const rest = filePath.slice(ATTACHMENTS_ROOT.length + 1);
  const parts = rest.split("/");
  // exactly <dir>/<name>, both non-empty, no dot-segments (the attachment
  // confinement — a crafted deeper/shallower path never chips).
  if (parts.length !== 2 || parts.some((p) => p === "" || p === "." || p === "..")) {
    return null;
  }
  const name = displayName(label) || displayName(parts[1]);
  return { file: { path: filePath, name, embed } };
}
