// Typed client for the node's `forge` module — the TS mirror of
// `crates/apps/forge-interface`. forge is git-backed: one Commit msg == one git
// commit, so HEAD (and thus the module root) advances per write; the only read
// is the current HEAD oid. Same contract as chat-client/tasks-client: pure
// functions over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

const TARGET = "forge";

// ── Msgs (writes — one commit per submit) ───────────────

export const commit = (
  transport: NodeTransport,
  params: { path: string; content: string; message: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      Commit: {
        path: params.path,
        content: params.content,
        message: params.message,
      },
    },
    params.origin,
  );

// ── Queries (reads over committed state) ────────────────

/** The current HEAD commit oid (40-char sha1 hex), or null on an unborn repo
 *  (no commits yet). This hex is the state root's preimage: forge's `root()` is
 *  sha256 of the oid's raw bytes. */
export const head = (transport: NodeTransport): Promise<string | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "Head"))
    .then((reply) => replyVariant<string | null>(reply, "Head"));
