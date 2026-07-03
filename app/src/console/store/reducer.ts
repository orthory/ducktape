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
    case "update":
      return { ...state, ...action.fn(state) };
    default:
      return state;
  }
}
