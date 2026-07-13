// The Docs surface over the node's `pages` module: a Notion-like block-tree
// editor with a nested page tree, document tabs, and comment threads. This
// file is the orchestrator — store wiring, focus management, keyboard
// shortcuts, layout. The editable row (and its keyboard grammar) lives in
// BlockRow.tsx, the rail in PageRail.tsx, the header in PageHeader.tsx and the
// title (with its icon) in PageTitle.tsx.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { SetStateAction } from "react";

import { ConfirmDialog } from "../../components/ConfirmDialog";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import { selfAuthorBytes } from "../../store/state";
import { useDucktape } from "../../store/use-ducktape";
import { selfAuthorKeyOf } from "../chat/chat-helpers";
import { color, font, radius } from "../../theme/tokens";
import { EDIT_BOUNDARY_MS, buildRows, subtreePlan } from "./pages-model";
import type { DuplicateOp } from "./pages-model";
import type { FocusIntent } from "./block-keys";
import { dropTarget } from "./page-drag";
import { loadCollapsed, saveCollapsed } from "./page-collapse";
import { MAX_PASTE_BLOCKS } from "./page-paste";
import { ancestorChain } from "./page-tree";
import { COLUMN_PAD_X } from "./pages-style";
import { useRowHandlers } from "./use-row-handlers";
import type { DragState } from "./use-row-handlers";
import { BlockRow } from "./BlockRow";
import { CommentCard } from "./CommentCard";
import type { CommentAnchor } from "./CommentCard";
import { DocTabs } from "./DocTabs";
import { PageHeader } from "./PageHeader";
import { PageRail } from "./PageRail";
import { PageTitle } from "./PageTitle";
import { CommentsPanel } from "./CommentsPanel";
import { Subpages } from "./Subpages";

export { EDIT_BOUNDARY_MS };

// ── The view ─────────────────────────────────────────────

