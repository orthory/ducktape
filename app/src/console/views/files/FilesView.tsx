// The duckfs browser: a live directory tree over the node's `files` module (a
// consensus-replicated, copy-on-write filesystem). It pages `ls` off the live
// transport (context.transport) with breadcrumb navigation, opens a file panel
// (preview + download) on click, uploads into the current directory (staging
// chunks with per-chunk progress), deletes with a confirm, and — via the
// history panel — browses any past snapshot and diffs it against head.
//
// Reads/writes go straight to domain/files-client (like the forge browser);
// the store keeps only a flat Find projection for the command palette.

import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ChangeEvent,
  DragEvent as ReactDragEvent,
  FormEvent,
  MouseEvent as ReactMouseEvent,
  ReactNode,
} from "react";

import { deletePath, joinPath, ls, mkdir, readAll, refs, uploadFile } from "../../../domain/files-client";
import type { FileEntry } from "../../../domain/files-client";
import { Icon, type IconName } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";
import { FilePreview } from "./FilePreview";
import { errMsg, humanBytes } from "./files-format";
import { HistoryPanel } from "./HistoryPanel";

/** Where the browser opens and where root-level writes land by default. */
const DEFAULT_DIR = "/shared";

interface UploadState {
  name: string;
  staged: number;
  total: number;
}

interface ContextMenuState {
  x: number;
  y: number;
  entry: FileEntry | null;
}

interface DirectoryColumn {
  path: string;
  entries: FileEntry[];
  cursor: string | null;
  loading: boolean;
  error: string | null;
}

/** dirs before files, then case-insensitive by name — a stable browse order on
 *  top of the module's raw name-order page. */
const sortEntries = (entries: FileEntry[]): FileEntry[] =>
  [...entries].sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === "dir" ? -1 : b.kind === "dir" ? 1 : 0;
    return a.path.localeCompare(b.path);
  });

const writeTargetDir = (dir: string): string => (dir === "/" ? DEFAULT_DIR : dir);
const basename = (path: string): string => path.split("/").pop() || path;
const makeDirectoryColumn = (path: string): DirectoryColumn => ({
  path,
  entries: [],
  cursor: null,
  loading: true,
  error: null,
});
const parentDir = (path: string): string => {
  const trimmed = path.replace(/\/+$/, "");
  const slash = trimmed.lastIndexOf("/");
  return slash <= 0 ? "/" : trimmed.slice(0, slash);
};
const columnPathsFor = (path: string): string[] => {
  if (path === "/") return ["/"];
  const segments = path.replace(/^\/+|\/+$/g, "").split("/").filter(Boolean);
  let acc = "";
  return segments.map((segment) => {
    acc += `/${segment}`;
    return acc;
  });
};

