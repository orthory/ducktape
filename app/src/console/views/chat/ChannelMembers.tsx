// The members-only channel's member management surface: the header's lock pill,
// now clickable, opening a panel that lists the channel's current members (from
// the `Members` query into `state.channelMembers`) with a remove control each,
// and an add picker over the workspace roster (`state.nodeUsers`). Membership is
// what `check_post_policy` gates an external author's posts on — a members_only
// channel with an empty set locks EVERYONE out, so the panel warns when it is.
//
// Every add/remove is a tracked SetMembership op (actions.setChannelMembership),
// so each row carries its own FinalizationMark; the store refetches the set on
// settle. Own file so ChatView's header change stays a single pill+panel mount.

import { useEffect, useRef, useState } from "react";

import { authorName, keyBytes, keyHex } from "../../../domain/chat-client";
import type { Channel } from "../../../domain/chat-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { opKey } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";
import { HoverButton } from "./HoverButton";

// Local glyph — Icon.tsx isn't ours to extend (same pattern as the Huddle
// surface). Mirrors ChatView's retired inline lock so the pill reads the same.
function LockGlyph({ size = 10 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <rect x="5" y="11" width="14" height="9" rx="2" />
      <path d="M8 11V8a4 4 0 0 1 8 0v3" />
    </svg>
  );
}

/** One workspace user the add picker can grant. `hex` is the node key hex that
 *  keys `nodeUsers`/`authorNames` AND (as bytes) the member set. */
interface RosterUser {
  hex: string;
  label: string;
}

/** The clickable "Members only" pill and its management panel. Rendered in the
 *  channel header in place of the static lock pill, for a members_only channel
 *  only (the caller gates on post_policy). */
export function ChannelMembersButton({ channel }: { channel: Channel }) {
  const { state, actions } = useDucktape();
  const { refreshChannelMembers } = actions;
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);

  // Load the set fresh on open (the panel owns this slice — the per-block
  // refresh does not). Dismiss on Escape or a click outside the pill+panel;
  // an inside click keeps it open so the add/remove controls stay usable.
  useEffect(() => {
    if (!open) return;
    refreshChannelMembers(channel.id);
    const onDown = (event: MouseEvent) => {
      if (ref.current?.contains(event.target as Node)) return;
      setOpen(false);
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
  }, [open, channel.id, refreshChannelMembers]);

  const members = state.channelMembers;
  const memberSet = new Set(members.map(keyHex));
  // The add roster: every known workspace user (node key -> owning account) not
  // already a member, labelled by display name with a short-hex fallback.
  const roster: RosterUser[] = Object.entries(state.nodeUsers)
    .filter(([hex]) => !memberSet.has(hex))
    .map(([hex, user]) => ({
      hex,
      label: state.authorNames[hex] ?? user.name ?? `${hex.slice(0, 8)}…`,
    }))
    .sort((a, b) => a.label.localeCompare(b.label));

  return (
    <span ref={ref} style={{ position: "relative", display: "inline-flex", marginLeft: 2 }}>
      <HoverButton
        title="Manage channel members"
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
          background: open ? color.hover : color.sunken,
          border: `1px solid ${color.borderSoft}`,
          font: `600 10px ${font.mono}`,
          color: color.muted,
          whiteSpace: "nowrap",
        }}
        hoverStyle={{ background: color.hover, color: color.ink }}
      >
        <LockGlyph size={10} /> Members only
      </HoverButton>

      {open && (
        <div
          role="dialog"
          aria-label={`Members of #${channel.name}`}
          style={{
            position: "absolute",
            top: 28,
            left: 0,
            width: 250,
            maxHeight: 340,
            overflowY: "auto",
            zIndex: 6,
            background: color.paper,
            border: `1px solid ${color.borderSoft}`,
            borderRadius: radius.md,
            boxShadow: shadow.pop,
            padding: 4,
          }}
        >
          <div style={{ padding: "6px 9px 3px", font: `600 10px ${font.sans}`, color: color.muted, letterSpacing: ".04em" }}>
            MEMBERS · {members.length}
          </div>

          {members.length === 0 && (
            // A members_only channel with no members admits no external author —
            // the CreateChannel seed covers the creator, but a hand-cleared set
            // can still reach zero.
            <div style={{ padding: "3px 9px 7px", font: `400 11.5px ${font.sans}`, color: color.danger }}>
              Nobody can post — add a member.
            </div>
          )}

          {members.map((bytes) => {
            const hex = keyHex(bytes);
            return (
              <div
                key={hex}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  padding: "5px 6px 5px 9px",
                  borderRadius: radius.sm,
                }}
              >
                <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", font: `400 12.5px ${font.sans}`, color: color.inkSoft }}>
                  {authorName({ user: bytes }, state.authorNames)}
                </span>
                <FinalizationMark op={state.ops[opKey.membership(channel.id, hex)]} />
                <HoverButton
                  title="Remove from channel"
                  onClick={(event) => {
                    event.stopPropagation();
                    actions.setChannelMembership(channel.id, bytes, false);
                  }}
                  style={{ width: 20, height: 20, display: "flex", alignItems: "center", justifyContent: "center", borderRadius: 5, color: color.muted3, font: `500 12px ${font.sans}`, flexShrink: 0 }}
                  hoverStyle={{ background: color.hover, color: color.danger }}
                >
                  ✕
                </HoverButton>
              </div>
            );
          })}

          <div style={{ height: 1, background: color.borderSoft, margin: "5px 6px" }} />
          <div style={{ padding: "3px 9px", font: `600 10px ${font.sans}`, color: color.muted, letterSpacing: ".04em" }}>
            ADD MEMBER
          </div>
          {roster.length === 0 ? (
            <div style={{ padding: "3px 9px 7px", font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
              Everyone in the workspace is already a member.
            </div>
          ) : (
            roster.map((user) => (
              <HoverButton
                key={user.hex}
                onClick={(event) => {
                  event.stopPropagation();
                  actions.setChannelMembership(channel.id, keyBytes(user.hex), true);
                }}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  width: "100%",
                  padding: "5px 9px",
                  borderRadius: radius.sm,
                  font: `400 12.5px ${font.sans}`,
                  color: color.inkSoft,
                }}
                hoverStyle={{ background: color.hover }}
              >
                <span style={{ color: color.muted2, flexShrink: 0, font: `500 13px ${font.sans}` }}>+</span>
                <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", textAlign: "left" }}>
                  {user.label}
                </span>
              </HoverButton>
            ))
          )}
        </div>
      )}
    </span>
  );
}
