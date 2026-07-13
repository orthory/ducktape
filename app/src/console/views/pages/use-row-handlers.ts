// Row intents -> store ops + caret placement.
//
// BlockRow decides what a keystroke MEANS (block-keys.ts); this hook decides
// what the editor DOES about it. It lives apart from PagesView because the two
// answer different questions, and because PagesView was already at the repo's
// file-size cap.
//
// Every handler reads live state through a ref rather than closing over it, so
// the returned `handlers` object is referentially STABLE across store patches.
// That is load-bearing: a fresh `handlers` prop defeats BlockRow's memo on its
// own, and every patch would then re-render all N rows. pages-render-cost.test
// guards it.

import { useCallback, useMemo, useRef } from "react";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";

import type { PageBlock, RelativeAnchor, TargetThreads } from "../../../domain/pages-client";
import { mergeMarks, rebaseMarks, rebaseRange, splitMarks } from "../../../domain/pages-ranges";
import type { ConsoleActions } from "../../store/actions";
import { caretOffset, mergeText } from "./block-keys";
import type { Caret, FocusIntent } from "./block-keys";
import type { RowHandlers } from "./BlockRow";
import { dropTarget } from "./page-drag";
import type { DropEdge } from "./page-drag";
import { pastePlan } from "./page-paste";
import {
  continuationKind,
  duplicatePlan,
  indentTarget,
  moveDownTarget,
  moveUpTarget,
  outdentTarget,
  subtreePlan,
} from "./pages-model";
import type { DuplicateOp, Row } from "./pages-model";

/** A drag in flight: what is moving, and where it would land. */
export interface DragState {
  id: string;
  over: string | null;
  edge: DropEdge;
}

export interface RowHandlersDeps {
  actions: ConsoleActions;
  rows: Row[];
  blocks: PageBlock[];
  pageThreads: TargetThreads[];
  /** The page root — its children are the top-level blocks. */
  root: PageBlock | null;
  activePage: string | null;
  drag: DragState | null;
  inputs: MutableRefObject<Map<string, HTMLTextAreaElement>>;
  titleRef: MutableRefObject<HTMLInputElement | null>;
  setFocus: Dispatch<SetStateAction<FocusIntent | null>>;
  setCollapsed: Dispatch<SetStateAction<ReadonlySet<string>>>;
  setDrag: Dispatch<SetStateAction<DragState | null>>;
  openComments: (
    blockId: string,
    anchor: { x: number; y: number },
    range?: RelativeAnchor,
  ) => void;
  /** Deleting a block WITH CHILDREN takes its whole subtree with it, so it asks
   *  first. The view owns the dialog. */
  confirmRemove: (blockId: string) => void;
  /** A paste past the block cap dropped lines; the view says so. */
  onPasteCapped: (dropped: number) => void;
  /** A block was deleted; hand the view the pre-delete subtree snapshot so it
   *  can offer an Undo. Empty means the subtree was too big to restore. */
  onDeleted: (plan: DuplicateOp[]) => void;
}

export interface RowHandlersApi {
  handlers: RowHandlers;
  /** Append a paragraph to the end of the page and put the caret in it. */
  appendBlock: () => void;
  /** Focus a row at `caret`; `undefined` focuses the page title. */
  focusRow: (row: Row | undefined, caret?: Caret) => void;
}

