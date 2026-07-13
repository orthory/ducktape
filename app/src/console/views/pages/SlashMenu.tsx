// The "/" command palette over every block kind. Rendered by BlockRow inside the
// row's relative box, so it hangs beneath the caret.

import type { BlockKind } from "../../../domain/pages-client";
import { color, font, radius } from "../../theme/tokens";
import { filterSlashKinds } from "./pages-model";

export function SlashMenu({
  query,
  activeIndex,
  onPick,
}: {
  query: string;
  activeIndex: number;
  onPick: (kind: BlockKind) => void;
}) {
  const options = filterSlashKinds(query);
  if (options.length === 0) return null;
  return (
    <div
      role="listbox"
      aria-label="Block kind menu"
      style={{
        position: "absolute",
        zIndex: 20,
        top: "100%",
        left: 0,
        marginTop: 4,
        width: 220,
        maxHeight: 280,
        overflowY: "auto",
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        boxShadow: "0 20px 48px rgba(0,0,0,.50)",
        padding: 4,
      }}
    >
      {options.map((option, i) => (
        <button
          key={option.kind}
          type="button"
          role="option"
          aria-selected={i === activeIndex}
          onMouseDown={(event) => {
            // mousedown, not click: the textarea must not blur-commit first.
            event.preventDefault();
            onPick(option.kind);
          }}
          style={{
            all: "unset",
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: 8,
            width: "100%",
            boxSizing: "border-box",
            padding: "6px 9px",
            borderRadius: radius.sm,
            background: i === activeIndex ? color.hover : "transparent",
          }}
        >
          <span style={{ font: `600 12px ${font.sans}`, color: color.ink }}>
            {option.label}
          </span>
          <span
            style={{
              marginLeft: "auto",
              font: `400 10.5px ${font.mono}`,
              color: color.muted2,
            }}
          >
            {option.hint}
          </span>
        </button>
      ))}
    </div>
  );
}
