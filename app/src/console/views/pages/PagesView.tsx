// The Docs surface over the node's `pages` module: a Notion-like block-tree
// editor with a nested page tree, document tabs, and comment threads. Editing
// is KEYBOARD-FIRST:
//
//   Enter          split: a fresh sibling below (lists continue their kind)
//   Backspace      on an empty block: remove it, focus the previous one
//   Tab / S-Tab    indent under the previous sibling / outdent to grandparent
//   Alt+Up/Down    move among siblings
//   Up/Down        at the draft's edges: hop between blocks
//   "# " "- " …    markdown prefixes convert a paragraph's kind
//   "/"            slash menu over every block kind
//
// Text commits on debounced edit boundaries (a typing pause), on blur, and
// before any structural op — one consensus op per boundary, mirroring the
// rest of the console's server-authoritative writes. A landing snapshot never
// overwrites the draft of the block being edited; committed truth reconciles
// through the next boundary commit instead.

import { useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, KeyboardEvent, ReactNode } from "react";

import type { BlockKind } from "../../../domain/pages-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";
import {
  buildRows,
  continuationKind,
  filterSlashKinds,
  indentTarget,
  moveDownTarget,
  moveUpTarget,
  outdentTarget,
  shortcutFor,
} from "./pages-model";
import type { Row } from "./pages-model";
import { DocTabs } from "./DocTabs";
import { PageTree } from "./PageTree";
import { CommentsPanel } from "./CommentsPanel";
import { buildForest } from "./page-tree";

const INDENT = 26;
const TREE_COLLAPSE_KEY = "ducktape.docTreeCollapsed";
/** A pause this long while typing is one edit boundary — one consensus op.
 *  Exported for the tests that drive the boundary timer. */
export const EDIT_BOUNDARY_MS = 700;

const sectionLabelStyle: CSSProperties = {
  font: `600 9px ${font.mono}`,
  letterSpacing: ".11em",
  color: color.muted2,
  textTransform: "uppercase",
};

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

// ── Slash menu ───────────────────────────────────────────

