// @mention typeahead for BARE textareas — the pages comment composers. The
// chat Composer carries its own inline copy of this state machine (entangled
// with the `[[` page-ref menu and its toolbar); this hook packages the same
// contract for surfaces that only need `@`: mentionTokenAt over (value, caret),
// candidates from the console store (absent store — bare component tests — no
// menu, no crash), ArrowUp/Down to step, Enter/Tab to pick, Escape to dismiss
// THAT token until it changes.
//
// The menu PORTALS to document.body at fixed coordinates: both comment
// surfaces (the floating card, the aside panel) clip overflow, so an in-flow
// absolute menu would truncate. The wrapper carries data-mention-menu so
// outside-press/outside-scroll dismissal (CommentCard) can recognize presses
// and scrolls inside the menu as "inside".

import { useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ChangeEvent, KeyboardEvent, ReactNode, RefObject, SyntheticEvent } from "react";
import { createPortal } from "react-dom";

import type { AgentRecord } from "../../../domain/agent-client";
import { ConsoleContext } from "../../store/context";
import { MentionMenu } from "./MentionMenu";
import {
  insertMention,
  mentionCandidateToken,
  mentionCandidatesAll,
  mentionTokenAt,
  mentionableUsers,
} from "./mention";

const EMPTY_AGENTS: AgentRecord[] = [];
const EMPTY_NODE_USERS: Record<string, { accountId: string; name: string | null }> = {};

/** True when `target` sits inside a portaled mention menu — for dismissal
 *  handlers that treat "outside my box" as close (the menu is outside every
 *  box in the DOM, but inside in intent). */
export const inMentionMenu = (target: EventTarget | null): boolean =>
  target instanceof Element && target.closest("[data-mention-menu]") !== null;

export function useMentionMenu(
  value: string,
  onChange: (next: string) => void,
  ref: RefObject<HTMLTextAreaElement | null>,
): {
  /** Render this once, anywhere in the component's tree (it portals out). */
  menu: ReactNode;
  /** Textarea onChange — tracks the caret, then forwards to `onChange`. */
  onTextChange: (event: ChangeEvent<HTMLTextAreaElement>) => void;
  /** Textarea onSelect — caret moves without text changes (clicks, arrows). */
  onSelect: (event: SyntheticEvent<HTMLTextAreaElement>) => void;
  /** Run FIRST in the textarea's onKeyDown; true = the menu consumed the key
   *  (the caller must not submit/cancel on it). */
  onKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => boolean;
} {
  const store = useContext(ConsoleContext);
  const agents = store?.state.agents ?? EMPTY_AGENTS;
  const nodeUsers = store?.state.nodeUsers ?? EMPTY_NODE_USERS;
  const users = useMemo(() => mentionableUsers(nodeUsers, agents), [nodeUsers, agents]);

  const [caret, setCaret] = useState(0);
  const [index, setIndex] = useState(0);
  // The token.start the user Escaped out of — that token stays dismissed
  // until it's gone (the chat Composer's `mentionDismissedAt` idiom).
  const [dismissedAt, setDismissedAt] = useState<number | null>(null);
  const pendingCaret = useRef<number | null>(null);

  const token = mentionTokenAt(value, caret);
  const start = token?.start ?? null;
  const query = token?.query ?? "";
  const candidates = useMemo(
    () =>
      start !== null && start !== dismissedAt ? mentionCandidatesAll(agents, users, query) : [],
    [agents, users, query, start, dismissedAt],
  );
  const open = candidates.length > 0;
  const active = open ? Math.min(index, candidates.length - 1) : 0;

  // Restore the caret after a pick re-renders the controlled value.
  useEffect(() => {
    if (pendingCaret.current === null) return;
    const at = pendingCaret.current;
    pendingCaret.current = null;
    const el = ref.current;
    el?.focus();
    el?.setSelectionRange(at, at);
  }, [value, ref]);

  const pick = (tokenText: string) => {
    if (!token) return;
    const el = ref.current;
    const next = insertMention(value, token, el?.selectionStart ?? caret, tokenText);
    pendingCaret.current = next.caret;
    setCaret(next.caret);
    setIndex(0);
    onChange(next.text);
  };

  const onTextChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    const next = event.target.value;
    const nextCaret = event.target.selectionStart;
    setCaret(nextCaret);
    const nextToken = mentionTokenAt(next, nextCaret);
    if (nextToken) setIndex(0);
    if (!nextToken || nextToken.start !== dismissedAt) setDismissedAt(null);
    onChange(next);
  };

  const onSelect = (event: SyntheticEvent<HTMLTextAreaElement>) => {
    // React's select plugin re-fires DURING the pick's own keydown, reporting
    // the PRE-INSERT DOM caret — that stale write would overwrite the caret
    // the pick just set and hold the menu open on the consumed token. While a
    // programmatic restore is pending, selection reports describe the past;
    // drop them (the restore effect below clears the flag).
    if (pendingCaret.current !== null) return;
    setCaret(event.currentTarget.selectionStart);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>): boolean => {
    // IME guard: committing a Korean/Japanese/Chinese candidate must not pick.
    if (!open || event.nativeEvent.isComposing) return false;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setIndex((active + 1) % candidates.length);
      return true;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setIndex((active - 1 + candidates.length) % candidates.length);
      return true;
    }
    if (event.key === "Enter" || event.key === "Tab") {
      event.preventDefault();
      pick(mentionCandidateToken(candidates[active]!));
      return true;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      // stopPropagation: with the menu open, Escape means "dismiss the menu" —
      // it must not also close the comment card (a document-level listener).
      event.stopPropagation();
      setDismissedAt(start);
      return true;
    }
    return false;
  };

  // Anchor at render time: every keystroke re-renders, so the fixed wrapper
  // tracks the textarea as it grows. MentionMenu positions itself at
  // bottom:100% of the wrapper — a zero-height line at the textarea's top —
  // so the list opens UPWARD, clear of the text being typed.
  const rect = open ? ref.current?.getBoundingClientRect() : undefined;
  const menu =
    open && rect
      ? createPortal(
          <div
            data-mention-menu
            style={{ position: "fixed", left: rect.left, top: rect.top, zIndex: 60 }}
          >
            <MentionMenu candidates={candidates} activeIndex={active} onPick={pick} />
          </div>,
          document.body,
        )
      : null;

  return { menu, onTextChange, onSelect, onKeyDown };
}
