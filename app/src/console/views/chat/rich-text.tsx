// The console's one rich-body renderer: chat blocks (marked spans) → React.
// Lifted out of MessageItem so every surface that shows a body renders it the
// same way — the chat lane, the thread panel, and forge's discussion, which
// used to flatten the blocks to a string and threw the marks away.
//
// Three things in a body are click targets: a #tag (filter), a mention mark
// (open the agent or the person), and a `duck://` ref (a chip that deep-links
// through the protocol's open plane — page/files/forge/channel alike).
// Everything else is inert text. Navigation goes through the store from here
// rather than through props: the renderer is used from three views and the
// context is optional, so a bare component test just gets inert affordances.

import { useContext, useEffect, useMemo, useState } from "react";
import type { CSSProperties, ReactNode } from "react";

import type { AuthorNames, AuthorRef, ChatBlock, Span } from "../../../domain/chat-client";
import { parseItemChannelId } from "../../../domain/forge-client";
import { openExternal } from "../../dom/external-link";
import { ConsoleContext } from "../../store/context";
import { openDuckRef } from "../../store/open-duck-ref";
import type { HlToken } from "../forge/highlight";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { AttachmentChip } from "./AttachmentChip";
import { splitMentions } from "./chat-input";
import {
  isChannelSeg,
  isFileSeg,
  isForgeSeg,
  isPageSeg,
  splitDuckRefs,
  type ChannelRef,
  type DuckSegment,
  type ForgeRef,
} from "./duck-ref";
import { mentionableUsers, mentionLabel, mentionResolverOf, mentionTarget } from "./mention";

// The index's tag grammar, mirrored for display: `#` + 1..=64 Unicode
// letters/digits/`_`/`-` at a whitespace boundary (parts are already
// whitespace-split). Only what the node indexed reads as clickable.
const TAG_TOKEN = /^#([\p{L}\p{N}_-]{1,64})/u;

const REF_STYLE: CSSProperties = { color: accentVar, fontWeight: 500 };

// Inline code: a `run` renders as a mono chip. Split with the capture group so
// the code tokens come back as their own chunks; anything unclosed stays plain
// text. Like the fenced block below, the run is literal — no tags, refs, or
// mentions resolve inside it.
const INLINE_CODE = /(`[^`\n]+`)/;

function InlineCode({ text }: { text: string }) {
  return (
    <code
      style={{
        font: `400 12.5px ${font.mono}`,
        background: color.sunken,
        border: `1px solid ${color.borderSoft}`,
        borderRadius: 4,
        padding: "1px 4px",
        color: color.red,
        overflowWrap: "anywhere",
      }}
    >
      {text}
    </code>
  );
}

function TagToken({ token, onClick }: { token: string; onClick: (tag: string) => void }) {
  return (
    <button
      type="button"
      title={`Filter by ${token}`}
      onClick={(event) => {
        event.stopPropagation();
        onClick(token.slice(1));
      }}
      style={{ all: "unset", cursor: "pointer", ...REF_STYLE }}
    >
      {token}
    </button>
  );
}

// ── Mention ─────────────────────────────────────────────

function MentionToken({ mention, names }: { mention: AuthorRef; names: AuthorNames }) {
  const store = useContext(ConsoleContext);
  // Both the label and the click target come from the MARK, never from the
  // span's text: the authored text is the handle typed at the time ("@quackbot",
  // "@jess"), while the mark carries the durable ref. A renamed agent or person
  // therefore re-renders under the new name AND still navigates correctly.
  const label = `@${mentionLabel(mention, names, store?.state.agents ?? [])}`;
  const target = mentionTarget(mention);
  // A module/system mention has no principal to visit, and without a store
  // there is nowhere to navigate — either way it stays tinted but inert.
  if (!target || !store) return <span style={REF_STYLE}>{label}</span>;
  const open = () =>
    "agentId" in target
      ? store.actions.openAgent(target.agentId)
      : store.actions.openMember(target.accountId);
  const description =
    "agentId" in target ? `Open agent ${target.agentId}` : `Open ${label.slice(1)} in Members`;
  return (
    <button
      type="button"
      title={description}
      aria-label={description}
      onClick={(event) => {
        event.stopPropagation();
        open();
      }}
      style={{ all: "unset", cursor: "pointer", ...REF_STYLE }}
    >
      {label}
    </button>
  );
}

// ── duck:// ref chips ───────────────────────────────────

/** The one chip body every duck:// ref renders through: a small glyph, a
 *  canonical face (NEVER the authored markdown label — labels can lie), and a
 *  click-through when a store is present. `mono` marks an unresolved raw-id
 *  face — honest about what the text says, never a blank or a guessed name. */
function RefChip({
  glyph,
  face,
  mono,
  description,
  onOpen,
}: {
  glyph: string;
  face: string;
  mono: boolean;
  description: string;
  onOpen: (() => void) | null;
}) {
  const style: CSSProperties = {
    display: "inline-flex",
    alignItems: "baseline",
    gap: 4,
    padding: "0 5px",
    borderRadius: radius.sm,
    border: `1px solid ${color.borderSoft}`,
    background: color.sunken,
    font: `500 13.5px ${font.sans}`,
    color: mono ? color.muted3 : accentVar,
    verticalAlign: "baseline",
  };
  const body = (
    <>
      <span aria-hidden style={{ font: `400 11px ${font.mono}`, color: color.muted2 }}>
        {glyph}
      </span>
      <span style={{ font: mono ? `400 13px ${font.mono}` : undefined }}>{face}</span>
    </>
  );
  if (!onOpen) return <span style={style}>{body}</span>;
  return (
    <button
      type="button"
      title={description}
      aria-label={description}
      onClick={(event) => {
        event.stopPropagation();
        onOpen();
      }}
      style={{ all: "unset", cursor: "pointer", ...style }}
    >
      {body}
    </button>
  );
}

/** A `duck://page/<id>` ref carrying the page's live title. The title comes
 *  from `state.pages`, which is hydrated at boot and refreshed when the pages
 *  root moves — so a chip never fetches; a deleted/unknown id shows raw. */
