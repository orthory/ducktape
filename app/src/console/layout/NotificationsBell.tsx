// The in-app notification surface: a bell in the title bar with the unread
// count, and a dropdown of the engine's recent ring, STACKED by target — items
// sharing a channel (or, when channel-less, a category) collapse into one row
// that expands. Opening the dropdown is what marks notifications seen (window
// focus deliberately does not — see
// docs/superpowers/specs/2026-07-12-notification-bell-design.md). Desktop-only:
// web builds have no notifier and render nothing.

import { type MouseEvent as ReactMouseEvent, useEffect, useRef, useState } from "react";

import { isTauri } from "../../domain/node-bootstrap";
import * as notifyClient from "../../domain/notify-client";
import type { NotifyItem } from "../../domain/notify-client";
import { parseItemChannelId } from "../../domain/forge-client";
import { relTime } from "../views/forge/ui";
import { Icon } from "../components/Icon";
import { accentVar, color, font, radius } from "../theme/tokens";
import { useDucktape } from "../store/use-ducktape";

/// Mirrors the engine's ring cap so a long-lived webview doesn't outgrow it.
const RECENT_CAP = 50;

// Category → rail screen when the item carries no channel. Thread-level
// precision is a known ceiling: items carry only a channel (the deep-link
// target machinery stays deleted — see notify/present.rs).
const FALLBACK_SCREEN: Record<NotifyItem["category"], string> = {
  mention: "chat",
  reply: "chat",
  huddle: "chat",
  run: "agent",
  forge: "forge",
  governance: "governance",
};

// One item's identity for the boot merge: `at` is engine-stamped epoch millis,
// near-unique; the title breaks the same-millisecond tie.
const itemKey = (item: NotifyItem): string => `${item.at}:${item.title}`;

// Channel-less items (runs, forge, governance) stack under their category.
const CATEGORY_LABEL: Record<NotifyItem["category"], string> = {
  mention: "Mentions",
  reply: "Replies",
  huddle: "Huddles",
  run: "Agent runs",
  forge: "Forge",
  governance: "Governance",
};

/** A stack of notifications sharing one target: a channel, or a category when
 *  the items carry no channel. */
interface Group {
  key: string;
  label: string;
  items: NotifyItem[];
}

const groupLabel = (item: NotifyItem, channelName: (id: string) => string): string => {
  if (!item.channelId) return CATEGORY_LABEL[item.category];
  const forgeItem = parseItemChannelId(item.channelId);
  if (forgeItem) return `${forgeItem.repo} #${forgeItem.number}`;
  return `#${channelName(item.channelId)}`;
};

// `items` is newest-first, so first-seen order sorts the groups by their newest
// item — the same order the flat list had.
const groupItems = (
  items: NotifyItem[],
  channelName: (id: string) => string,
): Group[] => {
  const groups = new Map<string, Group>();
  for (const item of items) {
    const key = item.channelId ?? `category:${item.category}`;
    const group = groups.get(key);
    if (group) group.items.push(item);
    else groups.set(key, { key, label: groupLabel(item, channelName), items: [item] });
  }
  return [...groups.values()];
};

// Deliberately NOT "N new": the ring keeps its items after they are seen, so a
// reopened dropdown would keep claiming newness beside a cleared badge.
const stackSummary = (group: Group): string => {
  const chat = (item: NotifyItem) =>
    item.category === "mention" || item.category === "reply";
  const noun = group.items.every(chat) ? "messages" : "updates";
  return `${group.items.length} ${noun}`;
};