export function PagesView() {
  const { state, actions } = useDucktape();
  // toggle collapse is a view preference, persisted per page (page-collapse.ts)
  // exactly like the rail's tree collapse — it used to be plain component state,
  // so every remount re-expanded every toggle in the document.
  const [collapsed, setCollapsedState] = useState<ReadonlySet<string>>(() =>
    loadCollapsed(state.activePage),
  );
  // which block to focus next, and WHERE in it. This used to be a bare block
  // id, so every focus hop slammed the caret to the end of the text.
  const [focus, setFocus] = useState<FocusIntent | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  const [pendingPageDelete, setPendingPageDelete] = useState<string | null>(null);
  const [pendingBlockDelete, setPendingBlockDelete] = useState<string | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const [pasteNotice, setPasteNotice] = useState<number | null>(null);
  // the just-deleted subtree, snapshotted for Undo. null = no toast; [] = a
  // toast without the button (the subtree was over the op cap to restore); a
  // non-empty plan = the ops to re-insert. Superseded by a newer delete.
  const [deleteUndo, setDeleteUndo] = useState<DuplicateOp[] | null>(null);
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

  const blocks = state.activePageBlocks;
  const root =
    state.activePage && blocks.length > 0 && blocks[0].id === state.activePage
      ? blocks[0]
      : null;
  const rows = useMemo(() => buildRows(blocks, collapsed), [blocks, collapsed]);
  const chain = useMemo(
    () => (root ? ancestorChain(state.pages, root.id) : []),
    [state.pages, root?.id],
  );
  // committed authorship for OUR writes — a comment's Edit/Delete only exist
  // for the author, because the module rejects anyone else's.
  const selfKey = selfAuthorKeyOf(selfAuthorBytes(state.status, state.author));

  const activePage = state.activePage;
  // the persisted set belongs to the page that was open when it changed, so the
  // writer reads the page id from a ref, never from a stale closure — and
  // `setCollapsed` stays referentially stable, which BlockRow's memo needs.
  const pageRef = useRef(activePage);
  pageRef.current = activePage;
  const setCollapsed = useCallback(
    (update: SetStateAction<ReadonlySet<string>>) =>
      setCollapsedState((prev) => {
        const next = typeof update === "function" ? update(prev) : update;
        saveCollapsed(pageRef.current, next);
        return next;
      }),
    [],
  );

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

  // load comment threads when the active page changes; a card aimed at the
  // previous page's blocks must not survive the switch. The collapse set is
  // per page, so it reloads here too.
  useEffect(() => {
    setCommentCard(null);
    setCollapsedState(loadCollapsed(activePage));
    actions.loadPageThreads();
  }, [actions, activePage]);

  // a freshly-created empty page drops the cursor in the title.
  useEffect(() => {
    if (root && root.text === "") titleRef.current?.focus();
  }, [root?.id]);

  // NOTE: the caret is NOT placed from here. A row's textarea value is owned by
  // BlockRow's draft state, and writing a textarea's value stomps its selection
  // to the end — so a caret placed by the parent before the row adopts the new
  // text is silently lost (this is exactly what a merge does). The row applies
  // its own caret once its draft matches the committed text; `focus` below is
  // only ever "which row wants the caret, and where".

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

  // a capped paste says so, then gets out of the way.
  useEffect(() => {
    if (pasteNotice === null) return;
    const timer = setTimeout(() => setPasteNotice(null), 6000);
    return () => clearTimeout(timer);
  }, [pasteNotice]);

  // the delete-undo toast lingers a little longer than the paste notice — undo
  // is an action, not just an FYI — then dismisses itself.
  useEffect(() => {
    if (deleteUndo === null) return;
    const timer = setTimeout(() => setDeleteUndo(null), 8000);
    return () => clearTimeout(timer);
  }, [deleteUndo]);

  // Replay the snapshot as plain inserts: the wire FIFO (actions.ts) keeps them
  // in plan order, so the subtree comes back exactly as it was. A checked to-do
  // needs a second op — InsertBlock carries no `checked` bit. Comments on the
  // deleted blocks were purged in consensus and cannot be restored.
  const undoDelete = () => {
    if (!deleteUndo) return;
    for (const op of deleteUndo) {
      actions.insertPageBlock({
        blockId: op.blockId,
        parent: op.parent,
        after: op.after,
        kind: op.kind,
        text: op.text,
      });
      if (op.checked) actions.setPageBlockChecked({ blockId: op.blockId, checked: true });
    }
    setDeleteUndo(null);
  };

  const openBlockComments = useCallback((blockId: string, anchor: CommentAnchor) => {
    setCommentCard({ target: blockId, label: "this block", anchor });
  }, []);

  // row intents -> store ops + caret placement. `handlers` is referentially
  // stable, which is what lets BlockRow's memo hold.
  const { handlers, appendBlock, focusRow } = useRowHandlers({
    actions,
    rows,
    blocks,
    root,
    activePage,
    drag,
    inputs,
    titleRef,
    setFocus,
    setCollapsed,
    setDrag,
    openComments: openBlockComments,
    confirmRemove: setPendingBlockDelete,
    onPasteCapped: setPasteNotice,
    onDeleted: setDeleteUndo,
  });

  const commentOnPage = (anchor: CommentAnchor) => {
    if (!activePage) return;
    setCommentCard({ target: activePage, label: "this page", anchor });
  };

  // a reply must carry the THREAD's target (a block id or the page id) — the
  // module rejects an append whose target differs from the thread's. Never
  // assume the page here. Shared by the card and the panel.
  const replyToThread = (threadId: string, text: string) => {
    const target =
      state.pageThreads
        .flatMap((g) => g.threads)
        .find((v) => v.thread.id === threadId)?.thread.target ??
      activePage ??
      "";
    actions.addComment({ threadId, target, text });
  };

  const pendingPageDeleteTitle =
    state.pages.find((p) => p.id === pendingPageDelete)?.title || "Untitled";
  const pendingBlockChildren =
    blocks.find((b) => b.id === pendingBlockDelete)?.children.length ?? 0;

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
        activePage={activePage}
        onNewPage={() => actions.createChildPage(null)}
        onAddChild={(id) => actions.createChildPage(id)}
        onOpen={actions.openPage}
        onDelete={setPendingPageDelete}
        onMove={(id, parent) => actions.setPageParent({ pageId: id, parent })}
        onRefresh={actions.listPages}
      />

      <main style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}>
        <DocTabs
          open={state.openTabs}
          active={activePage}
          titleOf={(id) => state.pages.find((p) => p.id === id)?.title ?? ""}
          onSelect={actions.openPage}
          onClose={actions.closeTab}
        />
        <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
          <div style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}>
            <PageHeader
              chain={chain}
              panelOpen={panelOpen}
              onOpen={actions.openPage}
              onComment={commentOnPage}
              onTogglePanel={() => setPanelOpen((open) => !open)}
            />

            {pasteNotice !== null ? (
              <div
                role="status"
                style={{
                  padding: "7px 22px",
                  borderBottom: `1px solid ${color.borderSoft}`,
                  background: color.sunken,
                  color: color.muted3,
                  font: `500 11.5px ${font.sans}`,
                }}
              >
                Pasted the first {MAX_PASTE_BLOCKS} blocks — {pasteNotice} more line
                {pasteNotice === 1 ? "" : "s"} were dropped. Each block is one write, so a
                paste is capped.
              </div>
            ) : null}

            {deleteUndo !== null ? (
              <div
                role="status"
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  padding: "7px 22px",
                  borderBottom: `1px solid ${color.borderSoft}`,
                  background: color.sunken,
                  color: color.muted3,
                  font: `500 11.5px ${font.sans}`,
                }}
              >
                <span>Block deleted.</span>
                {deleteUndo.length > 0 ? (
                  <>
                    <button
                      type="button"
                      onClick={undoDelete}
                      style={{
                        all: "unset",
                        cursor: "pointer",
                        color: color.accent,
                        font: `600 11.5px ${font.sans}`,
                        textDecoration: "underline",
                      }}
                    >
                      Undo
                    </button>
                    <span>Comments on it are not restored.</span>
                  </>
                ) : null}
              </div>
            ) : null}

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
                // below so the page has no visible bottom end. The horizontal
                // padding houses the rows' hover gutters (pages-style keeps the
                // two in step) — without it the scroll box would clip them.
                <div
                  style={{
                    width: "100%",
                    maxWidth: 820,
                    margin: "0 auto",
                    padding: `36px ${COLUMN_PAD_X}px 0`,
                    boxSizing: "border-box",
                  }}
                >
                  <PageTitle
                    pageId={root.id}
                    raw={root.text}
                    titleRef={titleRef}
                    onCommit={(text) =>
                      actions.updatePageBlockText({ blockId: root.id, text })
                    }
                    onDescend={() => {
                      // descend into the body. On a page with no blocks yet
                      // there is nothing to descend into, so make one.
                      const first = rows.find((r) => inputs.current.has(r.block.id));
                      if (first) focusRow(first, "start");
                      else appendBlock();
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
                      prevKind={index > 0 ? rows[index - 1].block.kind : null}
                      caret={focus?.id === row.block.id ? focus.caret : null}
                      expanded={!collapsed.has(row.block.id)}
                      op={state.ops[opKey.pageBlock(row.block.id)]}
                      threadCount={threadsByTarget.get(row.block.id) ?? 0}
                      // the indicator only appears where the drop would ACTUALLY
                      // land: a drag into its own subtree is a cycle the module
                      // would reject, and must not be invited.
                      dropEdge={
                        drag?.over === row.block.id &&
                        dropTarget(blocks, drag.id, row.block.id, drag.edge)
                          ? drag.edge
                          : null
                      }
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
                    // keyboard activation (Enter/Space) synthesizes a click
                    // with detail 0 and no mousedown — only that path appends
                    // here; a pointer's trailing click (detail ≥ 1) was
                    // already handled above.
                    onClick={(event) => {
                      if (event.detail === 0) appendBlock();
                    }}
                    style={{
                      all: "unset",
                      cursor: "text",
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      width: "100%",
                      boxSizing: "border-box",
                      padding: "8px 0",
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
              selfKey={selfKey}
              ops={state.ops}
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
          selfKey={selfKey}
          ops={state.ops}
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
      {pendingBlockDelete && (
        <ConfirmDialog
          title="Delete this block?"
          confirmLabel="Delete block"
          onCancel={() => setPendingBlockDelete(null)}
          onConfirm={() => {
            // Snapshot for Undo before the subtree is gone (empty if too big).
            setDeleteUndo(subtreePlan(blocks, pendingBlockDelete));
            actions.removePageBlock(pendingBlockDelete);
            setPendingBlockDelete(null);
          }}
        >
          It has {pendingBlockChildren} nested block{pendingBlockChildren === 1 ? "" : "s"}, which
          go with it.
        </ConfirmDialog>
      )}
    </div>
  );
}
