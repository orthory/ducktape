// The Docs surface over the node's `pages` module: a Notion-like block-tree
// editor with a nested page tree, document tabs, and comment threads. This
// file is the orchestrator — store wiring, focus management, keyboard
// shortcuts, layout. The editable row (and its keyboard grammar) lives in
// BlockRow.tsx, the rail in PageRail.tsx.

import { useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";

import type { BlockKind } from "../../../domain/pages-client";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import {
  EDIT_BOUNDARY_MS,
  buildRows,
  continuationKind,
  indentTarget,
  moveDownTarget,
  moveUpTarget,
  outdentTarget,
} from "./pages-model";
import type { Row } from "./pages-model";
import { BlockRow } from "./BlockRow";
import type { RowHandlers } from "./BlockRow";
import { CommentCard } from "./CommentCard";
import type { CommentAnchor } from "./CommentCard";
import { DocTabs } from "./DocTabs";
import { PageRail } from "./PageRail";
import { CommentsPanel } from "./CommentsPanel";
import { Subpages } from "./Subpages";

export { EDIT_BOUNDARY_MS };

// ── The view ─────────────────────────────────────────────

export function PagesView() {
  const { state, actions } = useDucktape();
  const [titleDraft, setTitleDraft] = useState("");
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [focusId, setFocusId] = useState<string | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  const [pendingPageDelete, setPendingPageDelete] = useState<string | null>(null);
  // the floating comment card's aim: ONE target (a block id or the page id),
  // the label naming it, and the viewport anchor of the affordance that
  // opened it. Null = no card. The aside panel stays as the all-threads
  // overview behind the header toggle; composing happens in the card.
  const [commentCard, setCommentCard] = useState<{
    target: string;
    label: string;
    anchor: CommentAnchor;
  } | null>(null);
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

  // load comment threads when the active page changes; a card aimed at the
  // previous page's blocks must not survive the switch.
  useEffect(() => {
    setCommentCard(null);
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
  //   ⌘/Ctrl + W            close the active doc tab — browser muscle memory.
  //                         With no doc open it falls through to the window
  //                         untouched (close-to-tray).
  // Bracket keys are matched on `event.code` (physical key), so the shift-
  // produced "{"/"}" characters don't matter.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
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

      if (!event.shiftKey && event.code === "KeyW") {
        if (!state.activePage) return;
        event.preventDefault();
        actions.closeTab(state.activePage);
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

  const confirmThenDelete = (id: string) => {
    setPendingPageDelete(id);
  };

  const openBlockComments = (blockId: string, anchor: CommentAnchor) => {
    setCommentCard({ target: blockId, label: "this block", anchor });
  };

  const commentOnPage = (anchor: CommentAnchor) => {
    if (!state.activePage) return;
    setCommentCard({ target: state.activePage, label: "this page", anchor });
  };

  // a reply must carry the THREAD's target (a block id or the page id) — the
  // module rejects an append whose target differs from the thread's. Never
  // assume the page here. Shared by the card and the panel.
  const replyToThread = (threadId: string, text: string) => {
    const target =
      state.pageThreads
        .flatMap((g) => g.threads)
        .find((v) => v.thread.id === threadId)?.thread.target ??
      state.activePage ??
      "";
    actions.addComment({ threadId, target, text });
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
    // the action creates the untitled child and opens its tab (cursor lands
    // in the title via the fresh-page focus effect above).
    createSubpage: () => actions.createChildPage(state.activePage),
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

  const pendingPageDeleteTitle =
    state.pages.find((p) => p.id === pendingPageDelete)?.title || "Untitled";

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
                      onClick={(event) => {
                        const rect = event.currentTarget.getBoundingClientRect();
                        commentOnPage({ x: rect.left, y: rect.bottom });
                      }}
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
              data-testid="doc-scroll"
              style={{
                flex: 1,
                minHeight: 0,
                display: "flex",
                flexDirection: "column",
                overflowY: "auto",
                background: color.paper,
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
                // the Notion-style endless canvas: a plain centered column on
                // the paper, no card chrome, and a click-to-append filler
                // below so the page has no visible bottom end.
                <div
                  style={{
                    width: "100%",
                    maxWidth: 820,
                    margin: "0 auto",
                    padding: "36px 44px 0",
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

                  <Subpages
                    pages={state.pages}
                    activePage={root.id}
                    onOpen={actions.openPage}
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
                    // mousedown, not click: pressing while a block is focused
                    // must append BEFORE the blur commit re-renders the tree
                    // out from under the click (same dodge as the SlashMenu).
                    onMouseDown={(event) => {
                      event.preventDefault();
                      appendBlock();
                    }}
                    style={{
                      all: "unset",
                      cursor: "text",
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      width: "100%",
                      boxSizing: "border-box",
                      padding: "8px 0 8px 28px",
                      color: color.muted2,
                      font: `400 13px ${font.sans}`,
                    }}
                  >
                    <Icon name="plus" size={13} strokeWidth={1.8} />
                    {rows.length === 0 ? "Start writing — or press '/' for a block menu" : "Add a block"}
                  </button>
                </div>
              )}
              {root ? (
                <div
                  data-testid="page-canvas-filler"
                  aria-hidden="true"
                  onMouseDown={(event) => {
                    event.preventDefault();
                    appendBlock();
                  }}
                  style={{ flex: 1, minHeight: "40vh", cursor: "text" }}
                />
              ) : null}
            </div>
          </div>

          {panelOpen && root ? (
            <CommentsPanel
              threads={state.pageThreads}
              authorNames={state.authorNames}
              composer={null}
              onClose={() => setPanelOpen(false)}
              onSubmitNew={(target, text) => actions.addComment({ target, text })}
              onCancelNew={() => {}}
              onReply={replyToThread}
              onResolve={(threadId, resolved) => actions.resolveThread({ threadId, resolved })}
              onEdit={(commentId, text) => actions.editComment({ commentId, text })}
              onDelete={(commentId) => actions.deleteComment(commentId)}
            />
          ) : null}
        </div>
      </main>
      {commentCard ? (
        <CommentCard
          target={commentCard.target}
          label={commentCard.label}
          anchor={commentCard.anchor}
          threads={
            state.pageThreads.find((g) => g.target === commentCard.target)?.threads ?? []
          }
          authorNames={state.authorNames}
          onClose={() => setCommentCard(null)}
          onSubmitNew={(target, text) => actions.addComment({ target, text })}
          onReply={replyToThread}
          onResolve={(threadId, resolved) => actions.resolveThread({ threadId, resolved })}
          onEdit={(commentId, text) => actions.editComment({ commentId, text })}
          onDelete={(commentId) => actions.deleteComment(commentId)}
        />
      ) : null}
      {pendingPageDelete && (
        <ConfirmDialog
          title={`Delete ${pendingPageDeleteTitle}?`}
          confirmLabel="Delete page"
          onCancel={() => setPendingPageDelete(null)}
          onConfirm={() => {
            actions.deletePage(pendingPageDelete);
            setPendingPageDelete(null);
          }}
        >
          This deletes the page and its contents. Child pages move up a level.
        </ConfirmDialog>
      )}
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
