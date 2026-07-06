import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";

import {
  forgeHead as readLocalHead,
  forgeListRepos,
  forgeLog,
  forgeReadFile,
  forgeTree,
  isForgeGitAvailable,
  type CommitInfo,
  type RepoInfo,
  type TreeEntry,
} from "../../../domain/forge-git-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { CodeView } from "./CodeView";
import { fileIcon } from "./file-icons";
import { MarkdownPreview } from "./MarkdownPreview";
import { opKey } from "../../store/finalization";
import type { OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";

type ForgeTab = "code" | "commits";

interface TreeRow {
  path: string;
  name: string;
  isDir: boolean;
  depth: number;
  open: boolean;
}

const panelLabel: CSSProperties = {
  font: `700 9px ${font.mono}`,
  letterSpacing: ".08em",
  color: color.muted2,
};

const statusTone = {
  success: { text: color.green, bg: "#eef5f0", border: "#cfe3d7" },
  warning: { text: color.amber, bg: "#fbf4e6", border: "#ecdcae" },
  neutral: { text: color.purple, bg: "#f1edf5", border: "#ddd2e6" },
  info: { text: color.blue, bg: "#f1f4f8", border: "#d7e0eb" },
  danger: { text: color.red, bg: "#fbeeec", border: "#eccfc9" },
} as const;

function errMsg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function shortHash(value: string | null | undefined): string {
  return value ? `${value.slice(0, 10)}...` : "unborn";
}

function relTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  // The node's commit time is genesis-relative today (not wall-clock), so a
  // small value would render an absurd "20637d ago". Omit it until the node
  // stamps real time (> 2001); ordering/history are unaffected.
  if (seconds <= 978_307_200) return "";
  const diff = Math.max(0, Date.now() - seconds * 1000);
  const minute = 60 * 1000;
  const hour = 60 * minute;
  const day = 24 * hour;
  if (diff < minute) return "now";
  if (diff < hour) return `${Math.floor(diff / minute)}m ago`;
  if (diff < day) return `${Math.floor(diff / hour)}h ago`;
  return `${Math.floor(diff / day)}d ago`;
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
  const { state } = useDucktape();
  const desktop = isForgeGitAvailable();

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
  const [selected, setSelected] = useState<string | null>(null);
  const [fileText, setFileText] = useState<string | null>(null);
  const [fileLoading, setFileLoading] = useState(false);
  const [fileError, setFileError] = useState<string | null>(null);

  const fileRequestRef = useRef(0);
  const dirTokenRef = useRef(0);
  // the repo the tree/file readers target — the real on-disk name of the
  // currently-opened repo, so lazy dir/file loads read the right repo.
  const activeRepoRef = useRef<string | null>(null);

  const selectedRepo = useMemo(
    () => repos?.find((repo) => repo.id === selectedRepoId) ?? null,
    [repos, selectedRepoId],
  );
  const displayHead = localHead ?? selectedRepo?.head ?? state.forgeHead;

  const loadFile = useCallback((filePath: string) => {
    const repo = activeRepoRef.current;
    if (!repo) return;
    const req = ++fileRequestRef.current;
    setSelected(filePath);
    setFileText(null);
    setFileError(null);
    setFileLoading(true);
    forgeReadFile(repo, filePath)
      .then((text) => {
        if (fileRequestRef.current !== req) return;
        setFileText(text);
      })
      .catch((error) => {
        if (fileRequestRef.current !== req) return;
        setFileError(errMsg(error));
      })
      .finally(() => {
        if (fileRequestRef.current === req) setFileLoading(false);
      });
  }, []);

  const loadDir = useCallback((dir: string) => {
    const repo = activeRepoRef.current;
    if (!repo) return;
    const token = dirTokenRef.current;
    forgeTree(repo, dir)
      .then((entries) => {
        if (dirTokenRef.current !== token) return;
        setTreeCache((cache) => ({ ...cache, [dir]: entries }));
      })
      .catch((error) => {
        if (dirTokenRef.current !== token) return;
        setTreeError(errMsg(error));
      });
  }, []);

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
    forgeListRepos()
      .then((next) => {
        if (!alive) return;
        setRepos(next);
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
  }, [desktop, state.forgeHead]);

  useEffect(() => {
    if (!desktop || !selectedRepo) return;

    let alive = true;
    const token = dirTokenRef.current + 1;
    dirTokenRef.current = token;
    fileRequestRef.current += 1;
    activeRepoRef.current = selectedRepo.name;
    setRootLoading(true);
    setLocalHead(selectedRepo.head);
    setTreeError(null);
    setTreeCache({});
    setOpenDirs({});
    setSelected(null);
    setFileText(null);
    setFileError(null);
    setFileLoading(false);
    setCommits([]);

    if (!selectedRepo.browsable) {
      setRootLoading(false);
      return () => {
        alive = false;
      };
    }

    Promise.allSettled([
      readLocalHead(selectedRepo.name),
      forgeTree(selectedRepo.name, ""),
      forgeLog(selectedRepo.name),
    ])
      .then(([headResult, treeResult, logResult]) => {
        if (!alive || dirTokenRef.current !== token) return;
        if (headResult.status === "fulfilled") setLocalHead(headResult.value);
        if (logResult.status === "fulfilled") setCommits(logResult.value);
        else setCommits([]);
        if (treeResult.status === "fulfilled") {
          setTreeCache({ "": treeResult.value });
          const firstFile = sortEntries(treeResult.value).find((entry) => entry.kind === "file");
          if (firstFile) loadFile(firstFile.name);
        } else {
          setTreeError(errMsg(treeResult.reason));
        }
      })
      .finally(() => {
        if (alive && dirTokenRef.current === token) setRootLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [desktop, selectedRepo, state.forgeHead, loadFile]);

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
  };

  const goRepos = () => {
    setSelectedRepoId(null);
    setRepoMenuOpen(false);
  };

  return (
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
            rootLoading={rootLoading}
            rows={rows}
            treeError={treeError}
            selected={selected}
            fileText={fileText}
            fileLoading={fileLoading}
            fileError={fileError}
            onOpenRepo={openRepo}
            onGoRepos={goRepos}
            onToggleRepoMenu={() => setRepoMenuOpen((value) => !value)}
            onTab={setTab}
            onToggleDir={toggleDir}
            onSelectFile={loadFile}
          />
        ) : (
          <CenterNote title={reposLoading ? "Loading repository..." : "Repository not found"} />
        )
      ) : (
        <ReposOverview repos={repos} loading={reposLoading} error={reposError} onOpen={openRepo} />
      )}
    </div>
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
          {repos ? `${repos.length} repositories` : "local forge repositories"}
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
        Browse repositories backed by this node's local git forge.
      </div>

      <div style={{ marginTop: 18 }}>
        {error && <ErrorNote message={error} />}
        {!error && loading && !repos && <CenterNote title="Loading repositories..." />}
        {!error && repos && repos.length === 0 && (
          <CenterNote title="No local forge repositories" detail="This node did not report a browsable git repository." />
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
  rootLoading,
  rows,
  treeError,
  selected,
  fileText,
  fileLoading,
  fileError,
  onOpenRepo,
  onGoRepos,
  onToggleRepoMenu,
  onTab,
  onToggleDir,
  onSelectFile,
}: {
  repo: RepoInfo;
  repos: RepoInfo[];
  head: string | null;
  repoMenuOpen: boolean;
  tab: ForgeTab;
  commits: CommitInfo[];
  rootLoading: boolean;
  rows: TreeRow[];
  treeError: string | null;
  selected: string | null;
  fileText: string | null;
  fileLoading: boolean;
  fileError: string | null;
  onOpenRepo: (repoId: string) => void;
  onGoRepos: () => void;
  onToggleRepoMenu: () => void;
  onTab: (tab: ForgeTab) => void;
  onToggleDir: (dir: string) => void;
  onSelectFile: (path: string) => void;
}) {
  const latest = commits[0] ?? null;

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
          <StatusPill label={repo.defaultBranch} tone={repo.browsable ? "success" : "warning"} />
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
            <StatusPill label="desktop" tone="neutral" />
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
        </div>
      </div>

      {tab === "code" ? (
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
            repoName={repo.name}
            onToggleDir={onToggleDir}
            onSelectFile={onSelectFile}
          />
        ) : (
          <RepoUnavailable />
        )
      ) : (
        <CommitHistory commits={commits} loading={rootLoading} browsable={repo.browsable} />
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
  repoName,
  onToggleDir,
  onSelectFile,
}: {
  rows: TreeRow[];
  rootLoading: boolean;
  treeError: string | null;
  selected: string | null;
  latest: CommitInfo | null;
  fileLoading: boolean;
  fileError: string | null;
  fileText: string | null;
  repoName: string;
  onToggleDir: (dir: string) => void;
  onSelectFile: (path: string) => void;
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
}: {
  repoName: string;
  selected: string | null;
  latest: CommitInfo | null;
  loading: boolean;
  error: string | null;
  text: string | null;
}) {
  const [mdMode, setMdMode] = useState<"preview" | "raw">("preview");
  const title = selected ? `${repoName}/${selected}` : "Select a file";
  const isMarkdown = selected !== null && /\.mdx?$/i.test(selected);
  const showPreview = isMarkdown && mdMode === "preview";

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
          {isMarkdown && text !== null && (
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
          showPreview ? (
            <MarkdownPreview text={text} />
          ) : (
            <CodeView text={text} filename={selected} />
          )
        ) : null}
        {!loading && !error && text === null && (
          <CenterNote title={selected ? selected.split("/").pop() || selected : "Select a file"} />
        )}
      </div>
    </div>
  );
}

