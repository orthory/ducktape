// Typed client for the node's `upgrade` system module — the TS mirror of
// `crates/system/upgrade/src/interface.rs`. Read-only surface: the agreed
// `current_version`, the single pending `ScheduledUpgrade` (at most one), and
// the per-validator readiness verdict. Governance AUTHORIZES a schedule/cancel
// (a ScheduleUpgrade/CancelUpgrade proposal), each validator signals readiness
// out of band, and the boundary tick arms once every member is ready — so this
// console only READS status here and drives schedule/cancel through the
// governance module.

import type { NodeTransport } from "./transport";
import { replyVariant } from "./wire";

const TARGET = "upgrade";

/** The coordinates of a scheduled upgrade — at most one is ever pending. */
export interface ScheduledUpgrade {
  name: string;
  activation_height: number;
  to_version: number;
}

/** The readable projection of the upgrade module's state (UpgradeStatus). */
export interface UpgradeStatus {
  current_version: number;
  /** The pending upgrade, or null when none is scheduled. */
  pending: ScheduledUpgrade | null;
  /** The boundary member set the verdict was computed against, sorted. */
  members: number[][];
  /** Readiness keys, sorted. */
  ready: number[][];
  member_count: number;
  ready_count: number;
  /** `pending` set AND every boundary member has signalled ready. */
  armed: boolean;
}

// ── Queries (reads) ─────────────────────────────────────

// `UpgradeQuery::Status` is a snake_case unit variant, so it encodes as the bare
// string "status" — the same shape valset passes for its unit queries.
export const status = (transport: NodeTransport): Promise<UpgradeStatus> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "status"))
    .then((reply) => replyVariant<UpgradeStatus>(reply, "status"));
