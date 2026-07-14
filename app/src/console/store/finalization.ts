// Per-operation finalization tracking — the store half of the console's
// "preconfirmed render first, confirm inclusion separately" contract.
//
// Every consensus write begins an OpRecord under a stable ENTITY key (the row
// the operation touches), applies its optimistic projection immediately, and
// flips the record to finalized/failed when the node's submit receipt lands.
// Views look their entity's record up to draw the inline mark: a single check
// while in flight (sent + preconfirmed render), a double check once included
// (clicking reveals the submit/confirm times, the inclusion height and the
// op's addressable hash). Nothing here is committed state — a node switch
// resets the ledger.

import type { MessageView } from "../../domain/chat-client";

export type OpPhase = "pending" | "finalized" | "failed";

export interface OpRecord {
  /** Insertion counter — prune order, and "which record is newer" for rows
   *  that can match more than one key (see `opForMessage`). */
  seq: number;
  phase: OpPhase;
  /** Wall-clock ms at submit — lets the provider ignore stale pendings. */
  startedAt: number;
  /** Wall-clock ms when the receipt (or rejection) landed — with `startedAt`,
   *  the mark's stats popover derives the confirm time and latency. */
  settledAt?: number;
  /** The op's inclusion height, once the receipt lands. */
  height?: number;
  /** The op's content address (64-hex sha256), when the node returns one. */
  opHash?: string;
  /** The rejection, when phase === "failed". */
  error?: string;
}

/** Entity key → the newest operation that touched that entity. */
export type OpLedger = Record<string, OpRecord>;

/** Ledger size cap — oldest SETTLED records beyond it are pruned on insert. */
const MAX_OPS = 512;

/** A pending older than this stops gating block-stream refreshes — the submit
 *  is presumed lost (the record itself stays until the op settles or prunes). */
export const OP_STALE_MS = 10_000;

// ── Entity keys ─────────────────────────────────────────
//
// The ONE naming scheme actions (writers) and views (readers) share. Keys name
// the entity a row renders, not the op kind: successive ops on the same row
// overwrite each other's record, so a row always shows its latest write.

export const opKey = {
  /** The local identity account's canonical display-name row. */
  accountName: () => "account/name/self",
  /** The local identity account's optional `.duck` name row. */
  duckHandle: () => "account/duck-handle/self",
  channel: (channelId: string) => `chat/channel/${channelId}`,
  /** A NEW message — keyed by the client-minted message id (the committed row
   *  carries it in `head.message_id`, so the row matches after refresh too). */
  message: (channelId: string, messageId: string) =>
    `chat/${channelId}/id/${messageId}`,
  /** An op on an EXISTING message (edit/delete) — keyed by seq. Reactions are
   *  deliberately NOT keyed here: they use `reaction()` so a reaction submit
   *  never paints a finalization mark on the message body it targets. */
  messageSeq: (channelId: string, seq: number) => `chat/${channelId}/seq/${seq}`,
  /** A reaction toggle — its own key so `opForMessage` never picks it up (the
   *  reaction chip is its own optimistic feedback). */
  reaction: (channelId: string, seq: number, emoji: string) =>
    `chat/${channelId}/seq/${seq}/react/${emoji}`,
  /** A join/leave of a channel's voice huddle — keyed by channel so the pill's
   *  optimistic roster change carries a finalization record. */
  huddle: (channelId: string) => `chat/huddle/${channelId}`,
  /** An add/remove of one channel member — keyed by channel + the target's
   *  hex user key, so each member row in the panel carries its own mark. */
  membership: (channelId: string, userHex: string) =>
    `chat/members/${channelId}/${userHex}`,
  forgeHead: () => "forge/head",
  /** An op on an EXISTING forge issue/PR (edit/state/merge/review) — keyed by
   *  the repo-scoped item number the row renders. */
  forgeItem: (repo: string, number: number) => `forge/${repo}/item/${number}`,
  /** An OpenIssue/OpenPr submit — the item number is minted by the module, so
   *  the mark anchors to the repo's tracker list instead (cf. runRequest). */
  forgeItemOpen: (repo: string) => `forge/${repo}/item/open`,
  page: (pageId: string) => `page/${pageId}`,
  /** Page block ids are module-global — no page qualifier needed. */
  pageBlock: (blockId: string) => `page-block/${blockId}`,
  agent: (agentId: string) => `agent/${agentId}`,
  watch: (channelId: string) => `agent/watch/${channelId}`,
  run: (runId: string) => `agent/run/${runId}`,
  /** A RequestRun submit — the run id is minted by the module, so the mark
   *  anchors to the requesting agent's run controls instead. */
  runRequest: (agentId: string) => `agent/run-request/${agentId}`,
  jobWorker: () => "agent/job-worker",
  proposal: (proposalId: string) => `governance/${proposalId}`,
  /** A membership ceremony (admit/promote/demote/removeResident/leave) — the
   *  ceremony mints its own proposal id, so the mark keys on the subject key. */
  govMembership: (subjectHex: string) => `governance/membership/${subjectHex}`,
  file: (fileId: string) => `file/${fileId}`,
  /** A comment write (edit/delete) — keyed by the comment id. */
  comment: (commentId: string) => `comment/${commentId}`,
  /** A thread write (add first comment / reply / resolve) — keyed by thread. */
  commentThread: (threadId: string) => `comment-thread/${threadId}`,
};

// ── Ledger transitions (pure) ───────────────────────────