function CenterState({ title, detail, muted }: { title: string; detail: string; muted?: boolean }) {
  return (
    <div
      style={{
        minHeight: 280,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 9,
        padding: 24,
        textAlign: "center",
      }}
    >
      <span
        style={{
          width: 36,
          height: 36,
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: muted ? color.sunken : "#eef5f0",
          color: muted ? color.muted : color.green,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <Icon name="files" size={17} strokeWidth={1.7} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>{title}</div>
      <div style={{ maxWidth: 360, font: `400 11.5px ${font.sans}`, color: color.muted2, lineHeight: 1.55 }}>
        {detail}
      </div>
    </div>
  );
}

function HeaderButton({
  label,
  icon,
  disabled,
  active,
  onClick,
}: {
  label: string;
  icon: "plus" | "modules" | "metrics";
  disabled?: boolean;
  active?: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      disabled={disabled}
      aria-pressed={active}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        height: 32,
        padding: "0 12px",
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        borderRadius: radius.sm,
        border: `1px solid ${disabled ? color.borderSoft : color.borderStrong}`,
        background: disabled ? color.sunken : active ? color.sidebar : hover ? color.hover : color.paper,
        color: disabled ? color.muted2 : color.inkSoft,
        cursor: disabled ? "default" : "pointer",
        font: `600 12px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      <Icon name={icon} size={13} strokeWidth={1.9} />
      {label}
    </button>
  );
}

function Breadcrumb({ dir, onNavigate }: { dir: string; onNavigate: (path: string) => void }) {
  const segments = dir === "/" ? [] : dir.replace(/^\//, "").split("/");
  const crumbs = [{ name: "/", path: "/" }];
  let acc = "";
  for (const segment of segments) {
    acc += `/${segment}`;
    crumbs.push({ name: segment, path: acc });
  }
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 3, minWidth: 0, overflow: "hidden" }}>
      {crumbs.map((crumb, index) => {
        const last = index === crumbs.length - 1;
        return (
          <span key={crumb.path} style={{ display: "inline-flex", alignItems: "center", gap: 3, minWidth: 0 }}>
            {index > 0 && <Icon name="chevronRight" size={11} strokeWidth={1.9} color={color.muted2} />}
            <button
              type="button"
              disabled={last}
              onClick={() => onNavigate(crumb.path)}
              style={{
                all: "unset",
                cursor: last ? "default" : "pointer",
                font: `${last ? 600 : 500} 13px ${font.sans}`,
                color: last ? color.dark : color.muted,
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
            >
              {crumb.name === "/" ? "root" : crumb.name}
            </button>
          </span>
        );
      })}
    </div>
  );
}

function EntryRow({
  entry,
  selected,
  onOpen,
  onContextMenu,
  onPrepareDownload,
  onDragStart,
}: {
  entry: FileEntry;
  selected: boolean;
  onOpen: () => void;
  onContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  onPrepareDownload: () => void;
  onDragStart: (event: ReactDragEvent<HTMLButtonElement>) => void;
}) {
  const [hover, setHover] = useState(false);
  const isDir = entry.kind === "dir";
  const name = basename(entry.path);
  return (
    <button
      type="button"
      aria-label={`${isDir ? "Open folder" : "Open file"} ${name}`}
      draggable={!isDir}
      onClick={onOpen}
      onContextMenu={onContextMenu}
      onMouseDown={() => {
        if (!isDir) onPrepareDownload();
      }}
      onDragStart={(event) => {
        if (!isDir) onDragStart(event);
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        width: "100%",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "11px 16px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: selected ? color.sidebar : hover ? color.hover : "transparent",
      }}
    >
      <span
        style={{
          width: 28,
          height: 28,
          borderRadius: radius.sm,
          border: `1px solid ${color.border}`,
          background: isDir ? "#eef5f0" : color.sunken,
          color: isDir ? color.green : color.muted3,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
        }}
      >
        <Icon name={isDir ? "modules" : "files"} size={14} strokeWidth={1.7} />
      </span>
      <span
        title={name}
        style={{
          flex: 1,
          minWidth: 0,
          font: `${isDir ? 600 : 500} 13.5px ${font.sans}`,
          color: color.ink,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {name}
        {entry.kind === "symlink" ? " ↪" : ""}
      </span>
      <span style={{ font: `400 11px ${font.mono}`, color: color.muted2, flexShrink: 0 }}>
        {isDir ? "" : humanBytes(entry.size)}
        {entry.exec ? " · exec" : ""}
      </span>
      {isDir && <Icon name="chevronRight" size={14} strokeWidth={1.9} color={color.muted2} />}
    </button>
  );
}

function DirectoryColumnView({
  column,
  selectedPath,
  childPath,
  onOpen,
  onContextMenu,
  onLoadMore,
  onPrepareDownload,
  onFileDragStart,
  onDragOverFiles,
  onDropFiles,
}: {
  column: DirectoryColumn;
  selectedPath: string | null;
  childPath: string | null;
  onOpen: (entry: FileEntry) => void;
  onContextMenu: (
    event: ReactMouseEvent<HTMLButtonElement | HTMLElement>,
    entry: FileEntry | null,
  ) => void;
  onLoadMore: (path: string) => void;
  onPrepareDownload: (entry: FileEntry) => void;
  onFileDragStart: (event: ReactDragEvent<HTMLButtonElement>, entry: FileEntry) => void;
  onDragOverFiles: (event: ReactDragEvent<HTMLElement>, path: string) => void;
  onDropFiles: (event: ReactDragEvent<HTMLElement>, path: string) => void;
}) {
  const rows = sortEntries(column.entries);
  const label = column.path === "/" ? "root" : basename(column.path);

  return (
    <section
      role="region"
      aria-label={`Column ${column.path}`}
      onContextMenu={(event) => onContextMenu(event, null)}
      onDragOver={(event) => onDragOverFiles(event, column.path)}
      onDrop={(event) => onDropFiles(event, column.path)}
      style={{
        width: 286,
        flex: "0 0 286px",
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        borderRight: `1px solid ${color.borderSoft}`,
        background: color.paper,
      }}
    >
      <div
        style={{
          height: 38,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "0 12px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.sunken,
        }}
      >
        <Icon name="modules" size={13} strokeWidth={1.8} color={color.muted3} />
        <span
          title={column.path}
          style={{
            minWidth: 0,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            font: `700 12px ${font.sans}`,
            color: color.ink,
          }}
        >
          {label}
        </span>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {column.loading ? (
          <CenterState title="Loading…" detail="Reading this directory." muted />
        ) : column.error ? (
          <CenterState title="Could not read folder" detail={column.error} muted />
        ) : rows.length === 0 ? (
          <CenterState title="Empty directory" detail="Nothing here." muted />
        ) : (
          <>
            {rows.map((entry) => (
              <EntryRow
                key={entry.path}
                entry={entry}
                selected={selectedPath === entry.path || childPath === entry.path}
                onOpen={() => onOpen(entry)}
                onContextMenu={(event) => onContextMenu(event, entry)}
                onPrepareDownload={() => onPrepareDownload(entry)}
                onDragStart={(event) => onFileDragStart(event, entry)}
              />
            ))}
            {column.cursor && (
              <button
                type="button"
                onClick={() => onLoadMore(column.path)}
                style={{
                  all: "unset",
                  boxSizing: "border-box",
                  width: "100%",
                  cursor: "pointer",
                  textAlign: "center",
                  padding: "10px 0",
                  font: `600 11.5px ${font.sans}`,
                  color: color.muted,
                }}
              >
                Load more
              </button>
            )}
          </>
        )}
      </div>
    </section>
  );
}

function ContextMenuItem({
  label,
  icon,
  danger,
  disabled,
  onSelect,
}: {
  label: string;
  icon: IconName;
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      role="menuitem"
      disabled={disabled}
      onClick={(event) => {
        event.stopPropagation();
        if (!disabled) onSelect();
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        width: "100%",
        minHeight: 30,
        padding: "0 9px",
        display: "flex",
        alignItems: "center",
        gap: 8,
        borderRadius: radius.sm,
        background: hover && !disabled ? (danger ? color.dangerSoft : color.hover) : "transparent",
        color: disabled ? color.muted2 : danger ? color.danger : color.inkSoft,
        cursor: disabled ? "default" : "pointer",
        font: `500 12.5px ${font.sans}`,
      }}
    >
      <Icon name={icon} size={13} strokeWidth={1.8} />
      <span>{label}</span>
    </button>
  );
}

function FilesContextMenu({
  menu,
  readOnly,
  onClose,
  onOpen,
  onNewFolder,
  onUpload,
  onDelete,
  onRefresh,
}: {
  menu: ContextMenuState;
  readOnly: boolean;
  onClose: () => void;
  onOpen: (entry: FileEntry) => void;
  onNewFolder: () => void;
  onUpload: () => void;
  onDelete: (entry: FileEntry) => void;
  onRefresh: () => void;
}) {
  const entry = menu.entry;
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    const timer = setTimeout(() => document.addEventListener("click", onClose), 0);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("click", onClose);
      clearTimeout(timer);
    };
  }, [onClose]);

  return (
    <div
      role="menu"
      aria-label="File actions"
      onClick={(event) => event.stopPropagation()}
      style={{
        position: "fixed",
        left: Math.max(8, menu.x),
        top: Math.max(8, menu.y),
        zIndex: 60,
        width: 184,
        padding: 4,
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        boxShadow: shadow.pop,
      }}
    >
      {entry && (
        <>
          <ContextMenuItem
            label="Open"
            icon={entry.kind === "dir" ? "modules" : "files"}
            onSelect={() => {
              onOpen(entry);
              onClose();
            }}
          />
          <ContextMenuItem
            label="Delete"
            icon="close"
            danger
            disabled={readOnly}
            onSelect={() => {
              onDelete(entry);
              onClose();
            }}
          />
          <div style={{ height: 1, margin: "4px 6px", background: color.borderSoft }} />
        </>
      )}
      <ContextMenuItem
        label="New folder"
        icon="modules"
        disabled={readOnly}
        onSelect={() => {
          onNewFolder();
          onClose();
        }}
      />
      <ContextMenuItem
        label="Upload"
        icon="plus"
        disabled={readOnly}
        onSelect={() => {
          onUpload();
          onClose();
        }}
      />
      <ContextMenuItem
        label="Refresh"
        icon="refresh"
        onSelect={() => {
          onRefresh();
          onClose();
        }}
      />
    </div>
  );
}

function DialogButton({
  label,
  variant,
  disabled,
  onClick,
}: {
  label: string;
  variant?: "primary" | "danger";
  disabled?: boolean;
  onClick?: () => void;
}) {
  const [hover, setHover] = useState(false);
  const activeBg = variant === "danger" ? color.danger : color.green;
  return (
    <button
      type={onClick ? "button" : "submit"}
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        height: 32,
        minWidth: 74,
        padding: "0 12px",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        borderRadius: radius.sm,
        border: `1px solid ${variant ? activeBg : color.borderStrong}`,
        background: disabled
          ? color.sunken
          : variant
            ? activeBg
            : hover
              ? color.hover
              : color.paper,
        color: disabled ? color.muted2 : variant ? "#fff" : color.inkSoft,
        cursor: disabled ? "default" : "pointer",
        font: `600 12px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </button>
  );
}

function ModalShell({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 70,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 20,
        background: "rgba(38, 37, 31, 0.18)",
      }}
    >
      {children}
    </div>
  );
}

