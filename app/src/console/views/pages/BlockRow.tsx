// One editable block row of the Docs editor. Editing is KEYBOARD-FIRST:
//
//   Enter          split at the caret: this block keeps the left half, a fresh
//                  sibling below takes the right (lists continue their kind)
//   Mod+Enter      check a to-do / collapse a toggle — never splits
//   Backspace      at offset 0: merge into the block above (or delete a divider
//                  above); on an empty block: remove it
//   Tab / S-Tab    indent under the previous sibling / outdent to grandparent
//   Alt+Up/Down    move among siblings
//   Up/Left        at the start: hop to the previous block, caret at its END
//   Down/Right     at the end: hop to the next block, caret at its START
//   "# " "- " …    markdown prefixes convert a paragraph's kind
//   "/"            slash menu over every block kind
//
// What a keystroke MEANS lives in block-keys.ts as a pure resolveKey(); this
// file only carries the intent out.
//
// Text commits on debounced edit boundaries (a typing pause), on blur, and
// before any structural op — one consensus op per boundary, mirroring the
// rest of the console's server-authoritative writes. A landing snapshot never
// overwrites the draft of the block being edited; committed truth reconciles
// through the next boundary commit instead.

import { memo, useEffect, useRef, useState } from "react";
import type { KeyboardEvent, ReactNode } from "react";

import type { BlockKind } from "../../../domain/pages-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import type { OpRecord } from "../../store/finalization";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { EDIT_BOUNDARY_MS, filterSlashKinds, shortcutFor } from "./pages-model";
import { SlashMenu } from "./SlashMenu";
import type { Row } from "./pages-model";
import { FOCUS_NEXT_CARET, FOCUS_PREV_CARET, resolveKey } from "./block-keys";
import type { Caret } from "./block-keys";
import { INDENT, MARKER_HANG, ROW_PAD_Y, headingTopSpace } from "./pages-style";

/** Per-kind typography for the block textarea. */
function kindFont(kind: BlockKind): string {
  switch (kind) {
    case "heading1":
      return `650 24px/1.25 ${font.sans}`;
    case "heading2":
      return `650 19px/1.3 ${font.sans}`;
    case "heading3":
      return `600 16px/1.35 ${font.sans}`;
    case "code":
      return `400 12.5px/1.55 ${font.mono}`;
    case "quote":
      return `400 14.5px/1.6 ${font.sans}`;
    default:
      return `400 14.5px/1.6 ${font.sans}`;
  }
}

/** The placeholder shown ONLY on a focused, empty block. */
function focusPlaceholder(kind: BlockKind): string {
  switch (kind) {
    case "heading1":
    case "heading2":
    case "heading3":
      return "Heading";
    case "todo":
      return "To-do";
    case "bulleted":
    case "numbered":
      return "List item";
    case "toggle":
      return "Toggle";
    case "quote":
      return "Quote";
    case "code":
      return "Code";
    case "callout":
      return "Callout";
    default:
      return "Write, or press '/' for commands";
  }
}

// ── One editable block row ───────────────────────────────

export interface RowHandlers {
  commitText(blockId: string, text: string): void;
  /** This block keeps `left`; a fresh sibling below takes `right`. */
  split(row: Row, left: string, right: string): void;
  /** Join `text` onto the block above and drop this one. */
  mergePrev(row: Row, text: string): void;
  removeDividerAbove(row: Row): void;
  removeEmpty(row: Row): void;
  indent(row: Row): void;
  outdent(row: Row): void;
  moveUp(row: Row): void;
  moveDown(row: Row): void;
  setKind(blockId: string, kind: BlockKind): void;
  setChecked(blockId: string, checked: boolean): void;
  remove(blockId: string): void;
  toggleCollapse(blockId: string): void;
  focusRelative(row: Row, delta: -1 | 1, caret: Caret): void;
  registerInput(blockId: string, el: HTMLTextAreaElement | null): void;
  openComments(blockId: string, anchor: { x: number; y: number }): void;
  createSubpage(): void;
}

