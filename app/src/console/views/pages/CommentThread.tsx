// Shared comment-thread pieces: one thread's card (comments, reply, resolve,
// edit/delete) and the new-thread composer. Rendered by both the full
// CommentsPanel and the floating per-target CommentCard.
//
// Both composers carry the chat @mention typeahead (useMentionMenu): the
// submit path already parses @tokens into structured agent mentions, so the
// menu is what makes them typeable without knowing an agent id by heart.
// Bodies render through CommentText, which resolves the same tokens back to
// live mention chips.

import { useRef, useState } from "react";
import type { KeyboardEvent } from "react";

import { authorName } from "../../../domain/chat-client";
import type { AuthorNames } from "../../../domain/chat-client";
import type { Comment, ThreadView } from "../../../domain/pages-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpLedger } from "../../store/finalization";
import { wallClockMillisOf } from "../../../domain/wire";
import { authorKey } from "../chat/chat-helpers";
import { Avatar } from "../chat/MessageItem";
import { CommentText } from "../chat/rich-text";
import { useMentionMenu } from "../chat/use-mention-menu";
import { color, font, radius } from "../../theme/tokens";

const EMPTY_OPS: OpLedger = {};

/** Comment timestamps span days (unlike the chat lane, which has day
 *  dividers): today → HH:MM, this year → "May 3", older → "May 3, 2025".
 *  Empty when the stamp isn't real wall-clock (a validator's height counter,
 *  see domain/wire.ts) — better no time than a fake one. */
const commentTime = (stamp: number): string => {
  const ms = wallClockMillisOf(stamp);
  if (ms === null) return "";
  const date = new Date(ms);
  const now = new Date();
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return date.toLocaleDateString([], {
    month: "short",
    day: "numeric",
    ...(date.getFullYear() !== now.getFullYear() ? { year: "numeric" } : {}),
  });
};

/** What the new-thread composer is aimed at: the target id the thread will
 *  anchor to, plus a human label ("this page" / "this block") for the header. */
export interface ComposerTarget {
  target: string;
  label: string;
}

/** Composer for opening a NEW thread on a block or the page. Autofocused so
 *  the "Comment" affordances drop the user straight into typing; Enter
 *  submits, Shift+Enter breaks a line (comments render pre-wrap). */
export function NewThreadComposer({
  composer,
  onSubmit,
  onCancel,
}: {
  composer: ComposerTarget;
  onSubmit: (target: string, text: string) => void;
  onCancel: () => void;
}) {
  const [text, setText] = useState("");
  const ref = useRef<HTMLTextAreaElement | null>(null);
  const mention = useMentionMenu(text, setText, ref);
  const submit = () => {
    if (text.trim()) onSubmit(composer.target, text);
  };
  return (
    <div
      role="form"
      aria-label={`New comment on ${composer.label}`}
      style={{
        margin: "8px 12px",
        padding: "10px 12px",
        border: `1px solid ${color.borderStrong}`,
        borderRadius: radius.md,
        background: color.paper,
      }}
    >
      <div style={{ font: `600 11.5px ${font.sans}`, color: color.ink, marginBottom: 6 }}>
        New comment on {composer.label}
      </div>
      <textarea
        ref={ref}
        aria-label="New comment text"
        autoFocus
        value={text}
        onChange={mention.onTextChange}
        onSelect={mention.onSelect}
        onKeyDown={(e) => {
          if (mention.onKeyDown(e)) return;
          if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
            e.preventDefault();
            submit();
          }
          if (e.key === "Escape") onCancel();
        }}
        rows={3}
        placeholder="Write a comment… (@ to mention)"
        style={composerStyle}
      />
      {mention.menu}
      <div style={{ display: "flex", gap: 6, marginTop: 6 }}>
        <button type="button" aria-label="Add comment" onClick={submit} style={primaryBtn}>
          Comment
        </button>
        <button type="button" aria-label="Cancel new comment" onClick={onCancel} style={ghostBtn}>
          Cancel
        </button>
      </div>
    </div>
  );
}

/** One comment: identity row (avatar, name, time, edited, its op's
 *  finalization mark, own-comment Edit/Delete), body indented to the name. */
