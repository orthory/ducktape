// The build-time module registry. Adding a console surface = one folder under
// views/ + one entry here.

import { AgentView } from "../views/agent/AgentView";
import { ChatView } from "../views/chat/ChatView";
import { ExplorerView } from "../views/explorer/ExplorerView";
import { BrowserView } from "../views/browser/BrowserView";
import { FilesView } from "../views/files/FilesView";
import { ForgeView } from "../views/forge/ForgeView";
import { GatewayView } from "../views/gateway/GatewayView";
import { GovernanceView } from "../views/governance/GovernanceView";
import { MembersView } from "../views/members/MembersView";
import { ModulesView } from "../views/modules/ModulesView";
import { PagesView } from "../views/pages/PagesView";
import { MetricsView } from "../views/metrics/MetricsView";
import { SandboxView } from "../views/sandbox/SandboxView";
import { StatusView } from "../views/status/StatusView";
import type { AppModule, ModuleFilter, NavSection } from "./module-def";

// The sidebar's view-mode toggle partitions these into two rails:
//   USER — the account surfaces: participant apps plus the network-data
//          views (members, governance, explorer) any account with standing
//          may read. Admin affordances inside them stay role-gated in-view.
//   NODE — node control (console, gateway, modules, sandbox, metrics). This
//          rail exists only while node control is available (ADR A5/A6).
// `order` is a sort key WITHIN a section, so the two rails number from 0
// independently. Cross-module search is NOT a module — it is the ⌘K overlay
// the shell owns (see SearchModal), reachable from either rail.
export const MODULES: AppModule[] = [
  // ── User / account surfaces ──
  { id: "chat", nav: { icon: "chat", label: "Chat", order: 0, section: "user" }, Screen: ChatView },
  { id: "pages", nav: { icon: "pages", label: "Pages", order: 1, section: "user" }, Screen: PagesView },
  { id: "files", nav: { icon: "files", label: "Files", order: 2, section: "user" }, Screen: FilesView },
  { id: "browser", nav: { icon: "browser", label: "Browser", order: 3, section: "user" }, Screen: BrowserView },
  { id: "forge", nav: { icon: "forge", label: "Forge", order: 4, section: "user" }, Screen: ForgeView },
  { id: "agent", nav: { icon: "agent", label: "Agents", order: 5, section: "user" }, Screen: AgentView },
  { id: "members", nav: { icon: "members", label: "Members", order: 6, section: "user" }, Screen: MembersView },
  { id: "governance", nav: { icon: "governance", label: "Governance", order: 7, section: "user" }, Screen: GovernanceView },
  { id: "explorer", nav: { icon: "hash", label: "Explorer", order: 8, section: "user" }, Screen: ExplorerView },
  // ── Node control (conditional rail) ──
  { id: "status", nav: { icon: "node", label: "Node", order: 0, section: "operator" }, Screen: StatusView },
  { id: "gateway", nav: { icon: "link", label: "Gateway", order: 1, section: "operator" }, Screen: GatewayView },
  { id: "modules", nav: { icon: "modules", label: "Modules", order: 2, section: "operator" }, Screen: ModulesView },
  { id: "sandbox", nav: { icon: "sandbox", label: "Sandbox", order: 3, section: "operator" }, Screen: SandboxView },
  { id: "metrics", nav: { icon: "metrics", label: "Metrics", order: 4, section: "operator" }, Screen: MetricsView },
];

export const moduleById = (id: string): AppModule | undefined =>
  MODULES.find((m) => m.id === id);

/** Account surfaces whose client-mode projections aren't wired yet (the ADR's
 *  pending A3 work); a direct remote client hides them until that lands. */
const CLIENT_PENDING = new Set(["members", "governance"]);

/** Which modules exist for this connection. The operator section exists only
 *  while node control is available (ADR A5/A6) — absent, not disabled. */
export const moduleAvailable = (id: string, filter: ModuleFilter): boolean => {
  const mod = moduleById(id);
  if (!mod) return false;
  if (mod.nav.section === "operator") return filter.nodeControl;
  return !(filter.clientMode && CLIENT_PENDING.has(id));
};

/** The modules of one view-mode rail, ordered. */
export const modulesInSection = (section: NavSection, filter: ModuleFilter): AppModule[] =>
  MODULES.filter((m) => m.nav.section === section && moduleAvailable(m.id, filter)).sort(
    (a, b) => a.nav.order - b.nav.order,
  );

/** Which view-mode rail owns a screen id, or null for the shell's own screens
 *  (settings) and unknown ids. */
export const sectionForScreen = (screen: string): NavSection | null =>
  moduleById(screen)?.nav.section ?? null;

/** The default screen a rail lands on (its first available module; an empty
 *  rail — the operator section without node control — falls back to chat). */
export const defaultScreenForSection = (section: NavSection, filter: ModuleFilter): string =>
  modulesInSection(section, filter)[0]?.id ?? "chat";
