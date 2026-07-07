// The chat surface over the node's `chat` module: a channel rail, a
// Slack-style message stream (grouped by author, divided by day, with a
// floating hover action bar), a composer, and a side thread panel that opens
// in place of pushing the lane around. Messages are sequence-addressed
// MessageViews with block bodies; authorship comes back as AuthorRef (derived
// from the submit origin), decoded to a display name here.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { FormEvent } from "react";

import type { PostPolicy } from "../../../domain/chat-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { selfAuthorKeyOf } from "./chat-helpers";
import { Composer } from "./Composer";
import { HoverButton } from "./HoverButton";
import { HuddleHeaderButton, HuddleRailBadge } from "./Huddle";
import { MessageList } from "./MessageList";
import { ThreadPanel } from "./ThreadPanel";

function LockGlyph({ size = 11 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <rect x="5" y="11" width="14" height="9" rx="2" />
      <path d="M8 11V8a4 4 0 0 1 8 0v3" />
    </svg>
  );
}

// A segmented Open / Members-only toggle for a channel's post policy.
function PolicyToggle({ value, onChange }: { value: PostPolicy; onChange: (policy: PostPolicy) => void }) {
  const options: { key: PostPolicy; label: string; hint: string }[] = [
    { key: "open", label: "Open", hint: "Any member of the workspace can post" },
    { key: "members_only", label: "Members", hint: "Only channel members can post" },
  ];
  return (
    <div style={{ display: "flex", gap: 3, background: color.sunken, borderRadius: radius.sm, padding: 3 }}>
      {options.map((option) => {
        const active = value === option.key;
        return (
          <button
            key={option.key}
            type="button"
            title={option.hint}
            onClick={() => onChange(option.key)}
            style={{
              all: "unset",
              cursor: "pointer",
              flex: 1,
              textAlign: "center",
              padding: "4px 8px",
              borderRadius: 5,
              font: `600 11px ${font.sans}`,
              color: active ? color.ink : color.muted2,
              background: active ? color.paper : "transparent",
              boxShadow: active ? "0 1px 2px rgba(0,0,0,.05)" : "none",
            }}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

// ── Channel rail ────────────────────────────────────────

/** The channel rail's fixed width — ConsoleShell sizes the floating huddle
 *  dock to sit inside this rail, so the two must agree. */
export const CHANNEL_RAIL_WIDTH = 200;

function ChannelRail() {
  const { state, actions } = useDucktape();
  const [draft, setDraft] = useState("");
  const [policy, setPolicy] = useState<PostPolicy>("open");
  const [creating, setCreating] = useState(false);

  const create = (event: FormEvent) => {
    event.preventDefault();
    if (draft.trim()) actions.createChannel(draft, policy);
    setDraft("");
    setPolicy("open");
    setCreating(false);
  };

  return (
    <div
      style={{
        width: CHANNEL_RAIL_WIDTH,
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
        <form onSubmit={create} style={{ padding: "0 11px 9px", display: "flex", flexDirection: "column", gap: 6 }}>
          <input
            autoFocus
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="channel name"
            style={{
              width: "100%",
              boxSizing: "border-box",
              padding: "6px 9px",
              borderRadius: radius.sm,
              border: `1px solid ${color.borderStrong}`,
              background: color.paper,
              font: `400 12px ${font.sans}`,
              color: color.ink,
            }}
          />
          <PolicyToggle value={policy} onChange={setPolicy} />
          <button
            type="submit"
            disabled={!draft.trim()}
            style={{
              all: "unset",
              cursor: draft.trim() ? "pointer" : "not-allowed",
              textAlign: "center",
              padding: "6px 0",
              borderRadius: radius.sm,
              background: draft.trim() ? color.dark : color.borderSoft,
              color: draft.trim() ? color.onDark : color.muted2,
              font: `600 12px ${font.sans}`,
            }}
          >
            Create channel
          </button>
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
              <HuddleRailBadge channel={channel} />
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
  const [policy, setPolicy] = useState<PostPolicy>("open");
  const hasChannels = state.channels.length > 0;

  const create = (event: FormEvent) => {
    event.preventDefault();
    if (draft.trim()) actions.createChannel(draft, policy);
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
          <form
            onSubmit={create}
            style={{ display: "flex", flexDirection: "column", gap: 8, marginTop: 4, width: 240 }}
          >
            <input
              autoFocus
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              placeholder="channel name"
              style={{
                width: "100%",
                boxSizing: "border-box",
                padding: "8px 10px",
                borderRadius: radius.sm,
                border: `1px solid ${color.borderStrong}`,
                background: color.paper,
                font: `400 12.5px ${font.sans}`,
                color: color.ink,
              }}
            />
            <PolicyToggle value={policy} onChange={setPolicy} />
            <button
              type="submit"
              disabled={!draft.trim()}
              style={{
                all: "unset",
                cursor: draft.trim() ? "pointer" : "not-allowed",
                textAlign: "center",
                padding: "8px 13px",
                borderRadius: radius.sm,
                background: draft.trim() ? color.dark : color.borderSoft,
                color: draft.trim() ? color.onDark : color.muted2,
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
          {channel?.post_policy === "members_only" && (
            <span
              title="Only channel members can post"
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 4,
                marginLeft: 2,
                padding: "2px 8px",
                borderRadius: 999,
                background: color.sunken,
                border: `1px solid ${color.borderSoft}`,
                font: `600 10px ${font.mono}`,
                color: color.muted,
                whiteSpace: "nowrap",
              }}
            >
              <LockGlyph size={10} /> Members only
            </span>
          )}
          {channel && <HuddleHeaderButton channel={channel} />}
        </div>

        {channel ? (
          <>
            <MessageList
              channelName={channel.name}
              messages={state.messages}
              names={state.authorNames}
              ops={state.ops}
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
              onEdit={actions.editMessage}
              onDelete={actions.deleteMessage}
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
          ops={state.ops}
          selfKey={selfKey}
          workspaceId={workspaceId}
          hoverMsg={hoverMsg}
          menuOpenId={msgMenuId}
          onHover={setHoverMsg}
          onMenuToggle={setMsgMenuId}
          onReact={actions.toggleReaction}
          onEdit={actions.editMessage}
          onDelete={actions.deleteMessage}
          onReply={actions.replyInThread}
          onClose={actions.closeThread}
        />
      )}
    </div>
  );
}
