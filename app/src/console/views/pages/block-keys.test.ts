import { describe, expect, it } from "vitest";

import type { BlockKind } from "../../../domain/pages-client";
import {
  caretOffset,
  FOCUS_NEXT_CARET,
  FOCUS_PREV_CARET,
  type KeyContext,
  type KeyIntent,
  mergeText,
  resolveKey,
  splitText,
} from "./block-keys";

const ctx = (patch: Partial<KeyContext> = {}): KeyContext => ({
  key: "Enter",
  shiftKey: false,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  value: "hello",
  caretStart: 5,
  caretEnd: 5,
  kind: "paragraph",
  slashOpen: false,
  prevKind: null,
  ...patch,
});

const typeOf = (patch: Partial<KeyContext>): KeyIntent["type"] => resolveKey(ctx(patch)).type;

describe("splitText", () => {
  it("splits at the caret", () => {
    expect(splitText("helloworld", 5)).toEqual({ left: "hello", right: "world" });
  });

  it("keeps everything left at the end, everything right at the start", () => {
    expect(splitText("abc", 3)).toEqual({ left: "abc", right: "" });
    expect(splitText("abc", 0)).toEqual({ left: "", right: "abc" });
  });

  it("clamps a caret outside the value", () => {
    expect(splitText("abc", 99)).toEqual({ left: "abc", right: "" });
    expect(splitText("abc", -4)).toEqual({ left: "", right: "abc" });
  });
});

describe("mergeText", () => {
  it("joins and reports the seam", () => {
    expect(mergeText("foo", "bar")).toEqual({ text: "foobar", seam: 3 });
  });

  it("puts the seam at 0 when the previous block is empty", () => {
    expect(mergeText("", "bar")).toEqual({ text: "bar", seam: 0 });
  });
});

describe("caretOffset", () => {
  it("resolves the named ends", () => {
    expect(caretOffset("start", 7)).toBe(0);
    expect(caretOffset("end", 7)).toBe(7);
  });

  it("passes a numeric offset through, clamped", () => {
    expect(caretOffset(3, 7)).toBe(3);
    expect(caretOffset(99, 7)).toBe(7);
    expect(caretOffset(-1, 7)).toBe(0);
  });
});

describe("resolveKey — Enter", () => {
  it("splits at the caret, mid-text", () => {
    expect(resolveKey(ctx({ value: "helloworld", caretStart: 5, caretEnd: 5 }))).toEqual({
      type: "split",
      left: "hello",
      right: "world",
    });
  });

  it("hands Shift+Enter back to the textarea as a soft break", () => {
    expect(typeOf({ shiftKey: true })).toBe("none");
  });

  it("never splits a code block — Enter there is a newline", () => {
    expect(typeOf({ kind: "code", value: "fn()", caretStart: 2, caretEnd: 2 })).toBe("none");
  });

  it("exits an empty block of an escapable kind", () => {
    for (const kind of ["bulleted", "numbered", "todo", "quote", "code", "callout", "toggle"]) {
      expect(typeOf({ kind: kind as BlockKind, value: "" })).toBe("exit-to-paragraph");
    }
  });

  it("splits an empty paragraph rather than exiting it", () => {
    expect(typeOf({ kind: "paragraph", value: "", caretStart: 0, caretEnd: 0 })).toBe("split");
  });

  it("yields to an open slash menu", () => {
    expect(typeOf({ slashOpen: true })).toBe("none");
  });
});

describe("resolveKey — mod+Enter acts on the block, never splits it", () => {
  it("toggles a to-do with Cmd+Enter", () => {
    expect(typeOf({ kind: "todo", metaKey: true })).toBe("toggle-check");
  });

  it("toggles a to-do with Ctrl+Enter", () => {
    expect(typeOf({ kind: "todo", ctrlKey: true })).toBe("toggle-check");
  });

  it("collapses a toggle block", () => {
    expect(typeOf({ kind: "toggle", metaKey: true })).toBe("toggle-collapse");
  });

  it("does nothing on a paragraph — and crucially does not split it", () => {
    expect(typeOf({ kind: "paragraph", metaKey: true })).toBe("none");
  });

  it("does not exit an empty to-do under a modifier", () => {
    expect(typeOf({ kind: "todo", value: "", metaKey: true })).toBe("toggle-check");
  });
});

