// The chat surface over the node's `chat` module: a channel rail, the message
// list, a composer, and a thread side panel. Exactly what the module serves —
// channels, messages, thread replies — no reactions/pins/presence cosplay.

import { useState } from "react";
import type { CSSProperties, FormEvent } from "react";

import type { ChatMessage } from "../../../domain/chat-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius } from "../../theme/tokens";

// ── Shared bits ─────────────────────────────────────────

const timeOf = (millis: number): string =>
  new Date(millis).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

const avatarStyle: CSSProperties = {
  width: 28,
  height: 28,
  borderRadius: "50%",
  background: color.chip,
  color: color.muted3,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  font: `600 11px ${font.sans}`,
  flexShrink: 0,
};

function Composer({
  placeholder,
  onSubmit,
}: {
  placeholder: string;
  onSubmit: (body: string) => void;
}) {
  const [draft, setDraft] = useState("");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onSubmit(draft);
    setDraft("");
  };

  return (
    <form
      onSubmit={submit}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        margin: 13,
        padding: "9px 13px",
        borderRadius: radius.md,
        border: `1px solid ${color.borderStrong}`,
        background: color.paper,
      }}
    >
      <input
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        placeholder={placeholder}
        style={{ flex: 1, font: `400 13px ${font.sans}`, color: color.ink }}
      />
      <button
        type="submit"
        title="Send"
        style={{
          all: "unset",
          cursor: "pointer",
          width: 26,
          height: 26,
          borderRadius: 7,
          background: draft.trim() ? accentVar : color.chip,
          color: "#fff",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <Icon name="chevronRight" size={15} />
      </button>
    </form>
  );
}

