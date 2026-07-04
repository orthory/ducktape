// The scrollable message stream for the active channel: day dividers,
// same-author grouping, and one <MessageItem> per root message. Thread
// replies never appear here — only roots (`head.thread === null`); replies
// live in the ThreadPanel.

import type { RefObject, UIEventHandler } from "react";

import { authorName } from "../../../domain/chat-client";
import type { AuthorNames, MessageView } from "../../../domain/chat-client";
import { buildStreamRows } from "./chat-helpers";
import { MessageItem } from "./MessageItem";
import { color, font } from "../../theme/tokens";

function DayDivider({ label }: { label: string }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "11px 0 8px", minWidth: 0 }}>
      <div style={{ flex: 1, height: 1, background: color.borderSoft }} />
      <span style={{ font: `500 10.5px ${font.mono}`, color: color.muted2, whiteSpace: "nowrap" }}>{label}</span>
      <div style={{ flex: 1, height: 1, background: color.borderSoft }} />
    </div>
  );
}

export function MessageList({
  channelName,
  messages,
  names,
  selfKey,
  workspaceId,
  hoverMsg,
  menuOpenId,
  listRef,
  onScroll,
  onHover,
  onMenuToggle,
  onOpenThread,
  onReact,
  onEdit,
  onDelete,
}: {
  channelName: string;
  /** Every sequence of the active channel (roots + replies + tombstones) —
   *  the same array the store keeps; used both for the root lane and to look
   *  up a thread's last-reply author for the inline pill's "· name" hint. */
  messages: MessageView[];
  names: AuthorNames;
  selfKey: string;
  workspaceId: string | null;
  hoverMsg: number | null;
  menuOpenId: number | null;
  listRef: RefObject<HTMLDivElement | null>;
  onScroll: UIEventHandler<HTMLDivElement>;
  onHover: (seq: number | null) => void;
  onMenuToggle: (seq: number | null) => void;
  onOpenThread: (seq: number) => void;
  onReact: (seq: number, emoji: string) => void;
  onEdit: (seq: number, text: string) => void;
  onDelete: (seq: number) => void;
}) {
  const roots = messages.filter((m) => m.head.thread === null);
  const rows = buildStreamRows(roots);
  const bySeq = new Map(messages.map((m) => [m.seq, m]));

  return (
    <div
      ref={listRef}
      role="log"
      aria-label={`#${channelName} messages`}
      onScroll={onScroll}
      style={{
        flex: 1,
        minHeight: 0,
        minWidth: 0,
        overflowY: "auto",
        overflowX: "hidden",
        padding: "14px 18px 18px",
        boxSizing: "border-box",
      }}
    >
      <div
        data-testid="chat-message-column"
        style={{
          width: "100%",
          minWidth: 0,
          display: "flex",
          flexDirection: "column",
          gap: 1,
        }}
      >
        {rows.map(({ message, groupStart, dayDivider }) => {
          const lastReply = message.head.last_reply_seq !== null ? bySeq.get(message.head.last_reply_seq) : undefined;
          const replyHint = lastReply ? authorName(lastReply.head.author, names) : null;
          const linkRef = `ducktape://${workspaceId ?? "local"}/${message.channel_id}/${message.head.message_id}`;
          const refRef = `${message.channel_id}#${message.seq}:${message.head.message_id}`;
          return (
            <div key={message.seq} style={{ minWidth: 0 }}>
              {dayDivider && <DayDivider label={dayDivider} />}
              <MessageItem
                message={message}
                names={names}
                groupStart={groupStart}
                selfKey={selfKey}
                hovered={hoverMsg === message.seq}
                menuOpen={menuOpenId === message.seq}
                replyHint={replyHint}
                linkRef={linkRef}
                refRef={refRef}
                onHover={(over) => onHover(over ? message.seq : null)}
                onMenuToggle={(open) => onMenuToggle(open ? message.seq : null)}
                onOpenThread={() => onOpenThread(message.seq)}
                onReact={(emoji) => onReact(message.seq, emoji)}
                onEdit={(text) => onEdit(message.seq, text)}
                onDelete={() => onDelete(message.seq)}
              />
            </div>
          );
        })}
        {roots.length === 0 && (
          <div style={{ padding: "22px 0", font: `400 12.5px ${font.sans}`, color: color.muted2 }}>
            No messages in #{channelName} yet. Send the first one.
          </div>
        )}
      </div>
    </div>
  );
}