export function PageRefChip({ pageId }: { pageId: string }) {
  const store = useContext(ConsoleContext);
  const page = store?.state.pages.find((meta) => meta.id === pageId) ?? null;
  const title = page?.title.trim() || null;
  return (
    <RefChip
      glyph="¶"
      face={title ?? pageId}
      mono={!title}
      description={`Open page ${title ?? pageId}`}
      onOpen={
        store ? () => openDuckRef({ page: { id: pageId, label: "" } }, store.actions) : null
      }
    />
  );
}

/** A `duck://forge/<repo>[/<n>]` ref. The face is the canonical `repo#n`
 *  coordinate itself — no store lookup needed to be honest. */
export function ForgeRefChip({ forge }: { forge: ForgeRef }) {
  const store = useContext(ConsoleContext);
  const face = forge.number === null ? forge.repo : `${forge.repo}#${forge.number}`;
  return (
    <RefChip
      glyph="⑂"
      face={face}
      mono={false}
      description={`Open ${face} in Forge`}
      onOpen={store ? () => openDuckRef({ forge }, store.actions) : null}
    />
  );
}

/** A `duck://channel/<id>[#seq]` ref, faced with the live channel name. A
 *  forge item's hidden discussion channel (`forge:<repo>:<n>`) faces — and
 *  deep-links — as its forge item, where that discussion actually lives. */
export function ChannelRefChip({ channel }: { channel: ChannelRef }) {
  const store = useContext(ConsoleContext);
  const item = parseItemChannelId(channel.id);
  const name = store?.state.channels.find((c) => c.id === channel.id)?.name ?? null;
  const face = item ? `${item.repo}#${item.number}` : `#${name ?? channel.id}`;
  const description = item ? `Open ${face} in Forge` : `Open channel ${face}`;
  return (
    <RefChip
      glyph={item ? "⑂" : "#"}
      face={item ? face : (name ?? channel.id)}
      mono={!item && !name}
      description={description}
      onOpen={store ? () => openDuckRef({ channel }, store.actions) : null}
    />
  );
}

/** One duck segment → its surface: a chip for every valid ref kind, a literal
 *  run for the rest. The single mapping both body surfaces share. */
function DuckSeg({ seg, onTagClick }: { seg: DuckSegment; onTagClick?: (tag: string) => void }) {
  if (isPageSeg(seg)) return <PageRefChip pageId={seg.page.id} />;
  if (isFileSeg(seg)) return <AttachmentChip attachment={seg.file} />;
  if (isForgeSeg(seg)) return <ForgeRefChip forge={seg.forge} />;
  if (isChannelSeg(seg)) return <ChannelRefChip channel={seg.channel} />;
  return <LiteralRun text={seg.text} onTagClick={onTagClick} />;
}

