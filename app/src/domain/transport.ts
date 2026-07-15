// The node transport — how every build of the app talks to a ducktape node.
//
// There is exactly ONE data plane now: the daemon's http/ws surface
// (`ducktape-noded`). The web build dials it directly; the desktop build
// spawns the daemon as a detached subprocess and then talks to it the same
// way. Which URL to dial — and whether a daemon must be spawned first — is
// node-bootstrap.ts's job; this module only speaks the wire.
//
// Wire casing is the node's, verbatim: module payloads/replies use PascalCase
// enum variants + snake_case fields (serde defaults of the `*-interface`
// crates); the daemon envelope itself (appHash, height, version) is camelCase.

// ── Types ───────────────────────────────────────────────

import type { ClientMsg, ServerFrame, StreamErrorCode } from "./stream.gen";
import type {
  EventFrame,
  HeartbeatFrame,
  TailFrame,
  TermClientMsg,
} from "./stream";
import {
  isErrorFrame,
  isEventFrame,
  isHeartbeatFrame,
  isLaggedFrame,
  isServerFrame,
  isSubscribedFrame,
  isTailFrame,
  isTermChunkFrame,
  isTermCommandLogFrame,
} from "./stream";

export interface BlockEvent {
  height: number;
  appHash: string;
}

/** What `/v1/submit` resolves to: the block that INCLUDED the op, plus the
 *  op's content address — sha256 of the committed payload bytes, fetchable
 *  back via the blob lane (`GET /v1/files/blob/{opHash}`). Optional on the
 *  type because a node built before receipts shipped replies without it; the
 *  ui then shows the inclusion height alone. */
export interface SubmitReceipt extends BlockEvent {
  opHash?: string;
}

/** How the app groups a module in the Modules view. The node attaches this by
 *  id in its status catalog; it is presentation metadata only, never consensus
 *  identity. Optional: a node built before categories shipped omits it, and the
 *  view treats an absent/unknown value as `system`. */
export type ModuleCategory = "workspace" | "developer" | "automation" | "system";

export interface ModuleStatus {
  id: string;
  root: string;
  category?: ModuleCategory;
}

export interface NodeStatus {
  version: string;
  appHash: string;
  height: number;
  modules: ModuleStatus[];
  /** This node's mesh identity as 64-char hex — the voice fan-out address and
   *  the `node` key a join_huddle op carries. Empty string / absent on a legacy
   *  daemon that can't do voice; the ui hides every huddle affordance then. */
  publicKey?: string;
}

// ── Blocks (explorer) ───────────────────────────────────
//
// The explorer plane: one record per finalized block. Post tx-aggregation a
// block carries EVERY op drained from its ~2s window (no longer one-op-per-
// block), so a record holds an `ops` array instead of singular op fields; an
// idle window commits an empty-`ops` heartbeat nop. Pulled via GET /v1/blocks;
// a node without the surface reads as "no blocks".

/** One dispatch in an op's drain — a module ran, triggered by `origin`. */
export interface DispatchInfo {
  module: string;
  /** `"external"`, `"external:<name>"`, `"system"`, or `"module:<id>"`. */
  origin: string;
  emittedMsgs: number;
  emittedEvents: number;
}

/** How an op landed: an applied op mutated state; a rejected op finalized but
 *  rolled back — a failed tx. */
export type BlockDisposition = "applied" | "rejected";

/** One aggregated op inside a block — the "transaction" the explorer renders a
 *  row for. A block holds these in drain order; a block that aggregated its
 *  whole 2s window carries one entry per included op. */
export interface RootOp {
  /** Hex of the op author's origin — the submitter's key, or `"system"` /
   *  `"module:<id>"` for a module/system-triggered op. (On a frame-signing
   *  lane this is the op's own author, not the block's proposing validator —
   *  the aggregated record no longer carries a single block-level signer.) */
  proposer: string;
  disposition: BlockDisposition;
  /** This op's target module. */
  target: string;
  /** This op's dispatch trace, in drain order — the modules it triggered.
   *  Empty for a rejected op (a deterministic no-op leaves no trace). */
  operations: DispatchInfo[];
  /** Capped utf-8 preview of this op's payload (module `*Msg` json). */
  payload: string;
  /** Hex content address of this op — sha256 of the committed payload bytes,
   *  fetchable via the blob lane (`GET /v1/files/blob/{opHash}`). */
  opHash: string;
}

export interface BlockRecord {
  height: number;
  /** Hex content hash of the block's frame — the block's hash. */
  hash: string;
  /** Hex app-hash after this block settled — the commit. */
  commitHash: string;
  /** Every op aggregated into this block, in drain order. Empty for an idle
   *  window (a heartbeat nop): the block committed but carried no ops. */
  ops: RootOp[];
}

// ── duckfs (the `files` module's CoW filesystem) ────────
//
// Wire shapes for the daemon's `/v1/files/*` surface. duckfs replaced the old
// CAS manifest plane with a path-addressed, snapshot-versioned filesystem, so
// these are the exact json the noded `files_*` handlers emit/accept: snake_case
// like the module wire, except the commit reply, which is the camelCase block
// envelope (BlockEvent). files-client.ts re-exports these and layers the
// operations + consensus caps on top — one home for the whole files plane.

export type FileEntryKind = "file" | "dir" | "symlink";

/** One directory entry / stat result — the module's `EntryInfo`. `object` is
 *  the content object id (64-char hex); `meta` is a free-form string map. */
