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

import type { PageBlock } from "../../../domain/pages-client";
import type { ConsoleActions } from "../../store/actions";
import { caretOffset, mergeText } from "./block-keys";
import type { Caret, FocusIntent } from "./block-keys";
import type { RowHandlers } from "./BlockRow";
import {
  continuationKind,
  indentTarget,
  moveDownTarget,
  moveUpTarget,
  outdentTarget,
} from "./pages-model";
import type { Row } from "./pages-model";

export interface RowHandlersDeps {
  actions: ConsoleActions;
  rows: Row[];
  blocks: PageBlock[];
  /** The page root — its children are the top-level blocks. */
  root: PageBlock | null;
  activePage: string | null;
  inputs: MutableRefObject<Map<string, HTMLTextAreaElement>>;
  titleRef: MutableRefObject<HTMLInputElement | null>;
  setFocus: Dispatch<SetStateAction<FocusIntent | null>>;
  setCollapsed: Dispatch<SetStateAction<ReadonlySet<string>>>;
  openComments: (blockId: string, anchor: { x: number; y: number }) => void;
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
  root,
  activePage,
  inputs,
  titleRef,
  setFocus,
  setCollapsed,
  openComments,
}: RowHandlersDeps): RowHandlersApi {
  const live = useRef({ rows, blocks, root, actions, activePage: activePage });
  live.current = { rows, blocks, root, actions, activePage: activePage };

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

  const handlers: RowHandlers = useMemo(
    () => ({
      commitText: (blockId, text) => {
      live.current.actions.updatePageBlockText({ blockId, text });
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
      const blockId = crypto.randomUUID();
      const inserted = live.current.actions.insertPageBlock({
        blockId,
        parent: cur.parent,
        after: cur.id,
        kind: continuationKind(cur.kind),
        text: right,
      });
      live.current.actions.updatePageBlockText({ blockId: cur.id, text: left });
      void inserted.then((ok) => {
        // only an explicit false is a known failure; never compensate on a
        // merely absent answer.
        if (ok === false) live.current.actions.updatePageBlockText({ blockId: cur.id, text: left + right });
      });
      setFocus({ id: blockId, caret: "start" });
      },
      // Backspace at offset 0: this block's text joins the one above and this
      // one goes away. Same hazard, mirrored — here the *update* is the additive
      // op, so a failed update is compensated by putting this block back.
      mergePrev: (row, text) => {
      const index = live.current.rows.findIndex((r) => r.block.id === row.block.id);
      const prev = editableAbove(index);
      if (!prev) return;
      const cur = row.block;
      if (!cur.parent) return;
      const { text: joined, seam } = mergeText(liveText(prev.block), text);
      const merged = live.current.actions.updatePageBlockText({ blockId: prev.block.id, text: joined });
      live.current.actions.removePageBlock(cur.id);
      void merged.then((ok) => {
        if (ok !== false) return;
        live.current.actions.insertPageBlock({
          blockId: cur.id,
          parent: cur.parent as string,
          after: prev.block.id,
          kind: cur.kind,
          text,
        });
      });
      setFocus({ id: prev.block.id, caret: seam });
      },
      // a divider owns no textarea, so it can never be focused and deleted on
      // its own. Backspace from the block below is its only keyboard exit.
      removeDividerAbove: (row) => {
      const index = live.current.rows.findIndex((r) => r.block.id === row.block.id);
      const above = live.current.rows[index - 1];
      if (above?.block.kind === "divider") live.current.actions.removePageBlock(above.block.id);
      },
      removeEmpty: (row) => {
      const index = live.current.rows.findIndex((r) => r.block.id === row.block.id);
      live.current.actions.removePageBlock(row.block.id);
      focusRow(editableAbove(index), "end");
      },
      indent: (row) => move(row.block.id, indentTarget(live.current.blocks, row.block.id)),
      outdent: (row) => move(row.block.id, outdentTarget(live.current.blocks, row.block.id)),
      moveUp: (row) => move(row.block.id, moveUpTarget(live.current.blocks, row.block.id)),
      moveDown: (row) => move(row.block.id, moveDownTarget(live.current.blocks, row.block.id)),
      setKind: (blockId, kind) => {
      live.current.actions.setPageBlockKind({ blockId, kind });
      setFocus({ id: blockId, caret: "end" });
      },
      setChecked: (blockId, checked) =>
      live.current.actions.setPageBlockChecked({ blockId, checked }),
      remove: (blockId) => live.current.actions.removePageBlock(blockId),
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
      registerInput: (blockId, el) => {
      if (el) inputs.current.set(blockId, el);
      else inputs.current.delete(blockId);
      },
      openComments: openComments,
      // the action creates the untitled child and opens its tab (cursor lands
      // in the title via the fresh-page focus effect above).
      createSubpage: () => live.current.actions.createChildPage(live.current.activePage),
    }),
    [editableAbove, focusRow, liveText, move, openComments],
  );

  return { handlers, appendBlock, focusRow };
}
