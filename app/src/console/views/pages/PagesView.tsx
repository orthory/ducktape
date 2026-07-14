// The Docs surface over the node's `pages` module: a Notion-like block-tree
// editor with a nested page tree, document tabs, and comment threads. This
// file is the orchestrator — store wiring, focus management, keyboard
// shortcuts, layout. The editable row (and its keyboard grammar) lives in
// BlockRow.tsx, the rail in PageRail.tsx, the header in PageHeader.tsx and the
// title (with its icon) in PageTitle.tsx.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { SetStateAction } from "react";
import type { RelativeAnchor, ThreadView } from "../../../domain/pages-client";

import { ConfirmDialog } from "../../components/ConfirmDialog";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import {
  DEFAULT_AUTHOR,
  loadPendingDisplayName,
  selfAuthorBytes,
} from "../../store/state";
import { useDucktape } from "../../store/use-ducktape";
import { selfAuthorKeyOf } from "../chat/chat-helpers";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { EDIT_BOUNDARY_MS, buildRows, subtreePlan, threadsForRange } from "./pages-model";
import type { DuplicateOp } from "./pages-model";
import type { FocusIntent } from "./block-keys";
import { dropTarget } from "./page-drag";
import { loadCollapsed, saveCollapsed } from "./page-collapse";
import { MAX_PASTE_BLOCKS } from "./page-paste";
import { ancestorChain } from "./page-tree";
import { COLUMN_PAD_X, DOC_COLUMN_MAX } from "./pages-style";
import { useRowHandlers } from "./use-row-handlers";
import type { DragState } from "./use-row-handlers";
import { BlockRow } from "./BlockRow";
import { CommentCard } from "./CommentCard";
import type { CommentAnchor } from "./CommentCard";
import { DocTabs } from "./DocTabs";
import { PageHeader } from "./PageHeader";
import { PageNotice } from "./PageNotice";
import { PageRail } from "./PageRail";
import { PageTitle } from "./PageTitle";
import type { PagePresencePeer } from "./PagePresence";
import { Subpages } from "./Subpages";
import { usePagePresence } from "./use-page-presence";

