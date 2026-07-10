// Small shared vocabulary of the issue/PR tracker: state badges, kind glyphs,
// author-name resolution and #n formatting — the bits every items surface
// (lists, detail, merge box) renders identically.

import { useState } from "react";

import { authorName, keyHex } from "../../../../domain/chat-client";
import type { AuthorRef } from "../../../../domain/chat-client";
import type { ForgeItemKind, ForgeItemState, ForgeItemSummary } from "../../../../domain/forge-client";
import { useDucktape } from "../../../store/use-ducktape";
import { color, font, radius } from "../../../theme/tokens";
import { relTime, SegButton } from "../ui";

/** GitHub's item-number form: `#42`. */
export const itemNumber = (n: number): string => `#${n}`;

/** Badge tones per item state: open green / closed red / merged purple —
 *  the tracker's one non-negotiable color convention. */
export const stateTone = {
  open: { text: color.green, bg: "#eef5f0", border: "#cfe3d7" },
  closed: { text: color.red, bg: "#fbeeec", border: "#eccfc9" },
  merged: { text: color.purple, bg: "#f1edf5", border: "#ddd2e6" },
} as const;

export function StateBadge({ state }: { state: ForgeItemState }) {
  const tone = stateTone[state];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        height: 20,
        padding: "0 8px",
        borderRadius: radius.sm,
        border: `1px solid ${tone.border}`,
        background: tone.bg,
        color: tone.text,
        font: `700 9px ${font.mono}`,
        letterSpacing: ".06em",
        textTransform: "uppercase",
        flexShrink: 0,
      }}
    >
      {state}
    </span>
  );
}

/** Issue = circle-dot, PR = the branch/arrow mark — local glyphs in the
 *  MessageItem tradition (Icon.tsx stays untouched). Colored by state. */
export function KindGlyph({
  kind,
  state,
  size = 14,
}: {
  kind: ForgeItemKind;
  state: ForgeItemState;
  size?: number;
}) {
  const stroke = stateTone[state].text;
  if (kind === "issue") {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke={stroke}
        strokeWidth={1.8}
        strokeLinecap="round"
        style={{ flexShrink: 0 }}
      >
        <circle cx="12" cy="12" r="8.2" />
        <circle cx="12" cy="12" r="1.4" fill={stroke} stroke="none" />
      </svg>
    );
  }
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={stroke}
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ flexShrink: 0 }}
    >
      <circle cx="6.5" cy="5.5" r="2.3" />
      <circle cx="6.5" cy="18.5" r="2.3" />
      <circle cx="17.5" cy="18.5" r="2.3" />
      <path d="M6.5 7.8v8.4" />
      <path d="M12.5 5.5h3a2 2 0 0 1 2 2v8.7" />
      <path d="M13.6 3.4l-2.1 2.1 2.1 2.1" />
    </svg>
  );
}

/** Resolve an item/review author to a display name via the same registry the
 *  chat surface uses (identity projection in state.authorNames). */
export function useAuthorName(author: AuthorRef): string {
  const { state } = useDucktape();
  return authorName(author, state.authorNames);
}

/** Is `author` this client's own submit identity? On the embedded daemon a
 *  User author's bytes ARE the utf-8 origin string; on a networked node they
 *  are a pubkey and never match a readable origin — the affordance simply
 *  stays hidden there, which errs on the safe side. */
export function isSelfAuthor(author: AuthorRef, selfOrigin: string): boolean {
  if (author === "system" || !("user" in author)) return false;
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(author.user));
    if (text === selfOrigin) return true;
  } catch {
    // not utf-8 — fall through to the hex comparison
  }
  return keyHex(author.user) === selfOrigin;
}

/** The Open / Closed list filter both tabs share (closed folds in merged). */
export function StateFilterTabs({
  filter,
  openCount,
  closedCount,
  onFilter,
}: {
  filter: "open" | "closed";
  openCount: number;
  closedCount: number;
  onFilter: (filter: "open" | "closed") => void;
}) {
  return (
    <div
      style={{
        display: "inline-flex",
        border: `1px solid ${color.border}`,
        borderRadius: radius.sm,
        overflow: "hidden",
        flexShrink: 0,
      }}
    >
      <SegButton label={`Open ${openCount}`} active={filter === "open"} onClick={() => onFilter("open")} />
      <SegButton label={`Closed ${closedCount}`} active={filter === "closed"} onClick={() => onFilter("closed")} />
    </div>
  );
}

/** One list row: kind glyph, #n title, author + updated on the right. */
export function ItemRow({ item, onOpen }: { item: ForgeItemSummary; onOpen: () => void }) {
  const [hover, setHover] = useState(false);
  const author = useAuthorName(item.author);
  const updated = relTime(item.updated_at);
  return (
    <button
      type="button"
      onClick={onOpen}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        width: "100%",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "11px 13px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: hover ? color.sunken : "transparent",
      }}
    >
      <KindGlyph kind={item.kind} state={item.state} />
      <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>{item.title}</span>{" "}
        <span style={{ font: `400 12px ${font.mono}`, color: color.muted2 }}>{itemNumber(item.number)}</span>
      </span>
      <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2, whiteSpace: "nowrap", flexShrink: 0 }}>
        {[author, updated ? `updated ${updated}` : ""].filter(Boolean).join(" · ")}
      </span>
    </button>
  );
}