function NewFolderDialog({
  value,
  busy,
  onChange,
  onSubmit,
  onCancel,
}: {
  value: string;
  busy: boolean;
  onChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onCancel: () => void;
}) {
  return (
    <ModalShell>
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="new-folder-title"
        style={{
          width: "min(360px, 100%)",
          borderRadius: radius.lg,
          border: `1px solid ${color.border}`,
          background: color.paper,
          boxShadow: shadow.pop,
          padding: 16,
        }}
      >
        <form onSubmit={onSubmit}>
          <div id="new-folder-title" style={{ font: `700 15px ${font.sans}`, color: color.dark }}>
            New folder
          </div>
          <label
            htmlFor="new-folder-name"
            style={{
              display: "block",
              marginTop: 14,
              marginBottom: 6,
              font: `600 11px ${font.sans}`,
              color: color.muted3,
            }}
          >
            Folder name
          </label>
          <input
            id="new-folder-name"
            autoFocus
            value={value}
            disabled={busy}
            onChange={(event) => onChange(event.target.value)}
            style={{
              boxSizing: "border-box",
              width: "100%",
              height: 36,
              borderRadius: radius.sm,
              border: `1px solid ${color.borderStrong}`,
              background: color.paper,
              color: color.ink,
              padding: "0 10px",
              font: `500 13px ${font.sans}`,
              outline: "none",
            }}
          />
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
            <DialogButton label="Cancel" disabled={busy} onClick={onCancel} />
            <DialogButton label="Create folder" variant="primary" disabled={busy || value.trim() === ""} />
          </div>
        </form>
      </div>
    </ModalShell>
  );
}

