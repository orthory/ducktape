import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import {
  forgeHead as readLocalHead,
  forgeListBranches,
  forgeListRepos,
  forgeLog,
  forgeReadFilePage,
  forgeSyncRemote,
  forgeTree,
  isForgeGitAvailable,
  type BranchInfo,
  type CommitInfo,
  type FilePage,
  type RepoInfo,
  type TreeEntry,
} from "../../../domain/forge-git-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { BranchSelector } from "./BranchSelector";
import { CodeView } from "./CodeView";
import { fileIcon } from "./file-icons";
import { IssuesTab } from "./items/IssuesTab";
import { ItemDetailPanel } from "./items/ItemDetailPanel";
import { PullsTab } from "./items/PullsTab";
import { MarkdownPreview } from "./MarkdownPreview";
import {
  CenterNote,
  CommitDetails,
  CommitRow,
  ErrorNote,
  errMsg,
  InlineNote,
  panelLabel,
  relTime,
  SegButton,
  shortHash,
  StatusPill,
  TabButton,
} from "./ui";
import { opKey } from "../../store/finalization";
import type { OpRecord } from "../../store/finalization";
import { isClientMode } from "../../store/state";
import { ForgeRemoteContext, useForgeRemote } from "./remote";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";

type ForgeTab = "code" | "commits" | "issues" | "pulls";

// remark parses markdown synchronously on the main thread and is superlinear on
// long flat lists, so a very large .md/.mdx doc would freeze the webview. Above
// this size we don't offer the rendered preview — the file still shows as Raw
// text (mirrors CodeView's own highlight cap). Untrusted repo content, so this
// is a guard, not just a nicety.
const MARKDOWN_PREVIEW_MAX_BYTES = 200_000;
const COMMIT_PAGE_SIZE = 50;
const COMMIT_PAGE_REQUEST = COMMIT_PAGE_SIZE + 1;
const FILE_PAGE_BYTES = 64 * 1024;

interface TreeRow {
  path: string;
  name: string;
  isDir: boolean;
  depth: number;
  open: boolean;
}

type FilePageState = Pick<FilePage, "nextOffset" | "totalBytes">;

// Re-enumerations keep the OLD entry object whenever a repo's head is
// unchanged: selectedRepo (and the whole browse effect chain) key on entry
// identity, so a tracker-only root move must not reset the code browser —
// and a mirror-synced branch label must not be clobbered by the list's
// pre-sync "dev" guess.
function mergeRepos(prev: RepoInfo[] | null, next: RepoInfo[]): RepoInfo[] {
  return next.map((repo) => {
    const old = prev?.find((p) => p.id === repo.id);
    return old && old.head === repo.head ? old : repo;
  });
}

function commitPage(commits: CommitInfo[]): { commits: CommitInfo[]; hasMore: boolean } {
  return {
    commits: commits.slice(0, COMMIT_PAGE_SIZE),
    hasMore: commits.length > COMMIT_PAGE_SIZE,
  };
}

