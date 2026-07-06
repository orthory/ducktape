// The inbox surface over the node's `inbox` module: the local member's
// notification queue, a small self-notify composer, and explicit
// mark-read/clear controls.

import { useState } from "react";
import type { CSSProperties, FormEvent } from "react";

import type { Notification } from "../../../domain/inbox-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const inputBase: CSSProperties = {
  width: "100%",
  minWidth: 0,
  height: 36,
  padding: "0 12px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 13px ${font.sans}`,
  color: color.ink,
  outline: "none",
};

/** A short, human relative time from a UNIX-seconds timestamp. */
export const relativeTime = (unixSeconds: number): string => {
  if (!Number.isFinite(unixSeconds) || unixSeconds <= 0) return "unknown";
  const diffSeconds = Math.max(0, Date.now() / 1000 - unixSeconds);
  if (diffSeconds < 45) return "just now";
  const minutes = Math.floor(diffSeconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  return new Date(unixSeconds * 1000).toLocaleDateString();
};

function UnreadPill({ count }: { count: number }) {
  const active = count > 0;
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        borderRadius: 999,
        border: `1px solid ${active ? color.dangerBorder : color.borderSoft}`,
        background: active ? color.dangerSoft : color.sunken,
        color: active ? color.red : color.muted2,
        padding: "4px 9px",
        font: `600 10.5px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      <span
        style={{
          width: 6,
          height: 6,
          borderRadius: "50%",
          background: active ? color.red : color.muted2,
        }}
      />
      {count} unread
    </span>
  );
}

function HeaderButton({
  label,
  disabled,
  onClick,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);
  const active = !disabled;

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        height: 30,
        padding: "0 12px",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        borderRadius: radius.sm,
        border: `1px solid ${active ? color.borderStrong : color.borderSoft}`,
        background: active && hover ? color.hover : color.paper,
        color: active ? color.inkSoft : color.muted2,
        cursor: active ? "pointer" : "default",
        font: `600 11px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </button>
  );
}

function NotificationRow({
  item,
  onOpen,
}: {
  item: Notification;
  onOpen: (seq: number) => void;
}) {
  const [hover, setHover] = useState(false);
  const unread = !item.read;

  return (
    <div
      onClick={() => {
        if (unread) onOpen(item.seq);
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: 11,
        padding: "13px 16px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: unread && hover ? color.sidebar : "transparent",
        cursor: unread ? "pointer" : "default",
      }}
    >
      <span
        style={{
          width: 8,
          height: 8,
          marginTop: 5,
          borderRadius: "50%",
          background: unread ? color.red : "transparent",
          flexShrink: 0,
        }}
      />
      <div style={{ flex: 1, minWidth: 0 }}>
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            borderRadius: radius.sm,
            border: `1px solid ${color.border}`,
            background: color.sunken,
            color: unread ? color.inkSoft : color.muted2,
            padding: "2px 7px",
            font: `600 10px ${font.mono}`,
            whiteSpace: "nowrap",
          }}
        >
          {item.kind}
        </span>
        <div
          style={{
            marginTop: 6,
            font: `400 13px ${font.sans}`,
            color: unread ? color.ink : color.muted3,
            wordBreak: "break-word",
          }}
        >
          {item.body}
        </div>
        <div
          style={{
            marginTop: 6,
            font: `400 11px ${font.mono}`,
            color: color.muted2,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {item.source} · {relativeTime(item.created_at)}
        </div>
      </div>
    </div>
  );
}

