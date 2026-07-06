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
 *  in-flight/assigned. The join value for `state.runAssignee`. */
export const assigneeHex = (view: DispatchView | null): string | null =>
  view?.assignee ? keyHex(view.assignee) : null;
