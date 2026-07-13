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
import { RemoteCursors, type PagePresencePeer } from "./PagePresence";

export function PageTitle({
  pageId,
  raw,
  titleRef,
  onCommit,
  onDescend,
  presence,
  onCursor,
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
  presence: PagePresencePeer[];
  onCursor: (blockId: string | null, anchor: number, head: number) => void;
}) {
  const { icon, title } = splitTitleEmoji(raw);
  const [draft, setDraft] = useState(title);
  const [picking, setPicking] = useState(false);
  const [hover, setHover] = useState(false);
  const focusedRef = useRef(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const publishCursor = (el: HTMLInputElement) =>
    onCursor(pageId, el.selectionStart ?? 0, el.selectionEnd ?? 0);

  // adopt the store title only while the input is not being edited — the same
  // draft-protection contract as a block row...
  useEffect(() => {
    if (!focusedRef.current) setDraft(splitTitleEmoji(raw).title);
  }, [raw]);
  // ...but a page switch resets unconditionally (declared after, so it wins
  // when both fire): a still-focused input must never carry one page's draft
  // onto another.
  useEffect(() => setDraft(splitTitleEmoji(raw).title), [pageId]);

  // Two bugs lived in this commit, and they share one cause: `icon` is re-derived
  // from the STORE on every render, while `draft` is a local copy of an older
  // store. Composing the two is only sound while the invariant "the draft carries
  // no icon" holds — and typing breaks it.
  //
  //  1. THE EMOJI DOUBLED. Type "🚀 plan" into the input: it commits verbatim
  //     (raw had no icon), and now raw's LEADING EMOJI — its icon — is 🚀, while
  //     the still-focused draft (the focus guard blocks the store sync) also
  //     holds it. The next boundary composed 🚀 onto "🚀 plan" → "🚀 🚀 plan",
  //     and the one after that added another. So the draft is split too: a
  //     leading emoji in the DRAFT is the icon, and commit is idempotent.
  //
  //  2. IT RENAMED PAGES YOU ONLY OPENED. splitTitleEmoji eats the whitespace
  //     after the emoji and composeTitle always re-emits exactly one space, so
  //     the round-trip is not the identity for a title like "🦆Launch" — and the
  //     old boundary condition (`composeTitle(icon, draft) !== raw`) was
  //     therefore TRUE on mount, with no keystroke anywhere near it. Opening the
  //     page committed a rename. An untouched draft now commits nothing, ever:
  //     that is the only honest trigger for an edit boundary.
  const commit = () => {
    if (draft === title) return; // untouched — never rewrite a title nobody typed in
    const typed = splitTitleEmoji(draft);
    const next = composeTitle(typed.icon ?? icon, typed.title);
    if (next !== raw) onCommit(next);
  };
  // the ref keeps the boundary timer from resetting on unrelated store
  // re-renders; pageId in the deps cancels a pending boundary on a switch.
  const commitRef = useRef(() => {});
  commitRef.current = commit;
  useEffect(() => {
    if (draft === title) return;
    const timer = setTimeout(() => commitRef.current(), EDIT_BOUNDARY_MS);
    return () => clearTimeout(timer);
  }, [draft, title, pageId]);

  return (
    <div
      ref={rootRef}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{ position: "relative", marginBottom: 18 }}
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
            // taken off again. The draft is split first: if the user typed an
            // emoji into it, committing it verbatim would just re-make an icon.
            onClick={() => onCommit(splitTitleEmoji(draft).title)}
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
            // same split: the picked emoji is THE icon, never a second one in
            // front of an emoji the user already typed into the title.
            onPick={(emoji) => onCommit(composeTitle(emoji, splitTitleEmoji(draft).title))}
            onClose={() => setPicking(false)}
          />
        ) : null}
      </div>

      <input
        ref={titleRef}
        aria-label="Page title"
        value={draft}
        onChange={(event) => {
          setDraft(event.target.value);
          publishCursor(event.currentTarget);
        }}
        onSelect={(event) => publishCursor(event.currentTarget)}
        onFocus={(event) => {
          focusedRef.current = true;
          publishCursor(event.currentTarget);
        }}
        onBlur={() => {
          focusedRef.current = false;
          onCursor(null, 0, 0);
          commit();
          // an emoji the user typed at the front IS the icon now (commit just
          // wrote it as one), and the input holds the title WITHOUT the icon. The
          // focus guard blocks the store sync while the input is live, so leaving
          // it is the moment to drop it — during typing it would yank the caret.
          setDraft((current) => splitTitleEmoji(current).title);
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
      <RemoteCursors peers={presence} areaRef={titleRef} rowRef={rootRef} text={draft} />
    </div>
  );
}