function sortEntries(entries: TreeEntry[]): TreeEntry[] {
  return [...entries].sort((a, b) => {
    if ((a.kind === "dir") !== (b.kind === "dir")) return a.kind === "dir" ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
}

function buildRows(
  cache: Record<string, TreeEntry[]>,
  open: Record<string, boolean>,
  dir: string,
  depth: number,
  rows: TreeRow[],
): void {
  const entries = cache[dir];
  if (!entries) return;
  for (const entry of sortEntries(entries)) {
    const path = dir ? `${dir}/${entry.name}` : entry.name;
    const isDir = entry.kind === "dir";
    const isOpen = isDir && open[path] === true;
    rows.push({ path, name: entry.name, isDir, depth, open: isOpen });
    if (isOpen) buildRows(cache, open, path, depth + 1, rows);
  }
}

export function ForgeView() {
  const { state, actions } = useDucktape();
  const desktop = isForgeGitAvailable();
  // A direct remote client browses through its local mirror of the node's
  // git remote — every git read below then carries this origin. Null = the
  // local workspace repo (the classic desktop path).
  const remote = desktop && isClientMode(state) ? state.nodeUrl : null;

  const [repos, setRepos] = useState<RepoInfo[] | null>(null);
  const [reposLoading, setReposLoading] = useState(false);
  const [reposError, setReposError] = useState<string | null>(null);
  const [selectedRepoId, setSelectedRepoId] = useState<string | null>(null);
  const [repoMenuOpen, setRepoMenuOpen] = useState(false);
  const [tab, setTab] = useState<ForgeTab>("code");

  const [localHead, setLocalHead] = useState<string | null>(null);
  const [treeCache, setTreeCache] = useState<Record<string, TreeEntry[]>>({});
  const [openDirs, setOpenDirs] = useState<Record<string, boolean>>({});
  const [rootLoading, setRootLoading] = useState(false);
  const [treeError, setTreeError] = useState<string | null>(null);
  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const [commitHasMore, setCommitHasMore] = useState(false);
  const [commitLoadingMore, setCommitLoadingMore] = useState(false);
  const [commitError, setCommitError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [fileText, setFileText] = useState<string | null>(null);
  const [filePage, setFilePage] = useState<FilePageState | null>(null);
  const [fileWasPaged, setFileWasPaged] = useState(false);
  const [fileLoading, setFileLoading] = useState(false);
  const [fileLoadingMore, setFileLoadingMore] = useState(false);
  const [fileError, setFileError] = useState<string | null>(null);

  // Local branches (refs/heads/*) + the one being browsed. `branch` null means
  // the repo default — tree/log/file reads then omit the reference.
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [branch, setBranch] = useState<string | null>(null);
  const [branchMenuOpen, setBranchMenuOpen] = useState(false);

  // The deep-linked item to show instead of the tab bodies (a notification's
  // forgeFocus hand-off), and the last focus object already consumed.
  const [focusItem, setFocusItem] = useState<{
    number: number;
    messageId?: string;
    messageSeq?: number;
  } | null>(null);
  const consumedFocusRef = useRef<object | null>(null);

  const fileRequestRef = useRef(0);
  const dirTokenRef = useRef(0);
  // the repo/reference the tree/file readers target — the real on-disk name of
  // the currently-opened repo plus the browsed branch, so lazy dir/file loads
  // read the right tree.
  const activeRepoRef = useRef<string | null>(null);
  const activeRefRef = useRef<string | null>(null);

  const selectedRepo = useMemo(
    () => repos?.find((repo) => repo.id === selectedRepoId) ?? null,
    [repos, selectedRepoId],
  );
  // A remote client's branch list is the consensus refs the store already
  // loads per repo (list_refs) — zero extra git traffic; local mode keeps
  // reading the workspace repo directly.
  const branchList: BranchInfo[] = remote ? state.forgeBranches : branches;
  const branchHead = branch
    ? branchList.find((b) => b.name === branch)?.head ?? null
    : null;
  const displayHead = branch ? branchHead : localHead ?? selectedRepo?.head ?? state.forgeHead;

  const openIssues = state.forgeItems.filter(
    (item) => item.kind === "issue" && item.state === "open",
  ).length;
  const openPulls = state.forgeItems.filter(
    (item) => item.kind === "pr" && item.state === "open",
  ).length;

  // The forge module's state root — it rides every status patch, so it is the
  // one signal a REMOTE client also gets when any forge write lands (the
  // repo-list refresh and the tracker reload below both key on it).
  const forgeRoot = state.status?.modules.find((m) => m.id === "forge")?.root ?? null;
  const remoteForgeRoot = remote ? forgeRoot : null;

  const loadFile = useCallback((filePath: string) => {
    const repo = activeRepoRef.current;
    if (!repo) return;
    const req = ++fileRequestRef.current;
    setSelected(filePath);
    setFileText(null);
    setFilePage(null);
    setFileWasPaged(false);
    setFileError(null);
    setFileLoading(true);
    setFileLoadingMore(false);
    forgeReadFilePage(repo, filePath, {
      reference: activeRefRef.current ?? undefined,
      offset: 0,
      limit: FILE_PAGE_BYTES,
      remote,
    })
      .then((page) => {
        if (fileRequestRef.current !== req) return;
        setFileText(page?.text ?? null);
        setFilePage(
          page
            ? {
                nextOffset: page.nextOffset,
                totalBytes: page.totalBytes,
              }
            : null,
        );
        setFileWasPaged(page !== null && page.nextOffset !== null);
      })
      .catch((error) => {
        if (fileRequestRef.current !== req) return;
        setFileError(errMsg(error));
      })
      .finally(() => {
        if (fileRequestRef.current === req) setFileLoading(false);
      });
  }, [remote]);

  const loadDir = useCallback((dir: string) => {
    const repo = activeRepoRef.current;
    if (!repo) return;
    const token = dirTokenRef.current;
    forgeTree(repo, dir, activeRefRef.current ?? undefined, remote)
      .then((entries) => {
        if (dirTokenRef.current !== token) return;
        setTreeCache((cache) => ({ ...cache, [dir]: entries }));
      })
      .catch((error) => {
        if (dirTokenRef.current !== token) return;
        setTreeError(errMsg(error));
      });
  }, [remote]);

  const loadMoreCommits = useCallback(() => {
    const repo = activeRepoRef.current;
    const after = commits.length > 0 ? commits[commits.length - 1].id : null;
    if (!repo || !after || commitLoadingMore) return;

    const token = dirTokenRef.current;
    setCommitLoadingMore(true);
    setCommitError(null);
    forgeLog(repo, COMMIT_PAGE_REQUEST, activeRefRef.current ?? undefined, after, remote)
      .then((next) => {
        if (dirTokenRef.current !== token) return;
        const page = commitPage(next);
        setCommits((current) => [...current, ...page.commits]);
        setCommitHasMore(page.hasMore);
      })
      .catch((error) => {
        if (dirTokenRef.current !== token) return;
        setCommitError(errMsg(error));
      })
      .finally(() => {
        if (dirTokenRef.current === token) setCommitLoadingMore(false);
      });
  }, [commitLoadingMore, commits, remote]);

  const loadMoreFile = useCallback(() => {
    const repo = activeRepoRef.current;
    const filePath = selected;
    const offset = filePage?.nextOffset ?? null;
    if (!repo || !filePath || offset === null || fileLoadingMore) return;

    const req = fileRequestRef.current;
    setFileLoadingMore(true);
    setFileError(null);
    forgeReadFilePage(repo, filePath, {
      reference: activeRefRef.current ?? undefined,
      offset,
      limit: FILE_PAGE_BYTES,
      remote,
    })
      .then((page) => {
        if (fileRequestRef.current !== req) return;
        if (!page) {
          setFilePage(null);
          return;
        }
        setFileText((current) => (current ?? "") + page.text);
        setFileWasPaged(true);
        setFilePage({
          nextOffset: page.nextOffset,
          totalBytes: page.totalBytes,
        });
      })
      .catch((error) => {
        if (fileRequestRef.current !== req) return;
        setFileError(errMsg(error));
      })
      .finally(() => {
        if (fileRequestRef.current === req) setFileLoadingMore(false);
      });
  }, [fileLoadingMore, filePage?.nextOffset, selected, remote]);

  useEffect(() => {
    if (!desktop) {
      setRepos(null);
      setReposLoading(false);
      setReposError(null);
      return;
    }
    let alive = true;
    setReposLoading(true);
    setReposError(null);
    // A remote client enumerates over the node's committed namespace (one
    // list_repos query — it has no local forge storage to scan); the
    // integration branch is corrected per open by the mirror sync ("dev" is
    // every post-migration repo's answer until then).
    const load: Promise<RepoInfo[]> = remote
      ? actions.listForgeRepos().then((repos) =>
          repos.map((repo) => ({
            id: repo.name,
            name: repo.name,
            branch: "dev",
            defaultBranch: "dev",
            head: repo.head,
            browsable: repo.head !== null,
          })),
        )
      : forgeListRepos();
    load
      .then((next) => {
        if (!alive) return;
        setRepos((prev) => mergeRepos(prev, next));
        setSelectedRepoId((current) =>
          current && !next.some((repo) => repo.id === current) ? null : current,
        );
      })
      .catch((error) => {
        if (alive) setReposError(errMsg(error));
      })
      .finally(() => {
        if (alive) setReposLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [desktop, remote, actions, state.forgeHead, forgeRoot]);

  // Consume a deep-link hand-off (state.forgeFocus, set by the provider when
  // a forge notification is clicked): select the repo, land on its tracker
  // tab, and open the item. One-shot by OBJECT identity — the consumed ref
  // keeps the per-block repos churn from replaying it, while the provider
  // retires the state itself when the user leaves the forge screen. Tolerant:
  // an unknown repo just stays on what exists.
  useEffect(() => {
    const focus = state.forgeFocus;
    if (!focus || consumedFocusRef.current === focus) return;
    if (!desktop) {
      consumedFocusRef.current = focus; // web has no repo browser to drive
      return;
    }
    if (repos === null) return; // enumeration still loading — not consumed yet
    consumedFocusRef.current = focus;
    const match = repos.find((r) => r.name === focus.repo || r.id === focus.repo);
    if (!match) return;
    setSelectedRepoId(match.id);
    // The tracker slices may not be this repo's yet — a known PR lands on
    // Pulls, everything else defaults to Issues (only the back label differs;
    // the detail panel resolves the item's real kind itself).
    const kind =
      state.forgeRepo === match.name
        ? state.forgeItems.find((item) => item.number === focus.number)?.kind
        : undefined;
    setTab(kind === "pr" ? "pulls" : "issues");
    setFocusItem(
      focus.number === null
        ? null
        : {
            number: focus.number,
            ...(focus.messageId ? { messageId: focus.messageId } : {}),
            ...(focus.messageSeq ? { messageSeq: focus.messageSeq } : {}),
          },
    );
  }, [state.forgeFocus, repos, desktop, state.forgeRepo, state.forgeItems]);

  // A repo switch always lands back on the default branch.
  useEffect(() => {
    setBranch(null);
    setBranchMenuOpen(false);
  }, [selectedRepoId]);

  // Local branch heads for the picker; refreshed per committed forge write
  // (every tracker/merge op advances the forge HEAD). A remote client skips
  // this — its picker rides state.forgeBranches (consensus refs) instead.
  useEffect(() => {
    if (!desktop || !selectedRepo || remote) {
      setBranches([]);
      return;
    }
    let alive = true;
    forgeListBranches(selectedRepo.name)
      .then((next) => {
        if (alive) setBranches(next);
      })
      .catch(() => {
        if (alive) setBranches([]);
      });
    return () => {
      alive = false;
    };
  }, [desktop, selectedRepo, remote, state.forgeHead]);

  // The tracker's per-screen slices (issues/PRs + consensus branch heads) —
  // loaded on open/repo switch and re-pulled per forge WRITE. The forge
  // module's state root is the trigger, not main's head: an externally-pushed
  // branch (PushRefs over git smart-HTTP) and a peer's tracker op both move
  // the root without moving main, and the PR form's source branches must
  // follow list_refs without a reload. The root rides every status patch.
  useEffect(() => {
    if (!selectedRepo) return;
    void actions.loadForgeItems(selectedRepo.name);
    void actions.loadForgeBranches(selectedRepo.name);
    // actions is the store's stable facade — repo identity is the real dep.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedRepo?.name, forgeRoot]);

  useEffect(() => {
    if (!desktop || !selectedRepo) return;

    let alive = true;
    const token = dirTokenRef.current + 1;
    dirTokenRef.current = token;
    fileRequestRef.current += 1;
    activeRepoRef.current = selectedRepo.name;
    activeRefRef.current = branch;
    setRootLoading(true);
    setLocalHead(selectedRepo.head);
    setTreeError(null);
    setTreeCache({});
    setOpenDirs({});
    setSelected(null);
    setFileText(null);
    setFilePage(null);
    setFileWasPaged(false);
    setFileError(null);
    setFileLoading(false);
    setFileLoadingMore(false);
    setCommits([]);
    setCommitHasMore(false);
    setCommitLoadingMore(false);
    setCommitError(null);

    if (!selectedRepo.browsable) {
      setRootLoading(false);
      return () => {
        alive = false;
      };
    }

    const reference = branch ?? undefined;
    // A remote client refreshes its mirror FIRST — one fetch round-trip (a
    // bare ref advertisement when nothing moved, an incremental pack when
    // something did); every read below then serves from local disk. A failed
    // sync still falls through: a previously synced mirror keeps serving,
    // with the failure noted on the tree panel.
    const sync = remote
      ? forgeSyncRemote(remote, selectedRepo.name).then(
          (info) => {
            if (!alive || dirTokenRef.current !== token) return;
            // land the mirror's truth (real integration branch + head) on the
            // list entry — only when it differs, so the effect chain keyed on
            // entry identity settles instead of looping.
            setRepos(
              (current) =>
                current?.map((repo) =>
                  repo.id === info.id &&
                  (repo.head !== info.head || repo.defaultBranch !== info.defaultBranch)
                    ? info
                    : repo,
                ) ?? current,
            );
          },
          (error) => {
            if (alive && dirTokenRef.current === token) setTreeError(errMsg(error));
          },
        )
      : Promise.resolve();
    void sync
      .then(() => {
        if (!alive || dirTokenRef.current !== token) return;
        return Promise.allSettled([
          readLocalHead(selectedRepo.name, remote),
          forgeTree(selectedRepo.name, "", reference, remote),
          forgeLog(selectedRepo.name, COMMIT_PAGE_REQUEST, reference, null, remote),
        ]).then(([headResult, treeResult, logResult]) => {
          if (!alive || dirTokenRef.current !== token) return;
          if (headResult.status === "fulfilled" && !branch) setLocalHead(headResult.value);
          if (logResult.status === "fulfilled") {
            const page = commitPage(logResult.value);
            setCommits(page.commits);
            setCommitHasMore(page.hasMore);
          } else {
            setCommits([]);
            setCommitHasMore(false);
            setCommitError(errMsg(logResult.reason));
          }
          if (treeResult.status === "fulfilled") {
            setTreeCache({ "": treeResult.value });
            const firstFile = sortEntries(treeResult.value).find((entry) => entry.kind === "file");
            if (firstFile) loadFile(firstFile.name);
          } else {
            setTreeError(errMsg(treeResult.reason));
          }
        });
      })
      .finally(() => {
        if (alive && dirTokenRef.current === token) setRootLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [desktop, selectedRepo, branch, remote, state.forgeHead, remoteForgeRoot, loadFile]);

  const rows = useMemo(() => {
    const next: TreeRow[] = [];
    buildRows(treeCache, openDirs, "", 0, next);
    return next;
  }, [treeCache, openDirs]);

  const toggleDir = (dir: string) => {
    const willOpen = !openDirs[dir];
    setOpenDirs((current) => ({ ...current, [dir]: willOpen }));
    if (willOpen && !treeCache[dir]) loadDir(dir);
  };

  const openRepo = (repoId: string) => {
    setSelectedRepoId(repoId);
    setTab("code");
    setRepoMenuOpen(false);
    setFocusItem(null);
  };

  const goRepos = () => {
    setSelectedRepoId(null);
    setRepoMenuOpen(false);
    setFocusItem(null);
  };

  const selectTab = (next: ForgeTab) => {
    setTab(next);
    setFocusItem(null); // a tab click always leaves the focused-item panel
  };

  const selectBranch = (name: string) => {
    setBranchMenuOpen(false);
    setBranch(selectedRepo && name === selectedRepo.defaultBranch ? null : name);
  };

  return (
    <ForgeRemoteContext.Provider value={remote}>
    <div style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column", background: color.paper }}>
      {!desktop ? (
        <WebFallback head={state.forgeHead} op={state.ops[opKey.forgeHead()]} />
      ) : selectedRepoId ? (
        selectedRepo ? (
          <RepoListing
            repo={selectedRepo}
            repos={repos ?? []}
            head={displayHead}
            repoMenuOpen={repoMenuOpen}
            tab={tab}
            commits={commits}
            commitHasMore={commitHasMore}
            commitLoadingMore={commitLoadingMore}
            commitError={commitError}
            rootLoading={rootLoading}
            rows={rows}
            treeError={treeError}
            selected={selected}
            fileText={fileText}
            filePage={filePage}
            fileWasPaged={fileWasPaged}
            fileLoading={fileLoading}
            fileLoadingMore={fileLoadingMore}
            fileError={fileError}
            branches={branchList}
            branch={branch}
            branchMenuOpen={branchMenuOpen}
            openIssues={openIssues}
            openPulls={openPulls}
            focusItem={focusItem}
            onCloseFocus={() => setFocusItem(null)}
            onOpenRepo={openRepo}
            onGoRepos={goRepos}
            onToggleRepoMenu={() => setRepoMenuOpen((value) => !value)}
            onTab={selectTab}
            onToggleDir={toggleDir}
            onSelectFile={loadFile}
            onLoadMoreFile={loadMoreFile}
            onLoadMoreCommits={loadMoreCommits}
            onToggleBranchMenu={() => setBranchMenuOpen((value) => !value)}
            onSelectBranch={selectBranch}
          />
        ) : (
          <CenterNote title={reposLoading ? "Loading repository..." : "Repository not found"} />
        )
      ) : (
        <ReposOverview repos={repos} loading={reposLoading} error={reposError} onOpen={openRepo} />
      )}
    </div>
    </ForgeRemoteContext.Provider>
  );
}

function ReposOverview({
  repos,
  loading,
  error,
  onOpen,
}: {
  repos: RepoInfo[] | null;
  loading: boolean;
  error: string | null;
  onOpen: (repoId: string) => void;
}) {
  const remote = useForgeRemote();
  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "22px 26px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <span
          style={{
            width: 30,
            height: 30,
            borderRadius: radius.sm,
            background: color.dark,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="forge" size={16} color={color.onDark} strokeWidth={1.7} />
        </span>
        <div style={{ font: `600 18px ${font.sans}`, color: color.dark }}>ducktape</div>
        <StatusPill label="ORG" tone="neutral" />
        <span
          style={{
            marginLeft: "auto",
            font: `500 10.5px ${font.mono}`,
            color: color.muted2,
            whiteSpace: "nowrap",
          }}
        >
          {repos ? `${repos.length} repositories` : remote ? "remote forge repositories" : "local forge repositories"}
        </span>
      </div>
      <div
        style={{
          marginTop: 7,
          font: `400 12.5px ${font.sans}`,
          color: color.muted,
          lineHeight: 1.5,
          maxWidth: 560,
        }}
      >
        {remote
          ? "Browse repositories served by the connected node's git forge."
          : "Browse repositories backed by this node's local git forge."}
      </div>

      <div style={{ marginTop: 18 }}>
        {error && <ErrorNote message={error} />}
        {!error && loading && !repos && <CenterNote title="Loading repositories..." />}
        {!error && repos && repos.length === 0 && (
          <CenterNote
            title={remote ? "No forge repositories" : "No local forge repositories"}
            detail={
              remote
                ? "The connected node did not report a repository."
                : "This node did not report a browsable git repository."
            }
          />
        )}
        {!error && repos && repos.length > 0 && (
          <>
            <div style={{ ...panelLabel, marginBottom: 9 }}>REPOSITORIES</div>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))", gap: 13 }}>
              {repos.map((repo) => (
                <RepoCard key={repo.id} repo={repo} onOpen={() => onOpen(repo.id)} />
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function RepoCard({ repo, onOpen }: { repo: RepoInfo; onOpen: () => void }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      onClick={onOpen}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        display: "block",
        cursor: "pointer",
        border: `1px solid ${hover ? color.borderStrong : color.border}`,
        borderRadius: radius.lg,
        padding: "15px 17px",
        background: hover ? color.sunken : color.paper,
        boxShadow: shadow.card,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <Icon name="forge" size={14} color={color.muted} strokeWidth={1.7} />
        <span style={{ font: `600 14.5px ${font.sans}`, color: color.dark }}>ducktape/{repo.name}</span>
      </div>
      <div
        style={{
          marginTop: 12,
          display: "flex",
          alignItems: "center",
          gap: 14,
          font: `500 10.5px ${font.mono}`,
          color: color.muted,
        }}
      >
        <span style={{ display: "flex", alignItems: "center", gap: 5 }}>
          <span style={{ width: 9, height: 9, borderRadius: "50%", background: color.green }} />
          {repo.defaultBranch}
        </span>
        <span>{repo.browsable ? "browsable" : "no HEAD"}</span>
        <span title={repo.head ?? "unborn repo"} style={{ marginLeft: "auto", color: color.muted2, whiteSpace: "nowrap" }}>
          {shortHash(repo.head)}
        </span>
      </div>
    </button>
  );
}

function RepoListing({
  repo,
  repos,
  head,
  repoMenuOpen,
  tab,
  commits,
  commitHasMore,
  commitLoadingMore,
  commitError,
  rootLoading,
  rows,
  treeError,
  selected,
  fileText,
  filePage,
  fileWasPaged,
  fileLoading,
  fileLoadingMore,
  fileError,
  branches,
  branch,
  branchMenuOpen,
  openIssues,
  openPulls,
  focusItem,
  onCloseFocus,
  onOpenRepo,
  onGoRepos,
  onToggleRepoMenu,
  onTab,
  onToggleDir,
  onSelectFile,
  onLoadMoreFile,
  onLoadMoreCommits,
  onToggleBranchMenu,
  onSelectBranch,
}: {
  repo: RepoInfo;
  repos: RepoInfo[];
  head: string | null;
  repoMenuOpen: boolean;
  tab: ForgeTab;
  commits: CommitInfo[];
  commitHasMore: boolean;
  commitLoadingMore: boolean;
  commitError: string | null;
  rootLoading: boolean;
  rows: TreeRow[];
  treeError: string | null;
  selected: string | null;
  fileText: string | null;
  filePage: FilePageState | null;
  fileWasPaged: boolean;
  fileLoading: boolean;
  fileLoadingMore: boolean;
  fileError: string | null;
  branches: BranchInfo[];
  branch: string | null;
  branchMenuOpen: boolean;
  openIssues: number;
  openPulls: number;
  /** A deep-linked item to render in place of the tab bodies (null = none). */
  focusItem: { number: number; messageId?: string; messageSeq?: number } | null;
  onCloseFocus: () => void;
  onOpenRepo: (repoId: string) => void;
  onGoRepos: () => void;
  onToggleRepoMenu: () => void;
  onTab: (tab: ForgeTab) => void;
  onToggleDir: (dir: string) => void;
  onSelectFile: (path: string) => void;
  onLoadMoreFile: () => void;
  onLoadMoreCommits: () => void;
  onToggleBranchMenu: () => void;
  onSelectBranch: (branch: string) => void;
}) {
  const remote = useForgeRemote();
  const latest = commits[0] ?? null;
  const browsing = tab === "code" || tab === "commits";

  return (
    <>
      <div style={{ flexShrink: 0, padding: "16px 24px 0", position: "relative" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 9 }}>
          <span
            style={{
              width: 28,
              height: 28,
              borderRadius: radius.sm,
              background: color.dark,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
            }}
          >
            <Icon name="forge" size={15} color={color.onDark} strokeWidth={1.7} />
          </span>
          <Breadcrumb label="ducktape" onClick={onGoRepos} />
          <span style={{ font: `400 15px ${font.sans}`, color: color.iconIdle }}>/</span>
          <RepoMenuButton name={repo.name} onClick={onToggleRepoMenu} open={repoMenuOpen} />
          {browsing && repo.browsable ? (
            <BranchSelector
              branches={branches}
              current={branch ?? repo.defaultBranch}
              open={branchMenuOpen}
              onToggle={onToggleBranchMenu}
              onSelect={onSelectBranch}
            />
          ) : (
            <StatusPill label={repo.defaultBranch} tone={repo.browsable ? "success" : "warning"} />
          )}
          <span
            title={head ?? "unborn repo"}
            style={{
              font: `500 10.5px ${font.mono}`,
              color: head ? color.muted3 : color.muted2,
              border: `1px solid ${color.border}`,
              borderRadius: radius.sm,
              padding: "3px 8px",
              background: color.paper,
            }}
          >
            {shortHash(head)}
          </span>
          <span style={{ marginLeft: "auto" }}>
            <StatusPill label={remote ? "remote" : "desktop"} tone="neutral" />
          </span>
        </div>

        {repoMenuOpen && (
          <RepoMenu repos={repos} activeRepoId={repo.id} onOpenRepo={onOpenRepo} />
        )}

        <div
          style={{
            marginTop: 13,
            display: "flex",
            alignItems: "center",
            gap: 22,
            borderBottom: `1px solid ${color.borderSoft}`,
          }}
        >
          <TabButton label="Code" active={tab === "code"} onClick={() => onTab("code")} />
          <TabButton label="Commits" active={tab === "commits"} onClick={() => onTab("commits")} badge={commits.length} />
          <TabButton label="Issues" active={tab === "issues"} onClick={() => onTab("issues")} badge={openIssues} />
          <TabButton label="Pull requests" active={tab === "pulls"} onClick={() => onTab("pulls")} badge={openPulls} />
        </div>
      </div>

      {focusItem !== null ? (
        // A deep-linked item (notification click) takes the body over until
        // dismissed — the same shell the tabs' own detail views use.
        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            borderTop: `1px solid ${color.borderSoft}`,
            padding: "16px 24px",
          }}
        >
          <ItemDetailPanel
            repo={repo.name}
            number={focusItem.number}
            messageId={focusItem.messageId}
            messageSeq={focusItem.messageSeq}
            backLabel={tab === "pulls" ? "Pull requests" : "Issues"}
            onBack={onCloseFocus}
          />
        </div>
      ) : (
        <>
          {tab === "code" && (
            repo.browsable ? (
              <CodeBrowser
                rows={rows}
                rootLoading={rootLoading}
                treeError={treeError}
                selected={selected}
                latest={latest}
                fileLoading={fileLoading}
                fileError={fileError}
                fileText={fileText}
                filePage={filePage}
                fileWasPaged={fileWasPaged}
                fileLoadingMore={fileLoadingMore}
                repoName={repo.name}
                onToggleDir={onToggleDir}
                onSelectFile={onSelectFile}
                onLoadMoreFile={onLoadMoreFile}
              />
            ) : (
              <RepoUnavailable />
            )
          )}
          {tab === "commits" && (
            <CommitHistory
              repo={repo.name}
              commits={commits}
              loading={rootLoading}
              browsable={repo.browsable}
              hasMore={commitHasMore}
              loadingMore={commitLoadingMore}
              error={commitError}
              onLoadMore={onLoadMoreCommits}
            />
          )}
          {tab === "issues" && <IssuesTab repo={repo.name} />}
          {tab === "pulls" && <PullsTab repo={repo.name} />}
        </>
      )}
    </>
  );
}

function CodeBrowser({
  rows,
  rootLoading,
  treeError,
  selected,
  latest,
  fileLoading,
  fileError,
  fileText,
  filePage,
  fileWasPaged,
  fileLoadingMore,
  repoName,
  onToggleDir,
  onSelectFile,
  onLoadMoreFile,
}: {
  rows: TreeRow[];
  rootLoading: boolean;
  treeError: string | null;
  selected: string | null;
  latest: CommitInfo | null;
  fileLoading: boolean;
  fileError: string | null;
  fileText: string | null;
  filePage: FilePageState | null;
  fileWasPaged: boolean;
  fileLoadingMore: boolean;
  repoName: string;
  onToggleDir: (dir: string) => void;
  onSelectFile: (path: string) => void;
  onLoadMoreFile: () => void;
}) {
  return (
    <div style={{ flex: 1, minHeight: 0, display: "flex", borderTop: `1px solid ${color.borderSoft}` }}>
      <FileTree
        rows={rows}
        loading={rootLoading}
        error={treeError}
        selected={selected}
        onToggleDir={onToggleDir}
        onSelectFile={onSelectFile}
      />
      <FileViewer
        repoName={repoName}
        selected={selected}
        latest={latest}
        loading={fileLoading}
        error={fileError}
        text={fileText}
        page={filePage}
        paged={fileWasPaged}
        loadingMore={fileLoadingMore}
        onLoadMore={onLoadMoreFile}
      />
    </div>
  );
}

function FileTree({
  rows,
  loading,
  error,
  selected,
  onToggleDir,
  onSelectFile,
}: {
  rows: TreeRow[];
  loading: boolean;
  error: string | null;
  selected: string | null;
  onToggleDir: (dir: string) => void;
  onSelectFile: (path: string) => void;
}) {
  return (
    <div
      style={{
        width: 258,
        flexShrink: 0,
        borderRight: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        overflowY: "auto",
        padding: "11px 0",
      }}
    >
      <div style={{ padding: "0 16px 9px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={panelLabel}>FILES</span>
        <span style={{ font: `500 10px ${font.mono}`, color: color.muted2 }}>{rows.length}</span>
      </div>
      {loading && <InlineNote>Loading repository...</InlineNote>}
      {error && <ErrorNote message={error} />}
      {!loading && !error && rows.length === 0 && <InlineNote>Empty repository</InlineNote>}
      {rows.map((row) => (
        <TreeButton
          key={row.path}
          row={row}
          selected={selected === row.path}
          onClick={() => (row.isDir ? onToggleDir(row.path) : onSelectFile(row.path))}
        />
      ))}
    </div>
  );
}

function TreeButton({
  row,
  selected,
  onClick,
}: {
  row: TreeRow;
  selected: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);
  const indent = 13 + row.depth * 15;
  const fi = row.isDir ? null : fileIcon(row.name);
  return (
    <button
      type="button"
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        width: "100%",
        boxSizing: "border-box",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 6,
        padding: `5px 13px 5px ${indent}px`,
        background: selected ? color.hover : hover ? color.sunken : "transparent",
        color: selected ? color.ink : color.inkSofter,
        font: row.isDir ? `600 12.5px ${font.sans}` : `400 12px ${font.mono}`,
      }}
    >
      {row.isDir ? (
        <Icon
          name="chevronRight"
          size={11}
          color={color.muted2}
          strokeWidth={2.4}
          style={{ transform: `rotate(${row.open ? 90 : 0}deg)` }}
        />
      ) : (
        <span style={{ width: 11, flexShrink: 0 }} />
      )}
      {row.isDir ? (
        <Icon name="modules" size={13} color={color.accent} />
      ) : (
        <Icon name={fi!.icon} size={13} color={fi!.color} strokeWidth={1.7} />
      )}
      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{row.name}</span>
    </button>
  );
}

function FileViewer({
  repoName,
  selected,
  latest,
  loading,
  error,
  text,
  page,
  paged,
  loadingMore,
  onLoadMore,
}: {
  repoName: string;
  selected: string | null;
  latest: CommitInfo | null;
  loading: boolean;
  error: string | null;
  text: string | null;
  page: FilePageState | null;
  paged: boolean;
  loadingMore: boolean;
  onLoadMore: () => void;
}) {
  const [mdMode, setMdMode] = useState<"preview" | "raw">("preview");
  const title = selected ? `${repoName}/${selected}` : "Select a file";
  const isMarkdown = selected !== null && /\.mdx?$/i.test(selected);
  const canPreview = isMarkdown && !paged && text !== null && text.length <= MARKDOWN_PREVIEW_MAX_BYTES;
  const showPreview = canPreview && mdMode === "preview";
  const loadedBytes = page ? page.nextOffset ?? page.totalBytes : null;

  return (
    <div style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column", background: color.paper }}>
      <div
        style={{
          flexShrink: 0,
          minHeight: 42,
          borderBottom: `1px solid ${color.borderSoft}`,
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "8px 16px",
        }}
      >
        <span
          title={title}
          style={{
            font: `600 12px ${font.mono}`,
            color: selected ? color.inkSoft : color.muted2,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {title}
        </span>
        <div style={{ marginLeft: "auto", display: "flex", alignItems: "center", gap: 10, minWidth: 0 }}>
          {canPreview && (
            <div style={{ display: "flex", flexShrink: 0, border: `1px solid ${color.border}`, borderRadius: radius.sm, overflow: "hidden" }}>
              <SegButton label="Preview" active={mdMode === "preview"} onClick={() => setMdMode("preview")} />
              <SegButton label="Raw" active={mdMode === "raw"} onClick={() => setMdMode("raw")} />
            </div>
          )}
          {latest && (
            <span
              title={latest.id}
              style={{
                minWidth: 0,
                font: `400 10px ${font.mono}`,
                color: color.muted2,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {[latest.summary, latest.author, relTime(latest.time)].filter(Boolean).join(" · ")}
            </span>
          )}
        </div>
      </div>
      <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        {loading && <CenterNote title="Loading file..." />}
        {error && <ErrorNote message={error} padded />}
        {!loading && !error && text !== null ? (
          <>
            {showPreview ? (
              <MarkdownPreview text={text} />
            ) : (
              <CodeView text={text} filename={selected} />
            )}
            {page && page.nextOffset !== null && (
              <div
                style={{
                  borderTop: `1px solid ${color.borderSoft}`,
                  padding: "10px 16px",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 10,
                }}
              >
                <span style={{ font: `500 10.5px ${font.mono}`, color: color.muted2 }}>
                  {loadedBytes} / {page.totalBytes} bytes
                </span>
                <button
                  type="button"
                  onClick={onLoadMore}
                  disabled={loadingMore}
                  style={{
                    border: `1px solid ${color.border}`,
                    borderRadius: radius.sm,
                    background: loadingMore ? color.sunken : color.paper,
                    color: color.ink,
                    cursor: loadingMore ? "default" : "pointer",
                    font: `600 11px ${font.sans}`,
                    padding: "5px 9px",
                  }}
                >
                  Load more file
                </button>
              </div>
            )}
          </>
        ) : null}
        {!loading && !error && text === null && (
          <CenterNote title={selected ? selected.split("/").pop() || selected : "Select a file"} />
        )}
      </div>
    </div>
  );
}

function CommitHistory({
  repo,
  commits,
  loading,
  browsable,
  hasMore,
  loadingMore,
  error,
  onLoadMore,
}: {
  repo: string;
  commits: CommitInfo[];
  loading: boolean;
  browsable: boolean;
  hasMore: boolean;
  loadingMore: boolean;
  error: string | null;
  onLoadMore: () => void;
}) {
  const [selectedCommitId, setSelectedCommitId] = useState<string | null>(null);
  const selectedCommit = commits.find((commit) => commit.id === selectedCommitId) ?? null;

  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: "auto", borderTop: `1px solid ${color.borderSoft}` }}>
      <div style={{ padding: "18px 24px 12px", display: "flex", alignItems: "center", gap: 10 }}>
        <div>
          <div style={{ font: `600 15px ${font.sans}`, color: color.ink }}>Commit history</div>
          <div style={{ marginTop: 4, font: `400 11.5px ${font.sans}`, color: color.muted }}>
            Read-only log from the local git repository.
          </div>
        </div>
        <span style={{ marginLeft: "auto" }}>
          <StatusPill label={`${commits.length} commits`} tone="info" />
        </span>
      </div>
      {loading && <CenterNote title="Loading commits..." />}
      {!loading && error && <ErrorNote message={error} padded />}
      {!loading && commits.length === 0 && (
        <CenterNote
          title={browsable ? "No commits yet" : "No committed tree"}
          detail={browsable ? undefined : "This node has no local forge HEAD to browse."}
        />
      )}
      {!loading && commits.length > 0 && (
        <div style={{ padding: "0 24px 24px" }}>
          {commits.map((commit) => (
            <div key={commit.id}>
              <CommitRow
                commit={commit}
                selected={commit.id === selectedCommitId}
                onOpen={() => setSelectedCommitId((current) => (current === commit.id ? null : commit.id))}
              />
              {selectedCommit?.id === commit.id && <CommitDetails repo={repo} commit={selectedCommit} />}
            </div>
          ))}
          {hasMore && (
            <div style={{ display: "flex", justifyContent: "center", paddingTop: 12 }}>
              <button
                type="button"
                onClick={onLoadMore}
                disabled={loadingMore}
                style={{
                  border: `1px solid ${color.border}`,
                  borderRadius: radius.sm,
                  background: loadingMore ? color.sunken : color.paper,
                  color: color.ink,
                  cursor: loadingMore ? "default" : "pointer",
                  font: `600 11px ${font.sans}`,
                  padding: "6px 10px",
                }}
              >
                Load more commits
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function RepoMenu({
  repos,
  activeRepoId,
  onOpenRepo,
}: {
  repos: RepoInfo[];
  activeRepoId: string;
  onOpenRepo: (repoId: string) => void;
}) {
  return (
    <div
      style={{
        position: "absolute",
        left: 58,
        top: 48,
        zIndex: 14,
        width: 260,
        background: color.paper,
        border: `1px solid ${color.borderStrong}`,
        borderRadius: radius.lg,
        boxShadow: shadow.pop,
        padding: 6,
      }}
    >
      <div style={{ ...panelLabel, padding: "6px 9px 4px" }}>REPOSITORIES - {repos.length}</div>
      {repos.map((repo) => (
        <RepoMenuItem key={repo.id} repo={repo} active={repo.id === activeRepoId} onOpen={() => onOpenRepo(repo.id)} />
      ))}
    </div>
  );
}

function RepoMenuItem({ repo, active, onOpen }: { repo: RepoInfo; active: boolean; onOpen: () => void }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      onClick={onOpen}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 9,
        width: "100%",
        boxSizing: "border-box",
        padding: "8px 9px",
        borderRadius: radius.sm,
        background: hover || active ? color.panel : "transparent",
      }}
    >
      <span style={{ width: 9, height: 9, borderRadius: "50%", background: repo.browsable ? color.green : color.amber, flexShrink: 0 }} />
      <span style={{ font: `600 12.5px ${font.sans}`, color: color.ink }}>{repo.name}</span>
      <span style={{ marginLeft: "auto", font: `500 10px ${font.mono}`, color: color.muted2 }}>{repo.defaultBranch}</span>
    </button>
  );
}

function Breadcrumb({ label, onClick }: { label: string; onClick: () => void }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        cursor: "pointer",
        font: `600 15px ${font.sans}`,
        color: hover ? color.ink : color.muted,
      }}
    >
      {label}
    </button>
  );
}

function RepoMenuButton({ name, open, onClick }: { name: string; open: boolean; onClick: () => void }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 5,
        font: `600 15px ${font.sans}`,
        color: hover ? color.accent : color.ink,
      }}
    >
      {name}
      <Icon
        name="chevronRight"
        size={11}
        color="currentColor"
        strokeWidth={2.2}
        style={{ transform: `rotate(${open ? -90 : 90}deg)` }}
      />
    </button>
  );
}

function RepoUnavailable() {
  return (
    <div style={{ flex: 1, minHeight: 0, borderTop: `1px solid ${color.borderSoft}` }}>
      <CenterNote title="No committed tree" detail="This local forge repository has no HEAD yet, so there is no code to browse." />
    </div>
  );
}

function WebFallback({ head, op }: { head: string | null; op: OpRecord | undefined }) {
  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "22px 26px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <span
          style={{
            width: 30,
            height: 30,
            borderRadius: radius.sm,
            background: color.dark,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="forge" size={16} color={color.onDark} strokeWidth={1.7} />
        </span>
        <div style={{ font: `600 18px ${font.sans}`, color: color.dark }}>ducktape forge</div>
        <span style={{ marginLeft: "auto" }}>
          <StatusPill label="web" tone="warning" />
        </span>
      </div>
      <div style={{ marginTop: 18, maxWidth: 620 }}>
        <div
          style={{
            border: `1px solid ${color.border}`,
            borderRadius: radius.lg,
            background: color.paper,
            boxShadow: shadow.card,
            padding: 17,
          }}
        >
          <div style={panelLabel}>LOCAL GIT BROWSER</div>
          <div style={{ marginTop: 8, font: `600 15px ${font.sans}`, color: color.ink }}>Desktop app required</div>
          <div style={{ marginTop: 5, font: `400 12.5px ${font.sans}`, color: color.muted, lineHeight: 1.5 }}>
            This build can show the committed forge HEAD from the node, but the local git tree and file contents are
            only available through the native desktop reader.
          </div>
          <div style={{ marginTop: 13 }}>
            <HeadCard head={head} op={op} />
          </div>
        </div>
      </div>
    </div>
  );
}

function HeadCard({ head, op }: { head: string | null; op: OpRecord | undefined }) {
  return (
    <div
      title={head ?? "unborn repo"}
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.sm,
        background: color.sidebar,
        padding: "9px 10px",
        font: `400 12px ${font.mono}`,
        color: head ? color.inkSofter : color.muted2,
        wordBreak: "break-all",
        fontStyle: head ? "normal" : "italic",
      }}
    >
      <span style={{ display: "inline-flex", alignItems: "center", gap: 6, maxWidth: "100%" }}>
        {head ?? "no commits yet"}
        <FinalizationMark op={op} />
      </span>
    </div>
  );
}
