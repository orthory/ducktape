// The in-app notification surface: a bell in the title bar with the unread
// count, and a dropdown of the engine's recent ring. Opening the dropdown is
// what marks notifications seen (window focus deliberately does not — see
// docs/superpowers/specs/2026-07-12-notification-bell-design.md). Desktop-only:
// web builds have no notifier and render nothing.

import { useEffect, useRef, useState } from "react";

import { isTauri } from "../../domain/node-bootstrap";
import * as notifyClient from "../../domain/notify-client";
import type { NotifyItem } from "../../domain/notify-client";
import { parseItemChannelId } from "../../domain/forge-client";
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

const agoLabel = (at: number): string => {
  const seconds = Math.max(0, Math.floor((Date.now() - at) / 1000));
  if (seconds < 60) return "now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d`;
};

export function NotificationsBell() {
  const { actions } = useDucktape();
  const [items, setItems] = useState<NotifyItem[]>([]);
  const [unread, setUnread] = useState(0);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    const unlistens: Array<() => void> = [];
    void notifyClient.recent().then((initial) => {
      if (!cancelled) setItems(initial);
    });
    void notifyClient
      .onItem((item) => setItems((previous) => [item, ...previous].slice(0, RECENT_CAP)))
      .then((unlisten) => (cancelled ? unlisten() : unlistens.push(unlisten)));
    void notifyClient
      .onUnread(setUnread)
      .then((unlisten) => (cancelled ? unlisten() : unlistens.push(unlisten)));
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
    if (next) void notifyClient.markSeen();
  };

  const openItem = (item: NotifyItem) => {
    setOpen(false);
    if (item.channelId) {
      // A hidden forge-item channel is unroutable on the chat surface — the
      // provider's navigate listener makes the same detour.
      if (parseItemChannelId(item.channelId)) {
        actions.setScreen("forge");
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
          {items.length === 0 ? (
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
            items.map((item, index) => (
              <button
                key={`${item.at}-${index}`}
                onClick={() => openItem(item)}
                style={{
                  all: "unset",
                  boxSizing: "border-box",
                  cursor: "pointer",
                  display: "block",
                  width: "100%",
                  padding: "7px 9px",
                  borderRadius: radius.sm,
                }}
                onMouseEnter={(event) => {
                  event.currentTarget.style.background = color.sunken;
                }}
                onMouseLeave={(event) => {
                  event.currentTarget.style.background = "transparent";
                }}
              >
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
                    style={{
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {item.title}
                  </span>
                  <span
                    style={{
                      font: `500 9.5px ${font.mono}`,
                      color: color.muted2,
                      flexShrink: 0,
                    }}
                  >
                    {agoLabel(item.at)}
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
            ))
          )}
        </div>
      )}
    </div>
  );
}
