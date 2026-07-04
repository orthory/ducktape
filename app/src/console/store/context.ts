import { createContext } from "react";

import type { ConsoleActions } from "./actions";
import type { ConsoleState } from "./state";

export interface ConsoleContextValue {
  state: ConsoleState;
  actions: ConsoleActions;
}

export const ConsoleContext = createContext<ConsoleContextValue | null>(null);
