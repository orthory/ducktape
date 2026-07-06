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

/** One repo the node materialized under its forge base — its real on-disk name. */
interface RepoMeta {
  name: string;
  head: string | null;
}

/** Every repo the node has on disk, by its real name — no hardcoded identity. */
export const forgeListRepos = (): Promise<RepoInfo[]> =>
  desktopInvoke<RepoMeta[]>("forge_list_repos").then((repos) =>
    repos.map((repo) => ({
      id: repo.name,
      name: repo.name,
      branch: "main",
      defaultBranch: "main",
      head: repo.head,
      browsable: repo.head !== null,
    })),
  );

export const forgeHead = (repo: string): Promise<string | null> =>
  desktopInvoke<string | null>("forge_head", { repo });

export const forgeLog = (repo: string, limit = 24): Promise<CommitInfo[]> =>
  desktopInvoke<CommitInfo[]>("forge_log", { repo, limit });

export const forgeTree = (repo: string, path = ""): Promise<TreeEntry[]> =>
  desktopInvoke<TreeEntry[]>("forge_tree", { repo, path });

export const forgeReadFile = (repo: string, path: string): Promise<string | null> =>
  desktopInvoke<string | null>("forge_read_file", { repo, path });

export const forgeDiff = (
  repo: string,
  params: {
    from?: string | null;
    to?: string | null;
  },
): Promise<FileDiff[]> =>
  desktopInvoke<FileDiff[]>("forge_diff", {
    repo,
    from: params.from ?? null,
    to: params.to ?? null,
  });