function MessageRow({
  message,
  onOpenThread,
}: {
  message: ChatMessage;
  onOpenThread?: (rootId: string) => void;
}) {
  return (
    <div
      style={{
        display: "flex",
        gap: 10,
        padding: "7px 17px",
        animation: "msgIn .16s ease-out",
      }}
    >
      <span style={avatarStyle}>{message.author.slice(0, 2).toUpperCase()}</span>
      <div style={{ minWidth: 0, flex: 1 }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
          <span style={{ font: `600 12.5px ${font.sans}`, color: color.ink }}>
            {message.author}
          </span>
          <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
            {timeOf(message.sent_at)}
          </span>
        </div>
        <div
          style={{
            font: `400 13px ${font.sans}`,
            color: color.inkSofter,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }}
        >
          {message.body}
        </div>
        {onOpenThread && (
          <button
            onClick={() => onOpenThread(message.id)}
            style={{
              all: "unset",
              cursor: "pointer",
              marginTop: 3,
              font: `500 11px ${font.sans}`,
              color: message.reply_count > 0 ? accentVar : color.muted2,
            }}
          >
            {message.reply_count > 0
              ? `${message.reply_count} ${message.reply_count === 1 ? "reply" : "replies"}`
              : "reply in thread"}
          </button>
        )}
      </div>
    </div>
  );
}

// ── Channel rail ────────────────────────────────────────

function ChannelRail() {
  const { state, actions } = useDucktape();
  const [draft, setDraft] = useState("");
  const [creating, setCreating] = useState(false);

  const create = (event: FormEvent) => {
    event.preventDefault();
    if (draft.trim()) actions.createChannel(draft);
    setDraft("");
    setCreating(false);
  };

  return (
    <div
      style={{
        width: 200,
        flexShrink: 0,
        borderRight: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        display: "flex",
        flexDirection: "column",
        padding: "13px 0",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 15px 9px",
        }}
      >
        <span style={{ font: `600 11px ${font.sans}`, color: color.muted, letterSpacing: ".04em" }}>
          CHANNELS
        </span>
        <button
          onClick={() => setCreating((open) => !open)}
          title="New channel"
          style={{ all: "unset", cursor: "pointer", color: color.muted }}
        >
          <Icon name="plus" size={14} />
        </button>
      </div>

      {creating && (
        <form onSubmit={create} style={{ padding: "0 11px 8px" }}>
          <input
            autoFocus
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="channel name"
            style={{
              width: "100%",
              padding: "6px 9px",
              borderRadius: radius.sm,
              border: `1px solid ${color.borderStrong}`,
              background: color.paper,
              font: `400 12px ${font.sans}`,
              color: color.ink,
            }}
          />
        </form>
      )}

      <div style={{ overflowY: "auto", flex: 1 }}>
        {state.channels.map((channel) => {
          const active = channel.id === state.activeChannel;
          return (
            <button
              key={channel.id}
              onClick={() => actions.selectChannel(channel.id)}
              style={{
                all: "unset",
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
                gap: 7,
                width: "calc(100% - 16px)",
                margin: "1px 8px",
                padding: "6px 9px",
                borderRadius: radius.sm,
                background: active ? color.hover : "transparent",
                color: active ? color.ink : color.muted3,
                font: `${active ? 600 : 400} 12.5px ${font.sans}`,
              }}
            >
              <Icon name="hash" size={13} color={active ? color.ink : color.muted2} />
              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {channel.name}
              </span>
            </button>
          );
        })}
        {state.channels.length === 0 && (
          <div style={{ padding: "6px 15px", font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
            No channels yet — create one.
          </div>
        )}
      </div>
    </div>
  );
}

// ── Thread panel ────────────────────────────────────────

function ThreadPanel() {
  const { state, actions } = useDucktape();
  const thread = state.activeThread;
  if (!thread) return null;

  return (
    <div
      style={{
        width: 320,
        flexShrink: 0,
        borderLeft: `1px solid ${color.borderSoft}`,
        display: "flex",
        flexDirection: "column",
        background: color.paper,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "11px 15px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span style={{ font: `600 12.5px ${font.sans}`, color: color.ink }}>Thread</span>
        <button
          onClick={actions.closeThread}
          title="Close thread"
          style={{ all: "unset", cursor: "pointer", color: color.muted }}
        >
          <Icon name="close" size={14} />
        </button>
      </div>
      <div style={{ flex: 1, overflowY: "auto", padding: "9px 0" }}>
        <MessageRow message={thread.root} />
        {thread.replies.length > 0 && (
          <div
            style={{
              margin: "4px 17px",
              borderTop: `1px solid ${color.borderSoft}`,
            }}
          />
        )}
        {thread.replies.map((reply) => (
          <MessageRow key={reply.id} message={reply} />
        ))}
      </div>
      <Composer placeholder="Reply in thread" onSubmit={actions.replyInThread} />
    </div>
  );
}

// ── The screen ──────────────────────────────────────────

export function ChatView() {
  const { state, actions } = useDucktape();
  const channel = state.channels.find((c) => c.id === state.activeChannel);
  // thread replies render in the panel; the main lane shows roots only
  const roots = state.messages.filter((message) => message.thread_id === null);

  return (
    <div style={{ display: "flex", flex: 1, minWidth: 0 }}>
      <ChannelRail />
      <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 7,
            padding: "11px 17px",
            borderBottom: `1px solid ${color.borderSoft}`,
          }}
        >
          <Icon name="hash" size={15} color={color.muted} />
          <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>
            {channel?.name ?? "No channel"}
          </span>
        </div>

        <div style={{ flex: 1, overflowY: "auto", padding: "9px 0" }}>
          {roots.map((message) => (
            <MessageRow key={message.id} message={message} onOpenThread={actions.openThread} />
          ))}
          {channel && roots.length === 0 && (
            <div style={{ padding: "13px 17px", font: `400 12.5px ${font.sans}`, color: color.muted2 }}>
              No messages in #{channel.name} yet.
            </div>
          )}
        </div>

        {channel && (
          <Composer placeholder={`Message #${channel.name}`} onSubmit={actions.sendMessage} />
        )}
      </div>
      <ThreadPanel />
    </div>
  );
}
