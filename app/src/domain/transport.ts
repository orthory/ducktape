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
// The explorer plane: one record per NON-EMPTY finalized block — heartbeat
// nops never enter the node's ring, so this is real history, not idle ticks.
// Pulled via GET /v1/blocks; a node without the surface reads as "no blocks".

/** One dispatch in a block's drain — a module ran, triggered by `origin`. */
export interface DispatchInfo {
  module: string;
  /** `"external"`, `"external:<name>"`, `"system"`, or `"module:<id>"`. */
  origin: string;
  emittedMsgs: number;
  emittedEvents: number;
}

/** How a block's op landed: an applied op mutated state; a rejected op
 *  finalized but rolled back — a failed tx. */
export type BlockDisposition = "applied" | "rejected";

export interface BlockRecord {
  height: number;
  /** Hex content hash of the block's frame — the block's hash. */
  hash: string;
  /** Hex app-hash after this block settled — the commit. */
  commitHash: string;
  /** Hex ed25519 key of the proposing validator — the frame's VERIFIED
   *  signer, not a claimed identity. */
  proposer: string;
  disposition: BlockDisposition;
  /** The root op's target module. */
  target: string;
  /** The dispatch trace, in drain order — the transactions inside the block.
   *  Empty for a rejected op (a deterministic no-op leaves no trace). */
  operations: DispatchInfo[];
  /** Capped utf-8 preview of the root op's payload (module `*Msg` json). */
  payload: string;
  /** Hex content address of the root op — sha256 of the committed payload
   *  bytes, fetchable via the blob lane (`GET /v1/files/blob/{opHash}`).
   *  Optional: rings written before the field existed lack it. */
  opHash?: string;
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

export interface NodeTransport {
  /**
   * Submit one module msg — one block. Resolves once the block is committed.
   * `origin` is the submitter identity stamped into the block's
   * `Origin::External`; modules that derive authorship from origin (chat)
   * attribute the write to it. Omitted → the daemon's default identity.
   */
  submit(target: string, payload: unknown, origin?: string): Promise<SubmitReceipt>;
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
   * `submit` references the digest. The agent flow uses this to upload a
   * prompt's text so the oracle worker can fetch it by the registered
   * `prompt_hash` (which IS this digest, since the store keys by sha256).
   *
   * The bytes must be backed by a plain ArrayBuffer (what `TextEncoder.encode`
   * returns) so they go straight into the fetch body.
   */
  putBlob(bytes: Uint8Array<ArrayBuffer>): Promise<string>;
  /**
   * Read raw bytes back out of the node's content-addressed blob store by their
   * sha256 `digest` (64 lowercase hex) — the GET counterpart to `putBlob`. This
   * is how the files module's chunks are fetched for reassembly; the caller MUST
   * still `verifyChunk` the bytes against a committed manifest before trusting
   * them. Rejects when the digest is absent (the node replies 404).
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
  /** The bounded commit history, newest-first (GET /v1/files/history). */
  filesHistory(params?: { limit?: number }): Promise<FileSnapshot[]>;

  status(): Promise<NodeStatus>;
  /**
   * The node's Prometheus/OpenMetrics scrape (`GET /metrics`) as raw text —
   * commonware's runtime series plus this node's `ducktape_*` block series
   * (height, blocks, apply-latency histogram, per-module dispatch counters).
   * Parse with `domain/metrics`. Rejects when the node has no metrics surface.
   */
  metrics(): Promise<string>;
  /**
   * Recent non-empty blocks from the node's ring, oldest-first — the
   * explorer's backing read. `limit` caps the count (default: all buffered).
   */
  blocks(limit?: number): Promise<BlockRecord[]>;
  /** Subscribe to finalized blocks. Returns the unsubscribe. */
  onBlock(listener: (block: BlockEvent) => void): () => void;
}

// ── The transport ───────────────────────────────────────

interface WsBlockFrame {
  type: "block";
  height: number;
  appHash: string;
}

type WsFrame = WsBlockFrame;

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

export const remoteTransport = (baseUrl: string): NodeTransport => {
  const base = baseUrl.replace(/\/$/, "");
  const wsUrl = `${wsBase(baseUrl)}/v1/ws`;

  // One shared socket for every block subscriber; reconnects while any
  // remain, closes once all unsubscribe.
  const blockListeners = new Set<(block: BlockEvent) => void>();
  const hasSubscribers = (): boolean => blockListeners.size > 0;
  let socket: WebSocket | null = null;
  let retries = 0;

  const connect = (): void => {
    if (socket || !hasSubscribers()) return;
    const ws = new WebSocket(wsUrl);
    socket = ws;
    ws.onopen = () => {
      retries = 0; // a clean connection resets the backoff
    };
    ws.onmessage = (event) => {
      let frame: WsFrame;
      try {
        frame = JSON.parse(String(event.data)) as WsFrame;
      } catch {
        return; // a malformed / non-json frame is a no-op, not an uncaught throw
      }
      switch (frame.type) {
        case "block": {
          const block = { height: frame.height, appHash: frame.appHash };
          blockListeners.forEach((notify) => notify(block));
          break;
        }
        default:
          break; // unknown frame kinds are fine — the stream may grow
      }
    };
    ws.onclose = () => {
      socket = null;
      if (!hasSubscribers()) return;
      // exponential backoff (capped) + jitter, instead of the blind 2s retry
      // loop that spammed the console every 2s forever against a dead node.
      const backoff = Math.min(RECONNECT_CAP_MS, RECONNECT_BASE_MS * 2 ** retries);
      retries += 1;
      setTimeout(connect, backoff * (0.5 + Math.random() * 0.5));
    };
    ws.onerror = () => ws.close();
  };

  /** Drop the socket once nothing is subscribed. */
  const closeIfIdle = (): void => {
    if (!hasSubscribers()) {
      socket?.close();
      socket = null;
    }
  };

  return {
    // JSON.stringify drops an undefined origin, so the field only crosses the
    // wire when a caller set one
    submit: (target, payload, origin) =>
      postJson<SubmitReceipt>(`${base}/v1/submit`, { target, payload, origin }),
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
    filesCommit: (body) => postJson<BlockEvent>(`${base}/v1/files/commit`, body),
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
    filesHistory: async (params) => {
      const body = await getJson<{ snapshots: FileSnapshot[] }>(
        `${base}/v1/files/history${queryString({ limit: params?.limit })}`,
      );
      return body.snapshots;
    },
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
    // OpenMetrics text exposition (not json, not under /v1) — the scrape body.
    metrics: async () => {
      const res = await fetchDeadline(`${base}/metrics`, undefined, STATUS_TIMEOUT_MS);
      if (!res.ok) throw new NodeError("httpError", `node replied ${res.status}`, res.status);
      return res.text();
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
    onBlock: (listener) => {
      blockListeners.add(listener);
      connect();
      return () => {
        blockListeners.delete(listener);
        closeIfIdle();
      };
    },
  };
};
