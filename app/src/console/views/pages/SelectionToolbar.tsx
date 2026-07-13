import type { InlineMark, RelativeAnchor, SpanMark } from "../../../domain/pages-client";
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

/** Notion-style inline actions for the exact selected text. */
export function SelectionToolbar({
  blockId,
  marks,
  range,
  anchor,
  onMark,
  onComment,
  onDismiss,
}: {
  blockId: string;
  marks: SpanMark[];
  range: RelativeAnchor;
  anchor: CommentAnchor;
  onMark: (kind: InlineMark, active: boolean) => void;
  onComment: (anchor: CommentAnchor) => void;
  onDismiss: () => void;
}) {
  return (
    <div
      role="toolbar"
      aria-label="Selection actions"
      data-selection-toolbar={blockId}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) onDismiss();
      }}
      style={{
        position: "fixed",
        zIndex: 35,
        left: Math.max(124, Math.min(anchor.x, window.innerWidth - 124)),
        top: Math.min(anchor.y + 8, window.innerHeight - 104),
        transform: "translateX(-50%)",
        display: "flex",
        flexDirection: "column",
        width: 232,
        padding: 6,
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        boxShadow: shadow.pop,
      }}
    >
      <div
        style={{
          alignSelf: "stretch",
          display: "grid",
          gridTemplateColumns: "repeat(5, 1fr)",
          gap: 4,
        }}
      >
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
                all: "unset",
                cursor: "pointer",
                height: 30,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                borderRadius: radius.sm,
                background: active
                  ? `color-mix(in srgb, ${accentVar} 28%, ${color.paper})`
                  : color.hover,
                color: active ? accentVar : color.muted3,
                font: `${kind === "bold" ? 750 : 600} 12px ${kind === "code" ? font.mono : font.sans}`,
                fontStyle: kind === "italic" ? "italic" : "normal",
                textDecoration: kind === "underline" ? "underline" : kind === "strikethrough" ? "line-through" : "none",
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
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          onComment({ x: rect.left, y: rect.bottom });
        }}
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
      </button>
    </div>
  );
}
