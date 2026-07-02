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
  status(): Promise<NodeStatus>;
  /** Subscribe to finalized blocks. Returns the unsubscribe. */
  onBlock(listener: (block: BlockEvent) => void): () => void;
}

// ── The transport ───────────────────────────────────────

interface WsBlockFrame {
  type: "block";
  height: number;
  appHash: string;
}

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

  // One shared socket for every block subscriber; reconnects while any remain.
  const listeners = new Set<(block: BlockEvent) => void>();
  let socket: WebSocket | null = null;

  const connect = (): void => {
    if (socket || listeners.size === 0) return;
    const ws = new WebSocket(wsUrl);
    socket = ws;
    ws.onmessage = (event) => {
      const frame = JSON.parse(String(event.data)) as WsBlockFrame;
      switch (frame.type) {
        case "block": {
          const block = { height: frame.height, appHash: frame.appHash };
          listeners.forEach((notify) => notify(block));
          break;
        }
        default:
          break; // unknown frame kinds are fine — the stream may grow
      }
    };
    ws.onclose = () => {
      socket = null;
      if (listeners.size > 0) setTimeout(connect, RECONNECT_DELAY_MS);
    };
    ws.onerror = () => ws.close();
  };

  return {
    // JSON.stringify drops an undefined origin, so the field only crosses the
    // wire when a caller set one
    submit: (target, payload, origin) =>
      postJson<BlockEvent>(`${base}/v1/submit`, { target, payload, origin }),
    query: (target, query) =>
      postJson<unknown>(`${base}/v1/query`, { target, query }),
    status: () =>
      Promise.resolve()
        .then(() => fetch(`${base}/v1/status`))
        .then((res) => {
          if (!res.ok) throw new Error(`node replied ${res.status}`);
          return res.json() as Promise<NodeStatus>;
        }),
    onBlock: (listener) => {
      listeners.add(listener);
      connect();
      return () => {
        listeners.delete(listener);
        if (listeners.size === 0) {
          socket?.close();
          socket = null;
        }
      };
    },
  };
};
