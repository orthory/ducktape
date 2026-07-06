// The console's module contract — the frontend twin of the node's module
// registry. A screen appears by registering an AppModule; the shell (sidebar,
// screen resolver) knows no module by name. Ported from the Ducktape Console
// design source, trimmed to what this node serves: no activation rows yet, so
// every registered module is active.

import type { ComponentType } from "react";

import type { IconName } from "../components/Icon";

/** The two sidebar partitions the view-mode toggle switches between: the
 *  participant "user" apps, and the "operator" node/network surfaces. Neither
 *  section confers authority — it is purely which rail a surface lives on. */
export type NavSection = "user" | "operator";

export interface ModuleNav {
  icon: IconName;
  label: string;
  /** Sort key WITHIN the module's section (orders may repeat across sections). */
  order: number;
  /** Which view-mode rail the module joins. */
  section: NavSection;
}

export interface AppModule {
  /** Matches the node-side module id where one exists (chat, files). */
  id: string;
  nav: ModuleNav;
  Screen: ComponentType;
}
