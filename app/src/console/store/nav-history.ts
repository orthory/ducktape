// Browser-history navigation: the bridge between the store's nav slice and the
// webview's back/forward buttons.
//
// The console never touched the History API — the webview session held exactly
// one history entry, so mouse/keyboard back-forward were inert. This module
// keeps `window.history` in step with the store: every user-visible navigation
// (the Home layer, a screen switch, a channel/page selection, a mention's
// one-shot focus hand-off) becomes a history entry carrying a NavSnapshot, and
// traversal (popstate) re-applies that snapshot through the actions facade —
// whose entry points re-fetch the target's data from the node (enterChannel,
// enterPage, the open* hand-offs), so a restored surface renders the
// recentmost committed data, never a stale copy.
//
// Only pure pieces live here (state → snapshot projection, entry
// (de)serialization, the push/replace/none transition decision, the stack
// position the title bar's back/forward buttons enable from) so they are
// unit-testable without React. The provider owns the two side-effect wirings
// (nav-slice → history sync effect, the popstate listener); applying a
// snapshot lives in actions.ts (applyNavSnapshot) beside the rehydrating
// entry points it routes through.

import { docTabsScope } from "./state";
import type { ConsoleState, ViewMode } from "./state";

// ── Types ───────────────────────────────────────────────

/** The nav-relevant slice of ConsoleState one history entry preserves. `scope`
 *  pins intra-screen selections to the workspace/node that minted them (the
 *  docTabsScope idiom): a cross-scope entry still restores its surface, but
 *  never applies another workspace's channel/page/focus ids. */
export interface NavSnapshot {
  scope: string;
  atHome: boolean;
  screen: string;
  viewMode: ViewMode;
  channel: string | null;
  page: string | null;
  forgeRepo: string | null;
  forgeItem: number | null;
  explorer: number | null;
  agent: string | null;
  member: string | null;
}

export type NavTransition = "push" | "replace" | "none";

/** Intra-screen selection keys — everything below the surface triple. */
const SELECTION_KEYS = [
  "channel",
  "page",
  "forgeRepo",
  "forgeItem",
  "explorer",
  "agent",
  "member",
] as const;

// ── State → snapshot projection ─────────────────────────

export const navSnapshotOf = (state: ConsoleState): NavSnapshot => ({
  scope: docTabsScope(state.workspace?.id ?? null, state.nodeUrl),
  atHome: state.atHome,
  screen: state.screen,
  viewMode: state.viewMode,
  channel: state.activeChannel,
  page: state.activePage,
  // A pending forge hand-off names the repo ahead of the view's own stamp.
  forgeRepo: state.forgeFocus?.repo ?? state.forgeRepo,
  forgeItem: state.forgeFocus?.number ?? null,
  explorer: state.explorerFocus,
  agent: state.agentFocus,
  member: state.memberFocus,
});

/** One-shot focus hand-offs (explorer height, forge item, @agent, @member) are
 *  consumed and cleared by their views — the clear is not a navigation, so a
 *  snapshot taken after it inherits the entry's value for as long as the visit
 *  (same scope + screen) lasts. That keeps the entry restorable: traversing
 *  back to it re-issues the hand-off instead of landing on a blank focus. */
export const latchOneShots = (
  snap: NavSnapshot,
  entry: NavSnapshot | null,
): NavSnapshot =>
  entry && entry.scope === snap.scope && entry.screen === snap.screen
    ? {
        ...snap,
        forgeItem: snap.forgeItem ?? entry.forgeItem,
        explorer: snap.explorer ?? entry.explorer,
        agent: snap.agent ?? entry.agent,
        member: snap.member ?? entry.member,
      }
    : snap;

// ── Transition decision ─────────────────────────────────

export const sameNav = (a: NavSnapshot, b: NavSnapshot): boolean =>
  a.scope === b.scope &&
  a.atHome === b.atHome &&
  a.screen === b.screen &&
  a.viewMode === b.viewMode &&
  SELECTION_KEYS.every((key) => a[key] === b[key]);

