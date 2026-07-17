// Resolve the node this build talks to — the lifecycle side of the seam.
//
// Web build: the node URL comes from VITE_DUCKTAPE_NODE_URL (or the daemon's
// dev default); there is nothing to manage, we only connect. No onboarding —
// the web user's node is provisioned out of band.
//
// The native iced app owns workspace lifecycle independently. This static web
// twin never manages a local process; it only dials its configured node.
// Stopping rides the owner-gated control surface — POST /v1/admin/shutdown
// (ADR A2, see admin-client.ts) — because the node's port is its identity; no
// pid crosses this boundary.

import { remoteTransport } from "./transport";
import type { NodeTransport } from "./transport";
import { adminShutdown } from "./admin-client";
import type { AdminSigner } from "./admin-client";

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

export const hasNativeShell = (): boolean => false;

/** Explicit fallback for actions whose implementation lives only in iced. */
export const nativeCall = <T>(feature: string, _args?: unknown): Promise<T> =>
  Promise.reject(new Error(`${feature} is available only in the native desktop app`));

/** The static web twin never overlays native macOS window controls. */
export const isMacDesktop = (): boolean => false;

const webUrl = (): string =>
  import.meta.env.VITE_DUCKTAPE_NODE_URL || `http://${DEFAULT_LISTEN}`;

/** The static web twin has no local user-key custody. */
const filesFrameSigner = (): ((_payloadHex: string) => Promise<string>) | undefined =>
  undefined;

/** The desktop shell's generic content-op signer, or undefined on the web.
 *  Used for REMOTE connections so every op is authored as the connecting
 *  user's key (`ext:<user-pubkey>`) — authorized by the remote node's
 *  client-standing door — instead of the remote node re-signing with its own
 *  key. The shell command gates the target to content modules and rejects a
 *  locked key with `identity-locked` (the transport surfaces it, no silent
 *  fallback). */
const contentFrameSigner = ():
  | ((_target: string, _payloadHex: string) => Promise<string>)
  | undefined => undefined;

/** Web build: dial the configured node url. Nothing to manage. */
export const resolveNode = (): NodeResolution => {
  const url = webUrl();
  return { transport: remoteTransport(url), url, managed: false };
};

/** Desktop build: wrap a selected workspace's node url as a managed
 *  resolution. The Rust side already spawned/adopted the process. */
export const connectWorkspace = (httpUrl: string): NodeResolution => ({
  transport: remoteTransport(httpUrl, {
    signFilesPayload: filesFrameSigner(),
    // governance ops are account-signed on EVERY connection (ADR A1): local
    // content stays frameless (node re-signs), but admit/promote/demote/leave
    // ride `submitControl` → user-signed frames, so the module's standing ACL
    // resolves the user's account via BindNode.
    signControlPayload: contentFrameSigner(),
  }),
  url: httpUrl,
  managed: true,
});

/** Connect to a node running on ANOTHER device, reachable over plain
 *  http/https. Unmanaged: this app only dials the url the user gave — it never
 *  spawns, adopts, or stops the process (so the daemon controls stay hidden,
 *  same as the web build). The transport is already url-agnostic; this is just
 *  the lifecycle label. */
export const connectRemote = (httpUrl: string): NodeResolution => ({
  // every op is authored by THIS user's key, not the remote node's: `submit`
  // rides the authenticated frame lane (`signPayload`) so the remote node's
  // client-standing door authorizes the connecting user as their own bounded
  // identity, and files commits stay user-signed too (`signFilesPayload`).
  transport: remoteTransport(httpUrl, {
    signFilesPayload: filesFrameSigner(),
    signPayload: contentFrameSigner(),
    // governance ops sign on a remote connection too (same account-key lane).
    signControlPayload: contentFrameSigner(),
  }),
  url: httpUrl,
  managed: false,
});

/** Coerce user input into a dial-able node url: accept a full `http(s)://…`
 *  verbatim, and default a bare `host` / `host:port` to `http://`. Trailing
 *  slashes are the transport's to strip. Empty in → empty out (caller guards). */
export const normalizeNodeUrl = (raw: string): string => {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  const withScheme = /^https?:\/\//i.test(trimmed) ? trimmed : `http://${trimmed}`;
  try {
    const parsed = new URL(withScheme);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return "";
    // reduce to the origin — the transport appends its own /v1/… paths, so a
    // pasted url carrying a path/query would otherwise 404 every call.
    return parsed.origin;
  } catch {
    return ""; // unparseable — the caller guards on empty
  }
};

/** Poll /v1/status until the node answers, or reject after `attempts`. */
export const waitUntilUp = (
  transport: NodeTransport,
  attempts: number = POLL_ATTEMPTS,
): Promise<void> =>
  transport.status().then(
    () => undefined,
    (err: unknown) => {
      // Only keep polling while the node isn't answering YET (refused/timeout).
      // A node that IS up but erroring (httpError) or returning a non-ducktape
      // body (badBody) won't heal by waiting — fail fast with the real reason so
      // the UI shows "returned 500" / "not a ducktape node" instead of a 10s
      // spinner ending in a generic timeout. An error with no kind stays
      // transient, preserving the prior retry behaviour.
      const kind = (err as { kind?: string } | null)?.kind;
      const transient = kind === undefined || kind === "refused" || kind === "timeout";
      if (!transient || attempts <= 1) {
        const detail = err instanceof Error ? err.message : String(err);
        return Promise.reject(new Error(`the node did not come up: ${detail}`));
      }
      return wait(POLL_DELAY_MS).then(() => waitUntilUp(transport, attempts - 1));
    },
  );

/** Ask a node to exit gracefully through its owner-gated control surface
 *  (POST /v1/admin/shutdown — ADR A2). A local (loopback) node needs no
 *  signature; a remote owned node passes one via `adminShutdown`. */
export const shutdownNode = (url: string, sign?: AdminSigner): Promise<void> =>
  adminShutdown(url, sign);

// ── Helpers ─────────────────────────────────────────────

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));