describe("resolveKey — Backspace", () => {
  it("removes a wholly empty block", () => {
    expect(typeOf({ key: "Backspace", value: "", caretStart: 0, caretEnd: 0 })).toBe(
      "remove-empty",
    );
  });

  it("merges a non-empty block into the previous one from offset 0", () => {
    expect(
      typeOf({ key: "Backspace", value: "world", caretStart: 0, caretEnd: 0, prevKind: "paragraph" }),
    ).toBe("merge-prev");
  });

  it("deletes the divider above instead of merging into it", () => {
    expect(
      typeOf({ key: "Backspace", value: "world", caretStart: 0, caretEnd: 0, prevKind: "divider" }),
    ).toBe("remove-divider-above");
  });

  it("is an ordinary character delete anywhere but offset 0", () => {
    expect(typeOf({ key: "Backspace", value: "world", caretStart: 3, caretEnd: 3 })).toBe("none");
  });

  it("does not merge when a selection is active at offset 0", () => {
    expect(typeOf({ key: "Backspace", value: "world", caretStart: 0, caretEnd: 3 })).toBe("none");
  });
});

describe("resolveKey — structure", () => {
  it("indents with Tab and outdents with Shift+Tab", () => {
    expect(typeOf({ key: "Tab" })).toBe("indent");
    expect(typeOf({ key: "Tab", shiftKey: true })).toBe("outdent");
  });

  it("inserts a tab into code without moving the block", () => {
    expect(
      resolveKey(
        ctx({ key: "Tab", kind: "code", value: "abcd", caretStart: 1, caretEnd: 3 }),
      ),
    ).toEqual({ type: "insert-tab", value: "a\td", caret: 2 });
    expect(typeOf({ key: "Tab", kind: "code", shiftKey: true })).toBe("none");
  });

  it("moves the block with Alt+Arrow", () => {
    expect(typeOf({ key: "ArrowUp", altKey: true })).toBe("move-up");
    expect(typeOf({ key: "ArrowDown", altKey: true })).toBe("move-down");
  });
});

describe("resolveKey — leaving a block by its edges", () => {
  it("crosses up from offset 0 and down from the end", () => {
    expect(typeOf({ key: "ArrowUp", caretStart: 0, caretEnd: 0 })).toBe("focus-prev");
    expect(typeOf({ key: "ArrowDown", caretStart: 5, caretEnd: 5 })).toBe("focus-next");
  });

  it("crosses horizontally only at the edges", () => {
    expect(typeOf({ key: "ArrowLeft", caretStart: 0, caretEnd: 0 })).toBe("focus-prev");
    expect(typeOf({ key: "ArrowRight", caretStart: 5, caretEnd: 5 })).toBe("focus-next");
    expect(typeOf({ key: "ArrowLeft", caretStart: 2, caretEnd: 2 })).toBe("none");
    expect(typeOf({ key: "ArrowRight", caretStart: 2, caretEnd: 2 })).toBe("none");
  });

  it("stays inside the block when the arrow has somewhere to go", () => {
    expect(typeOf({ key: "ArrowUp", caretStart: 2, caretEnd: 2 })).toBe("none");
    expect(typeOf({ key: "ArrowDown", caretStart: 2, caretEnd: 2 })).toBe("none");
  });

  // the rule reads backwards until you think in visual lines: moving up lands
  // on the previous block's LAST line, moving down on the next block's FIRST.
  it("lands the caret on the adjacent line, not a fixed end", () => {
    expect(FOCUS_PREV_CARET).toBe("end");
    expect(FOCUS_NEXT_CARET).toBe("start");
  });
});