/** What a nav-slice change means for the history stack. Surface moves (home,
 *  screen, rail) push; a selection appearing where the entry had none
 *  replaces — that is boot hydration (or a first pick) filling an empty slot,
 *  and stamping it in place keeps boot from minting phantom entries the back
 *  button would have to walk through. A scope change alone never pushes:
 *  scope guards what an entry may APPLY (see applyNavSnapshot), it is not a
 *  navigation of its own — workspace switches push through the Home flip and
 *  the selection moves they cause. */
export const navTransition = (
  next: NavSnapshot,
  entry: NavSnapshot | null,
): NavTransition => {
  if (!entry) return "replace";
  if (sameNav(next, entry)) return "none";
  const surfaceMoved =
    next.atHome !== entry.atHome ||
    next.screen !== entry.screen ||
    next.viewMode !== entry.viewMode;
  if (surfaceMoved) return "push";
  return SELECTION_KEYS.some(
    (key) => next[key] !== entry[key] && entry[key] !== null,
  )
    ? "push"
    : "replace";
};

// ── Stack position ──────────────────────────────────────

/** The store's picture of where the session sits in the webview's history
 *  stack — drives the title bar's back/forward enablement. `count` covers our
 *  own entries only (each carries its position, see NavEntry.i): a push
 *  truncates the forward tail, while a replace or a traversal can only ever
 *  reveal a deeper stack (e.g. a reload restoring a mid-stack entry). */
export interface NavStack {
  index: number;
  count: number;
}

/** The stack after a move that lands on position `at` (a push's `at` is one
 *  past the entry it left; a replace stays on the current one; a traversal
 *  lands wherever popstate says). Pure so the provider's history effects stay
 *  thin wiring. */
export const navStackAfter = (
  move: "push" | "replace" | "traverse",
  at: number,
  prev: NavStack,
): NavStack => {
  switch (move) {
    case "push":
      // pushing discards everything the webview held beyond the new entry
      return { index: at, count: at + 1 };
    case "replace":
    case "traverse":
      return { index: at, count: Math.max(prev.count, at + 1) };
  }
};

// ── history.state entries ───────────────────────────────

// Versioned marker: entries outlive reloads (and dev HMR), so anything read
// back is data from an arbitrary past build — never trust its shape unchecked.
// @2 added `i`, the entry's stack position.
const NAV_MARKER = "ducktape-nav@2";

interface NavEntry extends NavSnapshot {
  k: typeof NAV_MARKER;
  i: number;
}

export const stampNav = (snap: NavSnapshot, index: number): NavEntry => ({
  k: NAV_MARKER,
  i: index,
  ...snap,
});

const isStringOrNull = (v: unknown): v is string | null =>
  v === null || typeof v === "string";

const isNumberOrNull = (v: unknown): v is number | null =>
  v === null || typeof v === "number";

export const readNavEntry = (
  raw: unknown,
): { snap: NavSnapshot; index: number } | null => {
  if (!raw || typeof raw !== "object") return null;
  const e = raw as Record<string, unknown>;
  const valid =
    e.k === NAV_MARKER &&
    typeof e.i === "number" &&
    typeof e.scope === "string" &&
    typeof e.atHome === "boolean" &&
    typeof e.screen === "string" &&
    (e.viewMode === "user" || e.viewMode === "operator") &&
    isStringOrNull(e.channel) &&
    isStringOrNull(e.page) &&
    isStringOrNull(e.forgeRepo) &&
    isNumberOrNull(e.forgeItem) &&
    isNumberOrNull(e.explorer) &&
    isStringOrNull(e.agent) &&
    isStringOrNull(e.member);
  if (!valid) return null;
  return {
    index: e.i as number,
    snap: {
      scope: e.scope as string,
      atHome: e.atHome as boolean,
      screen: e.screen as string,
      viewMode: e.viewMode as ViewMode,
      channel: e.channel as string | null,
      page: e.page as string | null,
      forgeRepo: e.forgeRepo as string | null,
      forgeItem: e.forgeItem as number | null,
      explorer: e.explorer as number | null,
      agent: e.agent as string | null,
      member: e.member as string | null,
    },
  };
};
