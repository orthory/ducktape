// Per-operation finalization tracking — the store half of the console's
// "preconfirmed render first, confirm inclusion separately" contract.
//
// Every consensus write begins an OpRecord under a stable ENTITY key (the row
// the operation touches), applies its optimistic projection immediately, and
// flips the record to finalized/failed when the node's submit receipt lands.
// Views look their entity's record up to draw the inline mark: a pending dot
// while in flight, a checkmark once included (hover reveals the inclusion
// height + the op's addressable hash). Nothing here is committed state — a
// node switch resets the ledger.

import type { MessageView } from "../../domain/chat-client";

export type OpPhase = "pending" | "finalized" | "failed";

export interface OpRecord {
  /** Insertion counter — prune order, and "which record is newer" for rows
   *  that can match more than one key (see `opForMessage`). */
  seq: number;
  phase: OpPhase;
  /** Wall-clock ms at submit — lets the provider ignore stale pendings. */
  startedAt: number;
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
  /** The local member's own profile (the Settings display-name row). */
  profile: () => "profile/self",
  channel: (channelId: string) => `chat/channel/${channelId}`,
  /** A NEW message — keyed by the client-minted message id (the committed row
   *  carries it in `head.message_id`, so the row matches after refresh too). */
  message: (channelId: string, messageId: string) =>
    `chat/${channelId}/id/${messageId}`,
  /** An op on an EXISTING message (edit/delete/reaction) — keyed by seq. */
  messageSeq: (channelId: string, seq: number) => `chat/${channelId}/seq/${seq}`,
  forgeHead: () => "forge/head",
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
  file: (fileId: string) => `file/${fileId}`,
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
): OpLedger => {
  const prev = ops[key];
  if (!prev) return ops;
  return {
    ...ops,
    [key]: {
      ...prev,
      phase: "finalized",
      height: receipt?.height,
      opHash: receipt?.opHash,
    },
  };
};

export const failOp = (ops: OpLedger, key: string, error: string): OpLedger => {
  const prev = ops[key];
  if (!prev) return ops;
  return { ...ops, [key]: { ...prev, phase: "failed", error } };
};

/** Any op still in flight and younger than OP_STALE_MS? While true, the
 *  provider skips block-stream refreshes so a mid-flight optimistic projection
 *  isn't clobbered by a re-query that predates the op's own commit — the op's
 *  completion refresh follows immediately anyway. */
export const hasFreshPending = (ops: OpLedger, now: number): boolean =>
  Object.values(ops).some(
    (op) => op.phase === "pending" && now - op.startedAt < OP_STALE_MS,
  );

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
