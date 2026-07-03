// The Slack-style thread side panel: root message, replies, and its own
// composer (separate draft state from the main channel composer). Opened
// from the inline "N replies" pill, the hover bar's thread icon, or the
// overflow menu's "Reply in thread" — all three converge on the same
// `actions.openThread`. It is a permanent-width flex sibling of the message
// lane, not an overlay, and only one can be open system-wide (`activeThread`
// is a single slot).

import { useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";

import { authorName } from "../../../domain/chat-client";
import type { AuthorNames, ChatThread } from "../../../domain/chat-client";
import { Icon } from "../../components/Icon";
import { Composer } from "./Composer";
import { MessageItem } from "./MessageItem";
import { color, font } from "../../theme/tokens";

const THREAD_COMPOSER_MAX_HEIGHT = 120;

export function ThreadPanel({
  thread,
  channelName,
  names,
  selfKey,
  workspaceId,
  hoverMsg,
  menuOpenId,
  onHover,
  onMenuToggle,
  onReact,
  onReply,
  onClose,
}: {
  thread: ChatThread;
  channelName: string;
  names: AuthorNames;
  selfKey: string;
  workspaceId: string | null;
  hoverMsg: number | null;
  menuOpenId: number | null;
  onHover: (seq: number | null) => void;
  onMenuToggle: (seq: number | null) => void;
  onReact: (seq: number, emoji: string) => void;
  onReply: (body: string) => void;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [thread.root.seq, thread.replies.length]);

  const send = () => {
    if (!draft.trim()) return;
    onReply(draft);
    setDraft("");
  };

  const handleComposerKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") onClose();
  };

  const linkRefOf = (messageId: string) => `ducktape://${workspaceId ?? "local"}/${thread.root.channel_id}/${messageId}`;
  const refRefOf = (seq: number, messageId: string) => `${thread.root.channel_id}#${seq}:${messageId}`;

  return (
    <div
      style={{
        width: 328,
        flexShrink: 0,
        borderLeft: `1px solid ${color.borderSoft}`,
        display: "flex",
        flexDirection: "column",
        background: color.sidebar,
        minHeight: 0,
        minWidth: 0,
        overflow: "hidden",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "11px 15px",
          borderBottom: `1px solid ${color.borderSoft}`,
          flexShrink: 0,
          minWidth: 0,
        }}
      >
        <div style={{ display: "flex", flexDirection: "column", gap: 1, minWidth: 0 }}>
          <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>Thread</span>
          <span
            style={{
              font: `400 11px ${font.sans}`,
              color: color.muted2,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            #{channelName}
          </span>
        </div>
        <button
          onClick={onClose}
          title="Close thread"
          style={{
            all: "unset",
            cursor: "pointer",
            width: 24,
            height: 24,
            borderRadius: 6,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: color.muted,
          }}
        >
          <Icon name="close" size={13} strokeWidth={1.8} />
        </button>
      </div>

      <div
        ref={scrollRef}
        style={{
          flex: 1,
          minHeight: 0,
          minWidth: 0,
          overflowY: "auto",
          overflowX: "hidden",
          padding: "14px 14px",
          boxSizing: "border-box",
        }}
      >
        <div style={{ padding: "0 0 13px", marginBottom: 11, borderBottom: `1px solid ${color.borderSoft}`, minWidth: 0 }}>
          <MessageItem
            message={thread.root}
            names={names}
            groupStart
            selfKey={selfKey}
            hovered={hoverMsg === thread.root.seq}
            menuOpen={menuOpenId === thread.root.seq}
            replyHint={null}
            linkRef={linkRefOf(thread.root.head.message_id)}
            refRef={refRefOf(thread.root.seq, thread.root.head.message_id)}
            onHover={(over) => onHover(over ? thread.root.seq : null)}
            onMenuToggle={(open) => onMenuToggle(open ? thread.root.seq : null)}
            onOpenThread={() => {}}
            onReact={(emoji) => onReact(thread.root.seq, emoji)}
            threadable={false}
          />
        </div>

        <div style={{ margin: "0 0 6px", font: `500 10.5px ${font.mono}`, color: color.muted2 }}>
          {thread.replies.length} {thread.replies.length === 1 ? "reply" : "replies"}
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: 1, minWidth: 0 }}>
          {thread.replies.map((reply) => (
            <div key={reply.seq} style={{ minWidth: 0 }}>
              <MessageItem
                message={reply}
                names={names}
                groupStart
                selfKey={selfKey}
                hovered={hoverMsg === reply.seq}
                menuOpen={menuOpenId === reply.seq}
                replyHint={null}
                linkRef={linkRefOf(reply.head.message_id)}
                refRef={refRefOf(reply.seq, reply.head.message_id)}
                onHover={(over) => onHover(over ? reply.seq : null)}
                onMenuToggle={(open) => onMenuToggle(open ? reply.seq : null)}
                onOpenThread={() => {}}
                onReact={(emoji) => onReact(reply.seq, emoji)}
                threadable={false}
              />
            </div>
          ))}
        </div>
        {thread.replies.length === 0 && (
          <div style={{ margin: "7px 0 0", font: `400 12px ${font.sans}`, color: color.muted2 }}>
            No replies yet. Start the thread below.
          </div>
        )}
      </div>

      <div onKeyDown={handleComposerKeyDown} style={{ flexShrink: 0 }}>
        <Composer
          value={draft}
          onChange={setDraft}
          onSend={send}
          placeholder={`Reply to ${authorName(thread.root.head.author, names)}…`}
          maxHeight={THREAD_COMPOSER_MAX_HEIGHT}
          autoFocus
        />
      </div>
    </div>
  );
}
