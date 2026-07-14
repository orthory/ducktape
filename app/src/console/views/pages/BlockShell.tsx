// What a block LOOKS like: the hanging marker, and the frame the textarea sits
// in. BlockRow owns behaviour (draft, keys, paste, caret, memo); everything
// here is a function of the block and its draft.

import type { ReactNode } from "react";

import type { PageBlock } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import { accentVar, color, font, radius, tint } from "../../theme/tokens";
import { MARKER_HANG, ROW_PAD_Y, kindFont } from "./pages-style";

/** The bullet / number / checkbox / chevron, hanging in the left margin. Null
 *  for kinds that carry no marker — prose must not pay for a box it never
 *  fills, which is what used to push every line right of the title. */
export function BlockMarker({
  block,
  blockNumber,
  listIndex,
  expanded,
  onSetChecked,
  onToggleCollapse,
}: {
  block: PageBlock;
  blockNumber: number;
  listIndex: number | undefined;
  expanded: boolean;
  onSetChecked: (checked: boolean) => void;
  onToggleCollapse: () => void;
}) {
  const glyph =
    block.kind === "bulleted" ? (
      <span style={{ font: `700 14px ${font.sans}`, color: color.muted3 }}>•</span>
    ) : block.kind === "numbered" ? (
      <span style={{ font: `500 12.5px ${font.mono}`, color: color.muted3 }}>
        {listIndex ?? 1}.
      </span>
    ) : block.kind === "todo" ? (
      <button
        type="button"
        aria-label={`${block.checked ? "Uncheck" : "Check"} to-do block ${blockNumber}`}
        onClick={() => onSetChecked(!block.checked)}
        style={{
          all: "unset",
          cursor: "pointer",
          width: 15,
          height: 15,
          borderRadius: 4,
          border: `1.5px solid ${block.checked ? accentVar : color.borderStrong}`,
          background: block.checked ? accentVar : "transparent",
          color: color.onDark,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        {block.checked ? <Icon name="check" size={10} strokeWidth={2.4} /> : null}
      </button>
    ) : // a toggle with NO children has nothing to collapse — its chevron was
    // pure theatre, and clicking it hid nothing at all.
    block.kind === "toggle" && block.children.length > 0 ? (
      <button
        type="button"
        aria-label={`${expanded ? "Collapse" : "Expand"} toggle block ${blockNumber}`}
        aria-expanded={expanded}
        onClick={onToggleCollapse}
        style={{
          all: "unset",
          cursor: "pointer",
          width: 16,
          height: 16,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: color.muted3,
        }}
      >
        <Icon
          name="chevronRight"
          size={13}
          strokeWidth={2}
          style={{ transform: `rotate(${expanded ? 90 : 0}deg)` }}
        />
      </button>
    ) : null;

  if (!glyph) return null;
  return (
    <div
      style={{
        position: "absolute",
        left: -MARKER_HANG,
        // an absolute box offsets from the row's PADDING box, but the marker
        // used to be a flex item aligned to its CONTENT box — one row-padding
        // lower. Match it, or every bullet rides high above its own line. (The
        // row's marginTop already moved the box; a heading's top space must NOT
        // be added again here.)
        top: ROW_PAD_Y,
        width: 20,
        height: 24,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {glyph}
    </div>
  );
}

/** The frame around a block's editor: a divider's rule, the code block's label
 *  and line-number gutter, the callout's icon and accent stripe, the quote's
 *  bar — or nothing at all, for prose. `children` is the textarea (plus its
 *  slash menu), which BlockRow owns. */
export function BlockShell({
  kind,
  blockNumber,
  draft,
  children,
}: {
  kind: PageBlock["kind"];
  blockNumber: number;
  /** The live draft — the code gutter numbers its lines. */
  draft: string;
  children: ReactNode;
}) {
  if (kind === "divider") {
    return (
      <div aria-label={`Divider block ${blockNumber}`} style={{ padding: "14px 0" }}>
        <div style={{ height: 1, background: color.border }} />
      </div>
    );
  }

  if (kind === "code") {
    // a real code block: a language label and a line-number gutter, neither of
    // which it had.
    // ponytail: the gutter counts NEWLINES, so a long line that soft-wraps
    // drifts a number out of step. An exact gutter (and a per-block language)
    // wants a wire field; this is the honest ceiling without one.
    return (
      <div
        style={{
          position: "relative",
          background: color.sunken,
          border: `1px solid ${color.borderSoft}`,
          borderRadius: radius.md,
          padding: "16px 18px",
        }}
      >
        <span
          aria-hidden="true"
          style={{
            position: "absolute",
            top: 7,
            right: 10,
            font: `600 8.5px ${font.mono}`,
            letterSpacing: ".12em",
            color: color.muted2,
          }}
        >
          CODE
        </span>
        <div style={{ display: "flex", gap: 10 }}>
          <div
            aria-hidden="true"
            style={{
              flexShrink: 0,
              minWidth: 13,
              textAlign: "right",
              userSelect: "none",
              font: kindFont("code"),
              color: color.muted2,
            }}
          >
            {draft.split("\n").map((_, line) => (
              <div key={line}>{line + 1}</div>
            ))}
          </div>
          <div style={{ position: "relative", flex: 1, minWidth: 0 }}>{children}</div>
        </div>
      </div>
    );
  }

  if (kind === "callout") {
    // an icon and an accent stripe, not a flat grey box. The wash is mixed
    // against the live --c-paper, so it reads right in both themes.
    // ponytail: the icon is FIXED — a per-block icon is a wire field, and a
    // wire field is an app-hash flag day.
    const wash = tint(accentVar);
    return (
      <div
        style={{
          position: "relative",
          display: "flex",
          gap: 12,
          background: wash.bg,
          borderLeft: `3px solid ${accentVar}`,
          borderRadius: `0 ${radius.md}px ${radius.md}px 0`,
          padding: "14px 16px",
        }}
      >
        <span
          aria-hidden="true"
          style={{ font: "14px/1.6 'Apple Color Emoji', 'Segoe UI Emoji', sans-serif" }}
        >
          💡
        </span>
        <div style={{ position: "relative", flex: 1, minWidth: 0 }}>{children}</div>
      </div>
    );
  }

  const quote = kind === "quote";
  return (
    <div
      style={{
        position: "relative",
        borderLeft: quote ? `3px solid ${color.borderStrong}` : "none",
        paddingLeft: quote ? 12 : 0,
      }}
    >
      {children}
    </div>
  );
}
