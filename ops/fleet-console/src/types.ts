// Shape of /fleet.json, emitted by ops/fleet.sh. Kept in one place so the data
// hook and every view (grid now, graph later) share one contract.

export type FleetStatus = "up" | "down" | "building";

export interface FleetHead {
  sha: string;
  subject: string;
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
  slot?: number;
  display?: string;
  vncPort?: number;
  token?: string;
}

export interface Fleet {
  generatedAt: string;
  host: string;
  webPort: number;
  base: string;
  worktrees: FleetNode[];
}
