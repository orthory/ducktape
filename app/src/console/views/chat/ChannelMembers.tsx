// The members-only channel's member management surface: the header's lock pill,
// now clickable, opening a panel that lists the channel's current members (from
// the `Members` query into `state.channelMembers`) with a remove control each,
// and an add picker over the workspace roster (`state.nodeUsers`). Membership is
// what `check_post_policy` gates an external author's posts on — a members_only
// channel with an empty set locks EVERYONE out, so the panel warns when it is.
//
// Every add/remove is a tracked SetMembership op (actions.setChannelMembership),
// so each row carries its own FinalizationMark — and the op's optimistic
// projection puts the row there from the click, before the settle refetch.
// Own file so ChatView's header change stays a single pill+panel mount.
//
// The member set is keyed by NODE key, not by account: `check_post_policy` gates
// posting on the posting node's pubkey, so an account posting from three devices
// needs all three node keys in the set. The panel therefore labels every row
// with its node tag and says so — three rows reading "Jess" is how an operator
// adds the wrong device and silently fails to admit her.

import { useEffect, useRef, useState } from "react";

import { authorName, keyBytes, keyHex } from "../../../domain/chat-client";
import type { Channel } from "../../../domain/chat-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { hasFreshPending, opKey } from "../../store/finalization";
import { selfAuthorBytes } from "../../store/state";
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

/** One workspace NODE the picker can grant. `hex` is the node key hex that keys
 *  `nodeUsers`/`authorNames` AND (as bytes) the member set — one entry per
 *  device, so an account with three nodes has three rows here. */
interface RosterNode {
  hex: string;
  name: string;
  member: boolean;
  self: boolean;
}

/** The node-key disambiguator every row carries beside the account name. */
const nodeTag = (hex: string): string => hex.slice(0, 6);

/** A row's label: whose device it is, and WHICH device. */
function RowLabel({ name, hex, self }: { name: string; hex: string; self: boolean }) {
  return (
    <span style={{ flex: 1, minWidth: 0, display: "flex", alignItems: "baseline", gap: 5, overflow: "hidden" }}>
      <span style={{ minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", font: `400 12.5px ${font.sans}`, color: color.inkSoft }}>
        {name}
      </span>
      <span style={{ flexShrink: 0, font: `400 9.5px ${font.mono}`, color: color.muted3 }}>
        {nodeTag(hex)}
        {self && " · this device"}
      </span>
    </span>
  );
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

  // A membership change made on ANOTHER node moves no slice this console reads
  // (hydration deliberately does not carry `channelMembers`), so an open panel
  // would sit stale until reopened. Re-pull per finalized block instead — the
  // same `state.lastBlock` hook the forge discussion and upgrade panels use.
  // Held while an op is in flight: a fetch that predates our own commit would
  // drop the optimistic row out from under its mark, and the op's own settle
  // refetch (submitMembership) follows immediately anyway.
  useEffect(() => {
    if (!open || hasFreshPending(state.ops, Date.now())) return;
    refreshChannelMembers(channel.id);
  }, [open, channel.id, state.lastBlock, state.ops, refreshChannelMembers]);

  const members = state.channelMembers;
  const memberSet = new Set(members.map(keyHex));
  const selfHex = keyHex(selfAuthorBytes(state.status, state.author));
  // The roster: every known workspace NODE (node key -> owning account), member
  // or not, labelled by display name (short-hex fallback) plus its node tag.
  // Already-added devices stay listed and marked — that is what makes a member's
  // OTHER, unadmitted devices visible instead of silently absent.
  const roster: RosterNode[] = Object.entries(state.nodeUsers)
    .map(([hex, user]) => ({
      hex,
      name: state.authorNames[hex] ?? user.name ?? `${hex.slice(0, 8)}…`,
      member: memberSet.has(hex),
      self: hex === selfHex,
    }))
    .sort((a, b) => a.name.localeCompare(b.name) || a.hex.localeCompare(b.hex));

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

          <div style={{ padding: "0 9px 6px", font: `400 11px ${font.sans}`, color: color.muted2, lineHeight: 1.35 }}>
            Posting is gated per node — every device a member posts from must be
            added separately.
          </div>

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
                <RowLabel
                  name={authorName({ user: bytes }, state.authorNames)}
                  hex={hex}
                  self={hex === selfHex}
                />
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
            WORKSPACE DEVICES
          </div>
          {roster.length === 0 ? (
            <div style={{ padding: "3px 9px 7px", font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
              No workspace devices are known yet.
            </div>
          ) : (
            // An added device stays on the list, marked and inert: a member's
            // OTHER devices are only visible as "unadmitted" next to it, and
            // removal belongs to the members list above (one control per row).
            roster.map((node) =>
              node.member ? (
                <div
                  key={node.hex}
                  aria-label={`${node.name} ${nodeTag(node.hex)} — added`}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    padding: "5px 9px",
                    borderRadius: radius.sm,
                    background: color.sunken,
                    opacity: 0.72,
                  }}
                >
                  <span style={{ color: color.green, flexShrink: 0, font: `500 12px ${font.sans}` }}>✓</span>
                  <RowLabel name={node.name} hex={node.hex} self={node.self} />
                </div>
              ) : (
                <HoverButton
                  key={node.hex}
                  title={`Add ${node.name}'s device ${nodeTag(node.hex)}`}
                  onClick={(event) => {
                    event.stopPropagation();
                    actions.setChannelMembership(channel.id, keyBytes(node.hex), true);
                  }}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    width: "100%",
                    padding: "5px 9px",
                    borderRadius: radius.sm,
                  }}
                  hoverStyle={{ background: color.hover }}
                >
                  <span style={{ color: color.muted2, flexShrink: 0, font: `500 13px ${font.sans}` }}>+</span>
                  <RowLabel name={node.name} hex={node.hex} self={node.self} />
                </HoverButton>
              ),
            )
          )}
        </div>
      )}
    </span>
  );
}