/** A pages COMMENT body: plain text on the wire, so every reference is
 *  re-derived at render — @tokens resolve through the SAME resolver + grammar
 *  the submit path used (`splitMentions` over `mentionResolverOf`), then
 *  `duck://` refs chip inside the non-mention runs (via `splitDuckRefs`).
 *  Mentions split FIRST, over the RAW text: that keeps the whitespace boundary
 *  identical to what the submit path saw. An @word the resolver doesn't know stays
 *  tinted-inert via LiteralRun — an address nobody claimed. Without a store
 *  (bare component tests) nothing resolves, everything tints.
 *
 *  ponytail: markdown-adjacent tokens can still disagree — the submit path
 *  parses bold marks and fences that comments render raw, so "**hi**@bot"
 *  invokes without a chip. Reconciling that means changing the WIRE's grammar
 *  for plain-text comments, not this renderer. */
export function CommentText({ text, names }: { text: string; names: AuthorNames }) {
  const store = useContext(ConsoleContext);
  const agents = store?.state.agents;
  const nodeUsers = store?.state.nodeUsers;
  const resolver = useMemo(
    () =>
      agents && nodeUsers
        ? mentionResolverOf(agents, mentionableUsers(nodeUsers, agents))
        : new Map<string, AuthorRef>(),
    [agents, nodeUsers],
  );
  return (
    <>
      {splitMentions({ text, marks: [] }, resolver).map((span, i) => {
        const mark = span.marks.find(
          (m): m is { mention: AuthorRef } => typeof m === "object" && "mention" in m,
        );
        if (mark) return <MentionToken key={i} mention={mark.mention} names={names} />;
        return splitDuckRefs(span.text).map((seg, j) => (
          <DuckSeg key={`${i}:${j}`} seg={seg} />
        ));
      })}
    </>
  );
}

// ── Spans and blocks ────────────────────────────────────

/** The @/# token scan over a literal run: a #tag matching the index's grammar
 *  becomes click-to-filter where the surface wires one up. An `@token` in plain
 *  text is NOT a mention — the composer emits a real Mark::Mention for every
 *  handle it resolved, so a bare @word is a handle nobody claimed. It stays
 *  tinted (it reads as an address) but inert: there is no principal behind it. */
function LiteralRun({ text, onTagClick }: { text: string; onTagClick?: (tag: string) => void }) {
  return (
    <>
      {text.split(/(\s+)/).map((part, i) => {
        const tag = part.startsWith("#") ? TAG_TOKEN.exec(part) : null;
        if (tag && onTagClick) {
          return (
            <span key={i}>
              <TagToken token={tag[0]} onClick={onTagClick} />
              {part.slice(tag[0].length)}
            </span>
          );
        }
        return part.startsWith("@") || part.startsWith("#") ? (
          <span key={i} style={REF_STYLE}>
            {part}
          </span>
        ) : (
          part
        );
      })}
    </>
  );
}

