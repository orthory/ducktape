// The row's LEFT hover affordances — Notion's `+` and `⋮⋮`.
//
// They replace a right-hand gutter whose bare `X` deleted a block AND ITS WHOLE
// SUBTREE on one click, with no confirm and no undo. Destruction now lives
// behind the handle menu (and, for a block with children, behind a confirm),
// while `+` and the drag handle sit where the hand expects them.
//
// The gutter is OVERLAID in the left margin (absolute, negative offset), so it
// reserves no column width: the text column keeps its full measure and every
// row keeps one left edge, flush with the page title.

import { useEffect, useState } from "react";
import type { CSSProperties, DragEvent } from "react";

import type { BlockKind } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import { color, font, radius, shadow } from "../../theme/tokens";
import { SLASH_KINDS } from "./pages-model";
import { GUTTER_WIDTH, MARKER_HANG, ROW_PAD_Y } from "./pages-style";

// "Turn into" is the slash menu's catalogue minus Page — picking Page there
// spawns a subpage, which is a creation, not a conversion.
const TURN_INTO = SLASH_KINDS.filter((option) => option.kind !== "page");

export function BlockGutter({
  blockNumber,
  kind,
  visible,
  onInsertBelow,
  onTurnInto,
  onDuplicate,
  onRemove,
  onDragStart,
  onDragEnd,
}: {
  /** 1-based row number — the whole editor labels rows this way. */
  blockNumber: number;
  kind: BlockKind;
  /** The row is hovered. The gutter also shows itself while its menu is open. */
  visible: boolean;
  onInsertBelow: () => void;
  onTurnInto: (kind: BlockKind) => void;
  onDuplicate: () => void;
  onRemove: () => void;
  onDragStart: (event: DragEvent) => void;
  onDragEnd: () => void;
}) {
  const [menuOpen, setMenuOpen] = useState(false);

  // Escape / outside-click dismiss, attached a tick late so the click that
  // OPENED the menu doesn't immediately close it (the EmojiPicker's pattern).
  useEffect(() => {
    if (!menuOpen) return;
    const close = () => setMenuOpen(false);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    const timer = setTimeout(() => document.addEventListener("click", close), 0);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("click", close);
      clearTimeout(timer);
    };
  }, [menuOpen]);

  const shown = visible || menuOpen;

  return (
    <div
      onClick={(event) => event.stopPropagation()}
      style={{
        position: "absolute",
        left: -(MARKER_HANG + GUTTER_WIDTH),
        top: ROW_PAD_Y,
        width: GUTTER_WIDTH,
        height: 24,
        display: "flex",
        alignItems: "center",
        justifyContent: "flex-end",
        gap: 1,
        // hidden by opacity, not unmounted: the buttons keep their box, so the
        // pointer never falls through a gap between them on the way over.
        opacity: shown ? 1 : 0,
        pointerEvents: shown ? "auto" : "none",
        transition: "opacity 90ms ease",
      }}
    >
      <button
        type="button"
        aria-label={`Insert block below block ${blockNumber}`}
        title="Insert a block below"
        // mousedown, not click: a focused block must append BEFORE its blur
        // commit re-renders the tree out from under the click.
        onMouseDown={(event) => {
          event.preventDefault();
          onInsertBelow();
        }}
        style={gutterBtn}
      >
        <Icon name="plus" size={14} strokeWidth={1.9} />
      </button>
      <button
        type="button"
        aria-label={`Block ${blockNumber} actions`}
        title="Drag to move, click to open actions"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        draggable
        onDragStart={onDragStart}
        onDragEnd={onDragEnd}
        onClick={() => setMenuOpen((open) => !open)}
        style={{ ...gutterBtn, cursor: "grab" }}
      >
        <span aria-hidden="true" style={{ font: `700 12px/1 ${font.sans}`, letterSpacing: "-1px" }}>
          ⋮⋮
        </span>
      </button>

      {menuOpen ? (
        <div
          role="menu"
          aria-label={`Block ${blockNumber} actions`}
          style={{
            position: "absolute",
            zIndex: 30,
            top: 24,
            left: 0,
            width: 208,
            maxHeight: 330,
            overflowY: "auto",
            border: `1px solid ${color.border}`,
            borderRadius: radius.md,
            background: color.paper,
            boxShadow: shadow.pop,
            padding: 4,
          }}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setMenuOpen(false);
              onDuplicate();
            }}
            style={menuItem}
          >
            Duplicate
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setMenuOpen(false);
              onRemove();
            }}
            style={{ ...menuItem, color: color.red }}
          >
            Delete
          </button>
          <div style={{ height: 1, background: color.borderSoft, margin: "4px 2px" }} />
          <div style={sectionLabel}>Turn into</div>
          {TURN_INTO.map((option) => (
            <button
              key={option.kind}
              type="button"
              role="menuitem"
              aria-current={option.kind === kind}
              onClick={() => {
                setMenuOpen(false);
                onTurnInto(option.kind);
              }}
              style={{
                ...menuItem,
                display: "flex",
                alignItems: "center",
                gap: 8,
                background: option.kind === kind ? color.hover : "transparent",
              }}
            >
              <span>{option.label}</span>
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
      ) : null}
    </div>
  );
}

const gutterBtn: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  width: 21,
  height: 22,
  borderRadius: 5,
  flexShrink: 0,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  color: color.muted2,
};

const menuItem: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  display: "block",
  width: "100%",
  boxSizing: "border-box",
  padding: "6px 9px",
  borderRadius: radius.sm,
  font: `500 13px ${font.sans}`,
  color: color.ink,
  whiteSpace: "nowrap",
};

const sectionLabel: CSSProperties = {
  padding: "6px 9px 4px",
  font: `600 10px ${font.sans}`,
  letterSpacing: ".05em",
  textTransform: "uppercase",
  color: color.muted2,
};
