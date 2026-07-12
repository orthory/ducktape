// The floating comment card: Notion-style popover anchored at the affordance
// that opened it, scoped to ONE target (a block id or the page id) — unlike
// the CommentsPanel, which lists every thread on the page. A target with no
// threads opens straight into the composer; otherwise the threads render with
// the composer behind an "Add comment" affordance. Dismissed by Escape, an
// outside press, or any outside scroll (the anchor coordinates go stale the
// moment the document moves).

import { useEffect, useRef, useState } from "react";

import type { AuthorNames } from "../../../domain/chat-client";
import type { ThreadView } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import type { OpLedger } from "../../store/finalization";
import { inMentionMenu } from "../chat/use-mention-menu";
import { color, font, radius, shadow } from "../../theme/tokens";
import { NewThreadComposer, ThreadCard } from "./CommentThread";

/** Where the card anchors, in viewport coordinates (the opener's rect). */
export interface CommentAnchor {
  x: number;
  y: number;
}

const CARD_WIDTH = 340;

export function CommentCard({
  target,
  label,
  anchor,
  threads,
  authorNames,
  selfKey,
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
  /** Threads for THIS target only. */
  threads: ThreadView[];
  authorNames: AuthorNames;
  /** The local author's key — Edit/Delete render only on our own comments. */
  selfKey: string;
  /** Finalization ledger, handed through to each thread's marks. */
  ops?: OpLedger;
  onClose: () => void;
  onSubmitNew: (target: string, text: string) => void;
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

  const left = Math.max(8, Math.min(anchor.x - CARD_WIDTH, window.innerWidth - CARD_WIDTH - 8));
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
        maxHeight: "min(480px, 70vh)",
        display: "flex",
        flexDirection: "column",
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        boxShadow: shadow.card,
        overflow: "hidden",
      }}
    >
      <div
        style={{
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "9px 14px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <div style={{ font: `600 12px ${font.sans}`, color: color.ink }}>
          Comments on {label}
        </div>
        <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
          {threads.length || ""}
        </div>
        <button
          type="button"
          aria-label="Close comments card"
          onClick={onClose}
          style={{
            all: "unset",
            cursor: "pointer",
            width: 22,
            height: 22,
            borderRadius: 6,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: color.muted3,
          }}
        >
          <Icon name="close" size={12} />
        </button>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "2px 0 6px" }}>
        {threads.map((view) => (
          <ThreadCard
            key={view.thread.id}
            view={view}
            authorNames={authorNames}
            selfKey={selfKey}
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
              onSubmitNew(t, text);
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
              margin: "6px 12px 4px",
              padding: "5px 8px",
              borderRadius: radius.sm,
              color: color.muted3,
              font: `500 11.5px ${font.sans}`,
            }}
          >
            <Icon name="plus" size={12} strokeWidth={1.9} /> Add comment
          </button>
        )}
      </div>
    </div>
  );
}
