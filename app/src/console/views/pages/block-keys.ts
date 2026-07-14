// The editor's key grammar, as a pure function of (keystroke, block, caret).
//
// A block used to be an atomic text cell: Enter appended an empty sibling
// instead of splitting at the caret, Backspace only fired on a wholly-empty
// block, and focus was a bare block id that could not say *where* the caret
// belonged. Everything that decides "what does this keystroke mean" now lives
// here, DOM-free, so it can be tested without a textarea.
//
// The view keeps the DOM: it reads selectionStart off the event, calls
// resolveKey, and dispatches the intent. It never re-derives the grammar.

import type { BlockKind } from "../../../domain/pages-client";
import { emptyEnterExits } from "./pages-model";

/** Where the caret lands in a block we are focusing. A number is an absolute
 *  offset — used by merge-prev, which lands the caret on the seam. */
export type Caret = "start" | "end" | number;

/** Which block to focus, and where in it. Focus used to be `string | null`,
 *  which could not express the caret, so every focus hop slammed it to the end
 *  of the text. */
export interface FocusIntent {
  id: string;
  caret: Caret;
}

/** Resolve a Caret against a concrete value length, clamped into range. */
export const caretOffset = (caret: Caret, len: number): number =>
  caret === "start"
    ? 0
    : caret === "end"
      ? len
      : Math.max(0, Math.min(caret, len));

/** Split a block's text at the caret. The left half stays, the right half
 *  moves to the new sibling. */
export function splitText(value: string, caret: number): { left: string; right: string } {
  const at = Math.max(0, Math.min(caret, value.length));
  return { left: value.slice(0, at), right: value.slice(at) };
}

/** Join a block into its predecessor. `seam` is where the caret belongs after
 *  the join — the previous block's original end. */
export function mergeText(prev: string, cur: string): { text: string; seam: number } {
  return { text: prev + cur, seam: prev.length };
}

/** Everything resolveKey needs to know, with no DOM in sight. */
export interface KeyContext {
  key: string;
  shiftKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  /** The live draft, not the committed block text. */
  value: string;
  caretStart: number;
  caretEnd: number;
  kind: BlockKind;
  /** True while the slash menu is capturing keys — it owns them all. */
  slashOpen: boolean;
  /** The kind of the row above, or null at the top. Backspace at offset 0
   *  needs it to tell "merge into prose" from "delete the divider above". */
  prevKind: BlockKind | null;
}

export type KeyIntent =
  | { type: "split"; left: string; right: string }
  | { type: "merge-prev" }
  | { type: "remove-empty" }
  | { type: "remove-divider-above" }
  | { type: "exit-to-paragraph" }
  | { type: "toggle-check" }
  | { type: "toggle-collapse" }
  | { type: "edit-code"; value: string; selectionStart: number; selectionEnd: number }
  | { type: "indent" }
  | { type: "outdent" }
  | { type: "move-up" }
  | { type: "move-down" }
  | { type: "focus-prev" }
  | { type: "focus-next" }
  | { type: "none" };

const NONE: KeyIntent = { type: "none" };

/** A collapsed caret sitting at offset 0. */
const atStart = (ctx: KeyContext): boolean => ctx.caretStart === 0 && ctx.caretEnd === 0;

/** A collapsed caret sitting at the very end. */
const atEnd = (ctx: KeyContext): boolean =>
  ctx.caretStart === ctx.value.length && ctx.caretEnd === ctx.value.length;

/** Tab edits code text; it never becomes a block-tree command. A selection is
 *  indented line-by-line so the selected code survives. Shift+Tab removes one
 *  leading tab from each affected line and is still consumed when there is
 *  nothing to remove, keeping focus inside the editor. */
