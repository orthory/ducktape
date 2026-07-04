// Composer text ⇄ chat blocks. The node stores rich bodies (Paragraph / Code /
// Quote / Divider blocks of marked Spans), but the composer is plain text, so
// on send we parse a small markdown subset into blocks, and on edit we render
// the stored blocks back to that same text so the round-trip is lossless.
//
// Supported: fenced ```lang\n…``` → Code blocks; **bold**, *italic*, and bare
// http(s) URLs → marked spans inside Paragraphs. Mentions are a later increment
// (an "@name" stays plain text here; MessageItem still tints @/# tokens).

import type { ChatBlock, Span } from "../../../domain/chat-client";

// One pass over a paragraph's text: **bold** | *italic* | bare URL, else plain.
// Ordered so `**` is tried before `*` (the bold arm consumes the double star).
// The marker content must START and END with a non-space char, so "2 * 3 * 4"
// (spaced asterisks used as math/bullets) is left as plain text rather than
// being read as *italic*.
const INLINE =
  /(\*\*(?<b>[^*\s](?:[^*]*[^*\s])?)\*\*)|(\*(?<i>[^*\s](?:[^*\n]*[^*\s])?)\*)|(?<url>https?:\/\/[^\s<]+)/g;

const parseInline = (text: string): Span[] => {
  const spans: Span[] = [];
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const idx = match.index ?? 0;
    if (idx > last) spans.push({ text: text.slice(last, idx), marks: [] });
    const groups = match.groups ?? {};
    if (groups.b !== undefined) spans.push({ text: groups.b, marks: ["Bold"] });
    else if (groups.i !== undefined) spans.push({ text: groups.i, marks: ["Italic"] });
    else if (groups.url !== undefined) spans.push({ text: groups.url, marks: [{ Link: groups.url }] });
    last = idx + match[0].length;
  }
  if (last < text.length) spans.push({ text: text.slice(last), marks: [] });
  return spans.length > 0 ? spans : [{ text, marks: [] }];
};

// Fenced code: ```lang\n…``` (lang optional). The trailing newline before the
// closing fence is stripped so the block text is exactly the code.
const FENCE = /```([\w+#-]*)\n?([\s\S]*?)```/g;

/** Parse composer text into the block body the node stores. Never returns an
 *  empty list — a message that is all whitespace still yields one paragraph so
 *  callers that already guarded `body.trim()` get a well-formed body. */
export const parseMessageInput = (raw: string): ChatBlock[] => {
  const text = raw.replace(/\r\n/g, "\n");
  const blocks: ChatBlock[] = [];
  const pushParagraphs = (chunk: string) => {
    const trimmed = chunk.trim();
    if (trimmed) blocks.push({ Paragraph: parseInline(trimmed) });
  };
  let last = 0;
  for (const match of text.matchAll(FENCE)) {
    const idx = match.index ?? 0;
    if (idx > last) pushParagraphs(text.slice(last, idx));
    blocks.push({ Code: { lang: match[1] || null, text: match[2].replace(/\n$/, "") } });
    last = idx + match[0].length;
  }
  if (last < text.length) pushParagraphs(text.slice(last));
  return blocks.length > 0 ? blocks : [{ Paragraph: [{ text: raw.trim(), marks: [] }] }];
};

const spanToInput = (span: Span): string => {
  if (span.marks.includes("Bold")) return `**${span.text}**`;
  if (span.marks.includes("Italic")) return `*${span.text}*`;
  // A Link mark's URL is already its own text; mentions render as their handle.
  return span.text;
};

/** Render stored blocks back to composer text, the inverse of
 *  `parseMessageInput`, so the inline editor is seeded with re-editable source
 *  rather than flattened display text. */
export const blocksToInput = (blocks: ChatBlock[]): string =>
  blocks
    .map((block) => {
      if (block === "Divider") return "---";
      if ("Code" in block) return "```" + (block.Code.lang ?? "") + "\n" + block.Code.text + "\n```";
      if ("Quote" in block) return block.Quote.map((span) => `> ${span.text}`).join("\n");
      return block.Paragraph.map(spanToInput).join("");
    })
    .join("\n\n");