function CommentRow({
  comment,
  authorNames,
  selfKey,
  ops,
  onEdit,
  onDelete,
}: {
  comment: Comment;
  authorNames: AuthorNames;
  selfKey: string;
  ops: OpLedger;
  onEdit: (commentId: string, text: string) => void;
  onDelete: (commentId: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [editText, setEditText] = useState("");
  const name = authorName(comment.author, authorNames);
  const own = authorKey(comment.author) === selfKey;
  return (
    <div style={{ marginBottom: 10 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
        <Avatar author={comment.author} name={name} size={20} />
        <span
          style={{
            font: `600 11.5px ${font.sans}`,
            color: color.ink,
            minWidth: 0,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {name}
        </span>
        <span style={{ font: `400 10px ${font.mono}`, color: color.muted2, flexShrink: 0 }}>
          {commentTime(comment.created_at)}
        </span>
        {comment.edited_at !== null ? (
          <span style={{ font: `400 9.5px ${font.sans}`, color: color.muted2, flexShrink: 0 }}>
            (edited)
          </span>
        ) : null}
        <FinalizationMark op={ops[opKey.comment(comment.id)]} />
        {own ? (
          <div style={{ marginLeft: "auto", display: "flex", gap: 2, flexShrink: 0 }}>
            <button
              type="button"
              aria-label="Edit comment"
              onClick={() => {
                setEditing(true);
                setEditText(comment.text);
              }}
              style={miniBtn}
            >
              Edit
            </button>
            <button
              type="button"
              aria-label="Delete comment"
              onClick={() => onDelete(comment.id)}
              style={miniBtn}
            >
              Delete
            </button>
          </div>
        ) : null}
      </div>
      {editing ? (
        <div style={{ marginTop: 4, marginLeft: 27 }}>
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
                if (editText.trim()) onEdit(comment.id, editText);
                setEditing(false);
              }}
              style={primaryBtn}
            >
              Save
            </button>
            <button type="button" onClick={() => setEditing(false)} style={ghostBtn}>
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div
          style={{
            marginTop: 2,
            marginLeft: 27,
            font: `400 12.5px/1.5 ${font.sans}`,
            color: color.ink,
            whiteSpace: "pre-wrap",
            overflowWrap: "anywhere",
          }}
        >
          <CommentText text={comment.text} names={authorNames} />
        </div>
      )}
    </div>
  );
}

export function ThreadCard({
  view,
  authorNames,
  selfKey,
  ops = EMPTY_OPS,
  onReply,
  onResolve,
  onEdit,
  onDelete,
}: {
  view: ThreadView;
  authorNames: AuthorNames;
  /** The local author's key (`selfAuthorKeyOf(selfAuthorBytes(...))`). Edit and
   *  Delete used to render on EVERY comment while the module enforces
   *  author-only — so a click on someone else's comment bought a rejected op
   *  and an error, never an edit. */
  selfKey: string;
  /** The finalization ledger — comment rows mark their own in-flight ops, the
   *  reply row marks thread-keyed ones (add/reply/resolve). Optional so bare
   *  renders stay passive. */
  ops?: OpLedger;
  onReply: (threadId: string, text: string) => void;
  onResolve: (threadId: string, resolved: boolean) => void;
  onEdit: (commentId: string, text: string) => void;
  onDelete: (commentId: string) => void;
}) {
  const { thread, comments } = view;
  const [reply, setReply] = useState("");
  const replyRef = useRef<HTMLTextAreaElement | null>(null);
  const mention = useMentionMenu(reply, setReply, replyRef);
  const submitReply = () => {
    if (!reply.trim()) return;
    onReply(thread.id, reply);
    setReply("");
  };
  const onReplyKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (mention.onKeyDown(e)) return;
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      submitReply();
    }
  };
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
      {thread.resolved ? (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 5,
            padding: "7px 12px",
            borderBottom: `1px solid ${color.borderSoft}`,
            color: color.muted3,
            font: `600 10.5px ${font.sans}`,
          }}
        >
          <Icon name="check" size={11} strokeWidth={2} />
          Resolved
          {thread.resolved_by ? ` by ${authorName(thread.resolved_by, authorNames)}` : ""}
        </div>
      ) : null}

      <div style={{ padding: "10px 12px 4px" }}>
        {comments.map((c) => (
          <CommentRow
            key={c.id}
            comment={c}
            authorNames={authorNames}
            selfKey={selfKey}
            ops={ops}
            onEdit={onEdit}
            onDelete={onDelete}
          />
        ))}
      </div>

      {/* the reply used to be a single-line <input> while the new-thread
          composer was a textarea with Shift+Enter — the same act of writing a
          comment, with two different grammars. One grammar now. Resolve moved
          to a titled icon so it reads as a thread action, not a send button. */}
      <div
        style={{
          display: "flex",
          alignItems: "flex-end",
          gap: 6,
          padding: "6px 12px 10px",
        }}
      >
        <textarea
          ref={replyRef}
          aria-label="Reply to thread"
          value={reply}
          onChange={mention.onTextChange}
          onSelect={mention.onSelect}
          onKeyDown={onReplyKeyDown}
          rows={1}
          placeholder="Reply… (@ to mention)"
          style={{ ...composerStyle, flex: 1 }}
        />
        {mention.menu}
        <FinalizationMark op={ops[opKey.commentThread(thread.id)]} />
        <button
          type="button"
          title={thread.resolved ? "Reopen thread" : "Resolve thread"}
          aria-label={thread.resolved ? "Reopen thread" : "Resolve thread"}
          onClick={() => onResolve(thread.id, !thread.resolved)}
          style={iconBtn}
        >
          <Icon name={thread.resolved ? "refresh" : "check"} size={13} strokeWidth={1.9} />
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
const iconBtn = {
  all: "unset" as const,
  cursor: "pointer",
  width: 26,
  height: 26,
  borderRadius: radius.sm,
  border: `1px solid ${color.border}`,
  color: color.muted3,
  display: "flex" as const,
  alignItems: "center" as const,
  justifyContent: "center" as const,
  flexShrink: 0,
};