export interface FileEntry {
  path: string;
  kind: FileEntryKind;
  size: number;
  exec: boolean;
  object: string;
  meta: Record<string, string>;
}

/** One commit in the bounded history window — the module's `SnapshotInfo`. */
export interface FileSnapshot {
  id: string;
  parent: string | null;
  root_tree: string;
  author: string;
  height: number;
  consensus_time: number;
  message: string;
}

/** The refs image — live head, named pins, and the history window length
 *  (`RefsInfo`). `head` is null on an empty filesystem (no commits yet). */
export interface FileRefs {
  head: string | null;
  pins: Record<string, string>;
  window_len: number;
}

export type FileDiffKind = "added" | "removed" | "modified";

export interface FileDiffEntry {
  path: string;
  kind: FileDiffKind;
}

/** A file's bytes in a commit: small files ride inline (b64, ≤256 KiB total
 *  inline per commit); large files reference chunks staged via `filesStage`. */
export type FileContent =
  | { inline: { b64: string } }
  | { chunks: { size: number; chunks: string[] } };

/** One path mutation in a commit — the module's `Change` enum. */
export type FileChange =
  | {
      put: {
        path: string;
        exec: boolean;
        meta: Record<string, string>;
        content: FileContent;
      };
    }
  | { mkdir: { path: string } }
  | { rm: { path: string } }
  | { mv: { from: string; to: string } }
  | { symlink: { path: string; target: string } };

/** POST /v1/files/commit body — snake_case, the `FilesMsg::Commit` spec.
 *  `base_snapshot` null means the empty tree (a first commit). */
export interface FilesCommitBody {
  base_snapshot: string | null;
  message: string;
  changes: FileChange[];
}

/** One page of a directory listing (or find): entries plus a `next` cursor to
 *  echo as the following `after`, null once the listing is exhausted. */
export interface FilePage {
  entries: FileEntry[];
  next: string | null;
}

/** A byte range read: base64 bytes plus whether the range reached end-of-file. */
export interface FileReadRange {
  b64: string;
  eof: boolean;
}

export interface GatewayRouteName {
  label: string | null;
}

export type GatewayMethod = "get" | "head" | "post" | "put" | "patch" | "delete";

export interface GatewayHeader {
  name: string;
  value: string;
}

/** Snake-case fields mirror `gateway::ProxyRequestHead` exactly. */
export interface GatewayProxyHead {
  account_id: number[];
  name: GatewayRouteName;
  revision: number;
  method: GatewayMethod;
  path_and_query: string;
  headers: GatewayHeader[];
  body_len: number;
}

export interface GatewayResponseHead {
  status: number;
  headers: GatewayHeader[];
}

export interface GatewayProxyRequest {
  head: GatewayProxyHead;
  body: Uint8Array<ArrayBuffer>;
}

export interface GatewayProxyReply {
  head: GatewayResponseHead;
  body: Uint8Array<ArrayBuffer>;
}

export interface NodeTransport {
  /**
   * Submit one module msg — one block. Resolves once the block is committed.
   * `origin` is the submitter identity stamped into the block's
   * `Origin::External`; modules that derive authorship from origin (chat)
   * attribute the write to it. Omitted → the daemon's default identity.
   */
  submit(target: string, payload: unknown, origin?: string): Promise<SubmitReceipt>;
  /**
   * Submit one CONTROL-plane op (governance) ALWAYS as an account-signed frame,
   * on local AND remote (ADR A1 — the W2 governance migration). Unlike `submit`
   * (which only signs on a remote connection), governance must be authored by
   * the user's account key on every connection so the module's standing ACL
   * resolves it via `BindNode`. Requires `signControlPayload`; throws
   * `identity-locked`-shaped errors loud (never mis-authors as the node key).
   */
  submitControl(target: string, payload: unknown): Promise<SubmitReceipt>;
  /** Read committed state. The reply is the module's `*Reply` enum as json. */
  query(target: string, query: unknown): Promise<unknown>;
  /**
   * Read the module's MATERIALIZED VIEW — its own endpoint on the node's
   * derived index tier (POST /v1/index/{module}/view), serving read shapes
   * canonical state can't (search, partitions). Request/reply are the
   * module's `*-index` wire: camelCase throughout, unlike the snake_case
   * canonical module wire. Rejects 404 for modules with no view (forge).
   */
  view(module: string, request: unknown): Promise<unknown>;
  /**
   * Stage raw bytes in the node's content-addressed blob store and get their
   * sha256 digest back (64 lowercase hex). NOTHING is committed — a later
   * `submit` references the digest. The blob plane carries run replies and
   * artifacts; agent PROMPTS left it — an agent's persona is a duckfs document
   * its skill refs pin, so registration never uploads prompt text.
   *
   * The bytes must be backed by a plain ArrayBuffer (what `TextEncoder.encode`
   * returns) so they go straight into the fetch body.
   */
  putBlob(bytes: Uint8Array<ArrayBuffer>): Promise<string>;
  /**
   * Read raw bytes back out of the node's content-addressed blob store by their
   * sha256 `digest` (64 lowercase hex) — the GET counterpart to `putBlob`. This
   * is the node-local op-receipt store (a submit's `opHash` bytes); it is NOT
   * the duckfs chunk plane (that rides `filesStage`/`filesRead`). Rejects when
   * the digest is absent (the node replies 404).
   */
  getBlob(digest: string): Promise<Uint8Array<ArrayBuffer>>;