function SpanText({
  span,
  names,
  onTagClick,
}: {
  span: Span;
  names: AuthorNames;
  onTagClick?: (tag: string) => void;
}) {
  const mentionMark = span.marks.find(
    (m): m is { mention: AuthorRef } => typeof m === "object" && "mention" in m,
  );
  if (mentionMark) return <MentionToken mention={mentionMark.mention} names={names} />;

  const linkMark = span.marks.find((m): m is { link: string } => typeof m === "object" && "link" in m);
  const linkHref = linkMark && /^https?:\/\//i.test(linkMark.link) ? linkMark.link : null;
  const style: CSSProperties = {
    fontWeight: span.marks.includes("bold") ? 600 : 400,
    fontStyle: span.marks.includes("italic") ? "italic" : "normal",
    color: linkHref ? accentVar : undefined,
    textDecoration: linkHref ? "underline" : undefined,
    overflowWrap: "anywhere",
    wordBreak: "break-word",
  };
  if (linkHref) {
    return (
      <a
        href={linkHref}
        target="_blank"
        rel="noreferrer"
        onClick={(e) => {
          e.preventDefault();
          openExternal(linkHref);
        }}
        style={{ ...style, cursor: "pointer" }}
      >
        {span.text}
      </a>
    );
  }
  // Inline code splits FIRST (a code run is literal), then one tokenizer for
  // every duck:// reference in the rest: page chips, file chips, and image
  // embeds, all from markdown link/image syntax in the plain span text.
  return (
    <span style={style}>
      {span.text.split(INLINE_CODE).map((chunk, c) =>
        // re-test rather than endpoint-check: a bare "```" run starts and ends
        // with a backtick but was never a captured code token.
        /^`[^`\n]+`$/.test(chunk) ? (
          <InlineCode key={c} text={chunk.slice(1, -1)} />
        ) : (
          splitDuckRefs(chunk).map((seg, i) => (
            <DuckSeg key={`${c}:${i}`} seg={seg} onTagClick={onTagClick} />
          ))
        ),
      )}
    </span>
  );
}

/** A message body. `onTagClick` absent (the thread panel) leaves #tags inert. */
export function RichText({
  blocks,
  names,
  onTagClick,
}: {
  blocks: ChatBlock[];
  names: AuthorNames;
  onTagClick?: (tag: string) => void;
}): ReactNode {
  return blocks.map((block, i) => {
    if (block === "divider") {
      return <div key={i} style={{ height: 1, background: color.borderSoft, margin: "7px 0" }} />;
    }
    if ("paragraph" in block) {
      return (
        <div
          key={i}
          style={{
            whiteSpace: "pre-wrap",
            overflowWrap: "anywhere",
            wordBreak: "break-word",
            maxWidth: "100%",
            minWidth: 0,
          }}
        >
          {block.paragraph.map((span, j) => (
            <SpanText key={j} span={span} names={names} onTagClick={onTagClick} />
          ))}
        </div>
      );
    }
    if ("quote" in block) {
      return (
        <div
          key={i}
          style={{
            borderLeft: `3px solid ${color.borderStrong}`,
            paddingLeft: 10,
            margin: "3px 0",
            color: color.muted3,
            whiteSpace: "pre-wrap",
            overflowWrap: "anywhere",
            wordBreak: "break-word",
            maxWidth: "100%",
          }}
        >
          {block.quote.map((span, j) => (
            <SpanText key={j} span={span} names={names} onTagClick={onTagClick} />
          ))}
        </div>
      );
    }
    return <CodeBlock key={i} lang={block.code.lang} text={block.code.text} />;
  });
}

// Code stays literal — a duck:// ref or #tag inside a fence is source, not a
// reference to chip. Long lines WRAP (pre-wrap) instead of scrolling sideways:
// a horizontal scrollbar inside a chat row hides content. A fence tag naming a
// language the forge viewer bundles gets shiki tokens (same `.code-tok`
// per-theme colors); highlighting is async, so the plain text paints first.
// The highlighter is dynamically imported like CodeView's — shiki + grammars
// must stay in their lazy chunk, out of the console's startup bundle.
const HIGHLIGHT_MAX_BYTES = 200_000; // same too-large-stays-plain bar as CodeView

function CodeBlock({ lang, text }: { lang: string | null; text: string }) {
  const [lines, setLines] = useState<HlToken[][] | null>(null);
  useEffect(() => {
    setLines(null); // never render a previous fence's tokens over new text
    if (!lang || text.length > HIGHLIGHT_MAX_BYTES) return;
    let live = true;
    void import("../forge/highlight")
      .then(({ highlightLines, langForTag }) => {
        const langId = langForTag(lang);
        return langId ? highlightLines(text, langId) : null;
      })
      .then((tokens) => {
        if (live && tokens) setLines(tokens);
      })
      .catch(() => {}); // any failure just stays plain text
    return () => {
      live = false;
    };
  }, [text, lang]);
  return (
    <pre
      style={{
        margin: "4px 0",
        padding: "8px 10px",
        borderRadius: radius.sm,
        background: color.sunken,
        border: `1px solid ${color.borderSoft}`,
        font: `400 12.5px ${font.mono}`,
        lineHeight: 1.5,
        color: color.inkSoft,
        maxWidth: "100%",
        minWidth: 0,
        boxSizing: "border-box",
        whiteSpace: "pre-wrap",
        overflowWrap: "anywhere",
      }}
    >
      {lines
        ? lines.map((line, i) => (
            <span key={i}>
              {line.map((token, j) => (
                <span
                  key={j}
                  className={token.style ? "code-tok" : undefined}
                  style={token.style as CSSProperties | undefined}
                >
                  {token.content}
                </span>
              ))}
              {i < lines.length - 1 ? "\n" : null}
            </span>
          ))
        : text}
    </pre>
  );
}
