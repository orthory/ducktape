// The chat surface over the node's `chat` module: a channel rail, a
// Slack-style message stream (grouped by author, divided by day, with a
// floating hover action bar), a composer, and a side thread panel that opens
// in place of pushing the lane around. Messages are sequence-addressed
// MessageViews with block bodies; authorship comes back as AuthorRef (derived
// from the submit origin), decoded to a display name here.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { FormEvent } from "react";

import { isModuleChannel } from "../../../domain/chat-client";
import type { Channel, PostPolicy } from "../../../domain/chat-client";
import { Icon } from "../../components/Icon";
import { PanelResizer } from "../../layout/PanelResizer";
import { selfAuthorBytes } from "../../store/state";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { ArchivedNotice } from "./ArchivedNotice";
import { ChannelMembersButton } from "./ChannelMembers";
import { ChannelMenu } from "./ChannelMenu";
import { selfAuthorKeyOf } from "./chat-helpers";
import { Composer } from "./Composer";
import { HoverButton } from "./HoverButton";
import { HuddleHeaderButton, HuddleRailBadge } from "./Huddle";
import { MessageList } from "./MessageList";
import { ChannelTagsButton, TagFilterBar, TagHitList } from "./TagFilter";
import { ThreadPanel } from "./ThreadPanel";

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

/** The channel rail's DEFAULT width. The live width is the `--chat-rail-w`
 *  CSS var (drag the rail's right edge to change it) — ConsoleShell sizes the
 *  floating huddle dock off the same var, so the two always agree. */
export const CHANNEL_RAIL_WIDTH = 200;
export const CHANNEL_RAIL_WIDTH_VAR = "--chat-rail-w";
export const CHANNEL_RAIL_MIN = 160;
export const CHANNEL_RAIL_MAX = 340;
export const channelRailWidth = `var(${CHANNEL_RAIL_WIDTH_VAR}, ${CHANNEL_RAIL_WIDTH}px)`;

/** One rail row. `muted` dims an archived channel — it still enters on click
 *  (that is how you get back to its "…" menu to unarchive it). */
