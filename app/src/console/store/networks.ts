// Per-network seat model — the store's Discord-style rail shape (epic W1).
//
// A "seat" is one joined network the rail shows: the local networks from the
// ~/.ducktape registry (state.workspaces) plus, when a client connection is
// live, the badged remote seat. Under the single-active premise there is one
// LIVE node at a time, so the flat node projection in ConsoleState (channels,
// messages, members, …) belongs to the ACTIVE seat; the other seats carry only
// their identity. W4 promotes this to N live projections by swapping the
// connection layer — no projection field is sharded here (the ledger's "reuse
// the existing multi-workspace registry mechanics underneath").
//
// Terminology: user-facing "workspace" is "network"; the internal registry
// identifiers (state.workspace, Workspace, workspace_* commands) are NOT
// renamed — epic mandate.

import { isClientMode, type ConsoleState } from "./state";

export type SeatKind = "local" | "remote";

/** One network chip on the rail. `id` is the registry id for a local seat, or
 *  the dialed url for the remote seat (the connect layer's identity). */
export interface NetworkSeat {
  id: string;
  name: string;
  /** The network's chain id; "" for a remote seat (chain id is not known off
   *  the connection). `colorSeed` is what the chip color hashes. */
  chainId: string;
  kind: SeatKind;
  /** This is the network whose node is live right now (the single-active one). */
  active: boolean;
}

const REMOTE_SEAT_NAME = "Remote node";

/** The rail's seats, in join order (registry order) with the remote seat — when
 *  a direct client connection is live — pinned last. */
export const networksFrom = (
  state: Pick<ConsoleState, "workspaces" | "workspace" | "nodeUrl">,
): NetworkSeat[] => {
  const activeId = state.workspace?.id ?? null;
  const seats: NetworkSeat[] = state.workspaces.map((w) => ({
    id: w.id,
    name: w.name,
    chainId: w.chainId,
    kind: "local",
    active: w.id === activeId,
  }));
  // A live remote/client connection (no local active workspace, a dialed url)
  // gets its own badged seat — A6: it uses the node's public RPC, no control
  // chrome. It is the active seat while connected.
  if (isClientMode(state)) {
    seats.push({
      id: state.nodeUrl as string,
      name: REMOTE_SEAT_NAME,
      chainId: "",
      kind: "remote",
      active: true,
    });
  }
  return seats;
};

/** The live network seat, or null when nothing is connected (account home with
 *  no context). */
export const activeSeat = (
  state: Pick<ConsoleState, "workspaces" | "workspace" | "nodeUrl">,
): NetworkSeat | null => networksFrom(state).find((s) => s.active) ?? null;

/** Node control availability for one seat (ADR A5, interim form): a local
 *  seat whose daemon this app manages. A remote seat is never controllable
 *  (A6 — no control chrome for someone else's node). W2 replaces the body with
 *  `owner(BindNode) ∧ private-RPC reachable`; this is the one seam that moves.
 *  Under single-active only the active local seat is `managed`, so this is the
 *  per-seat rule `nodeControlAvailable` evaluates for the active seat. */
export const nodeControlForSeat = (kind: SeatKind, managed: boolean): boolean =>
  kind === "local" && managed;

// ── Chip identity (deterministic, no avatar feature) ────

/** First glyph of a network's name for its rail chip. */
export const seatInitial = (name: string): string =>
  (name.trim()[0] ?? "?").toUpperCase();

/** A stable color for a network chip, hashed from its chain id (falling back to
 *  the seat id for a remote seat with no chain id). Theme-INVARIANT on purpose:
 *  a colored identity chip has nothing to invert against (same reasoning as the
 *  video scrim token). djb2 over the seed → an evenly-spread hue. */
export const seatColor = (seat: Pick<NetworkSeat, "chainId" | "id">): string => {
  const seed = seat.chainId || seat.id;
  let hash = 5381;
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 33 + seed.charCodeAt(i)) >>> 0;
  }
  return `hsl(${hash % 360}, 55%, 45%)`;
};
