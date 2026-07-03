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

export interface ModuleStatus {
  id: string;
  root: string;
}

export interface NodeStatus {
  version: string;
  appHash: string;
  height: number;
  modules: ModuleStatus[];
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

export interface NodeTransport {
  /**
   * Submit one module msg — one block. Resolves once the block is committed.
   * `origin` is the submitter identity stamped into the block's
   * `Origin::External`; modules that derive authorship from origin (chat)
   * attribute the write to it. Omitted → the daemon's default identity.
   */
  submit(target: string, payload: unknown, origin?: string): Promise<BlockEvent>;
  /** Read committed state. The reply is the module's `*Reply` enum as json. */
  query(target: string, query: unknown): Promise<unknown>;
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
  status(): Promise<NodeStatus>;
  /**
   * Recent per-block telemetry from the node's ring, oldest-first — the
   * backfill a client pulls on connect before following the live stream.
   * `limit` caps the count (default: all buffered).
   */
  telemetry(limit?: number): Promise<TelemetryFrame[]>;
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
      postJson<BlockEvent>(`${base}/v1/submit`, { target, payload, origin }),
    query: (target, query) =>
      postJson<unknown>(`${base}/v1/query`, { target, query }),
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
