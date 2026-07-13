import type { CSSProperties } from "react";

import type { RelativeAnchor, SpanMark } from "../../../domain/pages-client";
import { color, radius } from "../../theme/tokens";

/** Styled mirror behind the native textarea. The textarea keeps its proven
 * keyboard/IME/selection behavior; this layer only paints persistent spans. */
export function InlineText({
  text,
  marks,
  comments,
  fontStyle,
  done,
}: {
  text: string;
  marks: SpanMark[];
  comments: RelativeAnchor[];
  fontStyle: string;
  done: boolean;
}) {
  const boundaries = new Set([0, text.length]);
  for (const range of [...marks, ...comments]) {
    boundaries.add(Math.max(0, Math.min(text.length, range.start)));
    boundaries.add(Math.max(0, Math.min(text.length, range.end)));
  }
  const points = [...boundaries].sort((a, b) => a - b);
  return (
    <div
      aria-hidden="true"
      data-inline-text
      style={{
        position: "absolute",
        inset: 0,
        pointerEvents: "none",
        whiteSpace: "pre-wrap",
        overflowWrap: "break-word",
        color: done ? color.muted2 : color.ink,
        font: fontStyle,
        textDecoration: done ? "line-through" : "none",
      }}
    >
      {points.slice(0, -1).map((start, index) => {
        const end = points[index + 1]!;
        const active = marks.filter((mark) => mark.start <= start && mark.end >= end);
        const commented = comments.some((range) => range.start < end && range.end > start);
        const decoration = [
          done ? "line-through" : "",
          active.some((mark) => mark.kind === "underline") ? "underline" : "",
          active.some((mark) => mark.kind === "strikethrough") ? "line-through" : "",
        ].filter(Boolean).join(" ");
        const code = active.some((mark) => mark.kind === "code");
        const style: CSSProperties = {
          fontWeight: active.some((mark) => mark.kind === "bold") ? 750 : undefined,
          fontStyle: active.some((mark) => mark.kind === "italic") ? "italic" : undefined,
          textDecorationLine: decoration || undefined,
          background: commented
            ? `color-mix(in srgb, ${color.amber} 24%, transparent)`
            : code
              ? color.sunken
              : undefined,
          boxShadow: commented ? `inset 0 -2px ${color.amber}` : undefined,
          borderRadius: commented || code ? radius.sm / 2 : undefined,
        };
        return <span key={`${start}:${end}`} style={style}>{text.slice(start, end)}</span>;
      })}
    </div>
  );
}
