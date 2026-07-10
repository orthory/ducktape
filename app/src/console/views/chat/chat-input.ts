// Composer text ⇄ chat blocks. The node stores rich bodies (Paragraph / Code /
// Quote / Divider blocks of marked Spans), but the composer is plain text, so
// on send we parse a small markdown subset into blocks, and on edit we render
// the stored blocks back to that same text so the round-trip is lossless.
//
// Supported: fenced ```lang\n…``` → Code blocks; > quote lines → Quote blocks;
// ---/___/*** lines → Divider blocks; **bold**, *italic*, markdown links, and
// bare http(s) URLs → marked spans inside Paragraphs/Quotes. With a resolver,
// known "@name" tokens become mention-marked spans (unknown ones stay plain
// text; MessageItem still tints those visually).

import type { AuthorRef, ChatBlock, Span } from "../../../domain/chat-client";

/** agent_id → the AuthorRef its mention mark carries (see mention.ts's
 *  `mentionResolverOf` for why the agent module is always "runs"). */
export type MentionResolver = Map<string, AuthorRef>;

// One pass over text: [label](https://url) | **bold** | *italic* | bare URL,
// else plain.
// Ordered so `**` is tried before `*` (the bold arm consumes the double star).
// The marker content must START and END with a non-space char, so "2 * 3 * 4"
// (spaced asterisks used as math/bullets) is left as plain text rather than
// being read as *italic*.
const INLINE =
  /(\[(?<label>[^\]\n]+)\]\((?<href>https?:\/\/[^\s<)]+)\))|(\*\*(?<b>[^*\s](?:[^*]*[^*\s])?)\*\*)|(\*(?<i>[^*\s](?:[^*\n]*[^*\s])?)\*)|(?<url>https?:\/\/[^\s<]+)/g;

// The composer's mention-token grammar: `@` + the agent-id charset. Boundary
// checks live in `splitMentions` (start-of-span or whitespace before the `@`).
const MENTION_TOKEN = /@([a-z0-9._-]+)/g;

// Split a PLAIN span around @tokens the resolver knows, marking each hit with
// its AuthorRef. Marked spans (bold/link/…) pass through untouched — a
// mention nested in bold isn't a supported composition, and unknown @tokens
// stay plain text by simply not splitting on them.
const splitMentions = (span: Span, resolver: MentionResolver): Span[] => {
  if (span.marks.length > 0) return [span];
  const out: Span[] = [];
  let last = 0;
  for (const match of span.text.matchAll(MENTION_TOKEN)) {
    const idx = match.index ?? 0;
    if (idx > 0 && !/\s/.test(span.text[idx - 1]!)) continue;
    const ref = resolver.get(match[1]!);
    if (!ref) continue;
    if (idx > last) out.push({ text: span.text.slice(last, idx), marks: [] });
    out.push({ text: match[0], marks: [{ mention: ref }] });
    last = idx + match[0].length;
  }
  if (out.length === 0) return [span];
  if (last < span.text.length) out.push({ text: span.text.slice(last), marks: [] });
  return out;
};

const parseInline = (text: string, mentions?: MentionResolver): Span[] => {
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
  const flat = spans.length > 0 ? spans : [{ text, marks: [] } as Span];
  return mentions ? flat.flatMap((span) => splitMentions(span, mentions)) : flat;
};

// Fenced code: ```lang\n…``` (lang optional). The trailing newline before the
// closing fence is stripped so the block text is exactly the code.
const FENCE = /```([\w+#-]*)\n?([\s\S]*?)```/g;
const DIVIDER = /^\s*(?:-{3,}|\*{3,}|_{3,})\s*$/;

const pushMarkedBlock = (
  blocks: ChatBlock[],
  kind: "paragraph" | "quote",
  raw: string,
  mentions?: MentionResolver,
) => {
  const text = raw.trim();
  if (!text) return;
  blocks.push(
    kind === "paragraph"
      ? { paragraph: parseInline(text, mentions) }
      : { quote: parseInline(text, mentions) },
  );
};

const pushTextBlocks = (blocks: ChatBlock[], chunk: string, mentions?: MentionResolver) => {
  const paragraphLines: string[] = [];
  const quoteLines: string[] = [];

  const flushParagraph = () => {
    pushMarkedBlock(blocks, "paragraph", paragraphLines.join("\n"), mentions);
    paragraphLines.length = 0;
  };
  const flushQuote = () => {
    pushMarkedBlock(blocks, "quote", quoteLines.join("\n"), mentions);
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
 *  callers that already guarded `body.trim()` get a well-formed body. With a
 *  `mentions` resolver, known @tokens become mention-marked spans. */
export const parseMessageInput = (raw: string, mentions?: MentionResolver): ChatBlock[] => {
  const text = raw.replace(/\r\n/g, "\n");
  const blocks: ChatBlock[] = [];
  let last = 0;
  for (const match of text.matchAll(FENCE)) {
    const idx = match.index ?? 0;
    if (idx > last) pushTextBlocks(blocks, text.slice(last, idx), mentions);
    blocks.push({ code: { lang: match[1] || null, text: match[2].replace(/\n$/, "") } });
    last = idx + match[0].length;
  }
  if (last < text.length) pushTextBlocks(blocks, text.slice(last), mentions);
  return blocks.length > 0 ? blocks : [{ paragraph: [{ text: raw.trim(), marks: [] }] }];
};

const spanToInput = (span: Span): string => {
  const link = span.marks.find((mark): mark is { link: string } => typeof mark === "object" && "link" in mark);
  if (link) return span.text === link.link ? link.link : `[${span.text}](${link.link})`;
  if (span.marks.includes("bold")) return `**${span.text}**`;
  if (span.marks.includes("italic")) return `*${span.text}*`;
  const mention = span.marks.find(
    (mark): mark is { mention: AuthorRef } => typeof mark === "object" && "mention" in mark,
  );
  // An agent mention renders back to `@agent_id` so re-parsing with the same
  // resolver reproduces the mark; other mention kinds keep their span text.
  if (mention && typeof mention.mention === "object" && "agent" in mention.mention)
    return `@${mention.mention.agent.agent_id}`;
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
