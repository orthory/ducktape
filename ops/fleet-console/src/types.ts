// Shape of /fleet.json, emitted by ops/fleet.sh. Kept in one place so the data
// hook and every view (grid now, graph later) share one contract.

export type FleetStatus = "up" | "down" | "building";

export interface FleetHead {
  sha: string;
  subject: string;
}

export interface FleetCommit {
  sha: string;
  subject: string;
  age: string;
}

// What an agent has been doing to this branch. `dirty` (uncommitted edits) is
// the live pulse; `commits` is the recent trail.
export interface FleetActivity {
  dirty: number;
  commits: FleetCommit[];
}

export interface FleetAgentObserve {
  protocol: "tauri-agent-observe-ndjson";
  cwd: string;
  env: {
    XDG_RUNTIME_DIR: string;
  };
  argv: string[];
}

export interface FleetAgent {
  appId: string;
  runtimeDir: string;
  endpointPath: string;
  endpointReady: boolean;
  observe: FleetAgentObserve;
}

export interface FleetNode {
  id: string;
  branch: string;
  path: string;
  head: FleetHead;
  parent: string | null;
  ahead: number;
  behind: number;
  status: FleetStatus;
  activity?: FleetActivity;
  slot?: number;
  display?: string;
  vncPort?: number;
  token?: string;
  agent?: FleetAgent;
}

export interface Fleet {
  generatedAt: string;
  host: string;
  webPort: number;
  base: string;
  worktrees: FleetNode[];
}
