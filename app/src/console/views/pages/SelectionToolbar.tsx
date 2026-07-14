import { useEffect } from "react";

import type { BlockKind, InlineMark, RelativeAnchor, SpanMark } from "../../../domain/pages-client";
import { rangeHasMark } from "../../../domain/pages-ranges";
import { Icon } from "../../components/Icon";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";
import type { CommentAnchor } from "./CommentCard";

const MARKS: { kind: InlineMark; label: string; glyph: string }[] = [
  { kind: "bold", label: "Bold", glyph: "B" },
  { kind: "italic", label: "Italic", glyph: "I" },
  { kind: "underline", label: "Underline", glyph: "U" },
  { kind: "strikethrough", label: "Strikethrough", glyph: "S" },
  { kind: "code", label: "Inline code", glyph: "<>" },
];

const HEADINGS: { kind: BlockKind; label: string; glyph: string }[] = [
  { kind: "heading1", label: "Heading 1", glyph: "H1" },
  { kind: "heading2", label: "Heading 2", glyph: "H2" },
  { kind: "heading3", label: "Heading 3", glyph: "H3" },
];

const cell = (active: boolean): React.CSSProperties => ({
  all: "unset",
  cursor: "pointer",
  height: 30,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  borderRadius: radius.sm,
  background: active ? `color-mix(in srgb, ${accentVar} 22%, ${color.paper})` : color.hover,
  boxShadow: active ? `inset 0 0 0 1px color-mix(in srgb, ${accentVar} 50%, transparent)` : "none",
  color: active ? accentVar : color.muted3,
});

/** The selection guide menu: floats under the dragged-off selection with the
 *  reference's shape — a TEXT STYLE grid (headings + inline marks), then
 *  Comment. It must vanish the moment the user does anything else: collapse
 *  the selection, click elsewhere, scroll, or press Escape. */
export function SelectionToolbar({
  blockId,
  blockKind,
  marks,
  range,
  anchor,
  onMark,
  onTurnInto,
  onComment,
  onDismiss,
}: {
  blockId: string;
  blockKind: BlockKind;
  marks: SpanMark[];
  range: RelativeAnchor;
  anchor: CommentAnchor;
  onMark: (kind: InlineMark, active: boolean) => void;
  onTurnInto: (kind: BlockKind) => void;
  /** The caller anchors the card to the SELECTION, not this menu — the docked
   *  surface aligns with the commented line, not with wherever the menu sat. */
  onComment: () => void;
  onDismiss: () => void;
}) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.isComposing || event.keyCode === 229) return;
      if (event.key === "Escape") onDismiss();
    };
    // capture: scroll doesn't bubble, and any document movement leaves the
    // menu floating over stale coordinates.
    const onScroll = () => onDismiss();
    document.addEventListener("keydown", onKey);
    document.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("scroll", onScroll, true);
    };
  }, [onDismiss]);

  return (
    <div
      role="toolbar"
      aria-label="Selection actions"
      data-selection-toolbar={blockId}
      style={{
        position: "fixed",
        zIndex: 35,
        left: Math.max(126, Math.min(anchor.x, window.innerWidth - 126)),
        top: Math.min(anchor.y + 8, window.innerHeight - 148),
        transform: "translateX(-50%)",
        display: "flex",
        flexDirection: "column",
        width: 236,
        padding: 6,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        boxShadow: shadow.pop,
      }}
    >
      <div
        aria-hidden="true"
        style={{
          padding: "2px 4px 6px",
          font: `600 10px ${font.sans}`,
          letterSpacing: ".05em",
          color: color.muted2,
        }}
      >
        TEXT STYLE
      </div>
      <div
        style={{
          alignSelf: "stretch",
          display: "grid",
          gridTemplateColumns: "repeat(4, 1fr)",
          gap: 4,
        }}
      >
        {HEADINGS.map(({ kind, label, glyph }) => {
          const active = blockKind === kind;
          return (
            <button
              key={kind}
              type="button"
              aria-label={label}
              aria-pressed={active}
              title={label}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => onTurnInto(active ? "paragraph" : kind)}
              style={{ ...cell(active), font: `700 12px ${font.sans}` }}
            >
              {glyph}
            </button>
          );
        })}
        {MARKS.map(({ kind, label, glyph }) => {
          const active = rangeHasMark(marks, range, kind);
          return (
            <button
              key={kind}
              type="button"
              aria-label={label}
              aria-pressed={active}
              title={label}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => onMark(kind, !active)}
              style={{
                ...cell(active),
                font: `${kind === "bold" ? 750 : 600} 12px ${kind === "code" ? font.mono : font.sans}`,
                fontStyle: kind === "italic" ? "italic" : "normal",
                textDecoration:
                  kind === "underline" ? "underline" : kind === "strikethrough" ? "line-through" : "none",
              }}
            >
              {glyph}
            </button>
          );
        })}
      </div>
      <div
        aria-hidden="true"
        style={{
          alignSelf: "stretch",
          height: 1,
          margin: "6px 2px 2px",
          background: color.borderSoft,
        }}
      />
      <button
        type="button"
        aria-label="Comment on selected text"
        title="Comment"
        onMouseDown={(event) => event.preventDefault()}
        onClick={() => onComment()}
        style={{
          all: "unset",
          cursor: "pointer",
          alignSelf: "stretch",
          height: 32,
          display: "flex",
          alignItems: "center",
          gap: 9,
          padding: "0 9px",
          borderRadius: radius.sm,
          color: color.muted3,
          font: `600 12px ${font.sans}`,
        }}
      >
        <Icon name="chat" size={15} strokeWidth={1.9} />
        Comment
        <span
          aria-hidden="true"
          style={{ marginLeft: "auto", font: `600 11px ${font.mono}`, color: color.muted2 }}
        >
          ⌘/
        </span>
      </button>
    </div>
  );
}
