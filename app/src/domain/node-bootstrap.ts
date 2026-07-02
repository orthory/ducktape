// Resolve the node this build talks to — the lifecycle side of the seam.
//
// Web build: the node URL comes from VITE_DUCKTAPE_NODE_URL (or the daemon's
// dev default); there is nothing to manage, we only connect.
//
// Desktop build (tauri webview marker present): the daemon is OURS to manage.
// Probe /v1/status to adopt an already-running orphan; if nothing answers,
// invoke daemon_spawn (the shell launches `ducktape-noded` detached) and poll
// until it comes up. Stopping is plain http — POST /v1/shutdown — because the
// daemon's port is its identity; no pid crosses this boundary.

import { invoke } from "@tauri-apps/api/core";

import { remoteTransport } from "./transport";
import type { NodeTransport } from "./transport";

// ── Types ───────────────────────────────────────────────

export interface NodeResolution {
  transport: NodeTransport;
  url: string;
  /** True when this app owns the daemon lifecycle (desktop build). */
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

export const resolveNode = (): Promise<NodeResolution> => {
  if (!isTauri()) {
    const url = webUrl();
    return Promise.resolve({ transport: remoteTransport(url), url, managed: false });
  }
  const url = `http://${DEFAULT_LISTEN}`;
  const transport = remoteTransport(url);
  return Promise.resolve()
    .then(() => ensureDaemon(transport))
    .then(() => ({ transport, url, managed: true }));
};

/** Adopt a live daemon, or spawn one detached and wait for it to answer. */
export const ensureDaemon = (transport: NodeTransport): Promise<void> =>
  Promise.resolve()
    .then(() => transport.status())
    .then(
      () => undefined, // already up — adopt it
      () =>
        Promise.resolve()
          .then(() => invoke("daemon_spawn", { listen: DEFAULT_LISTEN }))
          .then(() => pollUntilUp(transport, POLL_ATTEMPTS)),
    );

/** Ask the daemon to exit gracefully. */
export const shutdownNode = (url: string): Promise<void> =>
  Promise.resolve()
    .then(() => fetch(`${url.replace(/\/$/, "")}/v1/shutdown`, { method: "POST" }))
    .then((res) => {
      if (!res.ok) throw new Error(`shutdown failed: ${res.status}`);
    });

// ── Helpers ─────────────────────────────────────────────

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

const pollUntilUp = (transport: NodeTransport, attempts: number): Promise<void> =>
  transport.status().then(
    () => undefined,
    (err) =>
      attempts <= 1
        ? Promise.reject(new Error(`the node daemon did not come up: ${err}`))
        : wait(POLL_DELAY_MS).then(() => pollUntilUp(transport, attempts - 1)),
  );
