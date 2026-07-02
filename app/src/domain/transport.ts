// The node transport seam — the ONE dependency-injection point between the UI
// and a ducktape node.
//
// Two variants implement the same NodeTransport:
//   - tauriTransport: the desktop build. The node runs in-process behind Tauri
//     commands (node_submit / node_query / node_status) and pushes finalized
//     blocks as `ducktape://block` window events.
//   - remoteTransport: the web build. Talks to a running gateway (`cargo run
//     -p gateway`) over http (/v1/submit, /v1/query, /v1/status) and a
//     websocket block stream (/v1/ws).
//
// getTransport() picks the variant at runtime: inside a Tauri webview the
// injected __TAURI_INTERNALS__ marker is present; anywhere else we are the web
// build and dial the gateway. Everything above this seam (typed module
// clients, store, views) is variant-blind.
//
// Wire casing is the node's, verbatim: module payloads/replies use PascalCase
// enum variants + snake_case fields (serde defaults of the `*-interface`
// crates); the gateway envelope itself (appHash, height) is camelCase.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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
  appHash: string;
  height: number;
  modules: ModuleStatus[];
}

export interface NodeTransport {
  /** Submit one module msg — one block. Resolves once the block is committed. */
  submit(target: string, payload: unknown): Promise<BlockEvent>;
  /** Read committed state. The reply is the module's `*Reply` enum as json. */
  query(target: string, query: unknown): Promise<unknown>;
  status(): Promise<NodeStatus>;
  /** Subscribe to finalized blocks. Returns the unsubscribe. */
  onBlock(listener: (block: BlockEvent) => void): () => void;
}

// ── Variant selection ───────────────────────────────────

const isTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// The web build's gateway address: VITE_DUCKTAPE_NODE_URL when baked in at
// build time, else the gateway's dev default. Resolved once at module load.
const nodeUrl: string =
  import.meta.env.VITE_DUCKTAPE_NODE_URL || "http://127.0.0.1:8844";

export const getTransport = (): NodeTransport =>
  isTauri() ? tauriTransport() : remoteTransport(nodeUrl);

// ── Tauri variant (embedded node) ───────────────────────

export const tauriTransport = (): NodeTransport => ({
  submit: (target, payload) =>
    invoke<BlockEvent>("node_submit", { target, payload }),
  query: (target, query) => invoke<unknown>("node_query", { target, query }),
  status: () => invoke<NodeStatus>("node_status"),
  onBlock: (listener) => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    listen<BlockEvent>("ducktape://block", (event) => listener(event.payload))
      .then((stop) => {
        // unsubscribed before the bridge resolved — stop immediately
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {}); // no event bridge — nothing to unlisten
    return () => {
      cancelled = true;
      unlisten?.();
    };
  },
});

// ── Remote variant (gateway over http/ws) ───────────────

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
      throw new Error(detail || `gateway replied ${res.status}`);
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
    submit: (target, payload) =>
      postJson<BlockEvent>(`${base}/v1/submit`, { target, payload }),
    query: (target, query) =>
      postJson<unknown>(`${base}/v1/query`, { target, query }),
    status: () =>
      Promise.resolve()
        .then(() => fetch(`${base}/v1/status`))
        .then((res) => {
          if (!res.ok) throw new Error(`gateway replied ${res.status}`);
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