  // ── duckfs (`files` module) ──
  //
  // The typed `/v1/files/*` surface. These wrap the daemon's dedicated files
  // routes; refs + diff have no route yet and ride the generic `query` lane
  // (see files-client). This is a DIFFERENT plane from putBlob/getBlob (the
  // node-local op-receipt store): `filesStage` moves consensus state.

  /**
   * Stage raw chunk bytes as a duckfs op (POST /v1/files/stage): the chunk
   * lands in the object store + the staging table (staging IS consensus state,
   * so a stage moves the files root) and its object-id `digest` (64-char hex)
   * comes back for a later `filesCommit` to reference. Body ≤ 1 MiB
   * (`CHUNK_SIZE`). Bytes must be plain-ArrayBuffer backed for the fetch body.
   */
  filesStage(bytes: Uint8Array<ArrayBuffer>): Promise<{ digest: string }>;
  /**
   * Commit an atomic multi-path change set (POST /v1/files/commit). Resolves to
   * the block that included it; a module rejection — notably a per-path CAS
   * conflict (`files: conflict: <path> changed since base`) — throws a NodeError
   * carrying that detail so the ui can surface it.
   */
  filesCommit(body: FilesCommitBody): Promise<BlockEvent>;
  /**
   * The entry at `path` (GET /v1/files/stat), or null when nothing is there
   * (the node's 404). `snapshot` reads a historical tree; omitted reads head.
   */
  filesStat(params: { path: string; snapshot?: string }): Promise<FileEntry | null>;
  /**
   * One page of a directory's entries in name order (GET /v1/files/ls), with a
   * `next` cursor to echo as the following `after`.
   */
  filesLs(params: {
    path: string;
    snapshot?: string;
    after?: string;
    limit?: number;
  }): Promise<FilePage>;
  /**
   * A byte range of a file (GET /v1/files/read); `len` is clamped by the module
   * to its 1 MiB read cap, and `eof` marks the range reaching end-of-file.
   */
  filesRead(params: {
    path: string;
    snapshot?: string;
    offset?: number;
    len?: number;
  }): Promise<FileReadRange>;
  /** A direct raw-byte URL for browser-native downloads and drag-out. Optional
   *  for injected transports that do not expose the daemon's HTTP surface; the
   *  remote transport falls back to ranged reads above the 64 MiB facade cap. */
  filesObjectUrl?(params: { path: string; snapshot?: string; size?: number }): string | undefined;
  /** The bounded commit history, newest-first (GET /v1/files/history). */
  filesHistory(params?: { limit?: number }): Promise<FileSnapshot[]>;

  /** Invoke one finalized, policy-bounded route over the authenticated
   * gateway plane. Optional because the embedded daemon has no mesh. */
  gatewayProxy?(request: GatewayProxyRequest): Promise<GatewayProxyReply>;
  /** Report the dedicated browser-gateway listener's loopback base
   * (`http://127.0.0.1:<port>`) so the `duck://` scheme handler can reach it.
   * That listener exposes no node API or cross-route primitive. */
  gatewayBrowserBase?(): Promise<{ base: string }>;

  status(): Promise<NodeStatus>;
  /**
   * Recent finalized blocks from the node's ring, oldest-first — the explorer's
   * backing read. Each record carries every op aggregated into its window (an
   * idle window rides as an empty-`ops` nop). `limit` caps the count (default:
   * all buffered).
   */
  blocks(limit?: number): Promise<BlockRecord[]>;
  /** Subscribe to one or more node stream topics. Returns the unsubscribe. */
  subscribe(
    topics: string[],
    handlers: TopicHandlers,
    resume?: Record<string, string>,
  ): () => void;
  /** Subscribe to stream connection/liveness signals. Returns the unsubscribe. */
  onStream(listener: (signal: StreamSignal) => void): () => void;

  // ── Interactive terminal sessions ──
  // Optional (like gatewayProxy): a transport that does not expose the node's
  // http/ws surface simply omits them and the Terminal view stays inert.

  /** Create a node-hosted interactive terminal session for `agent` (e.g.
   *  "codex"): POST /v1/term/sessions. `mode` picks the input discipline —
   *  `"single"` (raw keystrokes → pty, the solo terminal) or `"shared"` (the
   *  ordered, attributed `termCommand` lane). The node enforces its own
   *  per-node session cap and answers an error when over it — a NodeError. */
  createTermSession?(agent: string, mode: TermSessionMode): Promise<TermSession>;
  /** Close a session: POST /v1/term/sessions/<id>/close (idempotent). */
  closeTermSession?(sessionId: string): Promise<void>;
  /** Send one terminal op (input / resize) over the SAME ws the subscribe
   *  socket uses. A no-op if the socket is not currently open. */
  sendTerm?(msg: TermClientMsg): void;
}

/** How a terminal session takes input. `single`: raw keystrokes straight to the
 *  pty (the solo terminal). `shared`: the ordered, attributed command lane —
 *  raw input is refused, the only way in is `termCommand`. */
export type TermSessionMode = "single" | "shared";

/** A created terminal session: its id and the stream topic its output rides. */
export interface TermSession {
  sessionId: string;
  topic: string;
}