function SegButton({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        all: "unset",
        cursor: "pointer",
        padding: "3px 9px",
        font: `600 10px ${font.mono}`,
        letterSpacing: ".04em",
        textTransform: "uppercase",
        color: active ? color.ink : color.muted2,
        background: active ? color.panel : "transparent",
      }}
    >
      {label}
    </button>
  );
}

function CommitHistory({
  commits,
  loading,
  browsable,
}: {
  commits: CommitInfo[];
  loading: boolean;
  browsable: boolean;
}) {
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
      {!loading && commits.length === 0 && (
        <CenterNote
          title={browsable ? "No commits yet" : "No committed tree"}
          detail={browsable ? undefined : "This node has no local forge HEAD to browse."}
        />
      )}
      {!loading && commits.length > 0 && (
        <div style={{ padding: "0 24px 24px" }}>
          {commits.map((commit) => (
            <CommitRow key={commit.id} commit={commit} />
          ))}
        </div>
      )}
    </div>
  );
}

function CommitRow({ commit }: { commit: CommitInfo }) {
  return (
    <div
      title={commit.id}
      style={{
        display: "flex",
        gap: 13,
        padding: "13px 0",
        borderBottom: `1px solid ${color.borderSoft}`,
      }}
    >
      <span
        style={{
          width: 24,
          height: 24,
          borderRadius: radius.sm,
          background: statusTone.info.bg,
          border: `1px solid ${statusTone.info.border}`,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
          marginTop: 1,
        }}
      >
        <Icon name="forge" size={13} color={statusTone.info.text} strokeWidth={1.7} />
      </span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ font: `600 14px ${font.sans}`, color: color.ink }}>{commit.summary}</div>
        <div style={{ marginTop: 4, font: `400 11px ${font.mono}`, color: color.muted2 }}>
          {[shortHash(commit.id), commit.author, relTime(commit.time)].filter(Boolean).join(" · ")}
        </div>
      </div>
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

