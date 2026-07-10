import type { AuthorNames } from "../../../domain/chat-client";
import type { TargetThreads } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";
import { NewThreadComposer, ThreadCard } from "./CommentThread";
import type { ComposerTarget } from "./CommentThread";

export type { ComposerTarget } from "./CommentThread";

/** The right-hand comments panel: every thread on the open page (grouped
 *  flat), each with its comments, a reply composer, resolve/reopen, and
 *  edit/delete (the module enforces author-only). When `composer` is set, a
 *  new-thread composer for that target renders above the thread list. */
export function CommentsPanel({
  threads,
  authorNames,
  composer,
  onClose,
  onSubmitNew,
  onCancelNew,
  onReply,
  onResolve,
  onEdit,
  onDelete,
}: {
  threads: TargetThreads[];
  authorNames: AuthorNames;
  composer: ComposerTarget | null;
  onClose: () => void;
  onSubmitNew: (target: string, text: string) => void;
  onCancelNew: () => void;
  onReply: (threadId: string, text: string) => void;
  onResolve: (threadId: string, resolved: boolean) => void;
  onEdit: (commentId: string, text: string) => void;
  onDelete: (commentId: string) => void;
}) {
  const flat = threads.flatMap((g) => g.threads);
  return (
    <aside
      aria-label="Comments"
      style={{
        width: 320,
        flexShrink: 0,
        borderLeft: `1px solid ${color.borderSoft}`,
        background: color.paper,
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
      }}
    >
      <div
        style={{
          height: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "0 16px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <div style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>Comments</div>
        <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
          {flat.length}
        </div>
        <button
          type="button"
          aria-label="Close comments"
          onClick={onClose}
          style={{
            all: "unset",
            cursor: "pointer",
            width: 24,
            height: 24,
            borderRadius: 6,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: color.muted3,
          }}
        >
          <Icon name="close" size={13} />
        </button>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "10px 0" }}>
        {composer ? (
          <NewThreadComposer
            key={composer.target}
            composer={composer}
            onSubmit={onSubmitNew}
            onCancel={onCancelNew}
          />
        ) : null}
        {flat.length === 0 && !composer ? (
          <div
            style={{
              margin: "10px 16px",
              padding: "16px 14px",
              border: `1px dashed ${color.borderStrong}`,
              borderRadius: radius.md,
              font: `400 12px/1.5 ${font.sans}`,
              color: color.muted2,
              textAlign: "center",
            }}
          >
            No comments yet. Select a block or comment on the page to start a thread.
          </div>
        ) : (
          flat.map((view) => (
            <ThreadCard
              key={view.thread.id}
              view={view}
              authorNames={authorNames}
              onReply={onReply}
              onResolve={onResolve}
              onEdit={onEdit}
              onDelete={onDelete}
            />
          ))
        )}
      </div>
    </aside>
  );
}