export interface TopicHandlers {
  onEvent?(frame: EventFrame): void;
  onTail?(frame: TailFrame): void;
  /** A terminal output chunk on a `term:` topic — `item` is base64 of raw
   *  terminal bytes (see stream.ts TermChunkFrame). The ring replays on
   *  (re)subscribe, so these also carry catch-up. */
  onTermChunk?(item: string): void;
  /** One row of a shared session's ordered command log, on a `term-cmd:` topic
   *  (see stream.ts TermCommandLogFrame): the total-order `seq`, the author
   *  `origin`, and the command `text`. The ring replays on (re)subscribe, so
   *  these carry catch-up too — dedupe by `seq` on reconnect. */
  onTermCommandLog?(seq: number, origin: string, text: string): void;
  onLagged?(topic: string, cursor: string): void;
  /** The node refused this topic (unknown on an older build, unavailable, …):
   *  no frames will arrive on it for the rest of this connection, so a
   *  subscriber can stop waiting and say so instead of spinning forever. */
  onRefused?(topic: string, code: StreamErrorCode, detail: string): void;
}

export type StreamSignal =
  | { kind: "heartbeat"; frame: HeartbeatFrame }
  | { kind: "up" }
  | { kind: "down"; reason: string };

// ── Error classification + bounded fetch ────────────────

/** Why a node call failed — the UI (and waitUntilUp) branch on this instead of
 *  treating every failure identically as "down". `refused`: nothing answered
 *  (not listening yet / CSP-blocked). `timeout`: the node accepted the
 *  connection but never replied within the deadline — the old "10s" bound was a
 *  lie, no fetch had one, so a wedged node hung the UI far longer. `httpError`:
 *  it answered non-2xx (the node IS up, and erroring). `badBody`: it answered
 *  2xx with an unparseable / non-ducktape body. */
export type NodeErrorKind = "refused" | "timeout" | "httpError" | "badBody";

export class NodeError extends Error {
  readonly kind: NodeErrorKind;
  readonly status?: number;
  constructor(kind: NodeErrorKind, message: string, status?: number) {
    super(message);
    this.name = "NodeError";
    this.kind = kind;
    this.status = status;
  }
}

const STATUS_TIMEOUT_MS = 6_000; // the liveness probe must be bounded
const CALL_TIMEOUT_MS = 30_000; // submit/query may wait on a commit — looser

/** fetch with a per-attempt deadline: a node that accepts the TCP connection
 *  but never answers can no longer hang forever. An abort becomes a `timeout`
 *  NodeError; any other network failure (connection refused, CSP block) becomes
 *  `refused`. */
const fetchDeadline = (
  url: string,
  init?: RequestInit,
  timeoutMs: number = CALL_TIMEOUT_MS,
): Promise<Response> => {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  return fetch(url, { ...init, signal: controller.signal })
    .catch((err: unknown) => {
      if (controller.signal.aborted) {
        throw new NodeError(
          "timeout",
          `the node accepted the connection but did not answer within ${timeoutMs}ms`,
        );
      }
      throw new NodeError(
        "refused",
        `could not reach the node (${err instanceof Error ? err.message : String(err)})`,
      );
    })
    .finally(() => clearTimeout(timer));
};

/** A Response's error body — the node's json `{error}` or capped text — for the
 *  message, instead of discarding it behind a bare status code. */
const errorDetail = async (res: Response): Promise<string> => {
  const body = await res.text().catch(() => "");
  if (!body) return "";
  try {
    return String((JSON.parse(body) as { error?: string }).error ?? body).slice(0, 300);
  } catch {
    return body.slice(0, 300);
  }
};

const RECONNECT_BASE_MS = 1_000;
const RECONNECT_CAP_MS = 30_000;
export const STREAM_WATCHDOG_FALLBACK_MS = 7_500;

const postJson = async <T>(url: string, body: unknown): Promise<T> => {
  const res = await fetchDeadline(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    throw new NodeError("httpError", (await errorDetail(res)) || `node replied ${res.status}`, res.status);
  }
  try {
    return (await res.json()) as T;
  } catch {
    throw new NodeError("badBody", `${url} returned an invalid or empty response`);
  }
};

/** GET → json with the SAME deadline + error envelope as postJson: a non-2xx
 *  surfaces the node's `{error}` detail (so a `files: …` rejection reads
 *  through), a 2xx with an unparseable body is a `badBody`. */
const getJson = async <T>(url: string): Promise<T> => {
  const res = await fetchDeadline(url);
  if (!res.ok) {
    throw new NodeError("httpError", (await errorDetail(res)) || `node replied ${res.status}`, res.status);
  }
  try {
    return (await res.json()) as T;
  } catch {
    throw new NodeError("badBody", `${url} returned an invalid or empty response`);
  }
};

/** Build a `?a=1&b=2` query string, dropping undefined values. `path` and other
 *  present values are encoded verbatim — an empty string IS a value the caller
 *  chose (never elided), so the root path `/` and a deliberate `""` both ride. */
const queryString = (params: Record<string, string | number | undefined>): string => {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) search.set(key, String(value));
  }
  const rendered = search.toString();
  return rendered ? `?${rendered}` : "";
};

/** A node base url in its websocket form: trailing slash stripped, http→ws
 *  scheme swap — the ONE derivation both the block stream and the call
 *  socket dial through. */
const wsBase = (baseUrl: string): string =>
  baseUrl.replace(/\/$/, "").replace(/^http/, "ws");

/** The call websocket url for a channel on the node at `baseUrl` — same
 *  host/port as the daemon's http/ws surface. The huddle session
 *  (call-session.ts) dials this typed socket (audio + camera video + control);
 *  kept here because this is where the base url and its ws form live. */
