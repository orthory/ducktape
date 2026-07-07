// Desktop-only local git reader for the node's on-disk forge repository.
// Writes stay on the consensus wire in forge-client.ts; these calls only
// project committed refs (refs/heads/*, defaulting to main) through Tauri
// commands — plus forge_build_merge, which builds the client-computed merge
// commit for MergePr in a throwaway repo without touching the node repo.

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

/** One local branch: refs/heads/<name> short name + its 40-hex head oid. */
export interface BranchInfo {
  name: string;
  head: string;
}

/** One changed file in a compare — a PR "files changed" row. */
export interface CompareFile {
  path: string;
  /** "added" | "modified" | "deleted" | "renamed" (rarely "copied"/"typechange"). */
  status: string;
  additions: number;
  deletions: number;
  /** Unified patch text for this file; empty for binary deltas. */
  patch: string;
}

/** GitHub-style three-dot compare: diff merge_base(base, head) -> head. */
export interface CompareResult {
  mergeBase: string;
  files: CompareFile[];
  totalAdditions: number;
  totalDeletions: number;
  /** Commits on head not reachable from base, newest first. */
  commits: CommitInfo[];
}

/** Outcome of the client-computed merge for MergePr. */
export interface MergeBuildResult {
  /** 40-hex merge commit oid; null when the merge conflicts. */
  mergeOid: string | null;
  /** Hex-encoded git pack of the new objects; null when the merge conflicts. */
  packHex: string | null;
  /** Conflicting paths; non-empty means NO merge was built. */
  conflicts: string[];
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

/** Every local branch (refs/heads/*) by short name + head oid, sorted by name. */
export const forgeListBranches = (repo: string): Promise<BranchInfo[]> =>
  desktopInvoke<BranchInfo[]>("forge_list_branches", { repo });

/**
 * Commit log, newest first. `reference` is a branch short name or 40-hex oid
 * (omit for main); omit `limit` for the full history.
 */
export const forgeLog = (
  repo: string,
  limit?: number,
  reference?: string,
): Promise<CommitInfo[]> =>
  desktopInvoke<CommitInfo[]>("forge_log", {
    repo,
    limit: limit ?? null,
    reference: reference ?? null,
  });

export const forgeTree = (repo: string, path = "", reference?: string): Promise<TreeEntry[]> =>
  desktopInvoke<TreeEntry[]>("forge_tree", { repo, path, reference: reference ?? null });

export const forgeReadFile = (
  repo: string,
  path: string,
  reference?: string,
): Promise<string | null> =>
  desktopInvoke<string | null>("forge_read_file", { repo, path, reference: reference ?? null });

/**
 * GitHub-style compare (three-dot): files changed from merge_base(base, head)
 * to head, plus the commits on head not reachable from base. `base`/`head`
 * are branch short names or 40-hex oids.
 */
export const forgeCompare = (repo: string, base: string, head: string): Promise<CompareResult> =>
  desktopInvoke<CompareResult>("forge_compare", { repo, base, head });

/**
 * Build the client-computed merge commit for MergePr: merge `theirs` (source
 * head) into `ours` (target head) with `message`. A clean merge returns the
 * new oid + a hex-encoded pack of the new objects to hand to the node; a
 * conflicted one returns the conflicting paths and builds nothing.
 */
export const forgeBuildMerge = (
  repo: string,
  ours: string,
  theirs: string,
  message: string,
): Promise<MergeBuildResult> =>
  desktopInvoke<MergeBuildResult>("forge_build_merge", { repo, ours, theirs, message });

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
