import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type FormEvent,
  type ReactNode,
} from "react";

import {
  forgeHead as readLocalHead,
  forgeLog,
  forgeReadFile,
  forgeTree,
  isForgeGitAvailable,
  type CommitInfo,
  type TreeEntry,
} from "../../../domain/forge-git-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

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

const fieldStyle: CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "8px 10px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12px ${font.sans}`,
  color: color.ink,
  outline: "none",
};

function errMsg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function shortHash(value: string | null | undefined): string {
  return value ? `${value.slice(0, 10)}...` : "unborn";
}

function relTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "unknown";
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
  const { state, actions } = useDucktape();
  const desktop = isForgeGitAvailable();
  const [path, setPath] = useState("");
  const [message, setMessage] = useState("");
  const [content, setContent] = useState("");

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

  const displayHead = localHead ?? state.forgeHead;
  const canCommit = path.trim().length > 0 && content.length > 0;

  const loadFile = useCallback((filePath: string) => {
    const req = ++fileRequestRef.current;
    setSelected(filePath);
    setFileText(null);
    setFileError(null);
    setFileLoading(true);
    forgeReadFile(filePath)
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
    const token = dirTokenRef.current;
    forgeTree(dir)
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
    if (!desktop) return;
    let alive = true;
    const token = dirTokenRef.current + 1;
    dirTokenRef.current = token;
    fileRequestRef.current += 1;
    setRootLoading(true);
    setLocalHead(null);
    setTreeError(null);
    setTreeCache({});
    setOpenDirs({});
    setSelected(null);
    setFileText(null);
    setFileError(null);
    Promise.allSettled([readLocalHead(), forgeTree(""), forgeLog(32)])
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
  }, [desktop, state.forgeHead, loadFile]);

  const rows = useMemo(() => {
    const next: TreeRow[] = [];
    buildRows(treeCache, openDirs, "", 0, next);
    return next;
  }, [treeCache, openDirs]);

  const latest = commits[0] ?? null;
  const lines = fileText !== null ? fileText.split("\n") : [];

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!canCommit) return;
    actions.commitForge({ path, message, content });
    setPath("");
    setMessage("");
    setContent("");
  };

  const toggleDir = (dir: string) => {
    const willOpen = !openDirs[dir];
    setOpenDirs((current) => ({ ...current, [dir]: willOpen }));
    if (willOpen && !treeCache[dir]) loadDir(dir);
  };

  return (
    <div style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column", background: color.paper }}>
      <ForgeHeader head={displayHead} desktop={desktop} />

      {desktop ? (
        <div style={{ flex: 1, minHeight: 0, display: "flex", borderTop: `1px solid ${color.borderSoft}` }}>
          <FileTree
            rows={rows}
            loading={rootLoading}
            error={treeError}
            selected={selected}
            onToggleDir={toggleDir}
            onSelectFile={loadFile}
          />
          <FileViewer
            selected={selected}
            latest={latest}
            loading={fileLoading}
            error={fileError}
            text={fileText}
            lines={lines}
          />
          <div
            style={{
              width: 286,
              flexShrink: 0,
              borderLeft: `1px solid ${color.borderSoft}`,
              background: color.sidebar,
              display: "flex",
              flexDirection: "column",
              minHeight: 0,
            }}
          >
            <CommitLog commits={commits} loading={rootLoading} />
            <CommitForm
              path={path}
              message={message}
              content={content}
              canCommit={canCommit}
              onPath={setPath}
              onMessage={setMessage}
              onContent={setContent}
              onSubmit={submit}
            />
          </div>
        </div>
      ) : (
        <div style={{ flex: 1, overflowY: "auto", padding: 18, display: "grid", gap: 13, alignContent: "start" }}>
          <HeadCard head={displayHead} />
          <CommitForm
            path={path}
            message={message}
            content={content}
            canCommit={canCommit}
            onPath={setPath}
            onMessage={setMessage}
            onContent={setContent}
            onSubmit={submit}
          />
        </div>
      )}
    </div>
  );
}

