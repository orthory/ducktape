// The console's module contract — the frontend twin of the node's module
// registry. A screen appears by registering an AppModule; the shell (sidebar,
// screen resolver) knows no module by name. Ported from the Ducktape Console
// design source, trimmed to what this node serves: no activation rows yet, so
// every registered module is active.

import type { ComponentType } from "react";

import type { IconName } from "../components/Icon";

export interface ModuleNav {
  icon: IconName;
  label: string;
  order: number;
}

export interface AppModule {
  /** Matches the node-side module id where one exists (chat, tasks). */
  id: string;
  nav: ModuleNav;
  Screen: ComponentType;
}
