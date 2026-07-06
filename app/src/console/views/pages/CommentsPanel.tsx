import { useState } from "react";
import { authorName } from "../../../domain/chat-client";
import type { AuthorNames } from "../../../domain/chat-client";
import type { TargetThreads, ThreadView } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";

/** The right-hand comments panel: every thread on the open page (grouped
 *  flat), each with its comments, a reply composer, resolve/reopen, and
 *  edit/delete (the module enforces author-only). */
export function CommentsPanel({
  threads,
  authorNames,
  onClose,
  onReply,
  onResolve,
  onEdit,
  onDelete,
}: {
  threads: TargetThreads[];
  authorNames: AuthorNames;
  onClose: () => void;
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
        {flat.length === 0 ? (
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

function ThreadCard({
  view,
  authorNames,
  onReply,
  onResolve,
  onEdit,
  onDelete,
}: {
  view: ThreadView;
  authorNames: AuthorNames;
  onReply: (threadId: string, text: string) => void;
  onResolve: (threadId: string, resolved: boolean) => void;
  onEdit: (commentId: string, text: string) => void;
  onDelete: (commentId: string) => void;
}) {
  const { thread, comments } = view;
  const [reply, setReply] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [editText, setEditText] = useState("");
  return (
    <div
      style={{
        margin: "8px 12px",
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        opacity: thread.resolved ? 0.65 : 1,
      }}
    >
      <div style={{ padding: "10px 12px 4px" }}>
        {comments.map((c) => (
          <div key={c.id} style={{ marginBottom: 8 }}>
            <div style={{ display: "flex", alignItems: "baseline", gap: 6 }}>
              <span style={{ font: `600 11.5px ${font.sans}`, color: color.ink }}>
                {authorName(c.author, authorNames)}
              </span>
              {c.edited_at ? (
                <span style={{ font: `400 9.5px ${font.mono}`, color: color.muted2 }}>edited</span>
              ) : null}
              <div style={{ marginLeft: "auto", display: "flex", gap: 2 }}>
                <button
                  type="button"
                  aria-label="Edit comment"
                  onClick={() => {
                    setEditing(c.id);
                    setEditText(c.text);
                  }}
                  style={miniBtn}
                >
                  Edit
                </button>
                <button
                  type="button"
                  aria-label="Delete comment"
                  onClick={() => onDelete(c.id)}
                  style={miniBtn}
                >
                  Delete
                </button>
              </div>
            </div>
            {editing === c.id ? (
              <div style={{ marginTop: 4 }}>
                <textarea
                  aria-label="Edit comment text"
                  value={editText}
                  onChange={(e) => setEditText(e.target.value)}
                  rows={2}
                  style={composerStyle}
                />
                <div style={{ display: "flex", gap: 6, marginTop: 4 }}>
                  <button
                    type="button"
                    onClick={() => {
                      if (editText.trim()) onEdit(c.id, editText);
                      setEditing(null);
                    }}
                    style={primaryBtn}
                  >
                    Save
                  </button>
                  <button type="button" onClick={() => setEditing(null)} style={ghostBtn}>
                    Cancel
                  </button>
                </div>
              </div>
            ) : (
              <div style={{ marginTop: 2, font: `400 12.5px/1.5 ${font.sans}`, color: color.ink, whiteSpace: "pre-wrap" }}>
                {c.text}
              </div>
            )}
          </div>
        ))}
      </div>

      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "6px 12px 10px",
        }}
      >
        <input
          aria-label="Reply to thread"
          value={reply}
          onChange={(e) => setReply(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && reply.trim()) {
              onReply(thread.id, reply);
              setReply("");
            }
          }}
          placeholder="Reply…"
          style={{ ...composerStyle, height: 30, flex: 1 }}
        />
        <button
          type="button"
          aria-label={thread.resolved ? "Reopen thread" : "Resolve thread"}
          onClick={() => onResolve(thread.id, !thread.resolved)}
          style={ghostBtn}
        >
          {thread.resolved ? "Reopen" : "Resolve"}
        </button>
      </div>
    </div>
  );
}

const miniBtn = {
  all: "unset" as const,
  cursor: "pointer",
  padding: "1px 5px",
  borderRadius: 4,
  font: `500 10.5px ${font.sans}`,
  color: color.muted2,
};
const composerStyle = {
  width: "100%",
  boxSizing: "border-box" as const,
  border: `1px solid ${color.borderStrong}`,
  borderRadius: radius.sm,
  background: color.paper,
  padding: "6px 9px",
  font: `400 12px ${font.sans}`,
  color: color.ink,
  outline: "none",
  resize: "none" as const,
};
const primaryBtn = {
  all: "unset" as const,
  cursor: "pointer",
  padding: "4px 10px",
  borderRadius: radius.sm,
  background: color.dark,
  color: color.onDark,
  font: `600 11px ${font.sans}`,
};
const ghostBtn = {
  all: "unset" as const,
  cursor: "pointer",
  padding: "4px 10px",
  borderRadius: radius.sm,
  border: `1px solid ${color.border}`,
  color: color.muted3,
  font: `500 11px ${font.sans}`,
};