function SlashMenu({
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
        width: 240,
        maxHeight: 280,
        overflowY: "auto",
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        boxShadow: shadow.card,
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

// ── One editable block row ───────────────────────────────

interface RowHandlers {
  commitText(blockId: string, text: string): void;
  split(row: Row, draftLeft: string): void;
  removeEmpty(row: Row): void;
  indent(row: Row): void;
  outdent(row: Row): void;
  moveUp(row: Row): void;
  moveDown(row: Row): void;
  setKind(blockId: string, kind: BlockKind): void;
  setChecked(blockId: string, checked: boolean): void;
  remove(blockId: string): void;
  toggleCollapse(blockId: string): void;
  focusRelative(row: Row, delta: -1 | 1): void;
  registerInput(blockId: string, el: HTMLTextAreaElement | null): void;
  openComments(blockId: string): void;
}

function BlockRow({
  row,
  index,
  expanded,
  op,
  threadCount,
  handlers,
}: {
  row: Row;
  index: number;
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

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      // an empty list item exits the list instead of continuing it.
      if (draft.trim() === "" && continuationKind(block.kind) === block.kind
          && block.kind !== "paragraph") {
        handlers.setKind(block.id, "paragraph");
        return;
      }
      maybeCommit();
      handlers.split(row, draft);
      return;
    }
    if (event.key === "Backspace" && draft === "") {
      event.preventDefault();
      handlers.removeEmpty(row);
      return;
    }
    if (event.key === "Tab") {
      event.preventDefault();
      maybeCommit();
      if (event.shiftKey) handlers.outdent(row);
      else handlers.indent(row);
      return;
    }
    if (event.altKey && event.key === "ArrowUp") {
      event.preventDefault();
      maybeCommit();
      handlers.moveUp(row);
      return;
    }
    if (event.altKey && event.key === "ArrowDown") {
      event.preventDefault();
      maybeCommit();
      handlers.moveDown(row);
      return;
    }
    if (
      event.key === "ArrowUp" &&
      el.selectionStart === 0 &&
      el.selectionEnd === 0
    ) {
      event.preventDefault();
      handlers.focusRelative(row, -1);
      return;
    }
    if (
      event.key === "ArrowDown" &&
      el.selectionStart === el.value.length &&
      el.selectionEnd === el.value.length
    ) {
      event.preventDefault();
      handlers.focusRelative(row, 1);
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
        display: "flex",
        alignItems: "flex-start",
        gap: 8,
        padding: "2.5px 0",
        marginLeft: depth * INDENT,
      }}
    >
      <div
        style={{
          flexShrink: 0,
          width: 20,
          minHeight: 24,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          paddingTop: block.kind === "heading1" ? 6 : block.kind === "heading2" ? 3 : 0,
        }}
      >
        {marker}
      </div>
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
            onClick={() => handlers.openComments(block.id)}
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
            aria-label={`Copy link to block ${blockNumber}`}
            title="Copy block link"
            onClick={() => {
              void navigator.clipboard?.writeText(block.id);
            }}
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
            <Icon name="hash" size={11} />
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

// ── Page rail (nested page tree) ─────────────────────────

function PageRail({
  pages,
  activePage,
  collapsed,
  onToggleCollapse,
  onNewPage,
  onAddChild,
  onOpen,
  onDelete,
  onMove,
  onRefresh,
}: {
  pages: { id: string; title: string; parent: string | null }[];
  activePage: string | null;
  collapsed: ReadonlySet<string>;
  onToggleCollapse: (id: string) => void;
  onNewPage: () => void;
  onAddChild: (id: string) => void;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  onMove: (id: string, parent: string | null) => void;
  onRefresh: () => void;
}) {
  const forest = useMemo(() => buildForest(pages), [pages]);
  return (
    <aside
      style={{
        width: 272,
        flexShrink: 0,
        borderRight: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        display: "flex",
        flexDirection: "column",
        color: color.muted3,
      }}
    >
      <div
        style={{
          height: 56,
          flexShrink: 0,
          padding: "0 15px",
          display: "flex",
          alignItems: "center",
          gap: 9,
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span
          style={{
            width: 26,
            height: 26,
            borderRadius: 8,
            background: color.dark,
            color: color.onDark,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="pages" size={14} strokeWidth={1.7} />
        </span>
        <div style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>Pages</div>
        <button
          type="button"
          aria-label="Refresh pages"
          title="Refresh pages"
          onClick={onRefresh}
          style={{
            all: "unset",
            cursor: "pointer",
            marginLeft: "auto",
            width: 26,
            height: 26,
            borderRadius: 6,
            border: `1px solid ${color.border}`,
            background: color.paper,
            color: color.muted3,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="refresh" size={13} strokeWidth={1.7} />
        </button>
      </div>

      <button
        type="button"
        aria-label="New page"
        onClick={onNewPage}
        style={{
          all: "unset",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          gap: 8,
          margin: "12px 12px 6px",
          padding: "8px 10px",
          borderRadius: radius.sm,
          background: color.dark,
          color: color.onDark,
          font: `600 12.5px ${font.sans}`,
        }}
      >
        <Icon name="plus" size={14} strokeWidth={1.9} /> New page
      </button>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "6px 0 13px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "0 14px 8px" }}>
          <div style={sectionLabelStyle}>All pages</div>
        </div>
        {pages.length === 0 ? (
          <div
            style={{
              margin: "7px 14px",
              padding: "13px 12px",
              border: `1px dashed ${color.borderStrong}`,
              borderRadius: radius.md,
              background: color.paper,
              font: `400 12px/1.45 ${font.sans}`,
              color: color.muted2,
            }}
          >
            No pages on this node yet. Create one above to start writing.
          </div>
        ) : (
          <PageTree
            nodes={forest}
            activeId={activePage}
            collapsed={collapsed}
            onOpen={onOpen}
            onToggle={onToggleCollapse}
            onAddChild={onAddChild}
            onDelete={onDelete}
            onMove={onMove}
          />
        )}
      </div>
    </aside>
  );
}

// ── The view ─────────────────────────────────────────────

const loadTreeCollapsed = (): Set<string> => {
  try {
    const raw = localStorage.getItem(TREE_COLLAPSE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(parsed) ? parsed.filter((x) => typeof x === "string") : []);
  } catch {
    return new Set();
  }
};
const saveTreeCollapsed = (set: ReadonlySet<string>): void => {
  try {
    localStorage.setItem(TREE_COLLAPSE_KEY, JSON.stringify([...set]));
  } catch {
    // best-effort
  }
};

export function PagesView() {
  const { state, actions } = useDucktape();
  const [titleDraft, setTitleDraft] = useState("");
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [treeCollapsed, setTreeCollapsed] = useState<ReadonlySet<string>>(loadTreeCollapsed);
  const [focusId, setFocusId] = useState<string | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  const inputs = useRef(new Map<string, HTMLTextAreaElement>());
  const titleRef = useRef<HTMLInputElement | null>(null);
  const titleFocusedRef = useRef(false);

  const blocks = state.activePageBlocks;
  const root =
    state.activePage && blocks.length > 0 && blocks[0].id === state.activePage
      ? blocks[0]
      : null;
  const rows = useMemo(() => buildRows(blocks, collapsed), [blocks, collapsed]);

  // live thread count keyed by target (block id or page id).
  const threadsByTarget = useMemo(() => {
    const map = new Map<string, number>();
    for (const group of state.pageThreads) map.set(group.target, group.threads.length);
    return map;
  }, [state.pageThreads]);

  // enumerate the page list on mount; the rail's refresh re-runs it and every
  // committed block re-enumerates through the store's refresh.
  useEffect(() => {
    actions.listPages();
  }, [actions]);

  // adopt the store title only while the input is not being edited — the
  // same draft-protection contract as a block row...
  useEffect(() => {
    if (!titleFocusedRef.current) setTitleDraft(root?.text ?? "");
  }, [root?.text]);
  // ...but a page switch resets unconditionally (declared after, so it wins
  // when both fire): a still-focused input must never carry one page's draft
  // onto another.
  useEffect(() => setTitleDraft(root?.text ?? ""), [root?.id]);

  // load comment threads when the active page changes.
  useEffect(() => {
    actions.loadPageThreads();
  }, [actions, state.activePage]);

  // a freshly-created empty page drops the cursor in the title.
  useEffect(() => {
    if (root && root.text === "") titleRef.current?.focus();
  }, [root?.id]);

  // once the snapshot carries a block we queued focus for, focus it.
  useEffect(() => {
    if (!focusId) return;
    const el = inputs.current.get(focusId);
    if (el) {
      el.focus();
      el.setSelectionRange(el.value.length, el.value.length);
      setFocusId(null);
    }
  }, [rows, focusId]);

  // Docs-scoped keyboard shortcuts. This listener is registered only while the
  // Docs screen is mounted, so it never leaks into other modules:
  //   ⌘/Ctrl + ⇧ + [ / ]   previous / next tab (cycles, wraps at the ends)
  //   ⌘/Ctrl + T or N       new top-level page
  //   ⌘/Ctrl + W            deliberately NOT handled — it must fall through to
  //                         the window (close-to-tray), never close a doc tab.
  // Bracket keys are matched on `event.code` (physical key), so the shift-
  // produced "{"/"}" characters don't matter.
  useEffect(() => {
    // DocumentEventMap["keydown"] is the DOM KeyboardEvent; the bare name is
    // shadowed here by React's KeyboardEvent import used above in BlockRow.
    const onKey = (event: DocumentEventMap["keydown"]) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey) return;

      if (
        event.shiftKey &&
        (event.code === "BracketRight" || event.code === "BracketLeft")
      ) {
        const tabs = state.openTabs;
        if (tabs.length === 0) return;
        event.preventDefault();
        const step = event.code === "BracketRight" ? 1 : -1;
        const current = state.activePage ? tabs.indexOf(state.activePage) : -1;
        const base = current === -1 ? 0 : current;
        actions.openPage(tabs[(base + step + tabs.length) % tabs.length]);
        return;
      }

      if (!event.shiftKey && (event.code === "KeyT" || event.code === "KeyN")) {
        event.preventDefault();
        actions.createChildPage(null);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [actions, state.openTabs, state.activePage]);

  const insertAfterRow = (row: Row, kind: BlockKind) => {
    if (!row.block.parent) return;
    const blockId = crypto.randomUUID();
    actions.insertPageBlock({
      blockId,
      parent: row.block.parent,
      after: row.block.id,
      kind,
      text: "",
    });
    setFocusId(blockId);
  };

  const appendBlock = () => {
    if (!root) return;
    const blockId = crypto.randomUUID();
    actions.insertPageBlock({
      blockId,
      parent: root.id,
      after: root.children[root.children.length - 1] ?? null,
      kind: "paragraph",
      text: "",
    });
    setFocusId(blockId);
  };

  const move = (blockId: string, target: { parent: string; after: string | null } | null) => {
    if (!target) return;
    actions.movePageBlock({ blockId, ...target });
    setFocusId(blockId);
  };

  const focusRow = (row: Row | undefined) => {
    if (!row) {
      titleRef.current?.focus();
      return;
    }
    const el = inputs.current.get(row.block.id);
    if (el) {
      el.focus();
      el.setSelectionRange(el.value.length, el.value.length);
    }
  };

  const toggleTreeCollapse = (id: string) =>
    setTreeCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      saveTreeCollapsed(next);
      return next;
    });

  const confirmThenDelete = (id: string) => {
    const title = state.pages.find((p) => p.id === id)?.title || "Untitled";
    if (window.confirm(`Delete "${title}" and its contents? Child pages move up a level.`)) {
      actions.deletePage(id);
    }
  };

  const openBlockComments = (blockId: string) => {
    setPanelOpen(true);
    const has = state.pageThreads.some((g) => g.target === blockId && g.threads.length > 0);
    if (!has) {
      const text = window.prompt("Comment on this block");
      if (text && text.trim()) {
        actions.addComment({ target: blockId, text });
      }
    }
  };

  const commentOnPage = () => {
    if (!state.activePage) return;
    setPanelOpen(true);
    const text = window.prompt("Comment on this page");
    if (text && text.trim()) {
      actions.addComment({ target: state.activePage, text });
    }
  };

  const handlers: RowHandlers = {
    commitText: (blockId, text) => actions.updatePageBlockText({ blockId, text }),
    split: (row) => insertAfterRow(row, continuationKind(row.block.kind)),
    removeEmpty: (row) => {
      const index = rows.findIndex((r) => r.block.id === row.block.id);
      actions.removePageBlock(row.block.id);
      // walk back to the nearest row that has an editable input.
      for (let i = index - 1; i >= 0; i -= 1) {
        if (inputs.current.has(rows[i].block.id)) {
          focusRow(rows[i]);
          return;
        }
      }
      focusRow(undefined);
    },
    indent: (row) => move(row.block.id, indentTarget(blocks, row.block.id)),
    outdent: (row) => move(row.block.id, outdentTarget(blocks, row.block.id)),
    moveUp: (row) => move(row.block.id, moveUpTarget(blocks, row.block.id)),
    moveDown: (row) => move(row.block.id, moveDownTarget(blocks, row.block.id)),
    setKind: (blockId, kind) => {
      actions.setPageBlockKind({ blockId, kind });
      setFocusId(blockId);
    },
    setChecked: (blockId, checked) =>
      actions.setPageBlockChecked({ blockId, checked }),
    remove: (blockId) => actions.removePageBlock(blockId),
    toggleCollapse: (blockId) =>
      setCollapsed((prev) => {
        const next = new Set(prev);
        if (next.has(blockId)) next.delete(blockId);
        else next.add(blockId);
        return next;
      }),
    focusRelative: (row, delta) => {
      const index = rows.findIndex((r) => r.block.id === row.block.id);
      for (let i = index + delta; i >= 0 && i < rows.length; i += delta) {
        if (inputs.current.has(rows[i].block.id)) {
          focusRow(rows[i]);
          return;
        }
      }
      if (delta === -1) focusRow(undefined);
    },
    registerInput: (blockId, el) => {
      if (el) inputs.current.set(blockId, el);
      else inputs.current.delete(blockId);
    },
    openComments: openBlockComments,
  };

  const commitTitle = () => {
    if (root && titleDraft !== root.text) {
      actions.updatePageBlockText({ blockId: root.id, text: titleDraft });
    }
  };

  // the title rides the same edit-boundary contract as a block row: a typing
  // pause commits the rename as one op. The ref keeps the timer from
  // resetting on unrelated store re-renders; root?.id in the deps cancels a
  // pending boundary when the page switches.
  const commitTitleRef = useRef(() => {});
  commitTitleRef.current = commitTitle;
  useEffect(() => {
    if (!root || titleDraft === root.text) return;
    const timer = setTimeout(() => commitTitleRef.current(), EDIT_BOUNDARY_MS);
    return () => clearTimeout(timer);
  }, [titleDraft, root?.text, root?.id]);

  return (
    <div
      data-screen-label="Pages"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        background: color.paper,
      }}
    >
      <PageRail
        pages={state.pages}
        activePage={state.activePage}
        collapsed={treeCollapsed}
        onToggleCollapse={toggleTreeCollapse}
        onNewPage={() => actions.createChildPage(null)}
        onAddChild={(id) => actions.createChildPage(id)}
        onOpen={actions.openPage}
        onDelete={confirmThenDelete}
        onMove={(id, parent) => actions.setPageParent({ pageId: id, parent })}
        onRefresh={actions.listPages}
      />

      <main style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}>
        <DocTabs
          open={state.openTabs}
          active={state.activePage}
          titleOf={(id) => state.pages.find((p) => p.id === id)?.title ?? ""}
          onSelect={actions.openPage}
          onClose={actions.closeTab}
        />
        <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
          <div style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}>
            <header
              style={{
                height: 56,
                flexShrink: 0,
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "0 22px",
                borderBottom: `1px solid ${color.borderSoft}`,
                background: color.paper,
              }}
            >
              <div style={{ font: `600 15px ${font.sans}`, color: color.dark }}>Pages</div>
              {root ? (
                <>
                  <span style={{ color: color.muted2 }}>/</span>
                  <div
                    style={{
                      minWidth: 0,
                      font: `500 13px ${font.sans}`,
                      color: color.ink,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {root.text || "Untitled"}
                  </div>
                  <div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
                    <button
                      type="button"
                      aria-label="Comment on page"
                      onClick={commentOnPage}
                      style={headerBtn}
                    >
                      <Icon name="chat" size={13} strokeWidth={1.8} /> Comment
                    </button>
                    <button
                      type="button"
                      aria-label={panelOpen ? "Hide comments" : "Show comments"}
                      aria-pressed={panelOpen}
                      onClick={() => setPanelOpen((o) => !o)}
                      style={{
                        ...headerBtn,
                        background: panelOpen ? color.hover : color.paper,
                      }}
                    >
                      Comments
                    </button>
                  </div>
                </>
              ) : (
                <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
                  no page open
                </div>
              )}
            </header>

            <div
              style={{
                flex: 1,
                minHeight: 0,
                overflowY: "auto",
                background: color.sidebar,
                padding: root ? "22px 26px" : 0,
              }}
            >
              {!root ? (
                <div
                  style={{
                    minHeight: 240,
                    height: "100%",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    textAlign: "center",
                    color: color.muted2,
                  }}
                >
                  <div style={{ maxWidth: 330, padding: 22 }}>
                    <div
                      style={{
                        width: 42,
                        height: 42,
                        margin: "0 auto 13px",
                        borderRadius: radius.lg,
                        border: `1px solid ${color.border}`,
                        background: color.paper,
                        color: color.muted2,
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                      }}
                    >
                      <Icon name="pages" size={18} />
                    </div>
                    <div style={{ font: `600 14px ${font.sans}`, color: color.ink }}>
                      No page open
                    </div>
                    <div style={{ marginTop: 5, font: `400 12px/1.5 ${font.sans}`, color: color.muted }}>
                      Pick a page from the rail, or create one to start writing.
                    </div>
                  </div>
                </div>
              ) : (
                <div
                  style={{
                    maxWidth: 820,
                    margin: "0 auto",
                    minHeight: "100%",
                    border: `1px solid ${color.border}`,
                    borderRadius: radius.lg,
                    background: color.paper,
                    boxShadow: shadow.card,
                    overflow: "visible",
                    padding: "36px 44px 44px",
                    boxSizing: "border-box",
                  }}
                >
                  <input
                    ref={titleRef}
                    aria-label="Page title"
                    value={titleDraft}
                    onChange={(event) => setTitleDraft(event.target.value)}
                    onFocus={() => {
                      titleFocusedRef.current = true;
                    }}
                    onBlur={() => {
                      titleFocusedRef.current = false;
                      commitTitle();
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === "ArrowDown") {
                        event.preventDefault();
                        commitTitle();
                        focusRow(rows.find((r) => inputs.current.has(r.block.id)));
                      }
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
                      marginBottom: 18,
                      color: color.dark,
                      font: `650 30px/1.2 ${font.sans}`,
                    }}
                  />

                  {rows.map((row, index) => (
                    <BlockRow
                      key={row.block.id}
                      row={row}
                      index={index}
                      expanded={!collapsed.has(row.block.id)}
                      op={state.ops[opKey.pageBlock(row.block.id)]}
                      threadCount={threadsByTarget.get(row.block.id) ?? 0}
                      handlers={handlers}
                    />
                  ))}

                  <button
                    type="button"
                    aria-label="Add a block"
                    onClick={appendBlock}
                    style={{
                      all: "unset",
                      cursor: "text",
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      width: "100%",
                      boxSizing: "border-box",
                      padding: "8px 0 24px 28px",
                      color: color.muted2,
                      font: `400 13px ${font.sans}`,
                    }}
                  >
                    <Icon name="plus" size={13} strokeWidth={1.8} />
                    {rows.length === 0 ? "Start writing — or press '/' for a block menu" : "Add a block"}
                  </button>
                </div>
              )}
            </div>
          </div>

          {panelOpen && root ? (
            <CommentsPanel
              threads={state.pageThreads}
              authorNames={state.authorNames}
              onClose={() => setPanelOpen(false)}
              onReply={(threadId, text) => {
                // a reply must carry the THREAD's target (a block id or the
                // page id) — the module rejects an append whose target differs
                // from the thread's. Never assume the page here.
                const target =
                  state.pageThreads
                    .flatMap((g) => g.threads)
                    .find((v) => v.thread.id === threadId)?.thread.target ??
                  state.activePage ??
                  "";
                actions.addComment({ threadId, target, text });
              }}
              onResolve={(threadId, resolved) => actions.resolveThread({ threadId, resolved })}
              onEdit={(commentId, text) => actions.editComment({ commentId, text })}
              onDelete={(commentId) => actions.deleteComment(commentId)}
            />
          ) : null}
        </div>
      </main>
    </div>
  );
}

const headerBtn: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  gap: 5,
  padding: "5px 10px",
  borderRadius: radius.sm,
  border: `1px solid ${color.border}`,
  background: color.paper,
  color: color.muted3,
  font: `500 11.5px ${font.sans}`,
};
