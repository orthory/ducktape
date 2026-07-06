// Typed client for the node's `profiles` module — the TS mirror of
// `crates/apps/profiles-interface`. A profile is a display name keyed by the
// submitter's identity: SetName is ORIGIN-GATED (the module keys on the
// verified submit origin, so it sets the submitter's OWN name only — pass the
// origin like chat's writes). Same contract as tasks-client/chat-client:
// camelCase params in, verbatim serde wire out, pure functions over an injected
// NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (ProfileReply payloads, verbatim) ────────

export interface Profile {
  /** The origin bytes — identical to the bytes in `AuthorRef::User(bytes)`. */
  key: number[];
  display_name: string;
  updated_at: number;
}

const TARGET = "profiles";

/** Query page bound mirrored from the interface crate (MAX_QUERY_LIMIT). */
export const MAX_QUERY_LIMIT = 256;

// ── Msgs (writes — origin-gated: sets the submitter's own name) ──

export const setName = (
  transport: NodeTransport,
  params: { displayName: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { set_name: { display_name: params.displayName } },
    params.origin,
  );

// ── Queries (reads over committed state) ────────────────

/** Every profile, ascending by key. */
export const allProfiles = (
  transport: NodeTransport,
  { from = 0, limit = MAX_QUERY_LIMIT }: { from?: number; limit?: number } = {},
): Promise<Profile[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { all: { from, limit } }))
    .then((reply) => replyVariant<Profile[]>(reply, "profiles"));
