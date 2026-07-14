// The far-left network rail (epic W1) — the Discord-style switcher that frames
// the whole app: the "me" (account home) chip on top, one chip per joined
// network below in join order (no drag-reorder), and a "+" that opens the
// connect panel. Chips are the network initial on a color hashed from the chain
// id. The active network highlights; a remote/client seat wears a badge and no
// control chrome (A6). Layout is rail | module nav (Sidebar) | content.

import { useState } from "react";

import { isDesktop } from "../../domain/workspace-client";
import { networksFrom, seatColor, seatInitial, type NetworkSeat } from "../store/networks";
import { useDucktape } from "../store/use-ducktape";
import { color, font } from "../theme/tokens";
import { initialsOf } from "../views/home/ProfileCard";

/** The rail's fixed width. */
export const NETWORK_RAIL_WIDTH = 62;

const CHIP = 40;

/** The active/hover left indicator pill (Discord idiom): tall when active, a
 *  dot on hover, gone otherwise. */
function Indicator({ active, hovered }: { active: boolean; hovered: boolean }) {
  const height = active ? 26 : hovered ? 12 : 0;
  return (
    <span
      aria-hidden="true"
      style={{
        position: "absolute",
        left: 0,
        top: "50%",
        transform: "translateY(-50%)",
        width: 4,
        height,
        borderRadius: "0 4px 4px 0",
        background: color.ink,
        transition: "height .14s ease",
      }}
    />
  );
}

function RailButton({
  active,
  title,
  onClick,
  children,
}: {
  active: boolean;
  title: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <div style={{ position: "relative", width: "100%", display: "flex", justifyContent: "center" }}>
      <button
        title={title}
        aria-label={title}
        aria-current={active || undefined}
        onClick={onClick}
        style={{
          all: "unset",
          cursor: "pointer",
          position: "relative",
          boxSizing: "border-box",
          width: CHIP,
          height: CHIP,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          // active is a rounded square (squircle), idle is a circle — the
          // Discord "settle" without a hover animation library.
          borderRadius: active ? 13 : "50%",
          transition: "border-radius .14s ease",
        }}
      >
        {children}
      </button>
    </div>
  );
}

function MeChip() {
  const { state, actions } = useDucktape();
  return (
    <RailButton active={state.atHome} title="Account home" onClick={() => actions.goHome()}>
      <span
        style={{
          width: CHIP,
          height: CHIP,
          borderRadius: "inherit",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: state.atHome ? color.dark : color.iconIdle,
          color: state.atHome ? color.onDark : color.muted3,
          font: `600 13px ${font.sans}`,
        }}
      >
        {initialsOf(state.author)}
      </span>
    </RailButton>
  );
}

function NetworkChip({
  seat,
  hovered,
  onHover,
  onClick,
}: {
  seat: NetworkSeat;
  hovered: boolean;
  onHover: (id: string | null) => void;
  onClick: () => void;
}) {
  return (
    <div
      onMouseEnter={() => onHover(seat.id)}
      onMouseLeave={() => onHover(null)}
      style={{ position: "relative", width: "100%", display: "flex", justifyContent: "center" }}
    >
      <Indicator active={seat.active} hovered={hovered} />
      <RailButton
        active={seat.active}
        title={seat.kind === "remote" ? `${seat.name} (remote)` : seat.name}
        onClick={onClick}
      >
        <span
          style={{
            width: CHIP,
            height: CHIP,
            borderRadius: "inherit",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: seatColor(seat),
            color: "#ffffff",
            font: `600 15px ${font.sans}`,
            boxShadow: seat.active ? `0 0 0 2px ${color.canvas}, 0 0 0 4px ${color.ink}` : "none",
          }}
        >
          {seatInitial(seat.name)}
        </span>
        {seat.kind === "remote" && (
          // a small badge marks the seat as someone else's node reached over
          // the network — no control chrome hangs off it (A6).
          <span
            aria-hidden="true"
            title="Remote connection"
            style={{
              position: "absolute",
              right: -1,
              bottom: -1,
              width: 15,
              height: 15,
              borderRadius: "50%",
              background: color.blue,
              color: "#ffffff",
              border: `2px solid ${color.sidebar}`,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              font: `700 8px ${font.mono}`,
            }}
          >
            ↗
          </span>
        )}
      </RailButton>
    </div>
  );
}

export function NetworkRail() {
  const { state, actions } = useDucktape();
  const [hover, setHover] = useState<string | null>(null);
  const seats = networksFrom(state);

  const enter = (seat: NetworkSeat) => {
    if (seat.kind === "remote") {
      // the remote connection is already the live one; clicking it just drops
      // the Home layer back to its shell.
      if (state.atHome) actions.setScreen(state.screen);
      return;
    }
    // selectWorkspace is the single node-swap: re-entering the active member
    // network only drops Home; another connects its node (single-active).
    actions.selectWorkspace(seat.id);
  };

  return (
    <div
      data-rail="networks"
      style={{
        width: NETWORK_RAIL_WIDTH,
        flexShrink: 0,
        borderRight: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        padding: "10px 0",
        gap: 8,
        overflowY: "auto",
      }}
    >
      <MeChip />

      <span
        aria-hidden="true"
        style={{ width: 26, height: 1, background: color.border, flexShrink: 0 }}
      />

      {seats.map((seat) => (
        <NetworkChip
          key={seat.id}
          seat={seat}
          hovered={hover === seat.id}
          onHover={setHover}
          onClick={() => enter(seat)}
        />
      ))}

      {/* Minting a local network needs the desktop registry; web has only its
          configured node (the remote seat). */}
      {isDesktop() && (
        <RailButton active={false} title="Add a network" onClick={() => actions.newWorkspace()}>
          <span
            style={{
              width: CHIP,
              height: CHIP,
              borderRadius: "inherit",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              background: color.sunken,
              border: `1px dashed ${color.borderStrong}`,
              color: color.muted,
              font: `400 20px ${font.sans}`,
              lineHeight: 1,
            }}
          >
            +
          </span>
        </RailButton>
      )}
    </div>
  );
}