function TabButton({
  label,
  active,
  onClick,
  badge,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
  badge?: number;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 7,
        font: `600 13px ${font.sans}`,
        color: active ? color.ink : color.muted2,
        padding: "10px 0",
        borderBottom: `2px solid ${active ? color.dark : "transparent"}`,
        marginBottom: -1,
      }}
    >
      {label}
      {badge !== undefined && (
        <span
          aria-hidden="true"
          style={{
            font: `600 10px ${font.mono}`,
            color: color.muted2,
            background: color.panel,
            borderRadius: 9,
            padding: "1px 7px",
          }}
        >
          {badge}
        </span>
      )}
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
            only available through the desktop Tauri reader.
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

function CenterNote({ title, detail }: { title: string; detail?: string }) {
  return (
    <div
      style={{
        height: "100%",
        minHeight: 180,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        textAlign: "center",
        padding: 24,
      }}
    >
      <div style={{ font: `600 12.5px ${font.sans}`, color: color.muted2 }}>{title}</div>
      {detail && <div style={{ marginTop: 5, font: `400 11.5px ${font.sans}`, color: color.muted2, maxWidth: 360 }}>{detail}</div>}
    </div>
  );
}

function InlineNote({ children }: { children: ReactNode }) {
  return <div style={{ padding: "9px 16px", font: `400 11px ${font.sans}`, color: color.muted2 }}>{children}</div>;
}

function ErrorNote({ message, padded = false }: { message: string; padded?: boolean }) {
  return (
    <div style={{ padding: padded ? 18 : "8px 14px" }}>
      <div
        style={{
          border: `1px solid ${statusTone.danger.border}`,
          borderRadius: radius.sm,
          background: statusTone.danger.bg,
          color: statusTone.danger.text,
          font: `500 11px ${font.sans}`,
          padding: "7px 9px",
          wordBreak: "break-word",
        }}
      >
        {message}
      </div>
    </div>
  );
}

function StatusPill({ label, tone }: { label: string; tone: keyof typeof statusTone }) {
  const styles = statusTone[tone];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        height: 20,
        padding: "0 8px",
        borderRadius: radius.sm,
        border: `1px solid ${styles.border}`,
        background: styles.bg,
        color: styles.text,
        font: `700 9px ${font.mono}`,
        letterSpacing: ".06em",
        textTransform: "uppercase",
      }}
    >
      {label}
    </span>
  );
}