export const callSocketUrl = (baseUrl: string, channel: string): string =>
  `${wsBase(baseUrl)}/v1/call/ws?channel=${encodeURIComponent(channel)}`;

/** The off-consensus Pages presence socket. It shares the node's authenticated
 * overlay runtime with calls, but carries only page caret beacons. */
export const pagePresenceSocketUrl = (baseUrl: string, page: string): string =>
  `${wsBase(baseUrl)}/v1/presence/ws?page=${encodeURIComponent(page)}`;

/** Optional capabilities a host environment can graft onto [`remoteTransport`]. */
export interface RemoteTransportOptions {
  /**
   * Sign a files-module op payload (hex in) into a raw signed op frame (hex
   * out) with the USER key — the desktop shell's `user_sign_files_frame`
   * command. When present, `filesCommit` rides the authenticated
   * `POST /v1/submit/frame` lane so the commit's author is the user
   * (`ext:<user-pubkey>`, real `/home/<user>` authority) instead of the
   * daemon's shared `ext:noded`. Throwing the exact string `identity-locked`
   * falls back to the unsigned convenience lane (status-quo authorship);
   * any other failure propagates.
   */
  signFilesPayload?: (payloadHex: string) => Promise<string>;
  /**
   * Sign an arbitrary CONTENT-module op payload (hex in) into a raw signed op
   * frame (hex out) with the USER key — the shell's `user_sign_frame` command
   * (gated to content modules). When present, every `submit` rides the
   * authenticated `POST /v1/submit/frame` lane so the op is authored as the
   * connecting user (`ext:<user-pubkey>`) and authorized by the remote node's
   * client-standing door, instead of the frameless lane where the remote node
   * discards the origin and re-signs with its OWN key. Only wired for REMOTE
   * connections (a local workspace's own node signing is correct as-is).
   * Unlike `signFilesPayload`, a locked identity here FAILS LOUD: a silent
   * frameless fallback would mis-author the op as the remote node's key and be
   * refused by the door anyway.
   */
  signPayload?: (target: string, payloadHex: string) => Promise<string>;
  /**
   * Sign a CONTROL-plane op payload (governance) into a signed frame with the
   * USER key — like `signPayload`, but wired on BOTH local and remote
   * connections (the W2 governance migration, ADR A1). Governance ops must be
   * account-signed on every connection so the module's standing ACL resolves
   * the signer via `BindNode`; content ops on a LOCAL node still ride the
   * frameless convenience lane (this is why it is a SEPARATE slot from
   * `signPayload`, not the same all-targets remote signer). A locked identity
   * FAILS LOUD — a frameless fallback would mis-author as the node key.
   */
  signControlPayload?: (target: string, payloadHex: string) => Promise<string>;
}

const bytesToHexString = (bytes: Uint8Array): string =>
  Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

const hexStringToBytes = (hex: string): Uint8Array<ArrayBuffer> => {
  const clean = hex.trim();
  if (clean.length % 2 !== 0 || /[^0-9a-fA-F]/.test(clean)) {
    throw new Error("transport: signer returned invalid frame hex");
  }
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
};

