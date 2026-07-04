import type { ConsoleState } from "./state";

// A deliberately tiny reducer: two operations cover every transition. `patch`
// merges a partial; `update` merges the result of a function of current state.
// Domain behaviour lives in the actions facade.
export type Action =
  | { type: "patch"; patch: Partial<ConsoleState> }
  | { type: "update"; fn: (state: ConsoleState) => Partial<ConsoleState> };

export function reducer(state: ConsoleState, action: Action): ConsoleState {
  switch (action.type) {
    case "patch":
      return { ...state, ...action.patch };
    case "update": {
      // An empty result is a deliberate no-op (e.g. a telemetry frame deduped on
      // height) — keep the SAME reference so React bails out of the re-render
      // instead of churning on an identical-content copy.
      const next = action.fn(state);
      return Object.keys(next).length > 0 ? { ...state, ...next } : state;
    }
    default:
      return state;
  }
}
