// Resolve the node this build talks to — the lifecycle side of the seam.
//
// Web build: the node URL comes from VITE_DUCKTAPE_NODE_URL (or the daemon's
// dev default); there is nothing to manage, we only connect. No onboarding —
// the web user's node is provisioned out of band.
//
// Desktop build: the node is one of the user's ~/.ducktape WORKSPACES. Which
// workspace (and thus which http url) is the registry's call — see
// workspace-client.ts; the Rust `workspace_select` command spawns/adopts that
// workspace's node detached and hands back its url. This module only turns a
// url into a transport and polls it up; workspace selection lives in the store.
// Stopping is plain http — POST /v1/shutdown — because the node's port is its
// identity; no pid crosses this boundary.

import { remoteTransport } from "./transport";
import type { NodeTransport } from "./transport";

// ── Types ───────────────────────────────────────────────

export interface NodeResolution {
  transport: NodeTransport;
  url: string;
  /** True when this app owns the node lifecycle (desktop workspaces). */
  managed: boolean;
}

// ── Resolution ──────────────────────────────────────────

const DEFAULT_LISTEN = "127.0.0.1:8844";
const POLL_ATTEMPTS = 40;
const POLL_DELAY_MS = 250;

export const isTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const webUrl = (): string =>
  import.meta.env.VITE_DUCKTAPE_NODE_URL || `http://${DEFAULT_LISTEN}`;

/** Web build: dial the configured node url. Nothing to manage. */
export const resolveNode = (): NodeResolution => {
  const url = webUrl();
  return { transport: remoteTransport(url), url, managed: false };
};

/** Desktop build: wrap a selected workspace's node url as a managed
 *  resolution. The Rust side already spawned/adopted the process. */
export const connectWorkspace = (httpUrl: string): NodeResolution => ({
  transport: remoteTransport(httpUrl),
  url: httpUrl,
  managed: true,
});

/** Poll /v1/status until the node answers, or reject after `attempts`. */
export const waitUntilUp = (
  transport: NodeTransport,
  attempts: number = POLL_ATTEMPTS,
): Promise<void> =>
  transport.status().then(
    () => undefined,
    (err) =>
      attempts <= 1
        ? Promise.reject(new Error(`the node did not come up: ${err}`))
        : wait(POLL_DELAY_MS).then(() => waitUntilUp(transport, attempts - 1)),
  );

/** Ask a node to exit gracefully (POST /v1/shutdown). */
export const shutdownNode = (url: string): Promise<void> =>
  Promise.resolve()
    .then(() => fetch(`${url.replace(/\/$/, "")}/v1/shutdown`, { method: "POST" }))
    .then((res) => {
      if (!res.ok) throw new Error(`shutdown failed: ${res.status}`);
    });

// ── Helpers ─────────────────────────────────────────────

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));
