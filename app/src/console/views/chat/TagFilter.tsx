// The chat surface's #tag affordances, all fed by the node's derived chat
// index (never canonical state): the channel header's tag dropdown (the
// discovery entry point, from the `Tags` catalog query), the dismissible
// filter bar, and the read-only hit list that replaces the live stream while
// a tag filter is active (the `tagSearch` query, newest first).

import { useEffect, useState } from "react";

import type { ChatSearchHit } from "../../../domain/chat-client";
import { useDucktape } from "../../store/use-ducktape";
import { resolveHitAuthor } from "../search/SearchModal";
import { wallClockMillisOf } from "../../../domain/wire";
import { HoverButton } from "./HoverButton";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

// Tag hits can be days old, so the row shows a date, unlike the stream's
// time-only gutter. Empty when the node's consensus_time isn't wall-clock.
const dateTimeOf = (stamp: number): string => {
  const ms = wallClockMillisOf(stamp);
  return ms === null
    ? ""
    : new Date(ms).toLocaleString([], {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
};

function HashGlyph({ size = 12 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round">
      <path d="M9 4L7 20M17 4l-2 16M4.5 9h16M3.5 15h16" />
    </svg>
  );
}

// ── Header dropdown (discovery entry point) ──────────────

/** A small "# Tags" header button; open, it lists the active channel's top
 *  tags with live counts — clicking one sets the tag filter. */
export function ChannelTagsButton() {
  const { state, actions } = useDucktape();
  const [open, setOpen] = useState(false);
  const { loadChannelTags } = actions;

  // Load fresh on every open; dismissal mirrors the message OverflowMenu —
  // Escape or an outside click (attached a tick late so the opening click
  // doesn't immediately close it).
  useEffect(() => {
    if (!open) return;
    loadChannelTags();
    const close = () => setOpen(false);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    const timer = setTimeout(() => document.addEventListener("click", close), 0);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("click", close);
      clearTimeout(timer);
    };
  }, [open, loadChannelTags]);

  return (
    <span style={{ position: "relative", display: "inline-flex" }}>
      <HoverButton
        title="Filter by tag"
        onClick={(event) => {
          event.stopPropagation();
          setOpen((wasOpen) => !wasOpen);
        }}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 4,
          padding: "2px 8px",
          borderRadius: 999,
          border: `1px solid ${color.borderSoft}`,
          background: state.tagFilter ? color.sunken : color.paper,
          font: `600 10px ${font.mono}`,
          color: color.muted,
          whiteSpace: "nowrap",
        }}
        hoverStyle={{ background: color.hover }}
      >
        <HashGlyph size={10} /> Tags
      </HoverButton>
      {open && (
        <div
          style={{
            position: "absolute",
            top: 26,
            left: 0,
            width: 220,
            maxHeight: 280,
            overflowY: "auto",
            zIndex: 5,
            background: color.paper,
            border: `1px solid ${color.borderSoft}`,
            borderRadius: radius.md,
            boxShadow: shadow.pop,
            padding: 4,
          }}
        >
          {state.channelTags.length === 0 && (
            <div style={{ padding: "7px 9px", font: `400 12px ${font.sans}`, color: color.muted2 }}>
              No tags in this channel yet.
            </div>
          )}
          {state.channelTags.map((row) => (
            <HoverButton
              key={row.tag}
              onClick={(event) => {
                event.stopPropagation();
                actions.setTagFilter(row.tag);
                setOpen(false);
              }}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                width: "100%",
                padding: "6px 9px",
                borderRadius: radius.sm,
                font: `400 12.5px ${font.sans}`,
                color: color.inkSoft,
              }}
              hoverStyle={{ background: color.hover }}
            >
              <span style={{ color: accentVar, fontWeight: 500, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1, textAlign: "left" }}>
                #{row.tag}
              </span>
              <span style={{ font: `500 10.5px ${font.mono}`, color: color.muted2, flexShrink: 0 }}>
                {row.count}
              </span>
            </HoverButton>
          ))}
        </div>
      )}
    </span>
  );
}

// ── Filter bar + hit list (the filtered reading mode) ────

/** "#tag — N messages ✕" above the pane while a filter is active. */
export function TagFilterBar() {
  const { state, actions } = useDucktape();
  const filter = state.tagFilter;
  if (!filter) return null;
  const summary = state.tagHitsPending
    ? "searching…"
    : `${state.tagHits.length} ${state.tagHits.length === 1 ? "message" : "messages"}`;
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
      <span style={{ font: `600 12.5px ${font.sans}`, color: accentVar, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        #{filter.tag}
      </span>
      <span style={{ font: `400 12px ${font.sans}`, color: color.muted2, whiteSpace: "nowrap" }}>
        — {summary}
      </span>
      <span style={{ flex: 1 }} />
      <HoverButton
        title="Clear tag filter"
        onClick={() => actions.clearTagFilter()}
        style={{
          width: 22,
          height: 22,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          borderRadius: 6,
          color: color.muted3,
          font: `500 13px ${font.sans}`,
        }}
        hoverStyle={{ background: color.hover, color: color.ink }}
      >
        ✕
      </HoverButton>
    </div>
  );
}

function TagHitRow({ hit, names }: { hit: ChatSearchHit; names: Record<string, string> }) {
  const when = dateTimeOf(hit.time);
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 2,
        padding: "8px 10px",
        borderRadius: radius.sm,
        minWidth: 0,
      }}
    >
      <span style={{ font: `500 10.5px ${font.sans}`, color: color.muted2 }}>
        {resolveHitAuthor(hit.author, names)}
        {when ? ` · ${when}` : ""}
        {hit.edited ? " · edited" : ""}
      </span>
      <span
        style={{
          font: `400 13.5px ${font.sans}`,
          lineHeight: 1.5,
          color: color.inkSofter,
          overflowWrap: "anywhere",
          wordBreak: "break-word",
          whiteSpace: "pre-wrap",
        }}
      >
        {hit.text}
      </span>
    </div>
  );
}

/** The filtered pane: the active tag's hits (newest first, read-only rows),
 *  rendered in place of the live message stream. */
export function TagHitList() {
  const { state } = useDucktape();
  const filter = state.tagFilter;
  if (!filter) return null;
  return (
    <div
      role="list"
      aria-label={`Messages tagged #${filter.tag}`}
      style={{
        flex: 1,
        minHeight: 0,
        minWidth: 0,
        overflowY: "auto",
        overflowX: "hidden",
        padding: "10px 18px 18px",
        boxSizing: "border-box",
      }}
    >
      {!state.tagHitsPending && state.tagHits.length === 0 && (
        <div style={{ padding: "14px 2px", font: `400 12.5px ${font.sans}`, color: color.muted2 }}>
          No messages carry #{filter.tag}.
        </div>
      )}
      {state.tagHits.map((hit) => (
        <div key={`${hit.channelId}/${hit.seq}`} role="listitem" style={{ minWidth: 0 }}>
          <TagHitRow hit={hit} names={state.authorNames} />
        </div>
      ))}
    </div>
  );
}
