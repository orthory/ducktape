// The build-time module registry. Adding a console surface = one folder under
// views/ + one entry here.

import { AgentView } from "../views/agent/AgentView";
import { AutomationsView } from "../views/automations/AutomationsView";
import { ChatView } from "../views/chat/ChatView";
import { DocumentView } from "../views/document/DocumentView";
import { ExplorerView } from "../views/explorer/ExplorerView";
import { FilesView } from "../views/files/FilesView";
import { ForgeView } from "../views/forge/ForgeView";
import { GovernanceView } from "../views/governance/GovernanceView";
import { InboxView } from "../views/inbox/InboxView";
import { JobsView } from "../views/jobs/JobsView";
import { MemoryView } from "../views/memory/MemoryView";
import { MembersView } from "../views/members/MembersView";
import { ModulesView } from "../views/modules/ModulesView";
import { PagesView } from "../views/pages/PagesView";
import { SearchView } from "../views/search/SearchView";
import { StatusView } from "../views/status/StatusView";
import { TasksView } from "../views/tasks/TasksView";
import { TelemetryView } from "../views/telemetry/TelemetryView";
import type { AppModule, NavSection } from "./module-def";

// The sidebar's view-mode toggle partitions these into two rails:
//   USER          — the participant apps (chat, tasks, docs, forge, agents)
//   NODE OPERATOR — the node/network surfaces (members, approvals, modules,
//                   node, telemetry)
// `order` is a sort key WITHIN a section, so the two rails number from 0
// independently.
export const MODULES: AppModule[] = [
  // ── User apps ──
  { id: "search", nav: { icon: "search", label: "Search", order: -1, section: "user" }, Screen: SearchView },
  { id: "chat", nav: { icon: "chat", label: "Chat", order: 0, section: "user" }, Screen: ChatView },
  { id: "inbox", nav: { icon: "inbox", label: "Inbox", order: 1, section: "user" }, Screen: InboxView },
  { id: "tasks", nav: { icon: "tasks", label: "Tasks", order: 2, section: "user" }, Screen: TasksView },
  { id: "document", nav: { icon: "document", label: "Docs", order: 3, section: "user" }, Screen: DocumentView },
  { id: "pages", nav: { icon: "pages", label: "Pages", order: 4, section: "user" }, Screen: PagesView },
  { id: "files", nav: { icon: "files", label: "Files", order: 5, section: "user" }, Screen: FilesView },
  { id: "memory", nav: { icon: "memory", label: "Memory", order: 6, section: "user" }, Screen: MemoryView },
  { id: "forge", nav: { icon: "forge", label: "Forge", order: 7, section: "user" }, Screen: ForgeView },
  { id: "agent", nav: { icon: "agent", label: "Agents", order: 8, section: "user" }, Screen: AgentView },
  // ── Node operator surfaces ──
  { id: "members", nav: { icon: "members", label: "Members", order: 0, section: "operator" }, Screen: MembersView },
  { id: "governance", nav: { icon: "approvals", label: "Approvals", order: 1, section: "operator" }, Screen: GovernanceView },
  { id: "jobs", nav: { icon: "jobs", label: "Jobs", order: 2, section: "operator" }, Screen: JobsView },
  { id: "automations", nav: { icon: "automations", label: "Autos", order: 3, section: "operator" }, Screen: AutomationsView },
  { id: "modules", nav: { icon: "modules", label: "Modules", order: 4, section: "operator" }, Screen: ModulesView },
  { id: "status", nav: { icon: "node", label: "Node", order: 5, section: "operator" }, Screen: StatusView },
  { id: "telemetry", nav: { icon: "telemetry", label: "Telemetry", order: 6, section: "operator" }, Screen: TelemetryView },
  { id: "explorer", nav: { icon: "hash", label: "Explorer", order: 7, section: "operator" }, Screen: ExplorerView },
];

export const moduleById = (id: string): AppModule | undefined =>
  MODULES.find((m) => m.id === id);

/** The modules of one view-mode rail, ordered. */
export const modulesInSection = (section: NavSection): AppModule[] =>
  MODULES.filter((m) => m.nav.section === section).sort(
    (a, b) => a.nav.order - b.nav.order,
  );

/** Which view-mode rail owns a screen id, or null for the shell's own screens
 *  (settings) and unknown ids. */
export const sectionForScreen = (screen: string): NavSection | null =>
  moduleById(screen)?.nav.section ?? null;

/** The default screen a rail lands on (its first, lowest-order module). */
export const defaultScreenForSection = (section: NavSection): string =>
  modulesInSection(section)[0]?.id ?? "chat";
