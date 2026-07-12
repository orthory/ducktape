// Typed client for the consensus `duckdns` module. Identity owns accounts and
// display names; DuckDNS owns only an optional human handle -> AccountId alias.
// Registering a handle is never required to create/join a workspace or use an
// account, and resolution never returns nodes or routes.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

export const TARGET = "duckdns";
export const MAX_LABEL_LEN = 63;
export const MAX_QUERY_LIMIT = 256;
/** Mirrors `RESERVED_ROOT_LABELS` in crates/system/duckdns/src/wire.rs — that
 * const is the source of truth (consensus rejects these handles) and
 * duckdns-client.test.ts reads it to keep this copy from drifting. */
export const RESERVED_ROOT_LABELS = new Set(["net", "agents"]);

export interface HandleRegistration {
  handle: string;
  account_id: number[];
}

export interface DuckDnsName {
  handle: string;
}

export interface ResolvedAccount {
  account_id: number[];
}

/** Canonical form accepted by consensus. Registration is strict, while DNS
 * lookup itself remains case-insensitive. */
export const normalizeHandle = (value: string): string => value.trim().toLowerCase();

export const handleError = (handle: string): string | null => {
  if (!handle) return "Enter a name.";
  if (handle.length > MAX_LABEL_LEN) return `Use ${MAX_LABEL_LEN} characters or fewer.`;
  if (handle.startsWith("-") || handle.endsWith("-")) {
    return "A name cannot start or end with a hyphen.";
  }
  if (!/^[a-z0-9-]+$/.test(handle)) return "Use lowercase letters, numbers, and hyphens.";
  if (RESERVED_ROOT_LABELS.has(handle)) return `${handle}.duck is reserved.`;
  return null;
};

/** Declaratively set or clear this authenticated account's optional handle. */
export const setHandle = (
  transport: NodeTransport,
  params: { handle: string | null; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { set_handle: { handle: params.handle } },
    params.origin,
  );

/** One deterministic page of registered handles, ascending by handle. */
export const registrations = (
  transport: NodeTransport,
  { from = 0, limit = MAX_QUERY_LIMIT }: { from?: number; limit?: number } = {},
): Promise<HandleRegistration[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { registrations: { from, limit } }))
    .then((reply) => replyVariant<HandleRegistration[]>(reply, "registrations"));

/** Resolve a typed `.duck` account name to its stable AccountId. Identity and
 * peer management own node lookup and connectivity after this boundary. */
export const resolve = (
  transport: NodeTransport,
  name: DuckDnsName,
): Promise<ResolvedAccount | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { resolve: { name } }))
    .then((reply) => replyVariant<ResolvedAccount | null>(reply, "resolved"));
