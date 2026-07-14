// One editable block row of the Docs editor. Editing is KEYBOARD-FIRST:
//
//   Enter          split at the caret: this block keeps the left half, a fresh
//                  sibling below takes the right (lists continue their kind)
//   Mod+Enter      check a to-do / collapse a toggle — never splits
//   Backspace      at offset 0: merge into the block above (or delete a divider
//                  above); on an empty block: remove it
//   Tab / S-Tab    indent under the previous sibling / outdent to grandparent;
//                  in code, Tab inserts a tab character instead
//   Alt+Up/Down    move among siblings
//   Up/Left        at the start: hop to the previous block, caret at its END
//   Down/Right     at the end: hop to the next block, caret at its START
//   "# " "- " …    markdown prefixes convert a paragraph's kind
//   "/"            slash menu over every block kind
//   paste          a multi-line paste becomes BLOCKS, not literal newlines
//
// What a keystroke MEANS lives in block-keys.ts as a pure resolveKey(); this
// file only carries the intent out.
//
// Text commits on debounced edit boundaries (a typing pause), on blur, and
// before any structural op — one consensus op per boundary, mirroring the
// rest of the console's server-authoritative writes. A landing snapshot never
// overwrites the draft of the block being edited; committed truth reconciles
// through the next boundary commit instead.
//
// The hover affordances hang OUT of the row on both sides (BlockGutter on the
// left; the finalization mark and comment button on the right), so neither
// reserves column width and the text column keeps one straight left edge.

import { memo, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ClipboardEvent, DragEvent, KeyboardEvent } from "react";

import type {
  BlockKind,
  InlineMark,
  RelativeAnchor,
  SpanMark,
  ThreadView,
} from "../../../domain/pages-client";
import { rebaseMarks, rebaseRange } from "../../../domain/pages-ranges";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import type { OpRecord } from "../../store/finalization";
import { accentVar, color, font } from "../../theme/tokens";
import { EDIT_BOUNDARY_MS, filterSlashKinds, shortcutFor } from "./pages-model";
import { BlockGutter } from "./BlockGutter";
import { BlockMarker, BlockShell } from "./BlockShell";
import { SlashMenu } from "./SlashMenu";
import { SelectionToolbar } from "./SelectionToolbar";
import { InlineText } from "./InlineText";
import { RemoteCursors, type PagePresencePeer } from "./PagePresence";
import type { Row } from "./pages-model";
import { DRAG_MIME } from "./page-drag";
import type { DropEdge } from "./page-drag";
import { FOCUS_NEXT_CARET, FOCUS_PREV_CARET, caretOffset, resolveKey } from "./block-keys";
import type { Caret } from "./block-keys";
import {
  GUTTER_WIDTH,
  INDENT,
  ROW_PAD_Y,
  headingTopSpace,
  kindFont,
  kindPlaceholder,
  restPlaceholder,
} from "./pages-style";

// ── One editable block row ───────────────────────────────

export interface RowHandlers {
  commitText(blockId: string, text: string, marks?: SpanMark[]): void;
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
  setMark(blockId: string, range: RelativeAnchor, kind: InlineMark, active: boolean): void;
  setChecked(blockId: string, checked: boolean): void;
  /** Delete the block and its subtree. A block WITH children is confirmed
   *  first — the guard lives in the handler, so every caller inherits it. */
  remove(blockId: string): void;
  /** A fresh empty paragraph directly below this row. */
  insertBelow(row: Row): void;
  /** Copy this block and its subtree, directly below it. */
  duplicate(row: Row): void;
  /** A multi-line paste: `before`/`after` are the text this row keeps around
   *  the caret. Returns the text THIS row is left holding — the row owns its
   *  draft, so it adopts the return value. */
  pasteBlocks(row: Row, before: string, pasted: string, after: string): string;
  toggleCollapse(blockId: string): void;
  focusRelative(row: Row, delta: -1 | 1, caret: Caret): void;
  /** Reported by the row once it has placed a requested caret. */
  focusApplied(blockId: string): void;
  registerInput(blockId: string, el: HTMLTextAreaElement | null): void;
  openComments(
    blockId: string,
    anchor: { x: number; y: number },
    range?: RelativeAnchor,
  ): void;
  createSubpage(): void;
  dragStart(blockId: string): void;
  dragOver(blockId: string, edge: DropEdge): void;
  drop(blockId: string, edge: DropEdge): void;
  dragEnd(): void;
}