export const beginOp = (
  ops: OpLedger,
  key: string,
  startedAt: number,
): OpLedger => {
  const seq = 1 + Object.values(ops).reduce((max, op) => Math.max(max, op.seq), 0);
  const next: OpLedger = { ...ops, [key]: { seq, phase: "pending", startedAt } };
  const keys = Object.keys(next);
  if (keys.length <= MAX_OPS) return next;
  // prune the oldest settled records; in-flight ops are never dropped.
  keys
    .filter((k) => next[k].phase !== "pending")
    .sort((a, b) => next[a].seq - next[b].seq)
    .slice(0, keys.length - MAX_OPS)
    .forEach((k) => delete next[k]);
  return next;
};

export const finalizeOp = (
  ops: OpLedger,
  key: string,
  receipt: { height: number; opHash?: string } | null,
  settledAt: number,
): OpLedger => {
  const prev = ops[key];
  if (!prev) return ops;
  return {
    ...ops,
    [key]: {
      ...prev,
      phase: "finalized",
      settledAt,
      height: receipt?.height,
      opHash: receipt?.opHash,
    },
  };
};

export const failOp = (
  ops: OpLedger,
  key: string,
  error: string,
  settledAt: number,
): OpLedger => {
  const prev = ops[key];
  if (!prev) return ops;
  return { ...ops, [key]: { ...prev, phase: "failed", settledAt, error } };
};

/** Any op still in flight and younger than OP_STALE_MS? While true, the
 *  provider skips block-stream refreshes so a mid-flight optimistic projection
 *  isn't clobbered by a re-query that predates the op's own commit — the op's
 *  completion refresh follows immediately anyway. */
export const hasFreshPending = (ops: OpLedger, now: number): boolean =>
  Object.values(ops).some(
    (op) => op.phase === "pending" && now - op.startedAt < OP_STALE_MS,
  );

/** The read-your-writes floor: the highest inclusion height among finalized
 *  ops. A snapshot whose `status.height` sits below it predates a write this
 *  console already holds a receipt for — applying it would un-render the
 *  confirmed row until a later refresh (the "message disappears and
 *  reappears" bug on nodes whose local fold trails the receipt's validator).
 *  Unbounded on purpose: on one honest node heights are monotonic, so the
 *  floor can never wedge hydration — and the ledger resets on node switches,
 *  which is what makes that safe across connections. */
export const receiptFloor = (ops: OpLedger): number =>
  Object.values(ops).reduce(
    (max, op) =>
      op.phase === "finalized" && op.height !== undefined
        ? Math.max(max, op.height)
        : max,
    0,
  );

/** Entity-key prefixes whose optimistic projections live in the pages slices
 *  (`pages`, `activePageBlocks`). */
const PAGE_KEY_PREFIXES = ["page/", "page-block/"] as const;

/** Is a snapshot whose fetch began at `fetchStartedAt` already superseded by
 *  page ops? True while any page-scoped op is in flight (a fresh pending —
 *  the snapshot cannot reflect it) or began after the fetch started (it may
 *  even have settled since, but this snapshot predates it). The holder never
 *  starves: every such op ends in its own completion/rollback refresh, whose
 *  later fetch clears both conditions. A stale pending (a presumed-lost
 *  submit) stops superseding, mirroring hasFreshPending. */
export const pageSnapshotSuperseded = (
  ops: OpLedger,
  fetchStartedAt: number,
  now: number,
): boolean =>
  Object.entries(ops).some(([key, op]) => {
    if (!PAGE_KEY_PREFIXES.some((prefix) => key.startsWith(prefix))) return false;
    return op.phase === "pending"
      ? now - op.startedAt < OP_STALE_MS
      : op.startedAt >= fetchStartedAt;
  });

// ── Reading the ledger ──────────────────────────────────

/** Pull the finalization facts out of whatever a client write resolved to.
 *  Domain clients type their writes as Promise<BlockEvent>, but the transport
 *  actually resolves the submit receipt; anything unshaped (a composite write
 *  that resolves to something else) settles the op without inclusion facts. */
export const receiptOf = (
  result: unknown,
): { height: number; opHash?: string } | null => {
  if (typeof result !== "object" || result === null) return null;
  const { height, opHash } = result as { height?: unknown; opHash?: unknown };
  if (typeof height !== "number") return null;
  return { height, opHash: typeof opHash === "string" ? opHash : undefined };
};

/** The op behind a content address — how hash-addressed surfaces (anything
 *  rendering a 64-hex sha256 rather than an entity) find their record. Only
 *  settled ops carry a hash (the receipt brings it), and the ledger is
 *  session-local: an address submitted elsewhere resolves to nothing. */
export const opByHash = (ops: OpLedger, opHash: string): OpRecord | undefined =>
  Object.values(ops)
    .filter((op) => op.opHash === opHash)
    .reduce<OpRecord | undefined>(
      (newest, op) => (newest === undefined || op.seq > newest.seq ? op : newest),
      undefined,
    );

/** The freshest op touching a message row: a new post keys by the minted
 *  message id, edits/deletes/reactions by committed seq — the row shows
 *  whichever record is newer. */
export const opForMessage = (
  ops: OpLedger,
  message: MessageView,
): OpRecord | undefined => {
  const byId = ops[opKey.message(message.channel_id, message.head.message_id)];
  const bySeq = ops[opKey.messageSeq(message.channel_id, message.seq)];
  if (byId && bySeq) return byId.seq >= bySeq.seq ? byId : bySeq;
  return byId ?? bySeq;
};