export const remoteTransport = (
  baseUrl: string,
  opts?: RemoteTransportOptions,
): NodeTransport => {
  const base = baseUrl.replace(/\/$/, "");
  const wsUrl = `${wsBase(baseUrl)}/v1/ws`;

  // One shared socket for every topic and liveness subscriber; reconnects
  // while any remain, closes once all unsubscribe.
  const topicSubs = new Map<string, Set<TopicHandlers>>();
  const streamListeners = new Set<(signal: StreamSignal) => void>();
  const cursors = new Map<string, string>();
  const refusedTopics = new Set<string>();
  const loggedTopicErrors = new Set<string>();
  const hasSubscribers = (): boolean =>
    topicSubs.size > 0 || streamListeners.size > 0;
  const activeTopics = (): string[] =>
    [...topicSubs.keys()].filter((topic) => !refusedTopics.has(topic));
  let socket: WebSocket | null = null;
  let retries = 0;
  let watchdog: ReturnType<typeof setTimeout> | null = null;
  let watchdogMs = STREAM_WATCHDOG_FALLBACK_MS;
  let closeReason = "stream socket closed";

  const emitStream = (signal: StreamSignal): void => {
    streamListeners.forEach((notify) => notify(signal));
  };

  const clearWatchdog = (): void => {
    if (watchdog !== null) clearTimeout(watchdog);
    watchdog = null;
  };

  const closeWithReason = (reason: string): void => {
    closeReason = reason;
    socket?.close();
  };

  const armWatchdog = (timeoutMs = watchdogMs): void => {
    clearWatchdog();
    watchdogMs = timeoutMs;
    watchdog = setTimeout(
      () => closeWithReason("stream heartbeat timed out"),
      timeoutMs,
    );
  };

  const socketIsOpen = (): boolean => {
    const state = socket?.readyState as number | undefined;
    return state === WebSocket.OPEN || state === 1;
  };

  const sendFrame = (frame: ClientMsg): void => {
    if (!socketIsOpen()) return;
    socket?.send(JSON.stringify(frame));
  };

  const subscribeFrame = (topics: string[]): ClientMsg => {
    const resume: Record<string, string> = {};
    for (const topic of topics) {
      const cursor = cursors.get(topic);
      if (cursor) resume[topic] = cursor;
    }
    return { op: "subscribe", topics, resume };
  };

  const sendSubscribe = (topics: string[]): void => {
    const wanted = topics.filter((topic) => topicSubs.has(topic) && !refusedTopics.has(topic));
    if (wanted.length === 0) return;
    sendFrame(subscribeFrame(wanted));
  };

  const sendUnsubscribe = (topics: string[]): void => {
    const wanted = topics.filter((topic) => !refusedTopics.has(topic));
    if (wanted.length === 0) return;
    sendFrame({ op: "unsubscribe", topics: wanted });
  };

  const dispatchFrame = (frame: ServerFrame): void => {
    if (isSubscribedFrame(frame)) {
      for (const [topic, cursor] of Object.entries(frame.topics)) {
        if (typeof cursor === "string") cursors.set(topic, cursor);
      }
      return;
    }
    if (isEventFrame(frame)) {
      cursors.set(frame.topic, frame.cursor);
      topicSubs.get(frame.topic)?.forEach((handlers) => handlers.onEvent?.(frame));
      return;
    }
    if (isTermCommandLogFrame(frame)) {
      // no cursor: the frame carries none, and the node replays the whole
      // command ring on resubscribe (the view dedupes by seq).
      topicSubs
        .get(frame.topic)
        ?.forEach((handlers) => handlers.onTermCommandLog?.(frame.seq, frame.origin, frame.text));
      return;
    }
    if (isTailFrame(frame)) {
      cursors.set(frame.topic, frame.cursor);
      topicSubs.get(frame.topic)?.forEach((handlers) => handlers.onTail?.(frame));
      return;
    }
    if (isLaggedFrame(frame)) {
      cursors.set(frame.topic, frame.cursor);
      topicSubs
        .get(frame.topic)
        ?.forEach((handlers) => handlers.onLagged?.(frame.topic, frame.cursor));
      return;
    }
    if (isHeartbeatFrame(frame)) {
      const timeout = Math.max(
        STREAM_WATCHDOG_FALLBACK_MS,
        Math.ceil(frame.intervalMs * 2.5),
      );
      armWatchdog(timeout);
      emitStream({ kind: "heartbeat", frame });
      return;
    }
    if (isErrorFrame(frame)) {
      if (frame.topic) {
        refusedTopics.add(frame.topic);
        cursors.delete(frame.topic);
        topicSubs
          .get(frame.topic)
          ?.forEach((handlers) =>
            handlers.onRefused?.(frame.topic, frame.code, frame.detail),
          );
        const key = `${frame.topic}:${frame.code}`;
        if (!loggedTopicErrors.has(key)) {
          loggedTopicErrors.add(key);
          console.warn(
            `stream topic ${frame.topic} refused (${frame.code}): ${frame.detail}`,
          );
        }
      }
    }
  };

  const connect = (): void => {
    if (socket || !hasSubscribers()) return;
    const ws = new WebSocket(wsUrl);
    socket = ws;
    ws.onopen = () => {
      retries = 0; // a clean connection resets the backoff
      closeReason = "stream socket closed";
      watchdogMs = STREAM_WATCHDOG_FALLBACK_MS;
      // a fresh connection may face a different node build/module set —
      // retry refused topics once per connection instead of pinning them.
      refusedTopics.clear();
      emitStream({ kind: "up" });
      armWatchdog(STREAM_WATCHDOG_FALLBACK_MS);
      sendSubscribe(activeTopics());
    };
    ws.onmessage = (event) => {
      armWatchdog();
      let frame: unknown;
      try {
        frame = JSON.parse(String(event.data));
      } catch {
        return; // a malformed / non-json frame is a no-op, not an uncaught throw
      }
      // A term chunk is an event frame carrying `item` on a `term:` topic; it
      // has no `op`/`cursor`, so it fails the strict isServerFrame guard —
      // route it here before that gate. No cursor tracking: the node replays
      // the whole ring on resubscribe and xterm reconstructs from raw bytes.
      if (isTermChunkFrame(frame)) {
        topicSubs.get(frame.topic)?.forEach((handlers) => handlers.onTermChunk?.(frame.item));
        return;
      }
      if (isServerFrame(frame)) dispatchFrame(frame);
    };
    ws.onclose = () => {
      clearWatchdog();
      socket = null;
      if (!hasSubscribers()) return;
      emitStream({ kind: "down", reason: closeReason });
      // exponential backoff (capped) + jitter, instead of the blind 2s retry
      // loop that spammed the console every 2s forever against a dead node.
      const backoff = Math.min(RECONNECT_CAP_MS, RECONNECT_BASE_MS * 2 ** retries);
      retries += 1;
      setTimeout(connect, backoff * (0.5 + Math.random() * 0.5));
    };
    ws.onerror = () => closeWithReason("stream socket error");
  };

  /** Drop the socket once nothing is subscribed. */
  const closeIfIdle = (): void => {
    if (!hasSubscribers()) {
      clearWatchdog();
      socket?.close();
      socket = null;
    }
  };

  /** Sign a payload into a frame and POST it to /v1/submit/frame. Shared by the
   *  all-targets remote lane (`submit` + `signPayload`) and the always-signed
   *  control lane (`submitControl` + `signControlPayload`). */
  const postSignedFrame = async (
    sign: (target: string, payloadHex: string) => Promise<string>,
    target: string,
    payload: unknown,
  ): Promise<SubmitReceipt> => {
    // sign the JSON-encoded payload; module decoders are key-order /
    // whitespace-insensitive, and each lane content-addresses its own bytes, so
    // the signed frame need not be byte-identical to the frameless encoding.
    const payloadHex = bytesToHexString(new TextEncoder().encode(JSON.stringify(payload)));
    let frameHex: string;
    try {
      frameHex = await sign(target, payloadHex);
    } catch (err) {
      const detail = err instanceof Error ? err.message : String(err);
      // fail loud, no frameless fallback: a locked identity must not silently
      // re-author the op as the node's key (and be refused by the door anyway).
      if (detail === "identity-locked") {
        throw new NodeError(
          "httpError",
          "unlock your identity to act on this node — the op is signed by your key",
        );
      }
      throw err;
    }
    const res = await fetchDeadline(`${base}/v1/submit/frame`, {
      method: "POST",
      headers: { "content-type": "application/octet-stream" },
      body: hexStringToBytes(frameHex),
    });
    if (!res.ok) {
      throw new NodeError(
        "httpError",
        (await errorDetail(res)) || `node replied ${res.status}`,
        res.status,
      );
    }
    return (await res.json()) as SubmitReceipt;
  };

  return {
    // With a user-key signer (remote connections), every op is user-authored
    // over the authenticated frame lane. Without one (local workspace / web),
    // the frameless lane rides — JSON.stringify drops an undefined origin, so
    // the field only crosses the wire when a caller set one.
    submit: async (target, payload, origin) => {
      const sign = opts?.signPayload;
      if (sign) {
        return postSignedFrame(sign, target, payload);
      }
      return postJson<SubmitReceipt>(`${base}/v1/submit`, { target, payload, origin });
    },
    submitControl: async (target, payload) => {
      const sign = opts?.signControlPayload;
      if (!sign) {
        // no user-key custody (web build): control ops are not drivable here.
        throw new NodeError(
          "httpError",
          "this connection cannot sign control ops — governance needs an account key",
        );
      }
      return postSignedFrame(sign, target, payload);
    },
    query: (target, query) =>
      postJson<unknown>(`${base}/v1/query`, { target, query }),
    view: (module, request) =>
      postJson<unknown>(`${base}/v1/index/${module}/view`, request),
    // raw bytes in, `{"digest":"<64-hex>"}` out — not json in, so this bypasses
    // postJson; the error envelope is still the node's json `{error}` shape.
    putBlob: (bytes) =>
      Promise.resolve()
        .then(() =>
          fetch(`${base}/v1/files/blob`, {
            method: "POST",
            headers: { "content-type": "application/octet-stream" },
            body: bytes,
          }),
        )
        .then(async (res) => {
          if (res.ok) return ((await res.json()) as { digest: string }).digest;
          const detail = await res
            .json()
            .then((payload) => String((payload as { error?: string }).error ?? ""))
            .catch(() => "");
          throw new Error(detail || `node replied ${res.status}`);
        }),
    // GET the raw chunk bytes back; the error envelope is the node's json
    // `{error}` shape, matching putBlob.
    getBlob: (digest) =>
      Promise.resolve()
        .then(() => fetch(`${base}/v1/files/blob/${digest}`))
        .then(async (res) => {
          if (res.ok) return new Uint8Array(await res.arrayBuffer());
          const detail = await res
            .json()
            .then((payload) => String((payload as { error?: string }).error ?? ""))
            .catch(() => "");
          throw new Error(detail || `node replied ${res.status}`);
        }),
    // ── duckfs (`files` module) ──
    // raw chunk bytes in, `{"digest":"<64-hex>"}` out — octet-stream, not json
    // in, so it bypasses postJson; the error envelope is still the node's json
    // `{error}` shape (413 on an oversized body, a module rejection otherwise).
    filesStage: async (bytes) => {
      const res = await fetchDeadline(`${base}/v1/files/stage`, {
        method: "POST",
        headers: { "content-type": "application/octet-stream" },
        body: bytes,
      });
      if (!res.ok) {
        throw new NodeError(
          "httpError",
          (await errorDetail(res)) || `node replied ${res.status}`,
          res.status,
        );
      }
      try {
        return (await res.json()) as { digest: string };
      } catch {
        throw new NodeError("badBody", "/v1/files/stage returned an invalid response");
      }
    },
    filesCommit: async (body) => {
      const sign = opts?.signFilesPayload;
      if (sign) {
        // the exact FilesMsg the convenience route would build server-side
        // (externally-tagged serde enum, snake_case) — signed by the user key
        // so the commit's author is the user, not the daemon.
        const payload = new TextEncoder().encode(JSON.stringify({ commit: body }));
        try {
          const frameHex = await sign(bytesToHexString(payload));
          const res = await fetchDeadline(`${base}/v1/submit/frame`, {
            method: "POST",
            headers: { "content-type": "application/octet-stream" },
            body: hexStringToBytes(frameHex),
          });
          if (!res.ok) {
            throw new NodeError(
              "httpError",
              (await errorDetail(res)) || `node replied ${res.status}`,
              res.status,
            );
          }
          return (await res.json()) as SubmitReceipt;
        } catch (err) {
          // a locked identity falls back to the unsigned lane — the commit
          // still lands, with the daemon's status-quo authorship. anything
          // else (a real signer or node failure) propagates. Native calls
          // rejects with the raw string, not an Error — accept both shapes.
          const detail = err instanceof Error ? err.message : String(err);
          if (detail !== "identity-locked") throw err;
          console.warn(
            "[transport] user key locked; files commit rides the unsigned lane as ext:noded",
          );
        }
      }
      return postJson<BlockEvent>(`${base}/v1/files/commit`, body);
    },
    filesStat: async ({ path, snapshot }) => {
      try {
        return await getJson<FileEntry>(
          `${base}/v1/files/stat${queryString({ path, snapshot })}`,
        );
      } catch (err) {
        // the module answers a 404 for an absent path — the caller's "no entry"
        // signal, mapped to null (the CAS-era stat's absent shape), not an error.
        if (err instanceof NodeError && err.status === 404) return null;
        throw err;
      }
    },
    filesLs: ({ path, snapshot, after, limit }) =>
      getJson<FilePage>(`${base}/v1/files/ls${queryString({ path, snapshot, after, limit })}`),
    filesRead: ({ path, snapshot, offset, len }) =>
      getJson<FileReadRange>(
        `${base}/v1/files/read${queryString({ path, snapshot, offset, len })}`,
      ),
    filesObjectUrl: ({ path, snapshot, size }) =>
      size !== undefined && size > 64 * 1024 * 1024
        ? undefined
        : `${base}/v1/files/object${path
            .split("/")
            .map((segment) => encodeURIComponent(segment))
            .join("/")}${queryString({ snapshot })}`,
    filesHistory: async (params) => {
      const body = await getJson<{ snapshots: FileSnapshot[] }>(
        `${base}/v1/files/history${queryString({ limit: params?.limit })}`,
      );
      return body.snapshots;
    },
    gatewayProxy: async (request) => {
      const encode = (bytes: Uint8Array): string => {
        let binary = "";
        for (let offset = 0; offset < bytes.length; offset += 0x8000) {
          binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
        }
        return btoa(binary);
      };
      const wire = await postJson<{ head: GatewayResponseHead; bodyB64: string }>(
        `${base}/v1/gateway/proxy`,
        { head: request.head, bodyB64: encode(request.body) },
      );
      const binary = atob(wire.bodyB64);
      const body = new Uint8Array(binary.length);
      for (let index = 0; index < binary.length; index += 1) {
        body[index] = binary.charCodeAt(index);
      }
      return { head: wire.head, body };
    },
    gatewayBrowserBase: () => getJson<{ base: string }>(`${base}/v1/gateway/browser`),
    status: async () => {
      const res = await fetchDeadline(`${base}/v1/status`, undefined, STATUS_TIMEOUT_MS);
      if (!res.ok) throw new NodeError("httpError", `node replied ${res.status}`, res.status);
      let parsed: NodeStatus;
      try {
        parsed = (await res.json()) as NodeStatus;
      } catch {
        throw new NodeError("badBody", "/v1/status returned an invalid response");
      }
      // shape check: a non-ducktape process that answers 200 on this port must
      // not read as a healthy node — "connecting" to it makes everything
      // downstream fail confusingly (see NodeStatus).
      if (
        typeof parsed.version !== "string" ||
        typeof parsed.height !== "number" ||
        typeof parsed.appHash !== "string"
      ) {
        throw new NodeError("badBody", "the process answering this port is not a ducktape node");
      }
      return parsed;
    },
    blocks: async (limit) => {
      const res = await fetchDeadline(
        limit === undefined ? `${base}/v1/blocks` : `${base}/v1/blocks?limit=${limit}`,
        undefined,
        STATUS_TIMEOUT_MS,
      );
      if (!res.ok) throw new NodeError("httpError", `node replied ${res.status}`, res.status);
      // best-effort observability: a node without a blocks surface (or a
      // malformed body) reads as "no blocks", not an error.
      const body = (await res.json().catch(() => ({}))) as { blocks?: BlockRecord[] };
      return body.blocks ?? [];
    },
    subscribe: (topics, handlers, resume) => {
      const wanted = [...new Set(topics)];
      const firstTopics: string[] = [];
      for (const topic of wanted) {
        if (resume?.[topic]) cursors.set(topic, resume[topic]);
        let subs = topicSubs.get(topic);
        if (!subs) {
          subs = new Set();
          topicSubs.set(topic, subs);
          firstTopics.push(topic);
        }
        subs.add(handlers);
      }
      connect();
      sendSubscribe(firstTopics);
      return () => {
        const lastTopics: string[] = [];
        for (const topic of wanted) {
          const subs = topicSubs.get(topic);
          if (!subs) continue;
          subs.delete(handlers);
          if (subs.size === 0) {
            topicSubs.delete(topic);
            cursors.delete(topic);
            refusedTopics.delete(topic);
            lastTopics.push(topic);
          }
        }
        sendUnsubscribe(lastTopics);
        closeIfIdle();
      };
    },
    onStream: (listener) => {
      streamListeners.add(listener);
      connect();
      return () => {
        streamListeners.delete(listener);
        closeIfIdle();
      };
    },
    createTermSession: (agent, mode) =>
      postJson<TermSession>(`${base}/v1/term/sessions`, { agent, mode }),
    closeTermSession: async (sessionId) => {
      // idempotent on the node; a non-2xx still surfaces its detail.
      const res = await fetchDeadline(
        `${base}/v1/term/sessions/${encodeURIComponent(sessionId)}/close`,
        { method: "POST" },
      );
      if (!res.ok) {
        throw new NodeError(
          "httpError",
          (await errorDetail(res)) || `node replied ${res.status}`,
          res.status,
        );
      }
    },
    // TermClientMsg's two ops aren't in the generated ClientMsg yet (added to
    // noded's enum in parallel); cast until stream.gen.ts regenerates.
    sendTerm: (msg) => sendFrame(msg as unknown as ClientMsg),
  };
};