function ForgeHeader({ head, desktop }: { head: string | null; desktop: boolean }) {
  return (
    <div style={{ flexShrink: 0, padding: "15px 22px 0" }}>
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
        <span style={{ font: `600 15px ${font.sans}`, color: color.ink }}>ducktape</span>
        <span style={{ font: `400 15px ${font.sans}`, color: color.iconIdle }}>/</span>
        <span style={{ font: `600 15px ${font.sans}`, color: color.inkSoft }}>forge</span>
        <StatusPill label="main" tone="success" />
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
          <StatusPill label={desktop ? "desktop" : "web"} tone={desktop ? "neutral" : "warning"} />
        </span>
      </div>
      <div
        style={{
          marginTop: 13,
          display: "flex",
          alignItems: "center",
          gap: 18,
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span
          style={{
            font: `600 13px ${font.sans}`,
            color: color.ink,
            padding: "10px 0",
            borderBottom: `2px solid ${color.dark}`,
          }}
        >
          Code
        </span>
      </div>
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
      <Icon name={row.isDir ? "modules" : "document"} size={13} color={row.isDir ? color.accent : color.iconIdle} />
      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{row.name}</span>
    </button>
  );
}

function FileViewer({
  selected,
  latest,
  loading,
  error,
  text,
  lines,
}: {
  selected: string | null;
  latest: CommitInfo | null;
  loading: boolean;
  error: string | null;
  text: string | null;
  lines: string[];
}) {
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
          title={selected ?? ""}
          style={{
            font: `600 12px ${font.mono}`,
            color: selected ? color.inkSoft : color.muted2,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {selected ?? "Select a file"}
        </span>
        {latest && (
          <span
            title={latest.id}
            style={{
              marginLeft: "auto",
              minWidth: 0,
              font: `400 10px ${font.mono}`,
              color: color.muted2,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {latest.summary} - {latest.author} - {relTime(latest.time)}
          </span>
        )}
      </div>
      <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        {loading && <CenterNote title="Loading file..." />}
        {error && <ErrorNote message={error} padded />}
        {!loading && !error && text !== null && (
          <div style={{ minWidth: "max-content", padding: "8px 0 20px" }}>
            {lines.map((line, index) => (
              <div
                key={index}
                style={{
                  display: "flex",
                  font: `400 12px ${font.mono}`,
                  lineHeight: 1.65,
                  minWidth: "max-content",
                }}
              >
                <span
                  style={{
                    width: 48,
                    flexShrink: 0,
                    textAlign: "right",
                    paddingRight: 12,
                    color: color.iconIdle,
                    userSelect: "none",
                    background: color.sidebar,
                  }}
                >
                  {index + 1}
                </span>
                <span style={{ flex: 1, whiteSpace: "pre", color: color.inkSoft, paddingLeft: 13, paddingRight: 24 }}>
                  {line || " "}
                </span>
              </div>
            ))}
          </div>
        )}
        {!loading && !error && text === null && (
          <CenterNote title={selected ? selected.split("/").pop() || selected : "Select a file"} />
        )}
      </div>
    </div>
  );
}

function CommitLog({ commits, loading }: { commits: CommitInfo[]; loading: boolean }) {
  return (
    <div style={{ flex: "1 1 0", minHeight: 0, overflowY: "auto", padding: "14px 14px 10px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={panelLabel}>COMMITS</span>
        <span style={{ font: `500 10px ${font.mono}`, color: color.muted2 }}>{commits.length}</span>
      </div>
      {loading && <InlineNote>Loading commits...</InlineNote>}
      {!loading && commits.length === 0 && <InlineNote>No commits yet</InlineNote>}
      <div style={{ marginTop: 9, display: "grid", gap: 7 }}>
        {commits.map((commit) => (
          <div
            key={commit.id}
            title={commit.id}
            style={{
              border: `1px solid ${color.border}`,
              borderRadius: radius.sm,
              background: color.paper,
              padding: "9px 10px",
              boxShadow: shadow.card,
            }}
          >
            <div
              style={{
                font: `600 12px ${font.sans}`,
                color: color.ink,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {commit.summary}
            </div>
            <div
              style={{
                marginTop: 5,
                display: "flex",
                alignItems: "center",
                gap: 7,
                font: `400 10px ${font.mono}`,
                color: color.muted2,
              }}
            >
              <span>{commit.author}</span>
              <span>{relTime(commit.time)}</span>
              <span style={{ marginLeft: "auto", color: color.muted3 }}>{shortHash(commit.id)}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function CommitForm({
  path,
  message,
  content,
  canCommit,
  onPath,
  onMessage,
  onContent,
  onSubmit,
}: {
  path: string;
  message: string;
  content: string;
  canCommit: boolean;
  onPath: (value: string) => void;
  onMessage: (value: string) => void;
  onContent: (value: string) => void;
  onSubmit: (event: FormEvent) => void;
}) {
  return (
    <form
      onSubmit={onSubmit}
      style={{
        flexShrink: 0,
        margin: 14,
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        boxShadow: shadow.card,
        padding: 13,
        display: "grid",
        gap: 9,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={panelLabel}>NEW COMMIT</span>
        <StatusPill label="write" tone="neutral" />
      </div>
      <input value={path} onChange={(event) => onPath(event.target.value)} placeholder="README.md" style={fieldStyle} />
      <input
        value={message}
        onChange={(event) => onMessage(event.target.value)}
        placeholder="commit message"
        style={fieldStyle}
      />
      <textarea
        value={content}
        onChange={(event) => onContent(event.target.value)}
        placeholder="file content"
        rows={7}
        style={{ ...fieldStyle, resize: "vertical", minHeight: 116, font: `400 12px ${font.mono}` }}
      />
      <button
        type="submit"
        disabled={!canCommit}
        style={{
          all: "unset",
          cursor: canCommit ? "pointer" : "default",
          alignSelf: "start",
          display: "inline-flex",
          alignItems: "center",
          gap: 7,
          padding: "7px 12px",
          borderRadius: radius.sm,
          background: canCommit ? accentVar : color.chip,
          color: canCommit ? color.onDark : color.muted2,
          font: `600 12px ${font.sans}`,
        }}
      >
        <Icon name="check" size={14} color="currentColor" />
        Commit
      </button>
    </form>
  );
}

function HeadCard({ head }: { head: string | null }) {
  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        boxShadow: shadow.card,
        padding: 15,
        display: "grid",
        gap: 8,
      }}
    >
      <span style={panelLabel}>HEAD COMMIT</span>
      <div
        title={head ?? "unborn repo"}
        style={{
          font: `400 12.5px ${font.mono}`,
          color: head ? color.inkSofter : color.muted2,
          wordBreak: "break-all",
          fontStyle: head ? "normal" : "italic",
        }}
      >
        {head ?? "no commits yet"}
      </div>
    </div>
  );
}

function CenterNote({ title }: { title: string }) {
  return (
    <div
      style={{
        height: "100%",
        minHeight: 180,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `500 12px ${font.sans}`,
        color: color.muted2,
      }}
    >
      {title}
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
          border: `1px solid #eccfc9`,
          borderRadius: radius.sm,
          background: "#fbeeec",
          color: "#a35248",
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

function StatusPill({ label, tone }: { label: string; tone: "success" | "warning" | "neutral" }) {
  const styles =
    tone === "success"
      ? { color: "#5f9e74", bg: "#eef5f0", bd: "#cfe3d7" }
      : tone === "warning"
        ? { color: "#a07b32", bg: "#fbf4e6", bd: "#ecdcae" }
        : { color: "#7a6f9e", bg: "#f1edf5", bd: "#ddd2e6" };
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        height: 20,
        padding: "0 8px",
        borderRadius: radius.sm,
        border: `1px solid ${styles.bd}`,
        background: styles.bg,
        color: styles.color,
        font: `700 9px ${font.mono}`,
        letterSpacing: ".06em",
        textTransform: "uppercase",
      }}
    >
      {label}
    </span>
  );
}
