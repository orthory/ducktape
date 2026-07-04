// The pages surface over the node's `pages` module: a notion-like block-tree
// editor. A page is a tree of blocks (the page itself is the root block; its
// text is the title), every block id is globally unique and shown as a
// copyable handle, and editing is KEYBOARD-FIRST:
//
//   Enter          split: a fresh sibling below (lists continue their kind)
//   Backspace      on an empty block: remove it, focus the previous one
//   Tab / S-Tab    indent under the previous sibling / outdent to grandparent
//   Alt+Up/Down    move among siblings
//   Up/Down        at the draft's edges: hop between blocks
//   "# " "- " …    markdown prefixes convert a paragraph's kind
//   "/"            slash menu over every block kind
//
// Text commits on blur (and before any structural op) — one consensus op per
// change, mirroring the rest of the console's server-authoritative writes.

import { useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, FormEvent, KeyboardEvent, ReactNode } from "react";

import type { BlockKind } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
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

const INDENT = 26;

function shortId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 8)}…` : id;
}

const sectionLabelStyle: CSSProperties = {
  font: `600 9px ${font.mono}`,
  letterSpacing: ".11em",
  color: color.muted2,
  textTransform: "uppercase",
};

/** Per-kind typography for the block textarea. */
function kindFont(kind: BlockKind): string {
  switch (kind) {
    case "Heading1":
      return `650 24px/1.25 ${font.sans}`;
    case "Heading2":
      return `650 19px/1.3 ${font.sans}`;
    case "Heading3":
      return `600 16px/1.35 ${font.sans}`;
    case "Code":
      return `400 12.5px/1.55 ${font.mono}`;
    case "Quote":
      return `400 14.5px/1.6 ${font.sans}`;
    default:
      return `400 14.5px/1.6 ${font.sans}`;
  }
}

function kindPlaceholder(kind: BlockKind): string {
  switch (kind) {
    case "Heading1":
    case "Heading2":
    case "Heading3":
      return "Heading";
    case "Todo":
      return "To-do";
    case "Bulleted":
    case "Numbered":
      return "List item";
    case "Toggle":
      return "Toggle";
    case "Quote":
      return "Quote";
    case "Code":
      return "code";
    case "Callout":
      return "Callout";
    default:
      return "Type '/' for commands";
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
}

function BlockRow({
  row,
  index,
  expanded,
  handlers,
}: {
  row: Row;
  index: number;
  /** Only meaningful for Toggle rows: whether children are shown. */
  expanded: boolean;
  handlers: RowHandlers;
}) {
  const { block, depth } = row;
  const [draft, setDraft] = useState(block.text);
  const [slashDismissed, setSlashDismissed] = useState(false);
  const [slashIndex, setSlashIndex] = useState(0);
  const areaRef = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => setDraft(block.text), [block.text]);

  // auto-grow: the textarea is exactly as tall as its content.
  useEffect(() => {
    const el = areaRef.current;
    if (el) {
      el.style.height = "0";
      el.style.height = `${el.scrollHeight}px`;
    }
  }, [draft, block.kind]);

  const slashOpen =
    draft.startsWith("/") && !slashDismissed && block.kind !== "Code";
  const slashQuery = slashOpen ? draft.slice(1) : "";
  const slashOptions = filterSlashKinds(slashQuery);

  const dirty = () => draft !== block.text;
  const maybeCommit = () => {
    if (dirty()) handlers.commitText(block.id, draft);
  };

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
    if (block.kind === "Paragraph") {
      const shortcut = shortcutFor(next);
      if (shortcut) {
        handlers.setKind(block.id, shortcut.kind);
        if (shortcut.kind === "Divider") {
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
          && block.kind !== "Paragraph") {
        handlers.setKind(block.id, "Paragraph");
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

  const code = block.kind === "Code";
  const quote = block.kind === "Quote";
  const callout = block.kind === "Callout";
  const todoDone = block.kind === "Todo" && block.checked;
  const blockNumber = index + 1;

  // the left gutter marker per kind (bullet, number, checkbox, chevron).
  const marker: ReactNode =
    block.kind === "Bulleted" ? (
      <span style={{ font: `700 14px ${font.sans}`, color: color.muted3 }}>•</span>
    ) : block.kind === "Numbered" ? (
      <span style={{ font: `500 12.5px ${font.mono}`, color: color.muted3 }}>
        {row.listIndex ?? 1}.
      </span>
    ) : block.kind === "Todo" ? (
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
    ) : block.kind === "Toggle" ? (
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
    block.kind === "Divider" ? (
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
          onBlur={maybeCommit}
          onKeyDown={onKeyDown}
          placeholder={kindPlaceholder(block.kind)}
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
          paddingTop: block.kind === "Heading1" ? 6 : block.kind === "Heading2" ? 3 : 0,
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
          gap: 4,
          paddingTop: 3,
        }}
      >
        <button
          type="button"
          aria-label={`Copy id of block ${blockNumber}`}
          title={`copy block id\n${block.id}`}
          onClick={() => {
            void navigator.clipboard?.writeText(block.id);
          }}
          style={{
            all: "unset",
            cursor: "pointer",
            display: "inline-flex",
            alignItems: "center",
            gap: 3,
            padding: "2px 5px",
            borderRadius: 5,
            color: color.muted2,
            font: `500 9.5px ${font.mono}`,
          }}
        >
          <Icon name="hash" size={9} />
          {shortId(block.id)}
        </button>
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
      </div>
    </div>
  );
}

// ── Page rail (enumerated pages) ─────────────────────────

function PageRail({
  pages,
  activePage,
  newTitle,
  setNewTitle,
  onCreate,
  onRefresh,
  openPage,
}: {
  pages: { id: string; title: string }[];
  activePage: string | null;
  newTitle: string;
  setNewTitle: (title: string) => void;
  onCreate: (event: FormEvent) => void;
  onRefresh: () => void;
  openPage: (id: string) => void;
}) {
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
        <div style={{ minWidth: 0 }}>
          <div style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>Pages</div>
          <div style={{ marginTop: 1, font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
            block trees
          </div>
        </div>
        <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
          {pages.length}
        </div>
      </div>

      <div style={{ padding: 14, borderBottom: `1px solid ${color.borderSoft}` }}>
        <form onSubmit={onCreate} style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <label htmlFor="pages-new-title" style={sectionLabelStyle}>
            New page title
          </label>
          <div style={{ display: "flex", gap: 7 }}>
            <input
              id="pages-new-title"
              value={newTitle}
              onChange={(event) => setNewTitle(event.target.value)}
              placeholder="Launch plan"
              spellCheck={false}
              style={{
                width: "100%",
                minWidth: 0,
                boxSizing: "border-box",
                padding: "8px 10px",
                borderRadius: radius.sm,
                border: `1px solid ${color.borderStrong}`,
                background: color.paper,
                font: `400 12.5px ${font.sans}`,
                color: color.ink,
                outline: "none",
              }}
            />
            <button
              type="submit"
              aria-label="Create page"
              title="Create page"
              disabled={!newTitle.trim()}
              style={{
                all: "unset",
                cursor: newTitle.trim() ? "pointer" : "default",
                flexShrink: 0,
                width: 32,
                height: 32,
                borderRadius: 8,
                background: newTitle.trim() ? color.dark : color.chip,
                color: newTitle.trim() ? color.onDark : color.muted2,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Icon name="plus" size={14} strokeWidth={1.9} />
            </button>
          </div>
        </form>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "13px 0" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "0 14px 8px" }}>
          <div style={sectionLabelStyle}>All pages</div>
          <button
            type="button"
            aria-label="Refresh pages"
            title="Refresh pages"
            onClick={onRefresh}
            style={{
              all: "unset",
              cursor: "pointer",
              marginLeft: "auto",
              width: 24,
              height: 24,
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
          pages.map((page) => {
            const active = page.id === activePage;
            return (
              <button
                key={page.id}
                type="button"
                aria-label={`Open ${page.title || "untitled page"}`}
                title={page.id}
                onClick={() => openPage(page.id)}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  boxSizing: "border-box",
                  width: "calc(100% - 12px)",
                  margin: "1px 6px",
                  padding: "6px 8px",
                  borderRadius: radius.sm,
                  background: active ? color.hover : "transparent",
                  color: active ? color.ink : color.inkSofter,
                }}
              >
                <Icon
                  name="pages"
                  size={14}
                  strokeWidth={1.7}
                  style={{ flexShrink: 0, color: active ? accentVar : color.muted2 }}
                />
                <span
                  style={{
                    flex: 1,
                    minWidth: 0,
                    font: active ? `600 12.5px ${font.sans}` : `500 12.5px ${font.sans}`,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {page.title || "Untitled"}
                </span>
                {active ? (
                  <span
                    style={{
                      flexShrink: 0,
                      font: `600 8.5px ${font.mono}`,
                      color: color.onDark,
                      background: color.dark,
                      borderRadius: 4,
                      padding: "2px 5px",
                      letterSpacing: ".05em",
                    }}
                  >
                    OPEN
                  </span>
                ) : null}
              </button>
            );
          })
        )}
      </div>

      <div
        style={{
          padding: "12px 14px 14px",
          borderTop: `1px solid ${color.borderSoft}`,
          font: `400 11.5px/1.45 ${font.sans}`,
          color: color.muted2,
        }}
      >
        Every block has a globally unique id — copy it from a block&apos;s hash
        chip to reference it from anywhere.
      </div>
    </aside>
  );
}

// ── The view ─────────────────────────────────────────────

export function PagesView() {
  const { state, actions } = useDucktape();
  const [newTitle, setNewTitle] = useState("");
  const [titleDraft, setTitleDraft] = useState("");
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [focusId, setFocusId] = useState<string | null>(null);
  const inputs = useRef(new Map<string, HTMLTextAreaElement>());
  const titleRef = useRef<HTMLInputElement | null>(null);

  const blocks = state.activePageBlocks;
  const root =
    state.activePage && blocks.length > 0 && blocks[0].id === state.activePage
      ? blocks[0]
      : null;
  const rows = useMemo(() => buildRows(blocks, collapsed), [blocks, collapsed]);

  // enumerate the page list on mount; the rail's refresh re-runs it and every
  // committed block re-enumerates through the store's refresh.
  useEffect(() => {
    actions.listPages();
  }, [actions]);

  useEffect(() => setTitleDraft(root?.text ?? ""), [root?.text]);

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
      kind: "Paragraph",
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
  };

  const create = (event: FormEvent) => {
    event.preventDefault();
    if (!newTitle.trim()) return;
    actions.createPage(newTitle);
    setNewTitle("");
  };

  const commitTitle = () => {
    if (root && titleDraft !== root.text) {
      actions.updatePageBlockText({ blockId: root.id, text: titleDraft });
    }
  };

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
        newTitle={newTitle}
        setNewTitle={setNewTitle}
        onCreate={create}
        onRefresh={actions.listPages}
        openPage={actions.openPage}
      />

      <main style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}>
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
          <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Pages</div>
          {root ? (
            <>
              <div
                title={root.id}
                style={{
                  minWidth: 0,
                  font: `500 12px ${font.mono}`,
                  color: color.muted2,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {root.text || "Untitled"}
              </div>
              <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
                {rows.length} {rows.length === 1 ? "block" : "blocks"}
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
                  Pick a page from the rail, or create one to start a block tree.
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
                onBlur={commitTitle}
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
                  marginBottom: 4,
                  color: color.dark,
                  font: `650 30px/1.2 ${font.sans}`,
                }}
              />
              <div
                title={root.id}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 5,
                  marginBottom: 18,
                  color: color.muted2,
                }}
              >
                <Icon name="hash" size={10} />
                <span style={{ font: `500 10px ${font.mono}` }}>{shortId(root.id)}</span>
              </div>

              {rows.map((row, index) => (
                <BlockRow
                  key={row.block.id}
                  row={row}
                  index={index}
                  expanded={!collapsed.has(row.block.id)}
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
                {rows.length === 0 ? "Start writing — or type '/' for a block menu" : "Add a block"}
              </button>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