export { EDIT_BOUNDARY_MS };
const EMPTY_PRESENCE: PagePresencePeer[] = [];
const EMPTY_THREADS: ThreadView[] = [];

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
  const [pendingPageDelete, setPendingPageDelete] = useState<string | null>(null);
  const [pendingBlockDelete, setPendingBlockDelete] = useState<string | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const [pasteNotice, setPasteNotice] = useState<number | null>(null);
  // the just-deleted subtree, snapshotted for Undo, BOUND TO THE PAGE it came
  // from: this view survives a doc switch, and InsertBlock is parent/after-
  // anchored and page-agnostic, so a replay fired from another doc restores the
  // subtree into the original page — off-screen, looking like nothing happened.
  // null = no toast; `ops` [] = a toast without the button (the subtree was over
  // the op cap); a non-empty plan = the ops to re-insert. A newer delete wins.
  const [deleteUndo, setDeleteUndo] = useState<{ page: string; ops: DuplicateOp[] } | null>(
    null,
  );
  // a restore that could not land — see `undoDelete`.
  const [undoFailed, setUndoFailed] = useState(false);
  // the floating comment card's aim: ONE target (a block id or the page id),
  // the label naming it, and the viewport anchor of the affordance that
  // opened it. Null = no card. Threads live beside their page/block target;
  // there is no second, duplicate all-comments list to get lost inside.
  const [commentCard, setCommentCard] = useState<{
    target: string;
    label: string;
    anchor: CommentAnchor;
    range?: RelativeAnchor;
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
  const selfName =
    state.author === DEFAULT_AUTHOR
      ? loadPendingDisplayName() ?? state.author
      : state.author;

  const activePage = state.activePage;
  const presenceRecipients = useMemo(() => {
    const self = state.status?.publicKey?.toLowerCase();
    return [...new Set([...state.members, ...state.residents].map((key) => key.toLowerCase()))]
      .filter((key) => key !== self);
  }, [state.members, state.residents, state.status?.publicKey]);
  const { peers: livePeers, publishCursor } = usePagePresence({
    nodeUrl: state.nodeUrl,
    pageId: activePage,
    selfNode: state.status?.publicKey ?? null,
    recipients: presenceRecipients,
  });
  const onCursor = useCallback(
    (blockId: string | null, anchor: number, head: number) =>
      publishCursor({ blockId, anchor, head }),
    [publishCursor],
  );
  const presence = useMemo<PagePresencePeer[]>(
    () =>
      livePeers.map((peer) => ({
        ...peer,
        name:
          state.nodeUsers[peer.peer]?.name ??
          state.authorNames[peer.peer] ??
          `Peer ${peer.peer.slice(0, 6)}`,
      })),
    [livePeers, state.nodeUsers, state.authorNames],
  );
  const presenceByBlock = useMemo(() => {
    const byBlock = new Map<string, PagePresencePeer[]>();
    for (const peer of presence) {
      if (!peer.blockId) continue;
      const group = byBlock.get(peer.blockId) ?? [];
      group.push(peer);
      byBlock.set(peer.blockId, group);
    }
    return byBlock;
  }, [presence]);
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

  // the snapshot belongs to the page that was open when the block was deleted,
  // so the page id comes from the ref, never a stale closure — and `noteDeleted`
  // stays referentially stable, which the row handlers want.
  const noteDeleted = useCallback((ops: DuplicateOp[]) => {
    const page = pageRef.current;
    if (page) setDeleteUndo({ page, ops });
  }, []);

  // live threads keyed by target (block id or page id); rows need anchors as
  // well as counts so exact selections can be painted behind the textarea.
  const threadsByTarget = useMemo(() => {
    const map = new Map<string, ThreadView[]>();
    for (const group of state.pageThreads) map.set(group.target, group.threads);
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
  // is an action, not just an FYI — then dismisses itself. So does the failure
  // that a click on it can leave behind.
  useEffect(() => {
    if (deleteUndo === null) return;
    const timer = setTimeout(() => setDeleteUndo(null), 8000);
    return () => clearTimeout(timer);
  }, [deleteUndo]);

  useEffect(() => {
    if (!undoFailed) return;
    const timer = setTimeout(() => setUndoFailed(false), 6000);
    return () => clearTimeout(timer);
  }, [undoFailed]);

  // Replay the snapshot as plain inserts: the wire FIFO (actions.ts) keeps them
  // in plan order, so the subtree comes back exactly as it was. A checked to-do
  // needs a second op — InsertBlock carries no `checked` bit (only ever set on a
  // to-do: subtreePlan drops the module's stale bit, so this never fires a
  // NotTodo-rejected SetChecked on a converted block). Comments on the deleted
  // blocks were purged in consensus and cannot be restored.
  //
  // The ROOT op is the only one anchored to blocks this batch does not re-insert
  // itself: its parent and previous sibling belong to the live document and can
  // have been removed under us, and the module then rejects it (ParentNotFound /
  // AnchorNotFound) — and every child op chained behind it too, one error toast
  // each, while nothing comes back. So the batch is GATED on the root landing,
  // and the root goes out `quiet`: a failed restore says so ONCE, here.
  const undoDelete = async () => {
    const snapshot = deleteUndo;
    if (!snapshot || snapshot.ops.length === 0) return;
    setDeleteUndo(null);
    // Undo clicked after a doc switch restores into the page it came from, so
    // go back there first — a restore the user cannot see is not a restore.
    if (snapshot.page !== activePage) actions.openPage(snapshot.page);
    for (const [index, op] of snapshot.ops.entries()) {
      const insert = actions.insertPageBlock({
        blockId: op.blockId,
        parent: op.parent,
        after: op.after,
        kind: op.kind,
        text: op.text,
        ...(op.marks ? { marks: op.marks } : {}),
        quiet: index === 0,
      });
      // the rest of the plan only anchors onto blocks the root brought back, so
      // waiting on the root once is enough to know the restore has a floor.
      if (index === 0 && !(await insert)) {
        setUndoFailed(true);
        return;
      }
      if (op.checked) actions.setPageBlockChecked({ blockId: op.blockId, checked: true });
    }
  };

  const openBlockComments = useCallback((
    blockId: string,
    anchor: CommentAnchor,
    range?: RelativeAnchor,
  ) => {
    setCommentCard({ target: blockId, label: range ? "selected text" : "this block", anchor, range });
  }, []);

  // row intents -> store ops + caret placement. `handlers` is referentially
  // stable, which is what lets BlockRow's memo hold.
  const { handlers, appendBlock, focusRow } = useRowHandlers({
    actions,
    rows,
    blocks,
    pageThreads: state.pageThreads,
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
    onDeleted: noteDeleted,
  });

  const commentOnPage = (anchor: CommentAnchor) => {
    if (!activePage) return;
    setCommentCard({ target: activePage, label: "this page", anchor });
  };

  // a reply must carry the THREAD's target (a block id or the page id) — the
  // module rejects an append whose target differs from the thread's. Never
  // assume the page here.
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
  const pageCommentCount = root ? threadsByTarget.get(root.id)?.length ?? 0 : 0;

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
              presence={presence}
              onOpen={actions.openPage}
            />

            {pasteNotice !== null ? (
              <PageNotice>
                Pasted the first {MAX_PASTE_BLOCKS} blocks — {pasteNotice} more line
                {pasteNotice === 1 ? "" : "s"} were dropped. Each block is one write, so a
                paste is capped.
              </PageNotice>
            ) : null}

            {deleteUndo !== null ? (
              <PageNotice>
                <span>Block deleted.</span>
                {deleteUndo.ops.length > 0 ? (
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
              </PageNotice>
            ) : null}

            {undoFailed ? (
              <PageNotice role="alert" tone="danger">
                Couldn't restore the block — the place it sat in is gone. Someone else
                deleted or moved what it hung from.
              </PageNotice>
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
                    maxWidth: DOC_COLUMN_MAX,
                    margin: "0 auto",
                    padding: `44px ${COLUMN_PAD_X}px 100px`,
                    boxSizing: "border-box",
                  }}
                >
                  {chain.length > 1 ? (
                    <button
                      type="button"
                      aria-label={`Back to ${chain[chain.length - 2].title || "Untitled"}`}
                      onClick={() => actions.openPage(chain[chain.length - 2].id)}
                      style={{
                        all: "unset",
                        cursor: "pointer",
                        display: "inline-flex",
                        alignItems: "center",
                        gap: 7,
                        marginBottom: 20,
                        color: color.muted,
                        font: `500 13px ${font.sans}`,
                      }}
                    >
                      <span aria-hidden="true">‹</span>
                      {chain[chain.length - 2].title || "Untitled"}
                    </button>
                  ) : null}
                  <PageTitle
                    pageId={root.id}
                    raw={root.text}
                    titleRef={titleRef}
                    presence={presenceByBlock.get(root.id) ?? EMPTY_PRESENCE}
                    onCursor={onCursor}
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

                  <button
                    type="button"
                    aria-label="Comment on page"
                    onClick={(event) => {
                      const rect = event.currentTarget.getBoundingClientRect();
                      commentOnPage({ x: rect.right, y: rect.bottom });
                    }}
                    style={{
                      all: "unset",
                      cursor: "pointer",
                      width: "100%",
                      boxSizing: "border-box",
                      display: "flex",
                      alignItems: "center",
                      gap: 9,
                      padding: "0 0 12px",
                      marginBottom: 22,
                      borderBottom: `1px solid ${color.borderSoft}`,
                      color: color.muted2,
                      font: `400 13.5px ${font.sans}`,
                    }}
                  >
                    <span
                      aria-hidden="true"
                      style={{
                        width: 22,
                        height: 22,
                        borderRadius: "50%",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        background: color.panel,
                        color: color.muted3,
                      }}
                    >
                      <Icon name="chat" size={12} strokeWidth={1.8} />
                    </span>
                    <span>
                      {pageCommentCount > 0
                        ? `${pageCommentCount} page comment${pageCommentCount === 1 ? "" : "s"}`
                        : "Add a comment…"}
                    </span>
                    {pageCommentCount > 0 ? (
                      <span
                        style={{
                          marginLeft: "auto",
                          color: accentVar,
                          font: `650 11px ${font.mono}`,
                        }}
                      >
                        Open
                      </span>
                    ) : null}
                  </button>

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
                      threads={threadsByTarget.get(row.block.id) ?? EMPTY_THREADS}
                      presence={presenceByBlock.get(row.block.id) ?? EMPTY_PRESENCE}
                      onCursor={onCursor}
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
                    {rows.length === 0
                      ? "Start writing — or press '/' for the block menu"
                      : "Click to add a block, or type '/'"}
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

        </div>
      </main>
      {commentCard ? (
        <CommentCard
          target={commentCard.target}
          label={commentCard.label}
          anchor={commentCard.anchor}
          selection={commentCard.range}
          targetText={blocks.find((block) => block.id === commentCard.target)?.text ?? ""}
          threads={threadsForRange(
            state.pageThreads.find((g) => g.target === commentCard.target)?.threads ?? [],
            commentCard.range,
          )}
          authorNames={state.authorNames}
          selfKey={selfKey}
          selfName={selfName}
          ops={state.ops}
          onClose={() => setCommentCard(null)}
          onSubmitNew={(target, text, range) =>
            actions.addComment({ target, text, ...(range ? { anchor: range } : {}) })
          }
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
            noteDeleted(subtreePlan(blocks, pendingBlockDelete));
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
