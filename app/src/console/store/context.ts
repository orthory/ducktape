import { createContext } from "react";

import type { NodeTransport } from "../../domain/transport";
import type { ConsoleActions } from "./actions";
import type { ConsoleState } from "./state";

export interface ConsoleContextValue {
  state: ConsoleState;
  actions: ConsoleActions;
  /** The live node transport, for views that drive interactive reads directly
   *  (the files browser pages ls/read/history off it). Null when no node is
   *  resolved; optional so a test provider can omit it. Writes still ride the
   *  actions facade; this is the read seam. */
  transport?: NodeTransport | null;
}

export const ConsoleContext = createContext<ConsoleContextValue | null>(null);
