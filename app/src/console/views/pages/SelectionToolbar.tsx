import type { BlockKind } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import { color, font, radius, shadow } from "../../theme/tokens";
import { SLASH_KINDS } from "./pages-model";
import type { CommentAnchor } from "./CommentCard";

const STYLES = SLASH_KINDS.filter(
  ({ kind }) => kind !== "page" && kind !== "divider",
);

/** The one extra interaction depth text selection was missing: change the
 * whole block style or open its discussion without hunting either gutter. */
export function SelectionToolbar({
  blockId,
  kind,
  anchor,
  onStyle,
  onComment,
  onDismiss,
}: {
  blockId: string;
  kind: BlockKind;
  anchor: CommentAnchor;
  onStyle: (kind: BlockKind) => void;
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
        left: Math.max(110, Math.min(anchor.x, window.innerWidth - 110)),
        top: Math.max(8, anchor.y - 42),
        transform: "translateX(-50%)",
        display: "flex",
        alignItems: "center",
        gap: 3,
        padding: 4,
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        boxShadow: shadow.pop,
      }}
    >
      <select
        aria-label="Block style"
        value={kind}
        onChange={(event) => onStyle(event.target.value as BlockKind)}
        style={{
          height: 28,
          maxWidth: 124,
          border: 0,
          outline: 0,
          borderRadius: radius.sm,
          background: color.paper,
          color: color.ink,
          font: `600 11.5px ${font.sans}`,
          cursor: "pointer",
        }}
      >
        {STYLES.map((option) => (
          <option key={option.kind} value={option.kind}>
            {option.label}
          </option>
        ))}
      </select>
      <div aria-hidden="true" style={{ width: 1, height: 18, background: color.borderSoft }} />
      <button
        type="button"
        aria-label="Comment on selected block"
        title="Comment"
        onMouseDown={(event) => event.preventDefault()}
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          onComment({ x: rect.left, y: rect.bottom });
        }}
        style={{
          all: "unset",
          cursor: "pointer",
          width: 30,
          height: 28,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          borderRadius: radius.sm,
          color: color.muted3,
        }}
      >
        <Icon name="chat" size={15} strokeWidth={1.9} />
      </button>
    </div>
  );
}
