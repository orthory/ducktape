// Typed client for the consensus `duckdns` module. Identity owns accounts and
// display names; DuckDNS owns only an optional human handle -> AccountId alias
// and identity-only service discovery. Registering a handle is never required
// to create/join a workspace or to use an account.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

export const TARGET = "duckdns";
export const MAX_LABEL_LEN = 63;
export const MAX_QUERY_LIMIT = 256;
export const RESERVED_ROOT_LABELS = new Set(["net"]);

export interface HandleRegistration {
  handle: string;
  account_id: number[];
}

export type DuckDnsName =
  | { account: { handle: string } }
  | { account_service: { service: string; handle: string } }
  | { network_service: { service: string; chain: string } }
  | { node_service: { service: string; node: string; chain: string } };

export interface ResolvedNode {
  node: number[];
  node_label: string;
}

export type ResolvedName =
  | { account: { account_id: number[]; nodes: ResolvedNode[] } }
  | {
      service: {
        identity: { scope: "account" | "network"; service: string };
        authority: { account: { account_id: number[] } } | "network";
        providers: ResolvedNode[];
      };
    };

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

/** Resolve a typed `.duck` name to stable AccountId/NodeId identities. No IP,
 * endpoint, port, or transport metadata crosses this boundary. */
export const resolve = (
  transport: NodeTransport,
  name: DuckDnsName,
): Promise<ResolvedName | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { resolve: { name } }))
    .then((reply) => replyVariant<ResolvedName | null>(reply, "resolved"));
