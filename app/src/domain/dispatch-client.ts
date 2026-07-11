// Typed client for the node's `dispatch` module read surface — the
// single-dispatch view, which the console joins to a PendingRun (by
// `dispatch_id`) to show WHICH node is executing a run. `assignee` is the
// saga's lease holder, resolved at query time by the dispatch read facade;
// it is present only while the run is in flight (`awaiting_result`) and null
// once the result has delivered. Pure functions over an injected NodeTransport.

import { keyHex } from "./chat-client";
import type { NodeTransport } from "./transport";
import { replyVariant } from "./wire";

const TARGET = "dispatch";

/** Where a dispatch is in its lifecycle (mirrors `DispatchStatus`). */
export type DispatchStatus =
  | { awaiting_result: { saga_id: string } }
  | "awaiting_delivery"
  | "delivered";

/** One dispatch's observable state (mirrors `DispatchView`). `assignee` is the
 *  node key (raw bytes) holding the saga lease — present only while
 *  `awaiting_result`, null otherwise. `outcome` is unused here (left `unknown`
 *  to avoid pinning the Result wire shape). */
export interface DispatchView {
  dispatch_id: string;
  recipe_id: string;
  receiver: string;
  status: DispatchStatus;
  outcome: unknown;
  assignee: number[] | null;
  attempt: number | null;
  max_attempts: number | null;
  lease_expires_at: number | null;
  deadline: number | null;
  lease_updated_at: number | null;
  reassignable: boolean | null;
  created_at: number;
  updated_at: number;
}

/** One dispatch, addressed as its receiver knows it. `receiver` is "runs" for
 *  every agent run. Null when the dispatch is unknown (already pruned). */
export const dispatch = (
  transport: NodeTransport,
  params: { dispatchId: string; receiver?: string },
): Promise<DispatchView | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        dispatch: {
          receiver: params.receiver ?? "runs",
          dispatch_id: params.dispatchId,
        },
      }),
    )
    .then((reply) => replyVariant<DispatchView | null>(reply, "dispatch"));

/** The run's current executor node as a hex key, or null when it isn't
 *  in-flight/assigned. */
export const assigneeHex = (view: DispatchView | null): string | null =>
  view?.assignee ? keyHex(view.assignee) : null;

export interface RunLease {
  assigneeHex: string | null;
  attempt: number;
  maxAttempts: number;
  expiresAt: number | null;
  deadline: number | null;
  updatedAt: number | null;
  reassignable: boolean;
}

export const runLease = (view: DispatchView | null): RunLease | null =>
  view?.attempt === null || view?.attempt === undefined
    ? null
    : {
        assigneeHex: assigneeHex(view),
        attempt: view.attempt,
        maxAttempts: view.max_attempts ?? 1,
        expiresAt: view.lease_expires_at,
        deadline: view.deadline,
        updatedAt: view.lease_updated_at,
        reassignable: view.reassignable ?? false,
      };
