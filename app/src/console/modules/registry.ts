// The build-time module registry. Adding a console surface = one folder under
// views/ + one entry here.

import { AgentView } from "../views/agent/AgentView";
import { ChatView } from "../views/chat/ChatView";
import { DocumentView } from "../views/document/DocumentView";
import { ForgeView } from "../views/forge/ForgeView";
import { StatusView } from "../views/status/StatusView";
import { TasksView } from "../views/tasks/TasksView";
import type { AppModule } from "./module-def";

export const MODULES: AppModule[] = [
  { id: "chat", nav: { icon: "chat", label: "Chat", order: 0 }, Screen: ChatView },
  { id: "tasks", nav: { icon: "tasks", label: "Tasks", order: 1 }, Screen: TasksView },
  { id: "forge", nav: { icon: "forge", label: "Forge", order: 2 }, Screen: ForgeView },
  { id: "document", nav: { icon: "document", label: "Docs", order: 3 }, Screen: DocumentView },
  { id: "agent", nav: { icon: "agent", label: "Agents", order: 5 }, Screen: AgentView },
  // Node (status) stays last on the rail, so it sits after Agents.
  { id: "status", nav: { icon: "node", label: "Node", order: 6 }, Screen: StatusView },
];

export const moduleById = (id: string): AppModule | undefined =>
  MODULES.find((m) => m.id === id);
