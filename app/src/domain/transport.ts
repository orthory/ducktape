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

// ── Telemetry ───────────────────────────────────────────
//
// The node-local observability plane: one frame per finalized block, carrying
// the host's deterministic dispatch trace decorated with this node's wall-clock
// apply latency. Delivered live over the ws stream and pullable (recent ring)
// via GET /v1/telemetry. Keyed by (height, source) — the same space the future
// on-consensus telemetry module uses.

/** One dispatch in a block's drain — a module ran, triggered by `origin`. */
export interface TelemetryDispatch {
  module: string;
  /** `"external"`, `"external:<name>"`, `"system"`, or `"module:<id>"`. */
  origin: string;
  emittedMsgs: number;
  emittedEvents: number;
}

/** One observability event a module emitted during the block. */
export interface TelemetryEvent {
  source: string;
  /** Best-effort utf-8 preview of the module-defined payload. */
  payload: string;
}

export interface TelemetryFrame {
  height: number;
  /** The block's agreed logical clock — NOT this node's wall clock. */
  consensusTime: number;
  /** Node-local cost of applying the block, microseconds (non-deterministic). */
  latencyUs: number;
  dispatches: TelemetryDispatch[];
  events: TelemetryEvent[];
}

// ── Blocks (explorer) ───────────────────────────────────
//
// The explorer plane: one record per NON-EMPTY finalized block — heartbeat
// nops never enter the node's ring, so this is real history, not idle ticks.
// Node-local observability like telemetry: pulled from the ring via
// GET /v1/blocks; a node without the surface reads as "no blocks".

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
  operations: TelemetryDispatch[];
  /** Capped utf-8 preview of the root op's payload (module `*Msg` json). */
  payload: string;
  /** Hex content address of the root op — sha256 of the committed payload
   *  bytes, fetchable via the blob lane (`GET /v1/files/blob/{opHash}`).
   *  Optional: rings written before the field existed lack it. */
  opHash?: string;
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
  status(): Promise<NodeStatus>;
  /**
   * Recent per-block telemetry from the node's ring, oldest-first — the
   * backfill a client pulls on connect before following the live stream.
   * `limit` caps the count (default: all buffered).
   */
  telemetry(limit?: number): Promise<TelemetryFrame[]>;
  /**
   * Recent non-empty blocks from the node's ring, oldest-first — the
   * explorer's backing read. `limit` caps the count (default: all buffered).
   */
  blocks(limit?: number): Promise<BlockRecord[]>;
  /** Subscribe to finalized blocks. Returns the unsubscribe. */
  onBlock(listener: (block: BlockEvent) => void): () => void;
  /** Subscribe to live per-block telemetry frames. Returns the unsubscribe. */
  onTelemetry(listener: (frame: TelemetryFrame) => void): () => void;
}

// ── The transport ───────────────────────────────────────

interface WsBlockFrame {
  type: "block";
  height: number;
  appHash: string;
}

interface WsTelemetryFrame extends TelemetryFrame {
  type: "telemetry";
}

type WsFrame = WsBlockFrame | WsTelemetryFrame;

const RECONNECT_DELAY_MS = 2_000;

const postJson = <T>(url: string, body: unknown): Promise<T> =>
  Promise.resolve()
    .then(() =>
      fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      }),
    )
    .then(async (res) => {
      if (res.ok) return (await res.json()) as T;
      const detail = await res
        .json()
        .then((payload) => String((payload as { error?: string }).error ?? ""))
        .catch(() => "");
      throw new Error(detail || `node replied ${res.status}`);
    });

/** The voice websocket url for a channel on the node at `baseUrl` — same
 *  host/port as the daemon's http/ws surface, http→ws scheme swap, matching the
 *  block stream's `/v1/ws` derivation. The audio session (voice-session.ts)
 *  dials this; kept here because this is where the base url and its ws form
 *  live. */
export const voiceSocketUrl = (baseUrl: string, channel: string): string => {
  const ws = baseUrl.replace(/\/$/, "").replace(/^http/, "ws");
  return `${ws}/v1/voice/ws?channel=${encodeURIComponent(channel)}`;
};

export const remoteTransport = (baseUrl: string): NodeTransport => {
  const base = baseUrl.replace(/\/$/, "");
  const wsUrl = `${base.replace(/^http/, "ws")}/v1/ws`;

  // One shared socket for every subscriber (blocks + telemetry); reconnects
  // while any remain, closes once all unsubscribe.
  const blockListeners = new Set<(block: BlockEvent) => void>();
  const telemetryListeners = new Set<(frame: TelemetryFrame) => void>();
  const hasSubscribers = (): boolean =>
    blockListeners.size > 0 || telemetryListeners.size > 0;
  let socket: WebSocket | null = null;

  const connect = (): void => {
    if (socket || !hasSubscribers()) return;
    const ws = new WebSocket(wsUrl);
    socket = ws;
    ws.onmessage = (event) => {
      const frame = JSON.parse(String(event.data)) as WsFrame;
      switch (frame.type) {
        case "block": {
          const block = { height: frame.height, appHash: frame.appHash };
          blockListeners.forEach((notify) => notify(block));
          break;
        }
        case "telemetry": {
          telemetryListeners.forEach((notify) => notify(frame));
          break;
        }
        default:
          break; // unknown frame kinds are fine — the stream may grow
      }
    };
    ws.onclose = () => {
      socket = null;
      if (hasSubscribers()) setTimeout(connect, RECONNECT_DELAY_MS);
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
    status: () =>
      Promise.resolve()
        .then(() => fetch(`${base}/v1/status`))
        .then((res) => {
          if (!res.ok) throw new Error(`node replied ${res.status}`);
          return res.json() as Promise<NodeStatus>;
        }),
    telemetry: (limit) =>
      Promise.resolve()
        .then(() =>
          fetch(
            limit === undefined
              ? `${base}/v1/telemetry`
              : `${base}/v1/telemetry?limit=${limit}`,
          ),
        )
        .then((res) => {
          if (!res.ok) throw new Error(`node replied ${res.status}`);
          return res.json() as Promise<{ frames?: TelemetryFrame[] }>;
        })
        // best-effort observability: a node without a telemetry surface (or a
        // malformed body) reads as "no telemetry", not an error.
        .then((body) => body.frames ?? []),
    blocks: (limit) =>
      Promise.resolve()
        .then(() =>
          fetch(
            limit === undefined
              ? `${base}/v1/blocks`
              : `${base}/v1/blocks?limit=${limit}`,
          ),
        )
        .then((res) => {
          if (!res.ok) throw new Error(`node replied ${res.status}`);
          return res.json() as Promise<{ blocks?: BlockRecord[] }>;
        })
        // same best-effort contract as telemetry: a node without a blocks
        // surface (or a malformed body) reads as "no blocks", not an error.
        .then((body) => body.blocks ?? []),
    onBlock: (listener) => {
      blockListeners.add(listener);
      connect();
      return () => {
        blockListeners.delete(listener);
        closeIfIdle();
      };
    },
    onTelemetry: (listener) => {
      telemetryListeners.add(listener);
      connect();
      return () => {
        telemetryListeners.delete(listener);
        closeIfIdle();
      };
    },
  };
};