function editCode(ctx: KeyContext): KeyIntent {
  const start = Math.max(0, Math.min(ctx.caretStart, ctx.value.length));
  const end = Math.max(start, Math.min(ctx.caretEnd, ctx.value.length));
  if (!ctx.shiftKey && start === end) {
    return {
      type: "edit-code",
      value: `${ctx.value.slice(0, start)}\t${ctx.value.slice(end)}`,
      selectionStart: start + 1,
      selectionEnd: start + 1,
    };
  }

  const first = ctx.value.slice(0, start).lastIndexOf("\n") + 1;
  const stop = end > start && ctx.value[end - 1] === "\n" ? end - 1 : end;
  const lines = [first];
  for (let i = first; i < stop; i += 1) {
    if (ctx.value[i] === "\n") lines.push(i + 1);
  }

  if (!ctx.shiftKey) {
    let value = ctx.value;
    for (const at of [...lines].reverse()) value = `${value.slice(0, at)}\t${value.slice(at)}`;
    return {
      type: "edit-code",
      value,
      selectionStart: start + 1,
      selectionEnd: end + lines.length,
    };
  }

  const removable = lines.filter((at) => ctx.value[at] === "\t");
  let value = ctx.value;
  for (const at of [...removable].reverse()) value = value.slice(0, at) + value.slice(at + 1);
  return {
    type: "edit-code",
    value,
    selectionStart: start - removable.filter((at) => at < start).length,
    selectionEnd: end - removable.filter((at) => at < end).length,
  };
}

export function resolveKey(ctx: KeyContext): KeyIntent {
  // the slash menu is a command in progress; it owns every key it handles.
  if (ctx.slashOpen) return NONE;

  const mod = ctx.metaKey || ctx.ctrlKey;

  // mod+Enter acts on the block, it never splits it. Guarding this first is
  // the whole fix for "Cmd+Enter on a to-do makes a new block": the old Enter
  // branch only excluded shiftKey.
  if (mod && ctx.key === "Enter") {
    if (ctx.kind === "todo") return { type: "toggle-check" };
    if (ctx.kind === "toggle") return { type: "toggle-collapse" };
    return NONE;
  }

  if (ctx.key === "Enter" && !ctx.shiftKey && !mod) {
    // an empty block of a kind you can "escape" drops back to a paragraph
    // rather than nesting another one below itself.
    if (ctx.value.trim() === "" && emptyEnterExits(ctx.kind)) {
      return { type: "exit-to-paragraph" };
    }
    // Enter inside a code block is a newline, not a split. The textarea's
    // default already does that, so hand the key back to it.
    if (ctx.kind === "code") return NONE;
    return { type: "split", ...splitText(ctx.value, ctx.caretStart) };
  }

  if (ctx.key === "Backspace") {
    if (ctx.value === "") return { type: "remove-empty" };
    if (atStart(ctx)) {
      // a divider has no textarea, so it can never be focused and deleted on
      // its own. Backspace from the block below is its only keyboard exit.
      if (ctx.prevKind === "divider") return { type: "remove-divider-above" };
      return { type: "merge-prev" };
    }
    return NONE;
  }

  if (ctx.key === "Tab") {
    if (ctx.kind === "code") return editCode(ctx);
    return ctx.shiftKey ? { type: "outdent" } : { type: "indent" };
  }

  if (ctx.altKey && ctx.key === "ArrowUp") return { type: "move-up" };
  if (ctx.altKey && ctx.key === "ArrowDown") return { type: "move-down" };

  // Leaving a block by its edges. ArrowLeft/Right only cross when the caret has
  // nowhere left to go inside this block; ArrowUp/Down cross from the first and
  // last offsets. The caret's landing spot is the caller's business — see
  // FOCUS_PREV_CARET / FOCUS_NEXT_CARET.
  if (!ctx.altKey && !mod) {
    if ((ctx.key === "ArrowUp" || ctx.key === "ArrowLeft") && atStart(ctx)) {
      return { type: "focus-prev" };
    }
    if ((ctx.key === "ArrowDown" || ctx.key === "ArrowRight") && atEnd(ctx)) {
      return { type: "focus-next" };
    }
  }

  return NONE;
}

// Where the caret lands when a keystroke walks off the edge of a block.
//
// The rule is line adjacency, and it reads backwards at first glance. Moving UP
// from a block's first line lands on the previous block's *last* line, so the
// caret goes to its END. Moving DOWN lands on the next block's *first* line, so
// the caret goes to its START. (The old code forced value.length for both,
// which made ArrowUp right by accident and ArrowDown wrong.)
export const FOCUS_PREV_CARET: Caret = "end";
export const FOCUS_NEXT_CARET: Caret = "start";
