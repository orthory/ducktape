// Desktop-only local git reader for the node's on-disk forge repository.
// Writes stay on the consensus wire in forge-client.ts; these calls only
// project committed refs/heads/main through Tauri commands.

import { invoke } from "@tauri-apps/api/core";

import { isTauri } from "./node-bootstrap";

export interface RepoInfo {
  id: string;
  name: string;
  branch: "main";
  defaultBranch: "main";
  head: string | null;
  browsable: boolean;
}

export interface CommitInfo {
  id: string;
  summary: string;
  author: string;
  /** Unix seconds from the git commit time. */
  time: number;
}

export interface TreeEntry {
  name: string;
  kind: "dir" | "file";
}

export interface DiffLine {
  origin: string;
  content: string;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  status: string;
  hunks: DiffHunk[];
}

export const isForgeGitAvailable = (): boolean => isTauri();

const desktopInvoke = <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
  if (!isTauri()) {
    return Promise.reject(new Error("local forge browsing is available in the desktop app only"));
  }
  return invoke<T>(command, args);
};

export const forgeHead = (): Promise<string | null> =>
  desktopInvoke<string | null>("forge_head");

export const forgeRepoInfo = (): Promise<RepoInfo> =>
  forgeHead().then((head) => ({
    id: "ducktape",
    name: "ducktape",
    branch: "main",
    defaultBranch: "main",
    head,
    browsable: head !== null,
  }));

export const forgeListRepos = (): Promise<RepoInfo[]> =>
  forgeRepoInfo().then((repo) => [repo]);

export const forgeLog = (limit = 24): Promise<CommitInfo[]> =>
  desktopInvoke<CommitInfo[]>("forge_log", { limit });

export const forgeTree = (path = ""): Promise<TreeEntry[]> =>
  desktopInvoke<TreeEntry[]>("forge_tree", { path });

export const forgeReadFile = (path: string): Promise<string | null> =>
  desktopInvoke<string | null>("forge_read_file", { path });

export const forgeDiff = (params: {
  from?: string | null;
  to?: string | null;
}): Promise<FileDiff[]> =>
  desktopInvoke<FileDiff[]>("forge_diff", {
    from: params.from ?? null,
    to: params.to ?? null,
  });