function BlockRowInner({
  row,
  index,
  prevKind,
  expanded,
  op,
  threadCount,
  handlers,
}: {
  row: Row;
  index: number;
  /** The kind of the row above, or null at the top. Backspace at offset 0
   *  needs it to tell "merge into the prose above" from "delete the divider
   *  above", which owns no textarea of its own. */
  prevKind: BlockKind | null;
  /** Only meaningful for Toggle rows: whether children are shown. */
  expanded: boolean;
  /** The block's finalization record — only rendered while pending/failed. */
  op: OpRecord | undefined;
  /** Number of live comment threads on this block. */
  threadCount: number;
  handlers: RowHandlers;
}) {
  const { block, depth } = row;
  const [draft, setDraft] = useState(block.text);
  const [slashDismissed, setSlashDismissed] = useState(false);
  const [slashIndex, setSlashIndex] = useState(0);
  const [focused, setFocused] = useState(false);
  const [hover, setHover] = useState(false);
  const areaRef = useRef<HTMLTextAreaElement | null>(null);
  // focus mirrored into a ref so the draft-sync effect below reads the live
  // value without re-running on focus flips.
  const focusedRef = useRef(false);

  // adopt store text only while the block is NOT being edited: a snapshot
  // landing mid-edit (another op's completion refresh) must never clobber the
  // live draft — the edit-boundary commit below reconciles it instead.
  useEffect(() => {
    if (!focusedRef.current) setDraft(block.text);
  }, [block.text]);

  // auto-grow: the textarea is exactly as tall as its content.
  useEffect(() => {
    const el = areaRef.current;
    if (el) {
      el.style.height = "0";
      el.style.height = `${el.scrollHeight}px`;
    }
  }, [draft, block.kind]);

  const slashOpen =
    draft.startsWith("/") && !slashDismissed && block.kind !== "code";
  const slashQuery = slashOpen ? draft.slice(1) : "";
  const slashOptions = filterSlashKinds(slashQuery);

  const dirty = () => draft !== block.text;
  const maybeCommit = () => {
    if (dirty()) handlers.commitText(block.id, draft);
  };

  // the latest commit closure lives in a ref so the boundary timer neither
  // resets when the store re-renders (handlers is rebuilt per store change)
  // nor commits a stale draft.
  const commitBoundaryRef = useRef(() => {});
  commitBoundaryRef.current = maybeCommit;

  // per-edit-boundary commits: a typing pause flows one consensus op, so
  // peers and the finalization mark track text without waiting for blur or a
  // structural op. An open slash menu is a command in progress, not text.
  useEffect(() => {
    if (draft === block.text || slashOpen) return;
    const timer = setTimeout(() => commitBoundaryRef.current(), EDIT_BOUNDARY_MS);
    return () => clearTimeout(timer);
  }, [draft, block.text, slashOpen]);

  const pickSlash = (kind: BlockKind) => {
    setDraft("");
    setSlashDismissed(false);
    // "page" is not a conversion — it spawns a child page of the open one.
    if (kind === "page") {
      handlers.createSubpage();
      return;
    }
    if (kind !== block.kind) handlers.setKind(block.id, kind);
  };

  const onChange = (next: string) => {
    if (!next.startsWith("/")) setSlashDismissed(false);
    if (slashOpen || next.startsWith("/")) {
      setSlashIndex(0);
    }
    // markdown prefixes convert only a plain paragraph — conversions never
    // chain, so "# " typed into a heading stays literal text.
    if (block.kind === "paragraph") {
      const shortcut = shortcutFor(next);
      if (shortcut) {
        handlers.setKind(block.id, shortcut.kind);
        if (shortcut.kind === "divider") {
          handlers.commitText(block.id, "");
          setDraft("");
        } else {
          setDraft(shortcut.rest);
        }
        return;
      }
    }
    setDraft(next);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    const el = event.currentTarget;

    if (slashOpen && slashOptions.length > 0) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSlashIndex((i) => (i + 1) % slashOptions.length);
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSlashIndex((i) => (i - 1 + slashOptions.length) % slashOptions.length);
        return;
      }
      if (event.key === "Enter") {
        event.preventDefault();
        pickSlash(slashOptions[slashIndex].kind);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setSlashDismissed(true);
        return;
      }
    }

    // everything below is grammar, and grammar is pure: resolveKey decides what
    // the keystroke MEANS, this switch carries it out.
    const intent = resolveKey({
      key: event.key,
      shiftKey: event.shiftKey,
      metaKey: event.metaKey,
      ctrlKey: event.ctrlKey,
      altKey: event.altKey,
      value: draft,
      caretStart: el.selectionStart ?? 0,
      caretEnd: el.selectionEnd ?? 0,
      kind: block.kind,
      slashOpen: slashOpen && slashOptions.length > 0,
      prevKind,
    });
    if (intent.type === "none") return;
    event.preventDefault();

    switch (intent.type) {
      case "split":
        // adopt the left half locally first: the block's own edit-boundary
        // timer must not fire afterwards and commit the whole pre-split draft
        // back over the truncation.
        setDraft(intent.left);
        handlers.split(row, intent.left, intent.right);
        return;
      case "merge-prev":
        handlers.mergePrev(row, draft);
        return;
      case "remove-divider-above":
        handlers.removeDividerAbove(row);
        return;
      case "remove-empty":
        handlers.removeEmpty(row);
        return;
      case "exit-to-paragraph":
        handlers.setKind(block.id, "paragraph");
        return;
      case "toggle-check":
        handlers.setChecked(block.id, !block.checked);
        return;
      case "toggle-collapse":
        handlers.toggleCollapse(block.id);
        return;
      case "indent":
        maybeCommit();
        handlers.indent(row);
        return;
      case "outdent":
        maybeCommit();
        handlers.outdent(row);
        return;
      case "move-up":
        maybeCommit();
        handlers.moveUp(row);
        return;
      case "move-down":
        maybeCommit();
        handlers.moveDown(row);
        return;
      case "focus-prev":
        maybeCommit();
        handlers.focusRelative(row, -1, FOCUS_PREV_CARET);
        return;
      case "focus-next":
        maybeCommit();
        handlers.focusRelative(row, 1, FOCUS_NEXT_CARET);
        return;
    }
  };

  const code = block.kind === "code";
  const quote = block.kind === "quote";
  const callout = block.kind === "callout";
  const todoDone = block.kind === "todo" && block.checked;
  const blockNumber = index + 1;

  // the left gutter marker per kind (bullet, number, checkbox, chevron).
  const marker: ReactNode =
    block.kind === "bulleted" ? (
      <span style={{ font: `700 14px ${font.sans}`, color: color.muted3 }}>•</span>
    ) : block.kind === "numbered" ? (
      <span style={{ font: `500 12.5px ${font.mono}`, color: color.muted3 }}>
        {row.listIndex ?? 1}.
      </span>
    ) : block.kind === "todo" ? (
      <button
        type="button"
        aria-label={`${block.checked ? "Uncheck" : "Check"} to-do block ${blockNumber}`}
        onClick={() => handlers.setChecked(block.id, !block.checked)}
        style={{
          all: "unset",
          cursor: "pointer",
          width: 15,
          height: 15,
          borderRadius: 4,
          border: `1.5px solid ${block.checked ? accentVar : color.borderStrong}`,
          background: block.checked ? accentVar : "transparent",
          color: "#fff",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        {block.checked ? <Icon name="check" size={10} strokeWidth={2.4} /> : null}
      </button>
    ) : block.kind === "toggle" ? (
      <button
        type="button"
        aria-label={`${expanded ? "Collapse" : "Expand"} toggle block ${blockNumber}`}
        aria-expanded={expanded}
        onClick={() => handlers.toggleCollapse(block.id)}
        style={{
          all: "unset",
          cursor: "pointer",
          width: 16,
          height: 16,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: color.muted3,
        }}
      >
        <Icon
          name="chevronRight"
          size={13}
          strokeWidth={2}
          style={{ transform: `rotate(${expanded ? 90 : 0}deg)` }}
        />
      </button>
    ) : null;

  const content =
    block.kind === "divider" ? (
      <div
        aria-label={`Divider block ${blockNumber}`}
        style={{ padding: "10px 0" }}
      >
        <div style={{ height: 1, background: color.borderStrong }} />
      </div>
    ) : (
      <div
        style={{
          position: "relative",
          borderLeft: quote ? `3px solid ${color.borderStrong}` : "none",
          paddingLeft: quote ? 12 : 0,
          background: code ? color.sunken : callout ? color.sidebar : "transparent",
          border: code
            ? `1px solid ${color.border}`
            : callout
              ? `1px solid ${color.border}`
              : undefined,
          borderRadius: code || callout ? radius.md : 0,
          padding: code ? "11px 13px" : callout ? "11px 13px" : undefined,
        }}
      >
        <textarea
          ref={(el) => {
            areaRef.current = el;
            handlers.registerInput(block.id, el);
          }}
          aria-label={`Edit ${block.kind} block ${blockNumber}`}
          value={draft}
          rows={1}
          onChange={(event) => onChange(event.target.value)}
          onFocus={() => {
            focusedRef.current = true;
            setFocused(true);
          }}
          onBlur={() => {
            focusedRef.current = false;
            setFocused(false);
            maybeCommit();
          }}
          onKeyDown={onKeyDown}
          placeholder={focused && draft === "" ? focusPlaceholder(block.kind) : ""}
          spellCheck={!code}
          style={{
            display: "block",
            width: "100%",
            boxSizing: "border-box",
            border: "none",
            outline: "none",
            resize: "none",
            overflow: "hidden",
            background: "transparent",
            padding: 0,
            color: todoDone ? color.muted2 : color.ink,
            textDecoration: todoDone ? "line-through" : "none",
            font: kindFont(block.kind),
          }}
        />
        {slashOpen ? (
          <SlashMenu
            query={slashQuery}
            activeIndex={slashIndex}
            onPick={pickSlash}
          />
        ) : null}
      </div>
    );

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        position: "relative",
        display: "flex",
        alignItems: "flex-start",
        gap: 8,
        padding: `${ROW_PAD_Y}px 0`,
        marginLeft: depth * INDENT,
        marginTop: headingTopSpace(block.kind),
      }}
    >
      {/* the marker hangs in the left margin instead of sitting in flow, so the
          text column below starts at offset 0 and lines up with the page title.
          Prose kinds render no marker at all and used to pay for the gutter
          anyway — that was the whole reason the body sat 28px right of the
          title. */}
      {marker ? (
        <div
          style={{
            position: "absolute",
            left: -MARKER_HANG,
            // an absolute box offsets from the row's PADDING box, but the
            // marker used to be a flex item aligned to its CONTENT box — one
            // row-padding lower. Match it, or every bullet rides high above
            // its own line.
            top: ROW_PAD_Y,
            width: 20,
            height: 24,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          {marker}
        </div>
      ) : null}
      <div style={{ flex: 1, minWidth: 0 }}>{content}</div>
      <div
        style={{
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 3,
          paddingTop: 3,
          minWidth: 44,
          justifyContent: "flex-end",
        }}
      >
        <FinalizationMark op={op} />
        {threadCount > 0 || hover ? (
          <button
            type="button"
            aria-label={`Comment on block ${blockNumber}`}
            title={threadCount > 0 ? `${threadCount} comment thread(s)` : "Comment"}
            onClick={(event) => {
              const rect = event.currentTarget.getBoundingClientRect();
              handlers.openComments(block.id, { x: rect.left, y: rect.bottom });
            }}
            style={{
              all: "unset",
              cursor: "pointer",
              display: "inline-flex",
              alignItems: "center",
              gap: 2,
              padding: "2px 4px",
              borderRadius: 5,
              color: threadCount > 0 ? accentVar : color.muted2,
              font: `600 9.5px ${font.mono}`,
            }}
          >
            <Icon name="chat" size={12} strokeWidth={1.8} />
            {threadCount > 0 ? threadCount : null}
          </button>
        ) : null}
        {hover ? (
          <button
            type="button"
            aria-label={`Remove block ${blockNumber}`}
            title="Remove block (and its subtree)"
            onClick={() => handlers.remove(block.id)}
            style={{
              all: "unset",
              cursor: "pointer",
              width: 20,
              height: 20,
              borderRadius: 5,
              color: color.muted2,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Icon name="close" size={11} />
          </button>
        ) : null}
      </div>
    </div>
  );
}

// Rows are re-rendered by every store patch — a typing pause, an op's finalize
// receipt, the refresh that follows it. Without this memo each of those
// reconciles all N rows, which is what makes a long list feel slow: building
// one is a burst of back-to-back Enters with no cheap keystrokes in between to
// space the patches out.
//
// The comparator cannot be a reference check on `row`: buildRows allocates a
// fresh { block, depth } wrapper every recompute, so the memo would never hit.
// Nor can it be a reference check on `row.block`: an authoritative refresh
// deserializes the whole snapshot, so every block is a new object even when
// nothing about it changed. It compares the fields this component actually
// reads.
//
// `handlers` must stay referentially stable (PagesView builds it once against a
// live ref) or this memo is defeated by that prop alone.
export const BlockRow = memo(BlockRowInner, (a, b) => {
  const x = a.row.block;
  const y = b.row.block;
  return (
    x.id === y.id &&
    x.kind === y.kind &&
    x.text === y.text &&
    x.checked === y.checked &&
    a.row.depth === b.row.depth &&
    a.row.listIndex === b.row.listIndex &&
    a.index === b.index &&
    a.prevKind === b.prevKind &&
    a.expanded === b.expanded &&
    a.op === b.op &&
    a.threadCount === b.threadCount &&
    a.handlers === b.handlers
  );
});