function ChannelRow({
  channel,
  active,
  muted = false,
  onSelect,
}: {
  channel: Channel;
  active: boolean;
  muted?: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      onClick={onSelect}
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
        color: active ? color.ink : muted ? color.muted2 : color.muted3,
        font: `${active ? 600 : 400} 13.5px ${font.sans}`,
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
}

function ChannelRail() {
  const { state, actions } = useDucktape();
  const [draft, setDraft] = useState("");
  const [policy, setPolicy] = useState<PostPolicy>("open");
  const [creating, setCreating] = useState(false);
  const [showArchived, setShowArchived] = useState(false);
  // module-reserved channels (forge's per-item discussion threads,
  // `forge:<repo>:<n>`) are hidden from the chat surface — their messages
  // render inside the owning module's view, never in this rail. Archived
  // channels leave the main list too, but keep their own collapsed section at
  // its foot — entering one is the only way back to the "…" menu that unarchives
  // it, so they must never become unreachable.
  const listed = state.channels.filter((channel) => !isModuleChannel(channel.id));
  const channels = listed.filter((channel) => !channel.archived);
  const archived = listed.filter((channel) => channel.archived);

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
        position: "relative",
        width: channelRailWidth,
        flexShrink: 0,
        borderRight: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        display: "flex",
        flexDirection: "column",
        padding: "13px 0",
        boxSizing: "border-box",
      }}
    >
      <PanelResizer
        varName={CHANNEL_RAIL_WIDTH_VAR}
        defaultWidth={CHANNEL_RAIL_WIDTH}
        min={CHANNEL_RAIL_MIN}
        max={CHANNEL_RAIL_MAX}
        side="right"
      />
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
        {channels.map((channel) => (
          <ChannelRow
            key={channel.id}
            channel={channel}
            active={channel.id === state.activeChannel}
            onSelect={() => actions.selectChannel(channel.id)}
          />
        ))}
        {channels.length === 0 && (
          <div style={{ padding: "6px 15px", font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
            No channels yet — create one.
          </div>
        )}

        {archived.length > 0 && (
          <div style={{ marginTop: 8 }}>
            <HoverButton
              onClick={() => setShowArchived((open) => !open)}
              title={showArchived ? "Hide archived channels" : "Show archived channels"}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 5,
                width: "calc(100% - 16px)",
                margin: "1px 8px",
                padding: "5px 9px",
                borderRadius: radius.sm,
                font: `600 11px ${font.sans}`,
                letterSpacing: ".04em",
                color: color.muted,
                boxSizing: "border-box",
              }}
              hoverStyle={{ background: color.hover, color: color.ink }}
            >
              <Icon
                name="chevronRight"
                size={11}
                style={{ transform: showArchived ? "rotate(90deg)" : "none" }}
              />
              <span>ARCHIVED · {archived.length}</span>
            </HoverButton>
            {showArchived &&
              archived.map((channel) => (
                <ChannelRow
                  key={channel.id}
                  channel={channel}
                  active={channel.id === state.activeChannel}
                  muted
                  onSelect={() => actions.selectChannel(channel.id)}
                />
              ))}
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
  // module-reserved channels are hidden and archived ones live in the rail's
  // own collapsed section (see ChannelRail) — a workspace whose only channels
  // are module-owned or archived still reads "No channels yet" and offers the
  // create form.
  const hasChannels = state.channels.some(
    (channel) => !isModuleChannel(channel.id) && !channel.archived,
  );

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

// The "you are reading history" bar: a jump-to-message older than the channel's
// loaded tail replaces the stream with a window centered on that message, so
// scrolling down inside it dead-ends short of the newest message. Say so, and
// offer the way back — re-entering the channel drops the window and reloads the
// tail. Shaped like TagFilterBar, the surface's other "this isn't live" strip.
function HistoryWindowBar() {
  const { state, actions } = useDucktape();
  const focused = state.chatWindow;
  if (!focused || focused.channelId !== state.activeChannel) return null;
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "7px 18px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.sunken,
        flexShrink: 0,
        minWidth: 0,
      }}
    >
      <span style={{ font: `400 12px ${font.sans}`, color: color.muted2, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        Older history — around message #{focused.seq}
      </span>
      <span style={{ flex: 1 }} />
      <HoverButton
        title="Back to the newest messages"
        onClick={() => actions.selectChannel(focused.channelId)}
        style={{
          padding: "3px 8px",
          borderRadius: 6,
          color: color.muted3,
          font: `600 12px ${font.sans}`,
        }}
        hoverStyle={{ background: color.hover, color: color.ink }}
      >
        Jump to latest
      </HoverButton>
    </div>
  );
}

// ── The screen ──────────────────────────────────────────

export function ChatView() {
  const { state, actions } = useDucktape();
  const channel = state.channels.find((c) => c.id === state.activeChannel);
  // An archived channel refuses posts, reactions and huddle joins in the module
  // — so those affordances go away here rather than fail silently. Edits and
  // deletes still land, and stay offered.
  const archived = channel?.archived === true;
  const selfKey = selfAuthorKeyOf(selfAuthorBytes(state.status, state.author));
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

  // Jump-to-message: `focusMessage` (tag/search hit) sets chatFocusSeq and
  // enters its channel. Once that channel's own slice has landed, scroll the row
  // into view and flash it. Wait for the slice — enterChannel swaps messages in
  // async, so an early pass may still see the prior channel's array; acting then
  // would clear the focus before the target row exists.
  //
  // No row for the seq means the hit is older than the tail slice: page in the
  // window centered on it (`loadMessageWindow` re-arms the focus when it lands,
  // so the next pass takes the branch above). `chatWindow` is the record that we
  // already asked — without it a seq the window can't produce (an impossible
  // one, or a node too old to answer) would re-request on every render.
  useLayoutEffect(() => {
    const seq = state.chatFocusSeq;
    const channelId = state.activeChannel;
    if (seq === null || channelId === null) return;
    const loaded = state.messages.some((m) => m.channel_id === channelId);
    if (!loaded) return;
    const el = listRef.current?.querySelector<HTMLElement>(`[data-seq="${seq}"]`);
    const asked =
      state.chatWindow?.channelId === channelId && state.chatWindow.seq === seq;
    if (el) {
      pinnedRef.current = false; // don't let the tail-pin yank us back down
      el.scrollIntoView({ block: "center" });
      el.classList.add("msg-focus");
      setTimeout(() => el.classList.remove("msg-focus"), 2000);
    } else if (!asked) {
      actions.loadMessageWindow(channelId, seq);
    }
    actions.clearChatFocus();
  }, [
    state.chatFocusSeq,
    state.messages,
    state.activeChannel,
    state.chatWindow,
    actions,
  ]);

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
              font: `600 15px ${font.sans}`,
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
          {channel?.post_policy === "members_only" && <ChannelMembersButton channel={channel} />}
          {channel && <ChannelTagsButton />}
          {channel && !archived && <HuddleHeaderButton channel={channel} />}
          {channel && <ChannelMenu channel={channel} />}
        </div>

        {channel ? (
          <>
            <TagFilterBar />
            {state.tagFilter ? (
              // filtering: the tag's hits (read-only, newest first) replace
              // the live slice until the bar's ✕ clears the filter.
              <TagHitList />
            ) : (
              <>
                <HistoryWindowBar />
                <MessageList
                  channelName={channel.name}
                  messages={state.messages}
                  names={state.authorNames}
                  ops={state.ops}
                  selfKey={selfKey}
                  workspaceId={workspaceId}
                  archived={archived}
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
                  onTagClick={actions.setTagFilter}
                />
              </>
            )}
            {archived ? (
              <ArchivedNotice channel={channel} />
            ) : (
              <Composer
                value={draft}
                onChange={setDraft}
                onSend={handleSend}
                placeholder={`Message #${channel.name}`}
              />
            )}
          </>
        ) : (
          <EmptyChannelState />
        )}
      </div>
      {state.activeThread && channel && (
        <ThreadPanel
          thread={state.activeThread}
          channel={channel}
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
