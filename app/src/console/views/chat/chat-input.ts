// Composer text ⇄ chat blocks. The node stores rich bodies (Paragraph / Code /
// Quote / Divider blocks of marked Spans), but the composer is plain text, so
// on send we parse a small markdown subset into blocks, and on edit we render
// the stored blocks back to that same text so the round-trip is lossless.
//
// Supported: fenced ```lang\n…``` → Code blocks; > quote lines → Quote blocks;
// ---/___/*** lines → Divider blocks; **bold**, *italic*, markdown links, and
// bare http(s) URLs → marked spans inside Paragraphs/Quotes. Mentions are a
// later increment (an "@name" stays plain text here; MessageItem still tints
// @/# tokens).

import type { ChatBlock, Span } from "../../../domain/chat-client";

// One pass over text: [label](https://url) | **bold** | *italic* | bare URL,
// else plain.
// Ordered so `**` is tried before `*` (the bold arm consumes the double star).
// The marker content must START and END with a non-space char, so "2 * 3 * 4"
// (spaced asterisks used as math/bullets) is left as plain text rather than
// being read as *italic*.
const INLINE =
  /(\[(?<label>[^\]\n]+)\]\((?<href>https?:\/\/[^\s<)]+)\))|(\*\*(?<b>[^*\s](?:[^*]*[^*\s])?)\*\*)|(\*(?<i>[^*\s](?:[^*\n]*[^*\s])?)\*)|(?<url>https?:\/\/[^\s<]+)/g;

const parseInline = (text: string): Span[] => {
  const spans: Span[] = [];
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const idx = match.index ?? 0;
    if (idx > last) spans.push({ text: text.slice(last, idx), marks: [] });
    const groups = match.groups ?? {};
    if (groups.label !== undefined && groups.href !== undefined)
      spans.push({ text: groups.label, marks: [{ link: groups.href }] });
    else if (groups.b !== undefined) spans.push({ text: groups.b, marks: ["bold"] });
    else if (groups.i !== undefined) spans.push({ text: groups.i, marks: ["italic"] });
    else if (groups.url !== undefined) spans.push({ text: groups.url, marks: [{ link: groups.url }] });
    last = idx + match[0].length;
  }
  if (last < text.length) spans.push({ text: text.slice(last), marks: [] });
  return spans.length > 0 ? spans : [{ text, marks: [] }];
};

// Fenced code: ```lang\n…``` (lang optional). The trailing newline before the
// closing fence is stripped so the block text is exactly the code.
const FENCE = /```([\w+#-]*)\n?([\s\S]*?)```/g;
const DIVIDER = /^\s*(?:-{3,}|\*{3,}|_{3,})\s*$/;

const pushMarkedBlock = (blocks: ChatBlock[], kind: "paragraph" | "quote", raw: string) => {
  const text = raw.trim();
  if (!text) return;
  blocks.push(kind === "paragraph" ? { paragraph: parseInline(text) } : { quote: parseInline(text) });
};

const pushTextBlocks = (blocks: ChatBlock[], chunk: string) => {
  const paragraphLines: string[] = [];
  const quoteLines: string[] = [];

  const flushParagraph = () => {
    pushMarkedBlock(blocks, "paragraph", paragraphLines.join("\n"));
    paragraphLines.length = 0;
  };
  const flushQuote = () => {
    pushMarkedBlock(blocks, "quote", quoteLines.join("\n"));
    quoteLines.length = 0;
  };

  for (const line of chunk.split("\n")) {
    if (line.trim() === "") {
      flushParagraph();
      flushQuote();
      continue;
    }

    const quote = line.match(/^\s*>\s?(.*)$/);
    if (quote) {
      flushParagraph();
      quoteLines.push(quote[1]);
      continue;
    }

    flushQuote();
    if (DIVIDER.test(line)) {
      flushParagraph();
      blocks.push("divider");
      continue;
    }

    paragraphLines.push(line);
  }

  flushParagraph();
  flushQuote();
};

/** Parse composer text into the block body the node stores. Never returns an
 *  empty list — a message that is all whitespace still yields one paragraph so
 *  callers that already guarded `body.trim()` get a well-formed body. */
export const parseMessageInput = (raw: string): ChatBlock[] => {
  const text = raw.replace(/\r\n/g, "\n");
  const blocks: ChatBlock[] = [];
  let last = 0;
  for (const match of text.matchAll(FENCE)) {
    const idx = match.index ?? 0;
    if (idx > last) pushTextBlocks(blocks, text.slice(last, idx));
    blocks.push({ code: { lang: match[1] || null, text: match[2].replace(/\n$/, "") } });
    last = idx + match[0].length;
  }
  if (last < text.length) pushTextBlocks(blocks, text.slice(last));
  return blocks.length > 0 ? blocks : [{ paragraph: [{ text: raw.trim(), marks: [] }] }];
};

const spanToInput = (span: Span): string => {
  const link = span.marks.find((mark): mark is { link: string } => typeof mark === "object" && "link" in mark);
  if (link) return span.text === link.link ? link.link : `[${span.text}](${link.link})`;
  if (span.marks.includes("bold")) return `**${span.text}**`;
  if (span.marks.includes("italic")) return `*${span.text}*`;
  // Mentions render as their handle.
  return span.text;
};

const quoteToInput = (spans: Span[]): string =>
  spans
    .map(spanToInput)
    .join("")
    .split("\n")
    .map((line) => `> ${line}`)
    .join("\n");

/** Render stored blocks back to composer text, the inverse of
 *  `parseMessageInput`, so the inline editor is seeded with re-editable source
 *  rather than flattened display text. */
export const blocksToInput = (blocks: ChatBlock[]): string =>
  blocks
    .map((block) => {
      if (block === "divider") return "---";
      if ("code" in block) return "```" + (block.code.lang ?? "") + "\n" + block.code.text + "\n```";
      if ("quote" in block) return quoteToInput(block.quote);
      return block.paragraph.map(spanToInput).join("");
    })
    .join("\n\n");
