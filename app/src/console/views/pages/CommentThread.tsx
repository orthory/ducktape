// Shared comment-thread pieces: one thread's card (comments, reply, resolve,
// edit/delete), participants, and the new-thread composer. Rendered by the
// floating per-target CommentCard.
//
// Both composers carry the chat @mention typeahead (useMentionMenu): the
// submit path already parses @tokens into structured user/agent mentions, so
// the menu is what makes them typeable without knowing an id by heart.
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

export function DiscussionParticipants({
  threads,
  authorNames,
  selfKey,
  selfName,
}: {
  threads: ThreadView[];
  authorNames: AuthorNames;
  selfKey: string;
  selfName: string;
}) {
  const participants: Comment[] = [];
  const seen = new Set<string>();
  for (const { comments } of threads) {
    for (const comment of comments) {
      const key = authorKey(comment.author);
      if (seen.has(key)) continue;
      seen.add(key);
      participants.push(comment);
    }
  }
  if (participants.length === 0) return null;
  const names = participants.map((comment) =>
    authorKey(comment.author) === selfKey
      ? selfName
      : authorName(comment.author, authorNames),
  );
  return (
    <div
      aria-label={`${participants.length} discussion participant${participants.length === 1 ? "" : "s"}`}
      title={names.join(", ")}
      style={{ display: "flex", alignItems: "center", paddingLeft: 5 }}
    >
      {participants.slice(0, 3).map((comment, index) => {
        const name = names[index]!;
        return (
          <span
            key={authorKey(comment.author)}
            style={{
              display: "inline-flex",
              marginLeft: index === 0 ? 0 : -6,
              border: `2px solid ${color.paper}`,
              borderRadius: "50%",
            }}
          >
            <Avatar author={comment.author} name={name} size={22} />
          </span>
        );
      })}
      {participants.length > 3 ? (
        <span style={{ marginLeft: 4, font: `600 10px ${font.mono}`, color: color.muted2 }}>
          +{participants.length - 3}
        </span>
      ) : null}
    </div>
  );
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
        margin: 0,
        padding: "14px 16px",
        borderTop: `1px solid ${color.borderSoft}`,
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
        onFocus={mention.onFocus}
        onBlur={mention.onBlur}
        onKeyDown={(e) => {
          if (mention.onKeyDown(e)) return;
          // IME guards: Enter committing a candidate must not submit, and the
          // Escape that CANCELS a composition must not cancel the composer.
          if (e.nativeEvent.isComposing) return;
          if (e.key === "Enter" && !e.shiftKey) {
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
  selfName,
  ops,
  threadPending,
  onEdit,
  onDelete,
}: {
  comment: Comment;
  authorNames: AuthorNames;
  selfKey: string;
  selfName: string;
  ops: OpLedger;
  /** True while the THREAD's op (add/reply/resolve) is in flight — an
   *  optimistic row's Edit/Delete would race the create it depends on (the
   *  transport gives no ordering), so own-comment actions hide until the
   *  thread settles. */
  threadPending: boolean;
  onEdit: (commentId: string, text: string) => void;
  onDelete: (commentId: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [editText, setEditText] = useState("");
  const editRef = useRef<HTMLTextAreaElement | null>(null);
  const editMention = useMentionMenu(editText, setEditText, editRef);
  const isSelf = authorKey(comment.author) === selfKey;
  const name = isSelf ? selfName : authorName(comment.author, authorNames);
  const own = isSelf && !threadPending;
  return (
    <div style={{ marginBottom: 14 }}>
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
            ref={editRef}
            aria-label="Edit comment text"
            autoFocus
            value={editText}
            onChange={editMention.onTextChange}
            onSelect={editMention.onSelect}
            onFocus={editMention.onFocus}
            onBlur={editMention.onBlur}
            onKeyDown={(event) => {
              if (editMention.onKeyDown(event)) return;
              if (event.nativeEvent.isComposing) return;
              if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                setEditing(false);
              }
            }}
            rows={2}
            style={composerStyle}
          />
          {editMention.menu}
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
  anchorText,
  authorNames,
  selfKey,
  selfName,
  ops = EMPTY_OPS,
  onReply,
  onResolve,
  onEdit,
  onDelete,
}: {
  view: ThreadView;
  /** Current text covered by this thread's relative anchor. */
  anchorText?: string;
  authorNames: AuthorNames;
  /** The local author's key (`selfAuthorKeyOf(selfAuthorBytes(...))`). Edit and
   *  Delete used to render on EVERY comment while the module enforces
   *  author-only — so a click on someone else's comment bought a rejected op
   *  and an error, never an edit. */
  selfKey: string;
  /** Available before identity hydration resolves our node key in authorNames. */
  selfName: string;
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
  const threadOp = ops[opKey.commentThread(thread.id)];
  const threadPending = threadOp?.phase === "pending";
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
        borderBottom: `1px solid ${color.borderSoft}`,
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
            padding: "8px 16px",
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

      {anchorText ? (
        <div
          aria-label="Commented text"
          style={{
            margin: "12px 16px 0",
            padding: "6px 9px",
            borderLeft: `2px solid ${color.amber}`,
            borderRadius: radius.sm,
            background: `color-mix(in srgb, ${color.amber} 10%, transparent)`,
            color: color.muted3,
            font: `500 11.5px/1.4 ${font.sans}`,
            whiteSpace: "pre-wrap",
            overflowWrap: "anywhere",
          }}
        >
          {anchorText}
        </div>
      ) : null}

      <div style={{ padding: "14px 16px 3px" }}>
        {comments.map((c) => (
          <CommentRow
            key={c.id}
            comment={c}
            authorNames={authorNames}
            selfKey={selfKey}
            selfName={selfName}
            ops={ops}
            threadPending={threadPending}
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
          padding: "5px 16px 14px",
        }}
      >
        <textarea
          ref={replyRef}
          aria-label="Reply to thread"
          value={reply}
          onChange={mention.onTextChange}
          onSelect={mention.onSelect}
          onFocus={mention.onFocus}
          onBlur={mention.onBlur}
          onKeyDown={onReplyKeyDown}
          rows={1}
          placeholder="Reply… (@ to mention)"
          style={{ ...composerStyle, flex: 1 }}
        />
        {mention.menu}
        <FinalizationMark op={threadOp} />
        <button
          type="button"
          title={thread.resolved ? "Reopen thread" : "Resolve thread"}
          aria-label={thread.resolved ? "Reopen thread" : "Resolve thread"}
          aria-disabled={threadPending}
          onClick={() => !threadPending && onResolve(thread.id, !thread.resolved)}
          style={threadPending ? { ...iconBtn, cursor: "default", opacity: 0.4 } : iconBtn}
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