export function NotificationsBell() {
  const { state, actions } = useDucktape();
  const [items, setItems] = useState<NotifyItem[]>([]);
  const [unread, setUnread] = useState(0);
  const [open, setOpen] = useState(false);
  // One stack expanded at a time — the dropdown is 320px wide and 400px tall.
  const [expanded, setExpanded] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    let sawLiveUnread = false;
    const unlistens: Array<() => void> = [];
    void notifyClient
      .onItem((item) => setItems((previous) => [item, ...previous].slice(0, RECENT_CAP)))
      .then((unlisten) => (cancelled ? unlisten() : unlistens.push(unlisten)));
    void notifyClient
      .onUnread((unread) => {
        sawLiveUnread = true;
        setUnread(unread);
      })
      .then((unlisten) => (cancelled ? unlisten() : unlistens.push(unlisten)));
    // The snapshot is the boot BASELINE — it carries the unread the engine
    // badged before this webview mounted. Live events can land before its IPC
    // resolves, so it merges UNDER any prepends (never overwrites them), and
    // it never clobbers an unread a live event (or mark-seen) already set.
    void notifyClient.recent().then((snapshot) => {
      if (cancelled) return;
      setItems((previous) => {
        const known = new Set(snapshot.items.map(itemKey));
        const fresh = previous.filter((item) => !known.has(itemKey(item)));
        return [...fresh, ...snapshot.items].slice(0, RECENT_CAP);
      });
      if (!sawLiveUnread) setUnread(snapshot.unread);
    });
    return () => {
      cancelled = true;
      unlistens.forEach((unlisten) => unlisten());
    };
  }, []);

  // Click-outside / Escape close.
  useEffect(() => {
    if (!open) return;
    const onDown = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  if (!isTauri()) return null;

  const toggle = () => {
    const next = !open;
    setOpen(next);
    setExpanded(null);
    if (next) void notifyClient.markSeen();
  };

  const channelName = (id: string): string =>
    state.channels.find((channel) => channel.id === id)?.name ?? id;
  const groups = groupItems(items, channelName);

  const openItem = (item: NotifyItem) => {
    setOpen(false);
    if (item.channelId) {
      // A hidden forge-item channel is unroutable on the chat surface — jump
      // to the item itself (the provider's navigate listener makes the same
      // detour for toast deep-links).
      const forgeItem = parseItemChannelId(item.channelId);
      if (forgeItem) {
        actions.openForgeItem(forgeItem.repo, forgeItem.number);
        return;
      }
      actions.setScreen("chat");
      actions.selectChannel(item.channelId);
      return;
    }
    actions.setScreen(FALLBACK_SCREEN[item.category]);
  };

  return (
    <div ref={rootRef} style={{ position: "relative", flexShrink: 0 }}>
      <button
        onClick={toggle}
        aria-label="Notifications"
        title="Notifications"
        style={{
          all: "unset",
          boxSizing: "border-box",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          gap: 4,
          padding: "3px 5px",
          borderRadius: radius.sm,
          color: unread > 0 ? color.ink : color.iconIdle,
        }}
      >
        <Icon name="bell" size={15} />
        {unread > 0 && (
          <span
            style={{
              font: `600 8.5px ${font.mono}`,
              color: "#fff",
              background: accentVar,
              borderRadius: 8,
              padding: "1px 5px",
            }}
          >
            {unread > 99 ? "99+" : unread}
          </span>
        )}
      </button>
      {open && (
        <div
          role="menu"
          aria-label="Recent notifications"
          style={{
            // Fixed, not absolute: the title bar's right cell clips overflow
            // (its status text must ellipsize), so an absolute panel inside it
            // would be cut to the bar's 44px. Anchored under the bar instead.
            position: "fixed",
            top: 48,
            right: 13,
            width: 320,
            maxHeight: 400,
            overflowY: "auto",
            background: color.canvas,
            border: `1px solid ${color.border}`,
            borderRadius: radius.md,
            boxShadow: "0 8px 24px rgba(0,0,0,.18)",
            zIndex: 40,
            padding: 4,
          }}
        >
          {groups.length === 0 ? (
            <div
              style={{
                padding: "18px 12px",
                textAlign: "center",
                font: `500 11px ${font.sans}`,
                color: color.muted,
              }}
            >
              No notifications
            </div>
          ) : (
            groups.map((group) =>
              group.items.length === 1 ? (
                <ItemRow
                  key={group.key}
                  item={group.items[0]}
                  onClick={() => openItem(group.items[0])}
                />
              ) : (
                <div key={group.key}>
                  <StackRow
                    group={group}
                    expanded={expanded === group.key}
                    onClick={() =>
                      setExpanded((current) => (current === group.key ? null : group.key))
                    }
                  />
                  {expanded === group.key && (
                    <div style={{ paddingLeft: 12 }}>
                      {group.items.map((item, index) => (
                        <ItemRow
                          key={`${item.at}-${index}`}
                          item={item}
                          onClick={() => openItem(item)}
                        />
                      ))}
                    </div>
                  )}
                </div>
              ),
            )
          )}
        </div>
      )}
    </div>
  );
}

const ROW_STYLE = {
  all: "unset",
  boxSizing: "border-box",
  cursor: "pointer",
  display: "block",
  width: "100%",
  padding: "7px 9px",
  borderRadius: radius.sm,
} as const;

const hover = {
  onMouseEnter: (event: ReactMouseEvent<HTMLButtonElement>) => {
    event.currentTarget.style.background = color.sunken;
  },
  onMouseLeave: (event: ReactMouseEvent<HTMLButtonElement>) => {
    event.currentTarget.style.background = "transparent";
  },
};

/** Title + relative time on one line, clamped body under it. */
function ItemRow({ item, onClick }: { item: NotifyItem; onClick: () => void }) {
  return (
    <button onClick={onClick} style={ROW_STYLE} {...hover}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 8,
          font: `600 11px ${font.sans}`,
          color: color.ink,
        }}
      >
        <span
          style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
        >
          {item.title}
        </span>
        <span
          style={{ font: `500 9.5px ${font.mono}`, color: color.muted2, flexShrink: 0 }}
        >
          {relTime(item.at)}
        </span>
      </div>
      <div
        style={{
          marginTop: 2,
          font: `400 10.5px ${font.sans}`,
          color: color.muted,
          display: "-webkit-box",
          WebkitLineClamp: 2,
          WebkitBoxOrient: "vertical",
          overflow: "hidden",
        }}
      >
        {item.body}
      </div>
    </button>
  );
}

/** The collapsed head of a stack: target, newest time, and how many are behind it. */
function StackRow({
  group,
  expanded,
  onClick,
}: {
  group: Group;
  expanded: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      aria-expanded={expanded}
      style={ROW_STYLE}
      {...hover}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          font: `600 11px ${font.sans}`,
          color: color.ink,
        }}
      >
        <span
          style={{
            display: "flex",
            color: color.muted2,
            transform: expanded ? "rotate(90deg)" : "none",
          }}
        >
          <Icon name="chevronRight" size={11} />
        </span>
        <span
          style={{
            flex: 1,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {group.label}
        </span>
        <span style={{ font: `500 9.5px ${font.mono}`, color: color.muted2 }}>
          {relTime(group.items[0].at)}
        </span>
      </div>
      <div
        style={{
          marginTop: 2,
          marginLeft: 17,
          font: `400 10.5px ${font.sans}`,
          color: color.muted,
        }}
      >
        {stackSummary(group)}
      </div>
    </button>
  );
}
