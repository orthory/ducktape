// The memory surface over the node's `memory` module: a filesystem-shaped
// namespace of write-once immutable generations. Paths are canonical absolute
// ("/" is the root). This view is a two-pane browser like DocumentView — a LEFT
// rail that walks the directory tree (breadcrumb + entries) or shows grep
// results, and a MAIN reader that opens a file's generation, prints its body,
// and hosts the publish composer that appends the next generation.

import { useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";

import type { FileStat, Generation, GrepHit, LsEntry } from "../../../domain/memory-client";
import { isDir } from "../../../domain/memory-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpLedger, OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

// ── Path helpers ────────────────────────────────────────

/** Last path segment — the label shown for a dir/file row. Root → "/". */
const lastSeg = (path: string): string => path.split("/").filter(Boolean).pop() ?? "/";

/** The parent directory of a canonical path. Root stays "/". */
const parentOf = (path: string): string =>
  "/" + path.split("/").filter(Boolean).slice(0, -1).join("/");

/** Join a relative name under a dir into a single-slash absolute path. */
const joinPath = (dir: string, name: string): string =>
  "/" + `${dir}/${name}`.split("/").filter(Boolean).join("/");

/** The clickable ancestors of a path, root excluded (rendered separately). */
const crumbsOf = (path: string): { label: string; path: string }[] => {
  const out: { label: string; path: string }[] = [];
  let acc = "";
  for (const segment of path.split("/").filter(Boolean)) {
    acc = `${acc}/${segment}`;
    out.push({ label: segment, path: acc });
  }
  return out;
};

// ── Shared styles ───────────────────────────────────────

const inputStyle: CSSProperties = {
  width: "100%",
  minWidth: 0,
  boxSizing: "border-box",
  padding: "8px 10px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12px ${font.mono}`,
  color: color.ink,
  outline: "none",
};

const sectionLabelStyle: CSSProperties = {
  font: `600 9px ${font.mono}`,
  letterSpacing: ".11em",
  color: color.muted2,
  textTransform: "uppercase",
};

// ── Small presentational pieces ─────────────────────────

function CenterState({
  title,
  detail,
  muted,
}: {
  title: string;
  detail: string;
  muted?: boolean;
}) {
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
          background: muted ? color.sunken : color.sidebar,
          color: muted ? color.muted : color.muted3,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <Icon name="memory" size={17} strokeWidth={1.7} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>{title}</div>
      <div
        style={{
          maxWidth: 360,
          font: `400 11.5px/1.55 ${font.sans}`,
          color: color.muted2,
        }}
      >
        {detail}
      </div>
    </div>
  );
}

/** One dir/file row in the entry list. */
function EntryRow({
  icon,
  label,
  badge,
  ariaLabel,
  active,
  op,
  onClick,
}: {
  icon: "folder" | "document";
  label: string;
  badge?: string;
  ariaLabel: string;
  active?: boolean;
  /** A file row's finalization record (publishes key by path). */
  op?: OpRecord;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      title={label}
      onClick={onClick}
      style={{
        all: "unset",
        cursor: "pointer",
        boxSizing: "border-box",
        width: "calc(100% - 12px)",
        margin: "1px 6px",
        display: "flex",
        alignItems: "center",
        gap: 9,
        padding: "7px 9px",
        borderRadius: radius.sm,
        background: active ? color.hover : "transparent",
        color: active ? color.ink : color.inkSofter,
      }}
    >
      <Icon
        name={icon}
        size={14}
        strokeWidth={1.7}
        style={{ flexShrink: 0, color: active ? accentVar : color.muted2 }}
      />
      <span
        style={{
          flex: 1,
          minWidth: 0,
          font: active ? `600 12.5px ${font.sans}` : `500 12.5px ${font.sans}`,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {label}
      </span>
      {badge ? (
        <span style={{ flexShrink: 0, font: `500 10px ${font.mono}`, color: color.muted2 }}>
          {badge}
        </span>
      ) : null}
      <FinalizationMark op={op} />
    </button>
  );
}

/** One key=value chip from a generation's meta map. */
function MetaChip({ label }: { label: string }) {
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        height: 20,
        padding: "0 7px",
        borderRadius: 5,
        border: `1px solid ${color.border}`,
        background: color.sunken,
        font: `500 10px ${font.mono}`,
        color: color.muted3,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </span>
  );
}

function PrimaryButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      style={{
        all: "unset",
        cursor: disabled ? "default" : "pointer",
        display: "inline-flex",
        alignItems: "center",
        gap: 7,
        borderRadius: 8,
        background: disabled ? color.chip : accentVar,
        color: disabled ? color.muted2 : "#fff",
        padding: "8px 13px",
        font: `600 12px ${font.sans}`,
      }}
    >
      {children}
    </button>
  );
}

