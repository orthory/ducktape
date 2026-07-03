// The chat surface over the node's `chat` module: a channel rail, a
// Slack-style message stream (grouped by author, divided by day, with a
// floating hover action bar), a composer, and a side thread panel that opens
// in place of pushing the lane around. Messages are sequence-addressed
// MessageViews with block bodies; authorship comes back as AuthorRef (derived
// from the submit origin), decoded to a display name here.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { FormEvent } from "react";

import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { selfAuthorKeyOf } from "./chat-helpers";
import { Composer } from "./Composer";
import { HoverButton } from "./HoverButton";
import { MessageList } from "./MessageList";
import { ThreadPanel } from "./ThreadPanel";

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
        boxSizing: "border-box",
        overflow: "hidden",
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
        <HoverButton
          onClick={() => setCreating((open) => !open)}
          title="New channel"
          style={{
            color: color.muted,
            width: 20,
            height: 20,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            borderRadius: 5,
          }}
          hoverStyle={{ background: color.hover, color: color.ink }}
        >
          <Icon name="plus" size={14} />
        </HoverButton>
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
                boxSizing: "border-box",
              }}
            >
              <Icon name="hash" size={13} color={active ? color.ink : color.muted2} />
              <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
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

// ── Empty states ─────────────────────────────────────────

function EmptyChannelState() {
  const { state, actions } = useDucktape();
  const [draft, setDraft] = useState("");
  const hasChannels = state.channels.length > 0;

  const create = (event: FormEvent) => {
    event.preventDefault();
    if (draft.trim()) actions.createChannel(draft);
    setDraft("");
  };

  return (
    <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center" }}>
      <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 10, maxWidth: 260 }}>
        <div
          style={{
            width: 48,
            height: 48,
            borderRadius: radius.lg,
            background: color.sunken,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="hash" size={22} color={color.muted2} />
        </div>
        <div style={{ font: `600 14px ${font.sans}`, color: color.ink, textAlign: "center" }}>
          {hasChannels ? "Pick a channel" : "No channels yet"}
        </div>
        <div style={{ font: `400 12.5px ${font.sans}`, color: color.muted2, textAlign: "center" }}>
          {hasChannels
            ? "Choose a channel from the list to start reading and posting."
            : "Create the first channel to start the conversation."}
        </div>
        {!hasChannels && (
          <form onSubmit={create} style={{ display: "flex", gap: 7, marginTop: 4 }}>
            <input
              autoFocus
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              placeholder="channel name"
              style={{
                padding: "7px 10px",
                borderRadius: radius.sm,
                border: `1px solid ${color.borderStrong}`,
                background: color.paper,
                font: `400 12.5px ${font.sans}`,
                color: color.ink,
              }}
            />
            <button
              type="submit"
              style={{
                all: "unset",
                cursor: "pointer",
                padding: "7px 13px",
                borderRadius: radius.sm,
                background: color.dark,
                color: color.onDark,
                font: `600 12.5px ${font.sans}`,
              }}
            >
              Create channel
            </button>
          </form>
        )}
      </div>
    </div>
  );
}

// ── The screen ──────────────────────────────────────────

export function ChatView() {
  const { state, actions } = useDucktape();
  const channel = state.channels.find((c) => c.id === state.activeChannel);
  const selfKey = selfAuthorKeyOf(state.author);
  const workspaceId = state.workspace?.id ?? null;
  const rootMessageCount = state.messages.filter((message) => message.head.thread === null).length;

  const [draft, setDraft] = useState("");
  const [hoverMsg, setHoverMsg] = useState<number | null>(null);
  const [msgMenuId, setMsgMenuId] = useState<number | null>(null);

  const listRef = useRef<HTMLDivElement>(null);
  // whether the reader is parked at the bottom. start pinned so the first paint
  // and every channel switch land on the newest message.
  const pinnedRef = useRef(true);

  // a channel switch is a fresh read — treat it as pinned to that channel's tail.
  useLayoutEffect(() => {
    pinnedRef.current = true;
  }, [state.activeChannel]);

  useEffect(() => {
    setHoverMsg(null);
    setMsgMenuId(null);
  }, [state.activeChannel, workspaceId]);

  // follow new messages to the bottom ONLY when the reader is already there (or
  // just sent) — never yank them up from scrolled-back history when someone
  // else's message lands.
  useLayoutEffect(() => {
    const el = listRef.current;
    if (el && pinnedRef.current) el.scrollTop = el.scrollHeight;
  }, [state.messages.length, state.activeChannel]);

  const handleSend = () => {
    if (!draft.trim()) return;
    pinnedRef.current = true;
    actions.sendMessage(draft);
    setDraft("");
  };

  return (
    <div style={{ display: "flex", flex: 1, minWidth: 0, minHeight: 0, overflow: "hidden" }}>
      <ChannelRail />
      <div
        style={{
          flex: 1,
          minWidth: 0,
          minHeight: 0,
          display: "flex",
          flexDirection: "column",
          background: color.paper,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 9,
            padding: "0 18px",
            height: 50,
            boxSizing: "border-box",
            borderBottom: `1px solid ${color.borderSoft}`,
            flexShrink: 0,
            minWidth: 0,
          }}
        >
          <Icon name="hash" size={15} color={color.muted} />
          <span
            style={{
              font: `600 14px ${font.sans}`,
              color: color.ink,
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {channel?.name ?? "No channel"}
          </span>
          {channel && (
            <span style={{ font: `400 12px ${font.sans}`, color: color.muted2, whiteSpace: "nowrap" }}>
              · {rootMessageCount} {rootMessageCount === 1 ? "message" : "messages"}
            </span>
          )}
        </div>

        {channel ? (
          <>
            <MessageList
              channelName={channel.name}
              messages={state.messages}
              names={state.authorNames}
              selfKey={selfKey}
              workspaceId={workspaceId}
              hoverMsg={hoverMsg}
              menuOpenId={msgMenuId}
              listRef={listRef}
              onScroll={(event) => {
                const el = event.currentTarget;
                pinnedRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
              }}
              onHover={setHoverMsg}
              onMenuToggle={setMsgMenuId}
              onOpenThread={actions.openThread}
              onReact={actions.toggleReaction}
            />
            <Composer
              value={draft}
              onChange={setDraft}
              onSend={handleSend}
              placeholder={`Message #${channel.name}`}
            />
          </>
        ) : (
          <EmptyChannelState />
        )}
      </div>
      {state.activeThread && channel && (
        <ThreadPanel
          thread={state.activeThread}
          channelName={channel.name}
          names={state.authorNames}
          selfKey={selfKey}
          workspaceId={workspaceId}
          hoverMsg={hoverMsg}
          menuOpenId={msgMenuId}
          onHover={setHoverMsg}
          onMenuToggle={setMsgMenuId}
          onReact={actions.toggleReaction}
          onReply={actions.replyInThread}
          onClose={actions.closeThread}
        />
      )}
    </div>
  );
}