/** Which half of a row the pointer is in — the drop lands on that side. */
const edgeOf = (event: DragEvent<HTMLElement>): DropEdge => {
  const rect = event.currentTarget.getBoundingClientRect();
  return event.clientY < rect.top + rect.height / 2 ? "before" : "after";
};

/** The native textarea owns selection, so mirror just enough of its layout to
 * place the floating toolbar over the selected glyphs (including wrapped
 * lines) instead of over the center of the whole block. */
const selectionAnchorOf = (
  area: HTMLTextAreaElement,
  start: number,
  end: number,
): { x: number; y: number } => {
  const rect = area.getBoundingClientRect();
  const computed = getComputedStyle(area);
  const mirror = document.createElement("div");
  Object.assign(mirror.style, {
    position: "fixed",
    visibility: "hidden",
    pointerEvents: "none",
    left: `${rect.left}px`,
    top: `${rect.top}px`,
    width: `${rect.width}px`,
    boxSizing: computed.boxSizing,
    padding: computed.padding,
    border: computed.border,
    font: computed.font,
    letterSpacing: computed.letterSpacing,
    whiteSpace: "pre-wrap",
    overflowWrap: "break-word",
  });
  const marker = () => {
    const span = document.createElement("span");
    span.textContent = "\u200b";
    return span;
  };
  const from = marker();
  const to = marker();
  mirror.append(area.value.slice(0, start), from, area.value.slice(start, end), to);
  document.body.append(mirror);
  const a = from.getBoundingClientRect();
  const b = to.getBoundingClientRect();
  mirror.remove();
  return {
    x: Math.abs(a.top - b.top) < 1 ? (a.left + b.left) / 2 : a.left,
    y: Math.max(a.bottom, b.bottom),
  };
};

