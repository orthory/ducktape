// The console's module contract — the frontend twin of the node's module
// registry. A screen appears by registering an AppModule; the shell (sidebar,
// screen resolver) knows no module by name. Ported from the Ducktape Console
// design source, trimmed to what this node serves: no activation rows yet, so
// every registered module is active.

import type { ComponentType } from "react";

import type { IconName } from "../components/Icon";

/** The two sidebar partitions: the "user" (account) surfaces — participant
 *  apps plus the network-data views any account with standing may read — and
 *  the "operator" node-control surfaces. The operator rail is a CONDITIONAL
 *  surface (ADR 2026-07-14 account-node-access-model, A5/A6): it exists only
 *  while node control is available, absent — not disabled — otherwise.
 *  Within a rail, in-view role checks own op-level authority. */
export type NavSection = "user" | "operator";

/** Availability inputs for the registry filters. */
export interface ModuleFilter {
  /** ADR A5: the operator section exists only while the connected node is
   *  controllable (today a managed local daemon; later also a remote node
   *  whose private RPC an owner key can reach). */
  nodeControl: boolean;
  /** Direct remote client (no local workspace): account surfaces whose
   *  client-mode data plane is pending (ADR A3) are hidden. */
  clientMode: boolean;
}

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
