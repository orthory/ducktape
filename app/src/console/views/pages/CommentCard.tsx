// The floating comment card: Notion-style popover anchored at the affordance
// that opened it, scoped to ONE target (a block id or the page id). A target
// with no threads opens straight into the composer; otherwise threads render with
// the composer behind an "Add comment" affordance. Dismissed by Escape, an
// outside press, or any outside scroll (the anchor coordinates go stale the
// moment the document moves).

import { useEffect, useRef, useState } from "react";

import type { AuthorNames } from "../../../domain/chat-client";
import type { RelativeAnchor, ThreadView } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import type { OpLedger } from "../../store/finalization";
import { inMentionMenu } from "../chat/use-mention-menu";
import { color, font, radius } from "../../theme/tokens";
import { DiscussionParticipants, NewThreadComposer, ThreadCard } from "./CommentThread";

/** Where the card anchors, in viewport coordinates (the opener's rect). */
export interface CommentAnchor {
  x: number;
  y: number;
}

const CARD_WIDTH = 360;

export function CommentCard({
  target,
  label,
  anchor,
  selection,
  targetText = "",
  threads,
  authorNames,
  selfKey,
  selfName,
  ops,
  onClose,
  onSubmitNew,
  onReply,
  onResolve,
  onEdit,
  onDelete,
}: {
  target: string;
  /** "this page" | "this block" — names the card and its composer. */
  label: string;
  anchor: CommentAnchor;
  /** Exact range for the new thread; absent for block/page-level comments. */
  selection?: RelativeAnchor;
  /** Current target text, used to show each thread's live selected quote. */
  targetText?: string;
  /** Threads for THIS target only. */
  threads: ThreadView[];
  authorNames: AuthorNames;
  /** The local author's key — Edit/Delete render only on our own comments. */
  selfKey: string;
  selfName: string;
  /** Finalization ledger, handed through to each thread's marks. */
  ops?: OpLedger;
  onClose: () => void;
  onSubmitNew: (target: string, text: string, selection?: RelativeAnchor) => void;
  onReply: (threadId: string, text: string) => void;
  onResolve: (threadId: string, resolved: boolean) => void;
  onEdit: (commentId: string, text: string) => void;
  onDelete: (commentId: string) => void;
}) {
  const cardRef = useRef<HTMLDivElement | null>(null);
  const [composing, setComposing] = useState(false);
  const empty = threads.length === 0;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      // an IME's Escape cancels the COMPOSITION, not the card — closing here
      // would throw away the draft mid-composition (isComposing, or the 229
      // keyCode some engines report during IME handling).
      if (event.isComposing || event.keyCode === 229) return;
      if (event.key === "Escape") onClose();
    };
    const onPress = (event: MouseEvent) => {
      const card = cardRef.current;
      if (inMentionMenu(event.target)) return; // portaled, but ours
      if (card && event.target instanceof Node && !card.contains(event.target)) {
        onClose();
      }
    };
    // capture: scroll events don't bubble. The card's own list scrolling must
    // not dismiss it — only movement outside (the doc canvas, the rail).
    const onScroll = (event: Event) => {
      const card = cardRef.current;
      if (card && event.target instanceof Node && card.contains(event.target)) return;
      if (inMentionMenu(event.target)) return; // scrolling the typeahead list
      onClose();
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onPress);
    document.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onPress);
      document.removeEventListener("scroll", onScroll, true);
    };
  }, [onClose]);

  const left = Math.max(
    8,
    Math.min(
      selection ? anchor.x : anchor.x - CARD_WIDTH,
      window.innerWidth - CARD_WIDTH - 8,
    ),
  );
  const top = Math.max(8, Math.min(anchor.y + 8, window.innerHeight - 120));

  return (
    <div
      ref={cardRef}
      role="dialog"
      aria-label={`Comments on ${label}`}
      style={{
        position: "fixed",
        zIndex: 40,
        left,
        top,
        width: CARD_WIDTH,
        maxHeight: "min(560px, 76vh)",
        display: "flex",
        flexDirection: "column",
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        boxShadow: "0 24px 56px rgba(0,0,0,.55)",
        overflow: "hidden",
      }}
    >
      <div
        style={{
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 8,
          minHeight: 48,
          padding: "8px 14px 8px 16px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <div style={{ font: `650 13px ${font.sans}`, color: color.ink }}>
          Comments on {label}
        </div>
        <div style={{ marginLeft: "auto" }}>
          <DiscussionParticipants
            threads={threads}
            authorNames={authorNames}
            selfKey={selfKey}
            selfName={selfName}
          />
        </div>
        <button
          type="button"
          aria-label="Close comments card"
          onClick={onClose}
          style={{
            all: "unset",
            cursor: "pointer",
            width: 28,
            height: 28,
            borderRadius: 6,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: color.muted3,
          }}
        >
          <Icon name="close" size={14} />
        </button>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {threads.map((view) => (
          <ThreadCard
            key={view.thread.id}
            view={view}
            anchorText={
              view.thread.anchor
                ? targetText.slice(view.thread.anchor.start, view.thread.anchor.end)
                : undefined
            }
            authorNames={authorNames}
            selfKey={selfKey}
            selfName={selfName}
            ops={ops}
            onReply={onReply}
            onResolve={onResolve}
            onEdit={onEdit}
            onDelete={onDelete}
          />
        ))}
        {empty || composing ? (
          <NewThreadComposer
            key={target}
            composer={{ target, label }}
            onSubmit={(t, text) => {
              if (selection) onSubmitNew(t, text, selection);
              else onSubmitNew(t, text);
              setComposing(false);
            }}
            // an empty target has nothing else to show — cancel closes the
            // card; with threads on screen it only puts the composer away.
            onCancel={empty ? onClose : () => setComposing(false)}
          />
        ) : (
          <button
            type="button"
            aria-label="Add comment thread"
            onClick={() => setComposing(true)}
            style={{
              all: "unset",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 6,
              margin: 0,
              padding: "12px 16px",
              borderTop: `1px solid ${color.borderSoft}`,
              color: color.muted3,
              font: `600 12px ${font.sans}`,
            }}
          >
            <Icon name="plus" size={12} strokeWidth={1.9} /> Add comment
          </button>
        )}
      </div>
    </div>
  );
}
