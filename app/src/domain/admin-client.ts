// The app's end of the node's OWNER CONTROL-PLANE namespace (ADR A2/A5): the
// `/v1/admin/*` routes that carry lifecycle, staging and diagnostics.
//
// Two exposure regimes, transparently:
//  - LOCAL (loopback) admin is loopback-trusted, so calls go UNSIGNED — the
//    signer is undefined and no PoP headers are attached.
//  - A REMOTE (Public) owned node requires a per-request proof-of-possession
//    signed by the account key. `adminSigner()` mints it via the Rust
//    `user_sign_admin` verb, which returns the exact `{key, ts, sig}` the node's
//    gate checks (single source of truth: `noded::admin::sign_admin`).
//
// This module is deliberately self-contained (no store imports) so it composes
// cleanly with whatever shape the console store settles into.

import { isTauri } from "./node-bootstrap";

/** The `x-ducktape-admin-*` header material for one owner request. */
export interface AdminAuth {
  key: string;
  ts: string;
  sig: string;
}

/** Signs one admin request: (method, path) -> the PoP the node verifies.
 *  Undefined outside Tauri (web build: no user-key custody). */
export type AdminSigner = (method: string, path: string) => Promise<AdminAuth>;

/** The desktop shell's admin signer, or undefined on the web build. Rejects
 *  with `identity-locked` when the account key is encrypted and uncached —
 *  the caller should treat that as "not reachable", never mis-attribute. */
export const adminSigner = (): AdminSigner | undefined =>
  isTauri()
    ? async (method: string, path: string) => {
        const { invoke } = await import("@tauri-apps/api/core");
        const raw = await invoke<string>("user_sign_admin", { method, path });
        return JSON.parse(raw) as AdminAuth;
      }
    : undefined;

const base = (url: string): string => url.replace(/\/$/, "");

/** Build the request headers for an admin call: the PoP triplet when a signer
 *  is supplied (remote owner control), or none (loopback-trusted local). */
const authHeaders = async (
  method: string,
  path: string,
  sign?: AdminSigner,
): Promise<Record<string, string>> => {
  if (!sign) return {};
  const { key, ts, sig } = await sign(method, path);
  return {
    "x-ducktape-admin-key": key,
    "x-ducktape-admin-ts": ts,
    "x-ducktape-admin-sig": sig,
  };
};

/** GET /v1/admin/ping — does the owner-gated control surface answer for us?
 *  This is exactly the "admin namespace reachable" term of nodeControlAvailable.
 *  Never throws: a refusal, a network error, or a locked key all read as false. */
export const probeAdmin = async (url: string, sign?: AdminSigner): Promise<boolean> => {
  const path = "/v1/admin/ping";
  try {
    const res = await fetch(`${base(url)}${path}`, {
      headers: await authHeaders("GET", path, sign),
    });
    return res.ok;
  } catch {
    return false;
  }
};

/** POST /v1/admin/shutdown — retire the node through its owner-gated surface.
 *  Local nodes need no signer; a remote owned node passes one. */
export const adminShutdown = async (url: string, sign?: AdminSigner): Promise<void> => {
  const path = "/v1/admin/shutdown";
  const res = await fetch(`${base(url)}${path}`, {
    method: "POST",
    headers: await authHeaders("POST", path, sign),
  });
  if (!res.ok) throw new Error(`shutdown failed: ${res.status}`);
};