function DeleteEntryDialog({
  entry,
  busy,
  onCancel,
  onConfirm,
}: {
  entry: FileEntry;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const name = basename(entry.path);
  return (
    <ModalShell>
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-entry-title"
        style={{
          width: "min(360px, 100%)",
          borderRadius: radius.lg,
          border: `1px solid ${color.dangerBorder}`,
          background: color.paper,
          boxShadow: shadow.pop,
          padding: 16,
        }}
      >
        <div id="delete-entry-title" style={{ font: `700 15px ${font.sans}`, color: color.dark }}>
          Delete {name}
        </div>
        <div style={{ marginTop: 8, font: `400 12px ${font.sans}`, color: color.muted3, lineHeight: 1.5 }}>
          This removes the selected {entry.kind === "dir" ? "folder" : "file"} from the live filesystem.
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
          <DialogButton label="Cancel" disabled={busy} onClick={onCancel} />
          <DialogButton label="Delete" variant="danger" disabled={busy} onClick={onConfirm} />
        </div>
      </div>
    </ModalShell>
  );
}

export function FilesView() {
  const { state, transport } = useDucktape();
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const dragDownloadUrls = useRef<Map<string, string>>(new Map());
  const dragDownloadRequests = useRef<Set<string>>(new Set());

  const [columns, setColumns] = useState<DirectoryColumn[]>([makeDirectoryColumn(DEFAULT_DIR)]);
  const [snapshot, setSnapshot] = useState<string | null>(null);
  const [head, setHead] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [selected, setSelected] = useState<FileEntry | null>(null);
  const [upload, setUpload] = useState<UploadState | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [newFolderOpen, setNewFolderOpen] = useState(false);
  const [newFolderName, setNewFolderName] = useState("");
  const [creatingFolder, setCreatingFolder] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<FileEntry | null>(null);

  const backed = Boolean(state.status?.modules.some((m) => m.id === "files"));
  const readOnly = snapshot !== null;
  const bumpReload = useCallback(() => setReloadToken((n) => n + 1), []);
  const dir = columns[columns.length - 1]?.path ?? DEFAULT_DIR;
  const error = actionError ?? columns.find((column) => column.error)?.error ?? null;
  const columnKey = columns.map((column) => column.path).join("\0");
  const dragDownloadKey = (entry: FileEntry): string => `${snapshot ?? "live"}:${entry.path}`;

  useEffect(
    () => () => {
      dragDownloadUrls.current.forEach((url) => URL.revokeObjectURL?.(url));
      dragDownloadUrls.current.clear();
      dragDownloadRequests.current.clear();
    },
    [],
  );

  useEffect(() => {
    dragDownloadUrls.current.forEach((url) => URL.revokeObjectURL?.(url));
    dragDownloadUrls.current.clear();
    dragDownloadRequests.current.clear();
  }, [snapshot, transport]);

  // Track the live head for the history panel's diff base.
  useEffect(() => {
    if (!transport) return;
    let alive = true;
    refs(transport)
      .then((r) => alive && setHead(r.head))
      .catch(() => alive && setHead(null));
    return () => {
      alive = false;
    };
  }, [transport, reloadToken]);

  // Page each visible browser column. A fresh live network may not have /shared
  // yet; keep that as an empty writeable default instead of drifting writes to root.
  useEffect(() => {
    if (!transport) return;
    let alive = true;
    const paths = columns.map((column) => column.path);

    setColumns((prev) =>
      prev.map((column) =>
        paths.includes(column.path) ? { ...column, loading: true, error: null } : column,
      ),
    );

    paths.forEach((path) => {
      ls(transport, { path, snapshot: snapshot ?? undefined })
        .then((page) => {
          if (!alive) return;
          setColumns((prev) =>
            prev.map((column) =>
              column.path === path
                ? { ...column, entries: page.entries, cursor: page.next, loading: false, error: null }
                : column,
            ),
          );
        })
        .catch((err) => {
          if (!alive) return;
          setColumns((prev) =>
            prev.map((column) => {
              if (column.path !== path) return column;
              if (path === DEFAULT_DIR && snapshot === null) {
                return { ...column, entries: [], cursor: null, loading: false, error: null };
              }
              return { ...column, entries: [], cursor: null, loading: false, error: errMsg(err) };
            }),
          );
        });
    });

    return () => {
      alive = false;
    };
  }, [transport, columnKey, snapshot, reloadToken]);

  const navigate = (path: string) => {
    setContextMenu(null);
    setSelected(null);
    setColumns((prev) =>
      columnPathsFor(path).map(
        (columnPath) => prev.find((column) => column.path === columnPath) ?? makeDirectoryColumn(columnPath),
      ),
    );
  };

  const selectSnapshot = (id: string | null) => {
    setSelected(null);
    setSnapshot(id);
    setColumns((prev) =>
      prev.map((column) => ({
        ...column,
        entries: [],
        cursor: null,
        loading: true,
        error: null,
      })),
    );
  };

  const loadMore = (path: string) => {
    const cursor = columns.find((column) => column.path === path)?.cursor ?? null;
    if (!transport || !cursor) return;
    ls(transport, { path, snapshot: snapshot ?? undefined, after: cursor })
      .then((page) => {
        setColumns((prev) =>
          prev.map((column) =>
            column.path === path
              ? { ...column, entries: [...column.entries, ...page.entries], cursor: page.next, error: null }
              : column,
          ),
        );
      })
      .catch((err) =>
        setColumns((prev) =>
          prev.map((column) => (column.path === path ? { ...column, error: errMsg(err) } : column)),
        ),
      );
  };

  const openEntry = (entry: FileEntry) => {
    setContextMenu(null);
    if (entry.kind === "dir") {
      navigate(entry.path);
    } else {
      setColumns((prev) =>
        columnPathsFor(parentDir(entry.path)).map(
          (columnPath) => prev.find((column) => column.path === columnPath) ?? makeDirectoryColumn(columnPath),
        ),
      );
      setSelected(entry);
    }
  };

  const openContextMenu = (event: ReactMouseEvent, entry: FileEntry | null) => {
    if (!transport || !backed) return;
    event.preventDefault();
    event.stopPropagation();
    setContextMenu({ x: event.clientX, y: event.clientY, entry });
  };

  const openNewFolderDialog = () => {
    if (!transport || readOnly) return;
    setContextMenu(null);
    setNewFolderName("");
    setNewFolderOpen(true);
  };

  const openUploadPicker = () => {
    if (!transport || readOnly) return;
    setContextMenu(null);
    fileInputRef.current?.click();
  };

  const refreshDirectory = () => {
    setContextMenu(null);
    bumpReload();
  };

  const uploadBrowserFiles = async (files: File[], targetDir: string) => {
    if (!transport || readOnly || files.length === 0) return;
    setActionError(null);
    try {
      for (const file of files) {
        const bytes = new Uint8Array(await file.arrayBuffer());
        setUpload({ name: file.name, staged: 0, total: 0 });
        await uploadFile(transport, {
          path: joinPath(writeTargetDir(targetDir), file.name),
          bytes,
          meta: file.type ? { mime: file.type } : {},
          onProgress: (staged, total) => setUpload({ name: file.name, staged, total }),
        });
      }
      setUpload(null);
      bumpReload();
    } catch (err) {
      setUpload(null);
      setActionError(errMsg(err));
    }
  };

  const handleFileChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const input = event.target;
    const file = input.files?.[0] ?? null;
    input.value = "";
    if (!file) return;
    await uploadBrowserFiles([file], dir);
  };

  const hasDroppedFiles = (event: ReactDragEvent<HTMLElement>): boolean =>
    Array.from(event.dataTransfer.types ?? []).includes("Files") || event.dataTransfer.files.length > 0;

  const handleColumnDragOver = (event: ReactDragEvent<HTMLElement>) => {
    if (!transport || readOnly || !backed || !hasDroppedFiles(event)) return;
    event.preventDefault();
    event.stopPropagation();
    event.dataTransfer.dropEffect = "copy";
  };

  const handleColumnDrop = (event: ReactDragEvent<HTMLElement>, targetDir: string) => {
    if (!transport || readOnly || !backed || !hasDroppedFiles(event)) return;
    event.preventDefault();
    event.stopPropagation();
    event.dataTransfer.dropEffect = "copy";
    void uploadBrowserFiles(Array.from(event.dataTransfer.files), targetDir);
  };

  const prepareDragDownload = (entry: FileEntry) => {
    if (!transport || entry.kind !== "file" || typeof URL.createObjectURL !== "function") return;
    const key = dragDownloadKey(entry);
    if (dragDownloadUrls.current.has(key) || dragDownloadRequests.current.has(key)) return;
    dragDownloadRequests.current.add(key);
    readAll(transport, { path: entry.path, snapshot: snapshot ?? undefined })
      .then((bytes) => {
        const mime = entry.meta.mime || "application/octet-stream";
        const url = URL.createObjectURL(new Blob([bytes], { type: mime }));
        const previous = dragDownloadUrls.current.get(key);
        if (previous) URL.revokeObjectURL?.(previous);
        dragDownloadUrls.current.set(key, url);
      })
      .catch((err) => setActionError(errMsg(err)))
      .finally(() => dragDownloadRequests.current.delete(key));
  };

  const handleFileDragStart = (event: ReactDragEvent<HTMLButtonElement>, entry: FileEntry) => {
    if (entry.kind !== "file") return;
    const name = basename(entry.path);
    const mime = entry.meta.mime || "application/octet-stream";
    const downloadUrl = dragDownloadUrls.current.get(dragDownloadKey(entry));
    event.dataTransfer.effectAllowed = "copy";
    event.dataTransfer.setData("application/x-ducktape-file-path", entry.path);
    event.dataTransfer.setData("text/plain", entry.path);
    if (downloadUrl) {
      event.dataTransfer.setData("DownloadURL", `${mime}:${name}:${downloadUrl}`);
    } else {
      prepareDragDownload(entry);
    }
  };

  const handleNewFolderSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!transport || readOnly) return;
    const name = newFolderName.trim();
    if (!name) return;
    setActionError(null);
    setCreatingFolder(true);
    try {
      await mkdir(transport, { path: joinPath(writeTargetDir(dir), name) });
      setNewFolderOpen(false);
      setNewFolderName("");
      bumpReload();
    } catch (err) {
      setActionError(errMsg(err));
    } finally {
      setCreatingFolder(false);
    }
  };

  const deleteEntry = async (entry: FileEntry) => {
    if (!transport || readOnly) return;
    setDeleting(true);
    try {
      await deletePath(transport, { path: entry.path });
      setSelected((current) => (current?.path === entry.path ? null : current));
      if (entry.kind === "dir") {
        setColumns((prev) => {
          const next = prev.filter(
            (column) => column.path !== entry.path && !column.path.startsWith(`${entry.path}/`),
          );
          return next.length > 0 ? next : [makeDirectoryColumn(parentDir(entry.path))];
        });
      }
      setDeleteTarget(null);
      bumpReload();
    } catch (err) {
      setActionError(errMsg(err));
    } finally {
      setDeleting(false);
    }
  };

  const handleDelete = async () => {
    if (!selected) return;
    await deleteEntry(selected);
  };

  return (
    <div
      data-screen-label="Files"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.paper,
      }}
    >
      <div
        style={{
          minHeight: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "0 20px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Files</span>
        <Breadcrumb dir={dir} onNavigate={navigate} />
        {readOnly && (
          <span
            style={{
              font: `600 10px ${font.mono}`,
              color: color.amber,
              border: `1px solid ${color.borderSoft}`,
              borderRadius: radius.sm,
              padding: "2px 7px",
              whiteSpace: "nowrap",
            }}
          >
            snapshot
          </span>
        )}

        <div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
          <input ref={fileInputRef} type="file" onChange={handleFileChange} style={{ display: "none" }} />
          <HeaderButton
            label="New folder"
            icon="modules"
            disabled={!backed || readOnly}
            onClick={openNewFolderDialog}
          />
          <HeaderButton
            label="Upload"
            icon="plus"
            disabled={!backed || readOnly}
            onClick={openUploadPicker}
          />
          <HeaderButton
            label="History"
            icon="metrics"
            disabled={!backed}
            active={showHistory}
            onClick={() => setShowHistory((v) => !v)}
          />
        </div>
      </div>

      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        <div style={{ flex: 1, minWidth: 0, minHeight: 0, overflowY: "auto", padding: 18, background: color.sidebar }}>
          <div
            onContextMenu={(event) => openContextMenu(event, null)}
            style={{
              height: "100%",
              minHeight: 360,
              display: "flex",
              flexDirection: "column",
              borderRadius: radius.lg,
              border: `1px solid ${color.border}`,
              background: color.paper,
              boxShadow: shadow.card,
              overflow: "hidden",
            }}
          >
            {!transport ? (
              <CenterState title="No node connected" detail="Connect a node to browse its files." muted />
            ) : state.status === null ? (
              <CenterState title="Loading files…" detail="Waiting for this node's filesystem." muted />
            ) : !backed ? (
              <CenterState
                title="Files module is not available"
                detail="This node did not report a files module, so browsing and uploads are disabled."
                muted
              />
            ) : (
              <>
                {upload && (
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 9,
                      padding: "10px 16px",
                      borderBottom: `1px solid ${color.borderSoft}`,
                      background: color.sunken,
                      font: `500 12px ${font.sans}`,
                      color: color.muted3,
                    }}
                  >
                    <Icon name="refresh" size={13} strokeWidth={1.9} />
                    uploading {upload.name}
                    {upload.total > 0 ? ` — chunk ${upload.staged}/${upload.total}` : "…"}
                  </div>
                )}
                {error && (
                  <div
                    style={{
                      padding: "10px 16px",
                      borderBottom: `1px solid ${color.dangerBorder}`,
                      background: color.dangerSoft,
                      font: `500 12px ${font.sans}`,
                      color: color.danger,
                    }}
                  >
                    {error}
                  </div>
                )}

                <div
                  style={{
                    flex: 1,
                    minHeight: 0,
                    display: "flex",
                    overflowX: "auto",
                    overflowY: "hidden",
                  }}
                >
                  {columns.map((column, index) => (
                    <DirectoryColumnView
                      key={column.path}
                      column={column}
                      selectedPath={selected?.path ?? null}
                      childPath={columns[index + 1]?.path ?? null}
                      onOpen={openEntry}
                      onContextMenu={openContextMenu}
                      onLoadMore={loadMore}
                      onPrepareDownload={prepareDragDownload}
                      onFileDragStart={handleFileDragStart}
                      onDragOverFiles={handleColumnDragOver}
                      onDropFiles={handleColumnDrop}
                    />
                  ))}
                  {selected && transport && (
                    <FilePreview
                      transport={transport}
                      entry={selected}
                      snapshot={snapshot}
                      readOnly={readOnly}
                      deleting={deleting}
                      onClose={() => setSelected(null)}
                      onDelete={handleDelete}
                    />
                  )}
                </div>
              </>
            )}
          </div>
        </div>

        {contextMenu && (
          <FilesContextMenu
            menu={contextMenu}
            readOnly={readOnly}
            onClose={() => setContextMenu(null)}
            onOpen={openEntry}
            onNewFolder={openNewFolderDialog}
            onUpload={openUploadPicker}
            onDelete={setDeleteTarget}
            onRefresh={refreshDirectory}
          />
        )}

        {newFolderOpen && (
          <NewFolderDialog
            value={newFolderName}
            busy={creatingFolder}
            onChange={setNewFolderName}
            onSubmit={handleNewFolderSubmit}
            onCancel={() => {
              if (creatingFolder) return;
              setNewFolderOpen(false);
              setNewFolderName("");
            }}
          />
        )}

        {deleteTarget && (
          <DeleteEntryDialog
            entry={deleteTarget}
            busy={deleting}
            onCancel={() => {
              if (!deleting) setDeleteTarget(null);
            }}
            onConfirm={() => void deleteEntry(deleteTarget)}
          />
        )}

        {showHistory && transport && backed && (
          <HistoryPanel
            transport={transport}
            head={head}
            snapshot={snapshot}
            reloadToken={reloadToken}
            onSelectSnapshot={selectSnapshot}
            onClose={() => setShowHistory(false)}
          />
        )}
      </div>
    </div>
  );
}