function BlockRowInner({
  row,
  index,
  prevKind,
  caret,
  expanded,
  op,
  threads,
  commentOpen,
  dropEdge,
  presence,
  onCursor,
  handlers,
}: {
  row: Row;
  index: number;
  /** The kind of the row above, or null at the top. Backspace at offset 0
   *  needs it to tell "merge into the prose above" from "delete the divider
   *  above", which owns no textarea of its own. */
  prevKind: BlockKind | null;
  /** Where this row's caret should land, or null if it is not the focus target. */
  caret: Caret | null;
  /** Only meaningful for Toggle rows: whether children are shown. */
  expanded: boolean;
  /** The block's finalization record — only rendered while pending/failed. */
  op: OpRecord | undefined;
  /** Live comment threads on this block, including exact selection anchors. */
  threads: ThreadView[];
  /** The comment card is open on THIS block — its margin badge lights up. */
  commentOpen: boolean;
  /** A drag is hovering this row and would land on this edge. */
  dropEdge: DropEdge | null;
  /** Live, off-consensus peers whose caret is in this block. */
  presence: PagePresencePeer[];
  onCursor: (blockId: string | null, anchor: number, head: number) => void;
  handlers: RowHandlers;
}) {
  const { block, depth } = row;
  const [draft, setDraft] = useState(block.text);
  const [slashDismissed, setSlashDismissed] = useState(false);
  const [slashIndex, setSlashIndex] = useState(0);
  const [focused, setFocused] = useState(false);
  const [hover, setHover] = useState(false);
  const [selection, setSelection] = useState<{
    anchor: { x: number; y: number };
    range: RelativeAnchor;
  } | null>(null);
  const areaRef = useRef<HTMLTextAreaElement | null>(null);
  const rowRef = useRef<HTMLDivElement | null>(null);
  const localCaretRef = useRef<number | null>(null);
  // focus mirrored into a ref so the draft-sync effect below reads the live
  // value without re-running on focus flips.
  const focusedRef = useRef(false);

  // adopt store text only while the block is NOT being edited: a snapshot
  // landing mid-edit (another op's completion refresh) must never clobber the
  // live draft — the edit-boundary commit below reconciles it instead.
  useEffect(() => {
    if (!focusedRef.current) setDraft(block.text);
  }, [block.text]);

  // Place a requested caret — but only once our draft has adopted the committed
  // text. Writing a textarea's value moves its selection to the end, so a caret
  // set while `draft` is still the old text (a merge: the block above is still
  // showing its shorter half) would be stomped by the very next render. Waiting
  // for draft === block.text makes it deterministic; a rAF here would race
  // React's own re-render. jsdom's selection never showed this — a real browser
  // engine did.
  useLayoutEffect(() => {
    if (caret == null) return;
    const el = areaRef.current;
    if (!el || draft !== block.text) return;
    const at = caretOffset(caret, el.value.length);
    el.focus();
    el.setSelectionRange(at, at);
    handlers.focusApplied(block.id);
  }, [caret, draft, block.text, block.id, handlers]);

  useLayoutEffect(() => {
    const at = localCaretRef.current;
    if (at == null) return;
    areaRef.current?.setSelectionRange(at, at);
    localCaretRef.current = null;
  }, [draft]);

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
    if (dirty()) {
      handlers.commitText(
        block.id,
        draft,
        block.marks?.length ? visibleMarks : undefined,
      );
    }
  };
  const visibleMarks = rebaseMarks(block.text, draft, block.marks);
  // each unresolved anchored thread, with its range in BOTH coordinate spaces:
  // `live` (rebased against the draft) locates it under the caret; `anchor`
  // (committed text) is what the store's threads are keyed by, so a click
  // opens exactly that range's discussion.
  const anchoredThreads = threads
    .filter(({ thread }) => !thread.resolved && thread.anchor)
    .map(({ thread }) => ({
      anchor: thread.anchor as RelativeAnchor,
      live: rebaseRange(block.text, draft, thread.anchor as RelativeAnchor),
    }))
    .filter(({ live }) => live.start < live.end);
  const commentRanges = anchoredThreads.map(({ live }) => live);
  const threadCount = threads.length;
  // the margin badge counts OPEN discussions; a block whose threads are all
  // resolved is done talking and gets the quiet hover affordance back.
  const openThreads = threads.filter(({ thread }) => !thread.resolved).length;

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
    setSelection(null);
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
          handlers.commitText(block.id, "", block.marks?.length ? [] : undefined);
          setDraft("");
        } else {
          setDraft(shortcut.rest);
        }
        return;
      }
    }
    setDraft(next);
  };
  const publishCursor = (el: HTMLTextAreaElement) =>
    onCursor(block.id, el.selectionStart, el.selectionEnd);

  // A pasted DOCUMENT is blocks, not one wall of literal newlines. A single
  // line is left to the browser (it is just text at the caret).
  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const pasted = event.clipboardData.getData("text/plain");
    if (!pasted.includes("\n") || block.kind === "code") return;
    event.preventDefault();
    const el = event.currentTarget;
    const before = draft.slice(0, el.selectionStart ?? 0);
    const after = draft.slice(el.selectionEnd ?? draft.length);
    setDraft(handlers.pasteBlocks(row, before, pasted, after));
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    const el = event.currentTarget;

    if (event.key === "Escape" && selection) {
      setSelection(null);
      return;
    }

    // ⌘/ (Ctrl+/ elsewhere) comments on the live selection — the same door the
    // guide menu's Comment row opens.
    if (selection && (event.metaKey || event.ctrlKey) && event.key === "/") {
      event.preventDefault();
      handlers.openComments(block.id, selection.anchor, selection.range);
      setSelection(null);
      return;
    }

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
      case "insert-tab":
        localCaretRef.current = intent.caret;
        setDraft(intent.value);
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

  const todoDone = block.kind === "todo" && block.checked;
  const blockNumber = index + 1;

  const area = (
    <div style={{ position: "relative" }}>
      {visibleMarks.length > 0 || commentRanges.length > 0 ? (
        <InlineText
          text={draft}
          marks={visibleMarks}
          comments={commentRanges}
          fontStyle={kindFont(block.kind)}
          done={todoDone}
        />
      ) : null}
      <textarea
        ref={(el) => {
          areaRef.current = el;
          handlers.registerInput(block.id, el);
        }}
        aria-label={`Edit ${block.kind} block ${blockNumber}`}
        value={draft}
        rows={1}
        onChange={(event) => {
          onChange(event.target.value);
          publishCursor(event.currentTarget);
        }}
        onPaste={onPaste}
        onSelect={(event) => {
          const el = event.currentTarget;
          publishCursor(el);
          if (el.selectionStart === el.selectionEnd) {
            setSelection(null);
            return;
          }
          setSelection({
            anchor: selectionAnchorOf(el, el.selectionStart, el.selectionEnd),
            range: { start: el.selectionStart, end: el.selectionEnd },
          });
        }}
        onFocus={(event) => {
          focusedRef.current = true;
          setFocused(true);
          publishCursor(event.currentTarget);
        }}
        onBlur={() => {
          focusedRef.current = false;
          setFocused(false);
          // focus moved elsewhere: the guide menu must not linger over a
          // selection the user has abandoned. (Menu buttons preventDefault on
          // mousedown, so using the menu never blurs the textarea.)
          setSelection(null);
          onCursor(null, 0, 0);
          maybeCommit();
        }}
        onKeyDown={onKeyDown}
        placeholder={
          draft === ""
            ? focused
              ? kindPlaceholder(block.kind)
              : restPlaceholder(block.kind)
            : ""
        }
        spellCheck={block.kind !== "code"}
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
          color: visibleMarks.length > 0 || commentRanges.length > 0
            ? "transparent"
            : todoDone
              ? color.muted2
              : color.ink,
          caretColor: todoDone ? color.muted2 : color.ink,
          WebkitTextFillColor:
            visibleMarks.length > 0 || commentRanges.length > 0 ? "transparent" : undefined,
          textDecoration: todoDone ? "line-through" : "none",
          font: kindFont(block.kind),
        }}
        onClick={(event) => {
          const el = event.currentTarget;
          if (el.selectionStart !== el.selectionEnd) return;
          // narrowest highlighted range under the caret wins: nested ranges
          // mean nested discussions, and the click aims at the specific one.
          const hit = anchoredThreads
            .filter(({ live }) => live.start <= el.selectionStart && live.end > el.selectionStart)
            .sort((a, b) => (a.live.end - a.live.start) - (b.live.end - b.live.start))[0];
          if (!hit) return;
          const rect = el.getBoundingClientRect();
          handlers.openComments(block.id, { x: rect.right, y: rect.top }, hit.anchor);
        }}
      />
    </div>
  );

  return (
    <div
      ref={rowRef}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      onDragOver={(event) => {
        if (!event.dataTransfer.types.includes(DRAG_MIME)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
        handlers.dragOver(block.id, edgeOf(event));
      }}
      onDrop={(event) => {
        if (!event.dataTransfer.types.includes(DRAG_MIME)) return;
        event.preventDefault();
        handlers.drop(block.id, edgeOf(event));
      }}
      style={{
        position: "relative",
        display: "flex",
        alignItems: "flex-start",
        padding: `${ROW_PAD_Y}px 0`,
        marginLeft: depth * INDENT,
        marginTop: headingTopSpace(block.kind),
      }}
    >
      <BlockGutter
        blockNumber={blockNumber}
        kind={block.kind}
        visible={hover}
        onInsertBelow={() => handlers.insertBelow(row)}
        onTurnInto={(kind) => handlers.setKind(block.id, kind)}
        onDuplicate={() => handlers.duplicate(row)}
        onRemove={() => handlers.remove(block.id)}
        onDragStart={(event) => {
          event.dataTransfer.effectAllowed = "move";
          // the MIME is the drag's signature: rows only accept a drag that
          // carries it, so a file or a text selection dragged in from outside
          // never reorders the document.
          event.dataTransfer.setData(DRAG_MIME, block.id);
          handlers.dragStart(block.id);
        }}
        onDragEnd={handlers.dragEnd}
      />

      {/* the marker hangs in the left margin instead of sitting in flow, so the
          text column below starts at offset 0 and lines up with the page title.
          Prose kinds render no marker at all and used to pay for the gutter
          anyway — that was the whole reason the body sat 28px right of the
          title. */}
      <BlockMarker
        block={block}
        blockNumber={blockNumber}
        listIndex={row.listIndex}
        expanded={expanded}
        onSetChecked={(checked) => handlers.setChecked(block.id, checked)}
        onToggleCollapse={() => handlers.toggleCollapse(block.id)}
      />

      <div style={{ flex: 1, minWidth: 0 }}>
        <BlockShell kind={block.kind} blockNumber={blockNumber} draft={draft}>
          {area}
          {slashOpen ? (
            <SlashMenu query={slashQuery} activeIndex={slashIndex} onPick={pickSlash} />
          ) : null}
        </BlockShell>
      </div>

      {/* the finalization mark and the comment button hang in the RIGHT margin.
          They used to reserve 44px of every row — on every row, forever — which
          narrowed the text column and left it ragged. */}
      <div
        style={{
          position: "absolute",
          left: "100%",
          marginLeft: 8,
          top: ROW_PAD_Y,
          width: Math.max(GUTTER_WIDTH, 72),
          height: 28,
          display: "flex",
          alignItems: "center",
          gap: 3,
        }}
      >
        <FinalizationMark op={op} size={15} />
        {openThreads > 0 || hover ? (
          <button
            type="button"
            aria-label={`Comment on block ${blockNumber}`}
            title={
              threadCount > 0
                ? `${threadCount} discussion${threadCount === 1 ? "" : "s"}`
                : "Comment"
            }
            onClick={(event) => {
              const rect = event.currentTarget.getBoundingClientRect();
              handlers.openComments(block.id, { x: rect.left, y: rect.bottom });
            }}
            style={{
              all: "unset",
              cursor: "pointer",
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 4,
              minWidth: 30,
              height: 28,
              padding: "0 7px",
              borderRadius: 7,
              color: commentOpen ? accentVar : openThreads > 0 ? color.muted3 : color.muted2,
              font: `650 11.5px ${font.mono}`,
            }}
          >
            <Icon name="chat" size={16} strokeWidth={1.9} />
            {openThreads > 0 ? openThreads : null}
          </button>
        ) : null}
      </div>

      {selection ? (
        <SelectionToolbar
          blockId={block.id}
          blockKind={block.kind}
          marks={visibleMarks}
          range={selection.range}
          anchor={selection.anchor}
          onMark={(kind, active) => {
            maybeCommit();
            handlers.setMark(block.id, selection.range, kind, active);
          }}
          onTurnInto={(kind) => handlers.setKind(block.id, kind)}
          onComment={(anchor) => {
            handlers.openComments(block.id, anchor, selection.range);
            setSelection(null);
          }}
          onDismiss={() => setSelection(null)}
        />
      ) : null}

      <RemoteCursors peers={presence} areaRef={areaRef} rowRef={rowRef} text={draft} />

      {dropEdge ? (
        <div
          data-testid={`drop-${dropEdge}`}
          aria-hidden="true"
          style={{
            position: "absolute",
            left: 0,
            right: 0,
            top: dropEdge === "before" ? -1 : undefined,
            bottom: dropEdge === "after" ? -1 : undefined,
            height: 2,
            borderRadius: 2,
            background: accentVar,
          }}
        />
      ) : null}
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
    (x.marks ?? []).length === (y.marks ?? []).length &&
    (x.marks ?? []).every((mark, index) => {
      const other = (y.marks ?? [])[index];
      return other?.start === mark.start && other.end === mark.end && other.kind === mark.kind;
    }) &&
    x.checked === y.checked &&
    // a toggle's chevron exists only while it HAS children, so the row reads
    // the child count and must re-render when it changes.
    x.children.length === y.children.length &&
    a.row.depth === b.row.depth &&
    a.row.listIndex === b.row.listIndex &&
    a.index === b.index &&
    a.prevKind === b.prevKind &&
    a.caret === b.caret &&
    a.expanded === b.expanded &&
    a.op === b.op &&
    a.threads === b.threads &&
    a.commentOpen === b.commentOpen &&
    a.dropEdge === b.dropEdge &&
    a.presence === b.presence &&
    a.onCursor === b.onCursor &&
    a.handlers === b.handlers
  );
});