// ── Left rail ───────────────────────────────────────────

function Rail({
  path,
  entries,
  matches,
  openPath,
  ops,
  onBrowse,
  onOpen,
  onOpenHit,
  onSearch,
  onClearSearch,
  onNewFile,
}: {
  path: string;
  entries: LsEntry[];
  matches: GrepHit[] | null;
  openPath: string | null;
  /** The store's finalization ledger — file rows draw their marks. */
  ops: OpLedger;
  onBrowse: (path: string) => void;
  onOpen: (path: string) => void;
  onOpenHit: (hit: GrepHit) => void;
  onSearch: (pattern: string) => void;
  onClearSearch: () => void;
  onNewFile: (name: string) => void;
}) {
  const [pattern, setPattern] = useState("");
  const [newName, setNewName] = useState("");

  const dirs = entries.filter(isDir);
  const files = entries.filter((entry): entry is { File: FileStat } => !isDir(entry));

  const submitSearch = (event: FormEvent) => {
    event.preventDefault();
    if (!pattern.trim()) return;
    onSearch(pattern);
  };

  const submitNewFile = (event: FormEvent) => {
    event.preventDefault();
    if (!newName.trim()) return;
    onNewFile(newName.trim());
    setNewName("");
  };

  return (
    <aside
      style={{
        width: 264,
        flexShrink: 0,
        borderRight: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        display: "flex",
        flexDirection: "column",
        color: color.muted3,
      }}
    >
      <div
        style={{
          height: 56,
          flexShrink: 0,
          padding: "0 15px",
          display: "flex",
          alignItems: "center",
          gap: 9,
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span
          style={{
            width: 26,
            height: 26,
            borderRadius: 8,
            background: color.dark,
            color: color.onDark,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="memory" size={14} strokeWidth={1.7} />
        </span>
        <div style={{ minWidth: 0 }}>
          <div style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>Memory</div>
          <div style={{ marginTop: 1, font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
            agent workspace
          </div>
        </div>
        <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
          {entries.length}
        </div>
      </div>

      {/* Breadcrumb of the active directory. */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          flexWrap: "wrap",
          gap: 2,
          padding: "11px 14px 9px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <button
          type="button"
          aria-label="Browse /"
          onClick={() => onBrowse("/")}
          style={{
            all: "unset",
            cursor: "pointer",
            padding: "2px 5px",
            borderRadius: 5,
            font: `600 12px ${font.mono}`,
            color: path === "/" ? color.ink : color.muted3,
            background: path === "/" ? color.hover : "transparent",
          }}
        >
          /
        </button>
        {crumbsOf(path).map((crumb, index, all) => {
          const current = index === all.length - 1;
          return (
            <span key={crumb.path} style={{ display: "inline-flex", alignItems: "center" }}>
              <Icon
                name="chevronRight"
                size={11}
                strokeWidth={1.8}
                style={{ color: color.muted2 }}
              />
              <button
                type="button"
                aria-label={`Browse ${crumb.path}`}
                onClick={() => onBrowse(crumb.path)}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  padding: "2px 5px",
                  borderRadius: 5,
                  font: `${current ? 600 : 500} 12px ${font.mono}`,
                  color: current ? color.ink : color.muted3,
                  background: current ? color.hover : "transparent",
                  maxWidth: 130,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {crumb.label}
              </button>
            </span>
          );
        })}
      </div>

      {/* Grep search. */}
      <form
        onSubmit={submitSearch}
        style={{ padding: "12px 14px", borderBottom: `1px solid ${color.borderSoft}` }}
      >
        <label htmlFor="memory-search" style={sectionLabelStyle}>
          Search
        </label>
        <div style={{ display: "flex", gap: 7, marginTop: 8 }}>
          <input
            id="memory-search"
            value={pattern}
            onChange={(event) => setPattern(event.target.value)}
            placeholder="substring…"
            spellCheck={false}
            autoCapitalize="none"
            style={inputStyle}
          />
          <button
            type="submit"
            aria-label="Search memory"
            disabled={!pattern.trim()}
            style={{
              all: "unset",
              cursor: pattern.trim() ? "pointer" : "default",
              flexShrink: 0,
              width: 32,
              height: 32,
              borderRadius: 8,
              background: pattern.trim() ? color.dark : color.chip,
              color: pattern.trim() ? color.onDark : color.muted2,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Icon name="chevronRight" size={14} strokeWidth={1.9} />
          </button>
        </div>
      </form>

      {/* New file. */}
      <form
        onSubmit={submitNewFile}
        style={{ padding: "12px 14px", borderBottom: `1px solid ${color.borderSoft}` }}
      >
        <label htmlFor="memory-new-file" style={sectionLabelStyle}>
          New file
        </label>
        <div style={{ display: "flex", gap: 7, marginTop: 8 }}>
          <input
            id="memory-new-file"
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            placeholder="notes.md"
            spellCheck={false}
            autoCapitalize="none"
            style={inputStyle}
          />
          <button
            type="submit"
            aria-label="New file"
            disabled={!newName.trim()}
            style={{
              all: "unset",
              cursor: newName.trim() ? "pointer" : "default",
              flexShrink: 0,
              width: 32,
              height: 32,
              borderRadius: 8,
              background: newName.trim() ? accentVar : color.chip,
              color: newName.trim() ? "#fff" : color.muted2,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Icon name="plus" size={14} strokeWidth={1.9} />
          </button>
        </div>
      </form>

      {/* Entries, or grep results when a search is active. */}
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "12px 0" }}>
        {matches !== null ? (
          <div>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "0 14px 8px",
              }}
            >
              <span style={sectionLabelStyle}>{matches.length} matches</span>
              <button
                type="button"
                aria-label="Clear search"
                onClick={onClearSearch}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  marginLeft: "auto",
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 5,
                  padding: "3px 8px",
                  borderRadius: 6,
                  border: `1px solid ${color.border}`,
                  background: color.paper,
                  color: color.muted3,
                  font: `600 10px ${font.mono}`,
                }}
              >
                <Icon name="close" size={11} strokeWidth={1.9} />
                clear
              </button>
            </div>
            {matches.length === 0 ? (
              <div
                style={{
                  margin: "6px 14px",
                  font: `400 12px/1.45 ${font.sans}`,
                  color: color.muted2,
                }}
              >
                No matches under {path}.
              </div>
            ) : (
              matches.map((hit) => (
                <button
                  key={`${hit.uri}:${hit.line}`}
                  type="button"
                  aria-label={`Open ${hit.path} generation ${hit.generation} line ${hit.line}`}
                  title={hit.uri}
                  onClick={() => onOpenHit(hit)}
                  style={{
                    all: "unset",
                    cursor: "pointer",
                    boxSizing: "border-box",
                    width: "calc(100% - 12px)",
                    margin: "1px 6px",
                    display: "block",
                    padding: "8px 9px",
                    borderRadius: radius.sm,
                  }}
                >
                  <div
                    style={{
                      font: `600 10.5px ${font.mono}`,
                      color: color.muted3,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {`${hit.path}@${hit.generation} :L${hit.line}`}
                  </div>
                  <div
                    style={{
                      marginTop: 3,
                      font: `400 11px ${font.mono}`,
                      color: color.inkSofter,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {hit.text}
                  </div>
                </button>
              ))
            )}
          </div>
        ) : (
          <div>
            {path !== "/" ? (
              <EntryRow
                icon="folder"
                label=".."
                ariaLabel={`Browse ${parentOf(path)}`}
                onClick={() => onBrowse(parentOf(path))}
              />
            ) : null}
            {dirs.map((entry) => (
              <EntryRow
                key={entry.Dir.path}
                icon="folder"
                label={lastSeg(entry.Dir.path)}
                ariaLabel={`Browse ${entry.Dir.path}`}
                onClick={() => onBrowse(entry.Dir.path)}
              />
            ))}
            {files.map((entry) => (
              <EntryRow
                key={entry.File.path}
                icon="document"
                label={lastSeg(entry.File.path)}
                badge={`g${entry.File.latest_generation}`}
                ariaLabel={`Open ${entry.File.path}`}
                active={openPath === entry.File.path}
                op={ops[opKey.memory(entry.File.path)]}
                onClick={() => onOpen(entry.File.path)}
              />
            ))}
            {dirs.length === 0 && files.length === 0 && path === "/" ? (
              <div
                style={{
                  margin: "6px 14px",
                  padding: "13px 12px",
                  border: `1px dashed ${color.borderStrong}`,
                  borderRadius: radius.md,
                  background: color.paper,
                  font: `400 12px/1.45 ${font.sans}`,
                  color: color.muted2,
                }}
              >
                Nothing here yet. Publish a new file above to seed the namespace.
              </div>
            ) : null}
          </div>
        )}
      </div>
    </aside>
  );
}

// ── Main pane: open-file reader + publish composer ──────

function OpenFilePane({
  open,
  op,
  onPublish,
  onDelete,
  onClose,
}: {
  open: { stat: FileStat; generation: Generation };
  /** The open path's finalization record — the meta line draws the mark. */
  op: OpRecord | undefined;
  onPublish: (text: string) => void;
  onDelete: () => void;
  onClose: () => void;
}) {
  const { stat, generation } = open;
  const inline = generation.body.kind === "inline" ? generation.body.value : "";
  const [text, setText] = useState(inline);
  const [confirming, setConfirming] = useState(false);
  const metaEntries = Object.entries(generation.meta);

  return (
    <div
      style={{
        maxWidth: 880,
        margin: "0 auto",
        minHeight: "100%",
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        boxShadow: shadow.card,
        overflow: "hidden",
      }}
    >
      <div style={{ padding: "22px 26px 20px", borderBottom: `1px solid ${color.borderSoft}` }}>
        <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
          <div
            style={{
              width: 34,
              height: 34,
              borderRadius: radius.md,
              background: color.dark,
              color: color.onDark,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
            }}
          >
            <Icon name="document" size={16} />
          </div>
          <div style={{ minWidth: 0, flex: 1 }}>
            <div
              title={stat.path}
              style={{
                font: `650 18px ${font.mono}`,
                color: color.dark,
                overflowWrap: "anywhere",
              }}
            >
              {stat.path}
            </div>
            <div
              style={{
                marginTop: 6,
                display: "flex",
                alignItems: "center",
                gap: 8,
                flexWrap: "wrap",
                font: `500 11px ${font.mono}`,
                color: color.muted2,
              }}
            >
              <span>
                generation {generation.generation} of {stat.generations}
              </span>
              <span style={{ width: 3, height: 3, borderRadius: "50%", background: color.chip }} />
              <span title={generation.author} style={{ maxWidth: 180, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {generation.author || "—"}
              </span>
              <span style={{ width: 3, height: 3, borderRadius: "50%", background: color.chip }} />
              <span>h{generation.published_at_height}</span>
              <FinalizationMark op={op} />
            </div>
          </div>
          <button
            type="button"
            aria-label="Close file"
            onClick={onClose}
            style={{
              all: "unset",
              cursor: "pointer",
              flexShrink: 0,
              width: 27,
              height: 27,
              borderRadius: 7,
              border: `1px solid ${color.border}`,
              background: color.paper,
              color: color.muted3,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Icon name="close" size={13} />
          </button>
        </div>

        {metaEntries.length > 0 ? (
          <div style={{ marginTop: 12, display: "flex", flexWrap: "wrap", gap: 6 }}>
            {metaEntries.map(([key, value]) => (
              <MetaChip key={key} label={`${key}=${value}`} />
            ))}
          </div>
        ) : null}

        <div style={{ marginTop: 14 }}>
          {confirming ? (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "7px 11px",
                borderRadius: radius.sm,
                border: `1px solid ${color.dangerBorder}`,
                background: color.dangerSoft,
              }}
            >
              <span style={{ flex: 1, font: `400 12px ${font.sans}`, color: color.danger }}>
                Delete every generation of this file?
              </span>
              <button
                type="button"
                aria-label="Confirm delete"
                onClick={() => {
                  setConfirming(false);
                  onDelete();
                }}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  padding: "4px 11px",
                  borderRadius: radius.sm,
                  background: color.danger,
                  color: "#fff",
                  font: `600 11.5px ${font.sans}`,
                }}
              >
                Delete
              </button>
              <button
                type="button"
                aria-label="Cancel delete"
                onClick={() => setConfirming(false)}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  padding: "4px 11px",
                  borderRadius: radius.sm,
                  border: `1px solid ${color.borderStrong}`,
                  color: color.muted3,
                  font: `600 11.5px ${font.sans}`,
                }}
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              type="button"
              aria-label="Delete file"
              onClick={() => setConfirming(true)}
              style={{
                all: "unset",
                cursor: "pointer",
                display: "inline-flex",
                alignItems: "center",
                gap: 6,
                padding: "6px 11px",
                borderRadius: radius.sm,
                border: `1px solid ${color.dangerBorder}`,
                background: color.dangerSoft,
                color: color.danger,
                font: `600 11.5px ${font.sans}`,
              }}
            >
              <Icon name="close" size={12} strokeWidth={1.9} />
              Delete file
            </button>
          )}
        </div>
      </div>

      <div style={{ padding: "20px 26px 26px" }}>
        {generation.body.kind === "inline" ? (
          <pre
            style={{
              margin: 0,
              maxHeight: 360,
              overflow: "auto",
              padding: 15,
              borderRadius: radius.md,
              border: `1px solid ${color.border}`,
              background: color.sunken,
              font: `400 12.5px/1.55 ${font.mono}`,
              color: color.ink,
              whiteSpace: "pre-wrap",
              overflowWrap: "anywhere",
            }}
          >
            {generation.body.value}
          </pre>
        ) : (
          <div
            style={{
              padding: 15,
              borderRadius: radius.md,
              border: `1px dashed ${color.borderStrong}`,
              background: color.sunken,
              font: `400 12px ${font.mono}`,
              color: color.muted3,
            }}
          >
            binary file · id {generation.body.value.file_id} · {generation.body.value.size} bytes
          </div>
        )}

        <div style={{ marginTop: 20 }}>
          <label htmlFor="memory-publish" style={sectionLabelStyle}>
            Publish new generation
          </label>
          <textarea
            id="memory-publish"
            value={text}
            onChange={(event) => setText(event.target.value)}
            placeholder="Write the next generation's inline body…"
            style={{
              ...inputStyle,
              marginTop: 8,
              minHeight: 132,
              resize: "vertical",
              font: `400 12.5px/1.5 ${font.mono}`,
            }}
          />
          <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 10 }}>
            <PrimaryButton
              label="Publish new generation"
              disabled={text === inline}
              onClick={() => onPublish(text)}
            >
              <Icon name="plus" size={13} strokeWidth={1.9} />
              Publish new generation
            </PrimaryButton>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Main pane: draft composer for a brand-new file ──────

function DraftPane({
  path,
  onPublish,
  onCancel,
}: {
  path: string;
  onPublish: (text: string) => void;
  onCancel: () => void;
}) {
  const [text, setText] = useState("");

  return (
    <div
      style={{
        maxWidth: 880,
        margin: "0 auto",
        minHeight: "100%",
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        boxShadow: shadow.card,
        overflow: "hidden",
      }}
    >
      <div style={{ padding: "22px 26px 20px", borderBottom: `1px solid ${color.borderSoft}` }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <div
            style={{
              width: 34,
              height: 34,
              borderRadius: radius.md,
              background: color.dark,
              color: color.onDark,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
            }}
          >
            <Icon name="plus" size={16} strokeWidth={1.9} />
          </div>
          <div style={{ minWidth: 0, flex: 1 }}>
            <div style={{ font: `600 12px ${font.sans}`, color: color.muted2 }}>New file</div>
            <div
              title={path}
              style={{
                marginTop: 2,
                font: `650 17px ${font.mono}`,
                color: color.dark,
                overflowWrap: "anywhere",
              }}
            >
              {path}
            </div>
          </div>
          <button
            type="button"
            aria-label="Cancel new file"
            onClick={onCancel}
            style={{
              all: "unset",
              cursor: "pointer",
              flexShrink: 0,
              width: 27,
              height: 27,
              borderRadius: 7,
              border: `1px solid ${color.border}`,
              background: color.paper,
              color: color.muted3,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Icon name="close" size={13} />
          </button>
        </div>
      </div>

      <div style={{ padding: "20px 26px 26px" }}>
        <label htmlFor="memory-draft" style={sectionLabelStyle}>
          First generation
        </label>
        <textarea
          id="memory-draft"
          value={text}
          onChange={(event) => setText(event.target.value)}
          placeholder="Write the file's inline body…"
          style={{
            ...inputStyle,
            marginTop: 8,
            minHeight: 200,
            resize: "vertical",
            font: `400 12.5px/1.5 ${font.mono}`,
          }}
        />
        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 10 }}>
          <PrimaryButton label="Publish file" onClick={() => onPublish(text)}>
            <Icon name="plus" size={13} strokeWidth={1.9} />
            Publish file
          </PrimaryButton>
        </div>
      </div>
    </div>
  );
}

// ── The view ────────────────────────────────────────────

export function MemoryView() {
  const { state, actions } = useDucktape();
  const [draftPath, setDraftPath] = useState<string | null>(null);

  const loading = state.status === null;
  const backed = Boolean(state.status?.modules.some((mod) => mod.id === "memory"));

  // Opening a real file or browsing supersedes any in-progress draft.
  const openFile = (path: string) => {
    setDraftPath(null);
    actions.openMemoryFile({ path });
  };
  const openHit = (hit: GrepHit) => {
    setDraftPath(null);
    actions.openMemoryFile({ path: hit.path, generation: hit.generation });
  };
  const browse = (path: string) => {
    setDraftPath(null);
    actions.browseMemory(path);
  };
  const startDraft = (name: string) => {
    setDraftPath(joinPath(state.memoryPath || "/", name));
  };
  const publishDraft = (text: string) => {
    if (draftPath === null) return;
    actions.publishMemory({ path: draftPath, text });
    setDraftPath(null);
  };

  return (
    <div
      data-screen-label="Memory"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        background: color.paper,
      }}
    >
      {loading || !backed ? (
        <div style={{ flex: 1, minWidth: 0, display: "flex", background: color.sidebar }}>
          <div style={{ margin: "auto" }}>
            {loading ? (
              <CenterState
                title="Loading memory…"
                detail="Waiting for this node's committed status."
                muted
              />
            ) : (
              <CenterState
                title="Memory module is not available"
                detail="This node did not report a memory module, so the workspace is unavailable."
                muted
              />
            )}
          </div>
        </div>
      ) : (
        <>
          <Rail
            path={state.memoryPath}
            entries={state.memoryEntries}
            matches={state.memoryMatches}
            openPath={state.memoryOpen?.stat.path ?? null}
            ops={state.ops}
            onBrowse={browse}
            onOpen={openFile}
            onOpenHit={openHit}
            onSearch={(pattern) =>
              actions.searchMemory({ prefix: state.memoryPath || "/", pattern })
            }
            onClearSearch={actions.clearMemorySearch}
            onNewFile={startDraft}
          />

          <main
            style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}
          >
            <header
              style={{
                height: 56,
                flexShrink: 0,
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "0 22px",
                borderBottom: `1px solid ${color.borderSoft}`,
                background: color.paper,
              }}
            >
              <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Memory</div>
              <div
                title={state.memoryPath}
                style={{
                  marginLeft: "auto",
                  font: `500 11px ${font.mono}`,
                  color: color.muted2,
                  maxWidth: "60%",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {draftPath !== null
                  ? "new file"
                  : state.memoryOpen
                    ? state.memoryOpen.stat.path
                    : state.memoryPath}
              </div>
            </header>

            <div style={{ flex: 1, minHeight: 0, overflowY: "auto", background: color.sidebar, padding: 22 }}>
              {draftPath !== null ? (
                <DraftPane
                  key={draftPath}
                  path={draftPath}
                  onPublish={publishDraft}
                  onCancel={() => setDraftPath(null)}
                />
              ) : state.memoryOpen ? (
                <OpenFilePane
                  key={`${state.memoryOpen.stat.path}@${state.memoryOpen.generation.generation}`}
                  open={state.memoryOpen}
                  op={state.ops[opKey.memory(state.memoryOpen.stat.path)]}
                  onPublish={(text) =>
                    actions.publishMemory({ path: state.memoryOpen!.stat.path, text })
                  }
                  onDelete={() => actions.deleteMemory(state.memoryOpen!.stat.path)}
                  onClose={actions.closeMemoryFile}
                />
              ) : (
                <CenterState
                  title="No file open"
                  detail="Select a file to read, or publish a new one."
                />
              )}
            </div>
          </main>
        </>
      )}
    </div>
  );
}
