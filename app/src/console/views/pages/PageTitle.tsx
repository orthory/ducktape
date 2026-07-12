// The document title, with its icon.
//
// The icon is NOT a wire field — a new `icon` on the Block would move the
// committed bytes and force an app-hash flag day across every validator, for a
// decoration. It is the leading emoji of the title itself (page-icon.ts): shown
// as an icon, edited by the picker, and the input holds the title without it.
// The rename rides the same edit-boundary contract as a block row: a typing
// pause commits one op.

import { useEffect, useRef, useState } from "react";
import type { MutableRefObject } from "react";

import { EmojiPicker } from "../chat/EmojiPicker";
import { color, font, radius } from "../../theme/tokens";
import { EDIT_BOUNDARY_MS } from "./pages-model";
import { composeTitle, splitTitleEmoji } from "./page-icon";

export function PageTitle({
  pageId,
  raw,
  titleRef,
  onCommit,
  onDescend,
}: {
  /** The open page's id — a switch resets the draft unconditionally. */
  pageId: string;
  /** The page root's committed text: "🦆 Launch plan" — icon and all. */
  raw: string;
  titleRef: MutableRefObject<HTMLInputElement | null>;
  /** Commit the raw title (icon composed back in). */
  onCommit: (raw: string) => void;
  /** Enter / ArrowDown out of the title, into the body. */
  onDescend: () => void;
}) {
  const { icon, title } = splitTitleEmoji(raw);
  const [draft, setDraft] = useState(title);
  const [picking, setPicking] = useState(false);
  const [hover, setHover] = useState(false);
  const focusedRef = useRef(false);

  // adopt the store title only while the input is not being edited — the same
  // draft-protection contract as a block row...
  useEffect(() => {
    if (!focusedRef.current) setDraft(splitTitleEmoji(raw).title);
  }, [raw]);
  // ...but a page switch resets unconditionally (declared after, so it wins
  // when both fire): a still-focused input must never carry one page's draft
  // onto another.
  useEffect(() => setDraft(splitTitleEmoji(raw).title), [pageId]);

  const commit = () => {
    const next = composeTitle(icon, draft);
    if (next !== raw) onCommit(next);
  };
  // the ref keeps the boundary timer from resetting on unrelated store
  // re-renders; pageId in the deps cancels a pending boundary on a switch.
  const commitRef = useRef(() => {});
  commitRef.current = commit;
  useEffect(() => {
    if (composeTitle(icon, draft) === raw) return;
    const timer = setTimeout(() => commitRef.current(), EDIT_BOUNDARY_MS);
    return () => clearTimeout(timer);
  }, [draft, raw, icon, pageId]);

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{ marginBottom: 18 }}
    >
      <div style={{ position: "relative", display: "flex", alignItems: "center", gap: 6, height: 30 }}>
        <button
          type="button"
          aria-label={icon ? "Change page icon" : "Add page icon"}
          aria-expanded={picking}
          onClick={() => setPicking((open) => !open)}
          style={{
            all: "unset",
            cursor: "pointer",
            display: icon || hover || picking ? "flex" : "none",
            alignItems: "center",
            gap: 5,
            padding: icon ? 0 : "3px 7px",
            borderRadius: radius.sm,
            border: icon ? "none" : `1px dashed ${color.borderStrong}`,
            color: color.muted2,
            font: icon
              ? "27px/1 'Apple Color Emoji', 'Segoe UI Emoji', sans-serif"
              : `500 11px ${font.sans}`,
          }}
        >
          {icon ?? "Add icon"}
        </button>
        {icon && (hover || picking) ? (
          <button
            type="button"
            aria-label="Remove page icon"
            title="Remove icon"
            // the input holds the title WITHOUT the icon, so the emoji is
            // unreachable from the keyboard — without this it could never be
            // taken off again.
            onClick={() => onCommit(draft)}
            style={{
              all: "unset",
              cursor: "pointer",
              padding: "2px 5px",
              borderRadius: radius.sm,
              color: color.muted2,
              font: `500 10.5px ${font.sans}`,
            }}
          >
            Remove
          </button>
        ) : null}
        {picking ? (
          <EmojiPicker
            onPick={(emoji) => onCommit(composeTitle(emoji, draft))}
            onClose={() => setPicking(false)}
          />
        ) : null}
      </div>

      <input
        ref={titleRef}
        aria-label="Page title"
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onFocus={() => {
          focusedRef.current = true;
        }}
        onBlur={() => {
          focusedRef.current = false;
          commit();
        }}
        onKeyDown={(event) => {
          if (event.key !== "Enter" && event.key !== "ArrowDown") return;
          event.preventDefault();
          commit();
          onDescend();
        }}
        placeholder="Untitled"
        spellCheck={false}
        style={{
          width: "100%",
          boxSizing: "border-box",
          border: "none",
          outline: "none",
          background: "transparent",
          padding: 0,
          marginTop: 4,
          color: color.dark,
          font: `650 30px/1.2 ${font.sans}`,
        }}
      />
    </div>
  );
}