export function useRowHandlers({
  actions,
  rows,
  blocks,
  pageThreads,
  root,
  activePage,
  drag,
  inputs,
  titleRef,
  setFocus,
  setCollapsed,
  setDrag,
  openComments,
  confirmRemove,
  onPasteCapped,
  onDeleted,
}: RowHandlersDeps): RowHandlersApi {
  const live = useRef({ rows, blocks, pageThreads, root, actions, activePage, drag, confirmRemove, onPasteCapped, onDeleted });
  live.current = { rows, blocks, pageThreads, root, actions, activePage, drag, confirmRemove, onPasteCapped, onDeleted };

  const appendBlock = useCallback(() => {
    const { root, actions } = live.current;
    if (!root) return;
    const blockId = crypto.randomUUID();
    actions.insertPageBlock({
      blockId,
      parent: root.id,
      after: root.children[root.children.length - 1] ?? null,
      kind: "paragraph",
      text: "",
    });
    setFocus({ id: blockId, caret: "start" });
  }, []);

  const move = useCallback(
    (blockId: string, target: { parent: string; after: string | null } | null) => {
      if (!target) return;
      live.current.actions.movePageBlock({ blockId, ...target });
      setFocus({ id: blockId, caret: "end" });
    },
    [],
  );

  const focusRow = useCallback((row: Row | undefined, caret: Caret = "end") => {
    if (!row) {
      titleRef.current?.focus();
      return;
    }
    const el = inputs.current.get(row.block.id);
    if (el) {
      const at = caretOffset(caret, el.value.length);
      el.focus();
      el.setSelectionRange(at, at);
    }
  }, []);

  /** The live text of a block: its textarea's uncommitted draft if it has one,
   *  otherwise the committed snapshot. */
  const liveText = useCallback(
    (block: PageBlock): string => inputs.current.get(block.id)?.value ?? block.text,
    [],
  );

  /** The nearest row above `index` that owns a textarea. Dividers own none. */
  const editableAbove = useCallback((index: number): Row | undefined => {
    const { rows } = live.current;
    for (let i = index - 1; i >= 0; i -= 1) {
      if (inputs.current.has(rows[i].block.id)) return rows[i];
    }
    return undefined;
  }, []);

  /** Delete a block — THE path every "remove this" intent routes through.
   *
   *  RemoveBlock takes the block's ENTIRE SUBTREE with it (block_ops.rs runs
   *  delete_subtree) and there is no undo, so a block with children asks first.
   *  The guard lives here, in the one function all three intents call, not in
   *  whichever caller someone remembered: `removeDividerAbove` was written
   *  without it and could destroy a divider, the block holding the caret, and
   *  everything under that — a Backspace, no dialog, no undo. The menu's Delete
   *  and Backspace-on-empty had the guard; the third path did not, and the suite
   *  missed it because it only ever tested a CHILDLESS divider.
   *
   *  Returns false when it deferred to the dialog instead of removing. The merge
   *  does NOT come through here — it hands the children to the adopter first, so
   *  by the time it removes there is no subtree left to warn about. */
  const removeBlock = useCallback((blockId: string, undo = true): boolean => {
    const block = live.current.blocks.find((b) => b.id === blockId);
    if (block && block.children.length > 0) {
      // Deferred to the dialog — PagesView snapshots on confirm, since the
      // remove happens there, not here.
      live.current.confirmRemove(blockId);
      return false;
    }
    // Snapshot the (single-block) subtree BEFORE it's gone so the view can offer
    // an Undo. The backspace remove-empty typing flow opts out (undo=false).
    if (undo) live.current.onDeleted(subtreePlan(live.current.blocks, blockId));
    void live.current.actions.removePageBlock(blockId);
    return true;
  }, []);

  const handlers: RowHandlers = useMemo(
    () => ({
      commitText: (blockId, text, marks) => {
        live.current.actions.updatePageBlockText({ blockId, text, marks });
      },
      // Enter at the caret: this block keeps the left half, a new sibling takes
      // the right half. Two ops, and a failed op is never rolled back — it is
      // erased by the next authoritative refresh. So if the truncation commits
      // and the insert does not, the right half is gone for good. Both are
      // submitted now (the optimistic projections land in the same tick, so the
      // caret never waits on consensus) and a failed insert is compensated by
      // restoring the whole text. Worst case is a visible duplicate, never a
      // silent loss.
      split: (row, left, right) => {
        const cur = row.block;
        if (!cur.parent) return;
        const marks = splitMarks(
          rebaseMarks(cur.text, left + right, cur.marks),
          left.length,
        );
        const blockId = crypto.randomUUID();
        const inserted = live.current.actions.insertPageBlock({
          blockId,
          parent: cur.parent,
          after: cur.id,
          kind: continuationKind(cur.kind),
          text: right,
          ...(marks.right.length ? { marks: marks.right } : {}),
        });
        const update = () => live.current.actions.updatePageBlockText({
          blockId: cur.id,
          text: left,
          ...(cur.marks?.length ? { marks: marks.left } : {}),
        });
        const anchoredThreads = (live.current.pageThreads
          .find((group) => group.target === cur.id)?.threads ?? [])
          .flatMap(({ thread }) => {
            if (!thread.anchor) return [];
            return [{
              threadId: thread.id,
              anchor: rebaseRange(cur.text, left + right, thread.anchor),
            }];
          });
        const movedThreads = anchoredThreads.flatMap(({ threadId, anchor }) =>
          anchor.start >= left.length
            ? [{
                threadId,
                anchor: { start: anchor.start - left.length, end: anchor.end - left.length },
              }]
            : []);
        if (anchoredThreads.some(({ anchor }) => anchor.end > left.length)) {
          const migrated = movedThreads.reduce<Promise<boolean>>(
            (chain, move) => chain.then((ok) =>
              ok
                ? live.current.actions.moveCommentThread({
                    ...move,
                    target: blockId,
                  })
                : false),
            inserted,
          );
          void migrated.then((ok) => {
            // If either the new block or a thread move failed, keep the whole
            // original text. A duplicate right block is visible; lost
            // commented text is not recoverable.
            if (ok) void update();
          });
          setFocus({ id: blockId, caret: "start" });
          return;
        }
        void update();
        void inserted.then((ok) => {
          // only an explicit false is a known failure; never compensate on a
          // merely absent answer.
          if (ok === false) live.current.actions.updatePageBlockText({
            blockId: cur.id,
            text: left + right,
            ...(cur.marks?.length
              ? { marks: mergeMarks(marks.left, marks.right, left.length) }
              : {}),
          });
        });
        setFocus({ id: blockId, caret: "start" });
      },
      // Backspace at offset 0: this block's text joins the one above and this
      // one goes away. Same hazard as the split, mirrored — here the *update* is
      // the additive op, so a failed update is compensated by putting this block
      // back.
      //
      // RemoveBlock takes the whole subtree with it, so a block with children
      // must hand them over BEFORE it goes: they are re-parented onto the block
      // that just absorbed its text, which is where they belong.
      //
      // Handing them over is not enough on its own, and this is where the merge
      // was still losing work. The ops are ANCHOR-CHAINED (child 2 lands `after`
      // child 1, and the remove must follow both), but the transport orders
      // nothing: /v1/submit is one independent POST per op and the node drains
      // them as they ARRIVE. A merge of a parent with two children really did
      // lose the second one. Two things fix it, and both are needed:
      //
      //   1. actions.ts sends page-block ops through a wire FIFO, so they reach
      //      the node in issue order and every anchor exists when it is needed.
      //   2. the remove WAITS for the merge and every adoption to actually land.
      //      Order alone does not save a child whose move was REJECTED: it would
      //      still be sitting under `cur` when RemoveBlock ran, and go with it.
      //
      // Hence the two branches below. A childless merge — the common one, and the
      // one under the caret — keeps its instant remove, because there is no
      // subtree to lose and a failed merge can simply put the block back. A merge
      // that inherits children issues NOTHING destructive until the text has a
      // home and the children have a new parent.
      mergePrev: (row, text) => {
      const index = live.current.rows.findIndex((r) => r.block.id === row.block.id);
      const prev = editableAbove(index);
      if (!prev) return;
      const cur = live.current.blocks.find((b) => b.id === row.block.id) ?? row.block;
      const parent = cur.parent;
      if (!parent) return;
      const previousText = liveText(prev.block);
      const { text: joined, seam } = mergeText(previousText, text);
      const joinedMarks = mergeMarks(
        rebaseMarks(prev.block.text, previousText, prev.block.marks),
        rebaseMarks(cur.text, text, cur.marks),
        seam,
      );
      const threadMoves = (live.current.pageThreads
        .find((group) => group.target === cur.id)?.threads ?? [])
        .map(({ thread }) => {
          if (!thread.anchor) return { threadId: thread.id, anchor: null };
          const anchor = rebaseRange(cur.text, text, thread.anchor);
          return {
            threadId: thread.id,
            anchor: anchor.start < anchor.end
              ? { start: anchor.start + seam, end: anchor.end + seam }
              : null,
          };
        });
      const merged = live.current.actions.updatePageBlockText({
        blockId: prev.block.id,
        text: joined,
        ...((prev.block.marks?.length || cur.marks?.length) ? { marks: joinedMarks } : {}),
      });
      const commentsMoved = threadMoves.reduce<Promise<boolean>>(
        (chain, move) => chain.then((ok) =>
          ok
            ? live.current.actions.moveCommentThread({
                ...move,
                target: prev.block.id,
              })
            : false),
        merged,
      );
      setFocus({ id: prev.block.id, caret: seam });

      if (cur.children.length === 0) {
        if (threadMoves.length > 0) {
          void commentsMoved.then((ok) => {
            // The source block owns the only fallback copy until every thread
            // has followed its text into the adopter.
            if (ok) void live.current.actions.removePageBlock(cur.id);
          });
          return;
        }
        // Nothing to adopt: the row must vanish under the caret NOW. A failed
        // merge puts the block back — the wire FIFO keeps that re-insert behind
        // this remove, so the id is free by the time it lands. Worst case is a
        // visible duplicate, never a silent loss.
        void live.current.actions.removePageBlock(cur.id);
        void merged.then((ok) => {
          if (ok === false) {
            void live.current.actions.insertPageBlock({
              blockId: cur.id,
              parent,
              after: prev.block.id,
              kind: cur.kind,
              text,
              ...(cur.marks?.length
                ? { marks: rebaseMarks(cur.text, text, cur.marks) }
                : {}),
            });
          }
        });
        return;
      }

      // `cur` holds children. Adopt them onto the block that took its text, in
      // document order, each chained on the last — then, and only then, remove
      // `cur`. Seeded on `merged`: if the text never reached the adopter, not one
      // destructive op is issued and `cur` simply stays put, text and children
      // and all. There is nothing to compensate, and nothing to lose.
      //
      // It costs the merged row one commit on screen. A duplicate row is
      // recoverable; a deleted subtree is not.
      const adopter = live.current.blocks.find((b) => b.id === prev.block.id);
      let after = adopter?.children[adopter.children.length - 1] ?? null;
      const adopted = cur.children.reduce<Promise<boolean>>((chain, child) => {
        const anchor = after;
        after = child;
        return chain.then((ok) =>
          ok
            ? live.current.actions.movePageBlock({
                blockId: child,
                parent: prev.block.id,
                after: anchor,
              })
            : false,
        );
      }, commentsMoved);

      void adopted.then((ok) => {
        // a child did not make it across — it is still under `cur`, and
        // RemoveBlock would take it. Keep `cur`.
        if (ok) void live.current.actions.removePageBlock(cur.id);
      });
      },
      // a divider owns no textarea, so it can never be focused and deleted on
      // its own. Backspace from the block below is its only keyboard exit — and
      // it goes through `removeBlock`, because a divider CAN hold children (a
      // pre-existing document may carry one that Tab adopted under it before
      // indentTarget refused to). Deleting it bare took the caret's own block
      // with it.
      removeDividerAbove: (row) => {
      const index = live.current.rows.findIndex((r) => r.block.id === row.block.id);
      const above = live.current.rows[index - 1];
      if (above?.block.kind === "divider") removeBlock(above.block.id);
      },
      // Backspace on an EMPTY block. This is the same destructive act as the
      // menu's Delete — RemoveBlock takes the subtree — so it takes the same
      // guard. An empty toggle holding a page of notes must not vanish on one
      // keystroke.
      removeEmpty: (row) => {
      const index = live.current.rows.findIndex((r) => r.block.id === row.block.id);
      // No undo toast for the fluid backspace-on-empty typing motion.
      if (removeBlock(row.block.id, false)) focusRow(editableAbove(index), "end");
      },
      indent: (row) => move(row.block.id, indentTarget(live.current.blocks, row.block.id)),
      outdent: (row) => move(row.block.id, outdentTarget(live.current.blocks, row.block.id)),
      moveUp: (row) => move(row.block.id, moveUpTarget(live.current.blocks, row.block.id)),
      moveDown: (row) => move(row.block.id, moveDownTarget(live.current.blocks, row.block.id)),
      setKind: (blockId, kind) => {
      live.current.actions.setPageBlockKind({ blockId, kind });
      setFocus({ id: blockId, caret: "end" });
      },
      setMark: (blockId, range, kind, active) =>
        live.current.actions.setPageBlockSpanMark({ blockId, ...range, kind, active }),
      setChecked: (blockId, checked) =>
      live.current.actions.setPageBlockChecked({ blockId, checked }),
      // the menu's Delete — the same guarded path as Backspace-on-empty and
      // Backspace-onto-a-divider.
      remove: (blockId) => {
      removeBlock(blockId);
      },
      insertBelow: (row) => {
      const cur = row.block;
      if (!cur.parent) return;
      const blockId = crypto.randomUUID();
      live.current.actions.insertPageBlock({
        blockId,
        parent: cur.parent,
        after: cur.id,
        kind: "paragraph",
        text: "",
      });
      setFocus({ id: blockId, caret: "start" });
      },
      // The plan is a PREORDER of inserts: a copied child names its freshly
      // minted parent, and a copied sibling anchors `after` the copy before it.
      // Neither exists on the node until its own op lands, so these must arrive
      // in plan order — the wire FIFO (actions.ts) is what guarantees that.
      // Submitted out of order they are rejected (ParentNotFound / AnchorNotFound)
      // and the duplicate comes back with holes in it.
      duplicate: (row) => {
      const ops = duplicatePlan(live.current.blocks, row.block.id, () => crypto.randomUUID());
      for (const op of ops) {
        live.current.actions.insertPageBlock({
          blockId: op.blockId,
          parent: op.parent,
          after: op.after,
          kind: op.kind,
          text: op.text,
          ...(op.marks ? { marks: op.marks } : {}),
        });
        // InsertBlock carries no `checked` bit, so a checked to-do needs a
        // second op to come back checked.
        if (op.checked) live.current.actions.setPageBlockChecked({ blockId: op.blockId, checked: true });
      }
      if (ops[0]) setFocus({ id: ops[0].blockId, caret: "end" });
      },
      // A multi-line paste becomes BLOCKS: this row keeps the first line, the
      // rest are inserted below it in order, each converted by the same
      // markdown grammar typing uses. One op per block, and the plan is capped
      // — a 500-line paste is a 500-submit burst the ledger cannot even hold.
      //
      // Each insert anchors `after` the one before it, so — like duplicate —
      // the paste only survives because the wire FIFO (actions.ts) delivers them
      // in issue order. Fired in parallel, a line whose anchor had not landed yet
      // was simply rejected, and dropped out of the pasted document.
      pasteBlocks: (row, before, pasted, after) => {
      const cur = row.block;
      const plan = pastePlan(pasted);
      if (plan.blocks.length === 0 || !cur.parent) return before + after;
      const [first, ...rest] = plan.blocks;
      const head = before + first.text + (rest.length === 0 ? after : "");

      // Existing rich text on either side of the replaced selection stays
      // with that text. The pasted document is plain; when it creates several
      // blocks, the original tail (and its marks) moves to the final block.
      const source = liveText(cur);
      const sourceMarks = rebaseMarks(cur.text, source, cur.marks);
      const aroundSelection = splitMarks(sourceMarks, before.length);
      const tailMarks = splitMarks(
        aroundSelection.right,
        Math.max(0, source.length - before.length - after.length),
      ).right;
      const headMarks = rest.length === 0
        ? mergeMarks(aroundSelection.left, tailMarks, before.length + first.text.length)
        : aroundSelection.left;
      const tailStart = source.length - after.length;
      const tailThreads = rest.length === 0
        ? []
        : (live.current.pageThreads
            .find((group) => group.target === cur.id)?.threads ?? [])
            .flatMap(({ thread }) => {
              if (!thread.anchor) return [];
              const anchor = rebaseRange(cur.text, source, thread.anchor);
              return anchor.start >= tailStart && anchor.start < anchor.end
                ? [{
                    threadId: thread.id,
                    anchor: {
                      start: anchor.start - tailStart,
                      end: anchor.end - tailStart,
                    },
                  }]
                : [];
            });

      const updateHead = () => live.current.actions.updatePageBlockText({
        blockId: cur.id,
        text: head,
        ...(cur.marks?.length ? { marks: headMarks } : {}),
      });
      if (tailThreads.length === 0) void updateHead();
      // an EMPTY paragraph adopts the first line's kind — pasting a document
      // into a fresh block must not leave "# Title" sitting as a paragraph.
      if (cur.kind === "paragraph" && before === "" && first.kind !== "paragraph") {
        live.current.actions.setPageBlockKind({ blockId: cur.id, kind: first.kind });
      }

      let afterId = cur.id;
      let finalBlockId: string | null = null;
      let finalTextLength = 0;
      let inserted: Promise<boolean> = Promise.resolve(true);
      for (const [i, block] of rest.entries()) {
        const blockId = crypto.randomUUID();
        const last = i === rest.length - 1;
        const landed = live.current.actions.insertPageBlock({
          blockId,
          parent: cur.parent as string,
          after: afterId,
          kind: block.kind,
          // the text the caret was sitting in front of rides on the last
          // pasted block, exactly where a plain paste would have left it.
          text: last ? block.text + after : block.text,
          ...(last && tailMarks.length
            ? { marks: mergeMarks([], tailMarks, block.text.length) }
            : {}),
        });
        inserted = inserted.then((ok) => (ok ? landed : false));
        // the caret lands at the END of the pasted content — before the tail.
        if (last) {
          finalBlockId = blockId;
          finalTextLength = block.text.length;
          setFocus({ id: blockId, caret: block.text.length });
        }
        afterId = blockId;
      }
      if (finalBlockId && tailThreads.length > 0) {
        const targetBlockId = finalBlockId;
        const migrated = tailThreads.reduce<Promise<boolean>>(
          (chain, move) => chain.then((ok) =>
            ok
              ? live.current.actions.moveCommentThread({
                  threadId: move.threadId,
                  target: targetBlockId,
                  anchor: {
                    start: move.anchor.start + finalTextLength,
                    end: move.anchor.end + finalTextLength,
                  },
                })
              : false),
          inserted,
        );
        void migrated.then((ok) => {
          if (ok) void updateHead();
        });
      }

      if (plan.dropped > 0) live.current.onPasteCapped(plan.dropped);
      return head;
      },
      dragStart: (blockId) => setDrag({ id: blockId, over: null, edge: "after" }),
      dragOver: (blockId, edge) =>
      setDrag((prev) =>
        // a dragover fires ~60×/s: only re-render when the landing actually moved.
        !prev || (prev.over === blockId && prev.edge === edge)
          ? prev
          : { ...prev, over: blockId, edge },
      ),
      // ONE MoveBlock per drop — the wire op reparents and reorders, so the
      // whole gesture is a single consensus write, never one per dragover.
      drop: (blockId, edge) => {
      const { drag, blocks, actions } = live.current;
      setDrag(null);
      if (!drag) return;
      const target = dropTarget(blocks, drag.id, blockId, edge);
      if (!target) return;
      actions.movePageBlock({ blockId: drag.id, ...target });
      setFocus({ id: drag.id, caret: "end" });
      },
      dragEnd: () => setDrag(null),
      toggleCollapse: (blockId) =>
      setCollapsed((prev) => {
        const next = new Set(prev);
        if (next.has(blockId)) next.delete(blockId);
        else next.add(blockId);
        return next;
      }),
      focusRelative: (row, delta, caret) => {
      const index = live.current.rows.findIndex((r) => r.block.id === row.block.id);
      for (let i = index + delta; i >= 0 && i < live.current.rows.length; i += delta) {
        if (inputs.current.has(live.current.rows[i].block.id)) {
          focusRow(live.current.rows[i], caret);
          return;
        }
      }
      // walking off the top lands in the title; off the bottom, nowhere.
      if (delta === -1) focusRow(undefined);
      },
      // the row owns its textarea's value, so it owns placing the caret in it.
      // It calls this once the caret has actually landed.
      focusApplied: (blockId) => setFocus((f) => (f?.id === blockId ? null : f)),
      registerInput: (blockId, el) => {
      if (el) inputs.current.set(blockId, el);
      else inputs.current.delete(blockId);
      },
      openComments: openComments,
      // the action creates the untitled child and opens its tab (cursor lands
      // in the title via the fresh-page focus effect above).
      createSubpage: () => live.current.actions.createChildPage(live.current.activePage),
    }),
    [editableAbove, focusRow, liveText, move, openComments, removeBlock, setCollapsed, setDrag, setFocus],
  );

  return { handlers, appendBlock, focusRow };
}