function CenterState({
  title,
  detail,
  muted,
}: {
  title: string;
  detail: string;
  muted?: boolean;
}) {
  return (
    <div
      style={{
        minHeight: 280,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 9,
        padding: 24,
        textAlign: "center",
      }}
    >
      <span
        style={{
          width: 36,
          height: 36,
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: muted ? color.sunken : "#eef5f0",
          color: muted ? color.muted : color.green,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <Icon name="inbox" size={17} strokeWidth={1.7} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>{title}</div>
      <div
        style={{
          maxWidth: 360,
          font: `400 11.5px ${font.sans}`,
          color: color.muted2,
          lineHeight: 1.55,
        }}
      >
        {detail}
      </div>
    </div>
  );
}

export function InboxView() {
  const { state, actions } = useDucktape();
  const [kindDraft, setKindDraft] = useState("");
  const [bodyDraft, setBodyDraft] = useState("");
  const [kindFocus, setKindFocus] = useState(false);
  const [bodyFocus, setBodyFocus] = useState(false);
  const [sendHover, setSendHover] = useState(false);

  const loading = state.status === null;
  const backed = Boolean(state.status?.modules.some((m) => m.id === "inbox"));
  const writable = backed;
  const canSubmit = writable && kindDraft.trim().length > 0 && bodyDraft.trim().length > 0;
  const feed = [...state.inbox].reverse();

  const send = (event: FormEvent) => {
    event.preventDefault();
    if (!canSubmit) return;
    actions.deliverNotification({
      member: state.author,
      kind: kindDraft.trim(),
      body: bodyDraft.trim(),
    });
    setKindDraft("");
    setBodyDraft("");
  };

  return (
    <div
      data-screen-label="Inbox"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.paper,
      }}
    >
      <div
        style={{
          minHeight: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "0 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Inbox</span>
        <UnreadPill count={state.inboxUnread} />
        <div
          style={{
            marginLeft: "auto",
            display: "flex",
            alignItems: "center",
            justifyContent: "flex-end",
            gap: 7,
            flexWrap: "wrap",
          }}
        >
          <FinalizationMark op={state.ops[opKey.inbox()]} />
          <HeaderButton
            label="Mark all read"
            disabled={state.inboxUnread === 0}
            onClick={() => actions.markInboxRead()}
          />
          <HeaderButton
            label="Clear"
            disabled={state.inbox.length === 0}
            onClick={() => actions.clearInbox()}
          />
        </div>
      </div>

      <form
        onSubmit={send}
        style={{
          flexShrink: 0,
          display: "flex",
          alignItems: "flex-end",
          gap: 10,
          padding: "13px 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.sidebar,
        }}
      >
        <label
          htmlFor="inbox-kind"
          style={{
            width: 160,
            flexShrink: 0,
            display: "grid",
            gap: 6,
            font: `700 9px ${font.mono}`,
            letterSpacing: ".08em",
            color: writable ? color.muted2 : color.muted,
          }}
        >
          KIND
          <input
            id="inbox-kind"
            value={kindDraft}
            disabled={!writable}
            onChange={(event) => setKindDraft(event.target.value)}
            onFocus={() => setKindFocus(true)}
            onBlur={() => setKindFocus(false)}
            placeholder="reminder"
            style={{
              ...inputBase,
              borderColor: kindFocus ? accentVar : color.borderStrong,
              background: writable ? color.paper : color.sunken,
              color: writable ? color.ink : color.muted2,
            }}
          />
        </label>
        <label
          htmlFor="inbox-body"
          style={{
            flex: 1,
            minWidth: 0,
            display: "grid",
            gap: 6,
            font: `700 9px ${font.mono}`,
            letterSpacing: ".08em",
            color: writable ? color.muted2 : color.muted,
          }}
        >
          MESSAGE
          <input
            id="inbox-body"
            value={bodyDraft}
            disabled={!writable}
            onChange={(event) => setBodyDraft(event.target.value)}
            onFocus={() => setBodyFocus(true)}
            onBlur={() => setBodyFocus(false)}
            placeholder={loading ? "Loading notifications…" : "Notify yourself"}
            style={{
              ...inputBase,
              borderColor: bodyFocus ? accentVar : color.borderStrong,
              background: writable ? color.paper : color.sunken,
              color: writable ? color.ink : color.muted2,
            }}
          />
        </label>
        <button
          type="submit"
          aria-label="Send notification"
          disabled={!canSubmit}
          onMouseEnter={() => setSendHover(true)}
          onMouseLeave={() => setSendHover(false)}
          style={{
            all: "unset",
            boxSizing: "border-box",
            width: 36,
            height: 36,
            borderRadius: radius.sm,
            background: canSubmit ? (sendHover ? color.dark : accentVar) : color.chip,
            color: canSubmit ? color.paper : color.muted2,
            border: `1px solid ${canSubmit ? "transparent" : color.borderStrong}`,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
            cursor: canSubmit ? "pointer" : "default",
          }}
        >
          <Icon name="plus" size={15} strokeWidth={1.9} />
        </button>
      </form>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: 18, background: color.sidebar }}>
        <div
          style={{
            minHeight: "100%",
            borderRadius: radius.lg,
            border: `1px solid ${color.border}`,
            background: color.paper,
            boxShadow: shadow.card,
            overflow: "hidden",
          }}
        >
          {loading ? (
            <CenterState
              title="Loading notifications…"
              detail="Waiting for this node's inbox snapshot."
              muted
            />
          ) : !backed ? (
            <CenterState
              title="Inbox module is not available"
              detail="This node did not report an inbox module, so notification reads and writes are disabled."
              muted
            />
          ) : feed.length === 0 ? (
            <CenterState
              title="No notifications"
              detail="Notifications delivered to you will show up here."
            />
          ) : (
            feed.map((item) => (
              <NotificationRow
                key={item.seq}
                item={item}
                onOpen={(seq) => actions.markInboxReadTo(seq)}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
