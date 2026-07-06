// The docs surface over the node's `document` module: a block-based store where
// a document is exactly an ordered list of blocks keyed by doc_id.
//
// Doc ids are "/"-delimited PATHS, and the module keeps a reserved enumeration
// index (ListDocs) so the console can DISCOVER every doc on the node — not just
// the ones this session happened to open. This view turns that enumeration into
// a filesystem-like reader: a LEFT collapsible folder/document tree derived from
// the path ids, and a MAIN reader/editor pane that keeps the block editing and
// create/open flows.

import { useEffect, useMemo, useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";

import type { Block, BlockKind } from "../../../domain/document-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const KINDS: BlockKind[] = ["paragraph", "heading", "code"];

const KIND_META: Record<
  BlockKind,
  { label: string; editLabel: string; text: string; bg: string; border: string }
> = {
  paragraph: {
    label: "TEXT",
    editLabel: "paragraph",
    text: color.blue,
    bg: "#f1f4f8",
    border: "#d7e0eb",
  },
  heading: {
    label: "HEAD",
    editLabel: "heading",
    text: color.purple,
    bg: "#f1edf5",
    border: "#ddd2e6",
  },
  code: {
    label: "CODE",
    editLabel: "code",
    text: color.green,
    bg: "#eef5f0",
    border: "#cfe3d7",
  },
};

const ACTIVE_PILL = {
  label: "OPEN",
  text: color.purple,
  bg: "#f1edf5",
  border: "#ddd2e6",
};

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

const srOnly: CSSProperties = {
  position: "absolute",
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: "hidden",
  clip: "rect(0, 0, 0, 0)",
  whiteSpace: "nowrap",
  border: 0,
};

function shortId(id: string): string {
  return id.length > 16 ? `${id.slice(0, 9)}...${id.slice(-5)}` : id;
}

function SectionLabel({ children }: { children: ReactNode }) {
  return <div style={sectionLabelStyle}>{children}</div>;
}

function FieldLabel({
  htmlFor,
  children,
  hidden = false,
}: {
  htmlFor: string;
  children: ReactNode;
  hidden?: boolean;
}) {
  return (
    <label htmlFor={htmlFor} style={hidden ? srOnly : sectionLabelStyle}>
      {children}
    </label>
  );
}

function StatusPill({
  label,
  text,
  bg,
  border,
}: {
  label: string;
  text: string;
  bg: string;
  border: string;
}) {
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        height: 20,
        padding: "0 7px",
        borderRadius: 5,
        border: `1px solid ${border}`,
        background: bg,
        font: `600 9px ${font.mono}`,
        letterSpacing: ".06em",
        color: text,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </span>
  );
}

function EmptyState({
  title,
  body,
  icon = "document",
}: {
  title: string;
  body: string;
  icon?: "document" | "hash";
}) {
  return (
    <div
      style={{
        minHeight: 240,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        textAlign: "center",
        color: color.muted2,
      }}
    >
      <div style={{ maxWidth: 330, padding: 22 }}>
        <div
          style={{
            width: 42,
            height: 42,
            margin: "0 auto 13px",
            borderRadius: radius.lg,
            border: `1px solid ${color.border}`,
            background: color.paper,
            color: color.muted2,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name={icon} size={18} />
        </div>
        <div style={{ font: `600 14px ${font.sans}`, color: color.ink }}>{title}</div>
        <div
          style={{
            marginTop: 5,
            font: `400 12px/1.5 ${font.sans}`,
            color: color.muted,
          }}
        >
          {body}
        </div>
      </div>
    </div>
  );
}

function IconButton({
  name,
  label,
  onClick,
  disabled = false,
  rotate = 0,
  danger = false,
}: {
  name: "chevronRight" | "close";
  label: string;
  onClick: () => void;
  disabled?: boolean;
  rotate?: number;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      style={{
        all: "unset",
        cursor: disabled ? "default" : "pointer",
        width: 27,
        height: 27,
        borderRadius: 7,
        border: disabled ? "1px solid transparent" : `1px solid ${color.border}`,
        background: disabled ? "transparent" : color.paper,
        color: disabled ? color.iconIdle : danger ? color.red : color.muted3,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <Icon name={name} size={13} style={{ transform: `rotate(${rotate}deg)` }} />
    </button>
  );
}

function RailSubmitButton({
  label,
  disabled,
  children,
}: {
  label: string;
  disabled: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="submit"
      aria-label={label}
      title={label}
      disabled={disabled}
      style={{
        all: "unset",
        cursor: disabled ? "default" : "pointer",
        flexShrink: 0,
        width: 32,
        height: 32,
        borderRadius: 8,
        background: disabled ? color.chip : color.dark,
        color: disabled ? color.muted2 : color.onDark,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {children}
    </button>
  );
}

function DocIdForm({
  id,
  label,
  placeholder,
  value,
  setValue,
  submitLabel,
  icon,
  onSubmit,
}: {
  id: string;
  label: string;
  placeholder: string;
  value: string;
  setValue: (value: string) => void;
  submitLabel: string;
  icon: "plus" | "chevronRight";
  onSubmit: (event: FormEvent) => void;
}) {
  return (
    <form onSubmit={onSubmit} style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <div style={{ display: "flex", gap: 7 }}>
        <input
          id={id}
          value={value}
          onChange={(event) => setValue(event.target.value)}
          placeholder={placeholder}
          spellCheck={false}
          autoCapitalize="none"
          style={inputStyle}
        />
        <RailSubmitButton label={submitLabel} disabled={!value.trim()}>
          <Icon name={icon} size={14} strokeWidth={1.9} />
        </RailSubmitButton>
      </div>
    </form>
  );
}

// -- Folder / document tree --------------------------------------------------
//
// Doc ids are "/"-delimited paths; the enumeration index is a flat id list.
// buildDocTree folds those ids into a nested tree so the rail can render a
// filesystem browser: intermediate segments become folders, and every id is a
// document leaf at its full path. A path can be BOTH a folder and a document
// (e.g. "projects" alongside "projects/launch") — such a node opens on click
// and still expands to reveal its children.

interface DocNode {
  /** Last path segment — the label shown in the tree. */
  name: string;
  /** Full "/"-delimited path — the doc id and the open target. */
  path: string;
  /** True when a document exists exactly at this path (not just a prefix). */
  isDoc: boolean;
  children: DocNode[];
}

function buildDocTree(docIds: string[]): DocNode[] {
  const roots: DocNode[] = [];
  const byPath = new Map<string, DocNode>();

  for (const id of docIds) {
    const segments = id.split("/").filter((segment) => segment.length > 0);
    let prefix = "";
    let siblings = roots;
    for (let i = 0; i < segments.length; i += 1) {
      const segment = segments[i];
      prefix = prefix ? `${prefix}/${segment}` : segment;
      let node = byPath.get(prefix);
      if (!node) {
        node = { name: segment, path: prefix, isDoc: false, children: [] };
        byPath.set(prefix, node);
        siblings.push(node);
      }
      if (i === segments.length - 1) node.isDoc = true;
      siblings = node.children;
    }
  }

  // Folders first, then documents, each group alphabetical — the usual file
  // browser ordering. Recurse so every level is sorted the same way.
  const sortLevel = (nodes: DocNode[]): void => {
    nodes.sort((a, b) => {
      const aFolder = a.children.length > 0;
      const bFolder = b.children.length > 0;
      if (aFolder !== bFolder) return aFolder ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    for (const node of nodes) sortLevel(node.children);
  };
  sortLevel(roots);

  return roots;
}

function TreeItem({
  node,
  depth,
  activeDoc,
  collapsed,
  onToggle,
  openDoc,
}: {
  node: DocNode;
  depth: number;
  activeDoc: string | null;
  collapsed: ReadonlySet<string>;
  onToggle: (path: string) => void;
  openDoc: (path: string) => void;
}) {
  const isFolder = node.children.length > 0;
  const expanded = !collapsed.has(node.path);
  const active = node.isDoc && node.path === activeDoc;
  const indent = 8 + depth * 13;

  return (
    <div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          margin: "1px 6px",
          paddingLeft: indent,
          borderRadius: radius.sm,
          background: active ? color.hover : "transparent",
          color: active ? color.ink : color.inkSofter,
        }}
      >
        {isFolder ? (
          <button
            type="button"
            aria-label={`${expanded ? "Collapse" : "Expand"} ${node.path}`}
            aria-expanded={expanded}
            title={node.path}
            onClick={() => onToggle(node.path)}
            style={{
              all: "unset",
              cursor: "pointer",
              flexShrink: 0,
              width: 18,
              height: 28,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: color.muted2,
            }}
          >
            <Icon
              name="chevronRight"
              size={12}
              strokeWidth={1.9}
              style={{ transform: `rotate(${expanded ? 90 : 0}deg)` }}
            />
          </button>
        ) : (
          <span aria-hidden="true" style={{ flexShrink: 0, width: 18 }} />
        )}

        <button
          type="button"
          aria-label={
            node.isDoc
              ? `Open ${node.path}`
              : `${expanded ? "Collapse" : "Expand"} folder ${node.path}`
          }
          aria-expanded={isFolder && !node.isDoc ? expanded : undefined}
          title={node.path}
          onClick={() => (node.isDoc ? openDoc(node.path) : onToggle(node.path))}
          style={{
            all: "unset",
            cursor: "pointer",
            flex: 1,
            minWidth: 0,
            display: "flex",
            alignItems: "center",
            gap: 8,
            padding: "6px 8px 6px 2px",
            color: "inherit",
          }}
        >
          <Icon
            name={isFolder ? "folder" : "document"}
            size={14}
            strokeWidth={1.7}
            style={{
              flexShrink: 0,
              color: active ? accentVar : color.muted2,
            }}
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
            {node.name}
          </span>
          {active ? (
            <span
              style={{
                flexShrink: 0,
                font: `600 8.5px ${font.mono}`,
                color: color.onDark,
                background: color.dark,
                borderRadius: 4,
                padding: "2px 5px",
                letterSpacing: ".05em",
              }}
            >
              OPEN
            </span>
          ) : null}
        </button>
      </div>

      {isFolder && expanded ? (
        <div>
          {node.children.map((child) => (
            <TreeItem
              key={child.path}
              node={child}
              depth={depth + 1}
              activeDoc={activeDoc}
              collapsed={collapsed}
              onToggle={onToggle}
              openDoc={openDoc}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

// -- One editable block ------------------------------------------------------

function BlockRow({
  block,
  index,
  total,
  op,
  onUpdate,
  onRemove,
  onMoveUp,
  onMoveDown,
}: {
  block: Block;
  index: number;
  total: number;
  /** The block's finalization record — the id footer draws the inline mark. */
  op: OpRecord | undefined;
  onUpdate: (text: string) => void;
  onRemove: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}) {
  const [draft, setDraft] = useState(block.text);

  useEffect(() => setDraft(block.text), [block.text]);

  const commit = () => {
    if (draft !== block.text) onUpdate(draft);
  };

  const code = block.kind === "code";
  const heading = block.kind === "heading";
  const meta = KIND_META[block.kind];
  const blockNumber = index + 1;

  return (
    <article
      style={{
        display: "grid",
        gridTemplateColumns: "74px minmax(0, 1fr) 34px",
        gap: 15,
        padding: "22px 0",
        borderBottom: `1px solid ${color.borderSoft}`,
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 8, alignItems: "flex-start" }}>
        <StatusPill label={meta.label} text={meta.text} bg={meta.bg} border={meta.border} />
        <span style={{ font: `500 10px ${font.mono}`, color: color.muted2 }}>
          {String(blockNumber).padStart(2, "0")}
        </span>
      </div>

      <div style={{ minWidth: 0 }}>
        <textarea
          aria-label={`Edit ${meta.editLabel} block ${blockNumber}`}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={commit}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
              event.preventDefault();
              commit();
            }
          }}
          rows={code ? 7 : heading ? 2 : 4}
          placeholder={heading ? "Untitled heading" : code ? "code block" : "write a paragraph"}
          spellCheck={!code}
          style={{
            width: "100%",
            boxSizing: "border-box",
            minHeight: heading ? 58 : code ? 158 : 102,
            resize: "vertical",
            border: code ? `1px solid ${color.border}` : "none",
            borderRadius: code ? radius.md : 0,
            outline: "none",
            background: code ? color.sunken : "transparent",
            color: color.ink,
            padding: code ? 13 : "1px 0",
            font: heading
              ? `650 25px/1.2 ${font.sans}`
              : code
                ? `400 12.5px/1.55 ${font.mono}`
                : `400 14.5px/1.62 ${font.sans}`,
          }}
        />
        <div
          title={block.id}
          style={{
            marginTop: 7,
            display: "flex",
            alignItems: "center",
            gap: 6,
            color: color.muted2,
          }}
        >
          <Icon name="hash" size={11} />
          <span style={{ font: `500 10px ${font.mono}` }}>{shortId(block.id)}</span>
          <FinalizationMark op={op} />
        </div>
      </div>

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 6,
          alignItems: "flex-end",
        }}
      >
        <IconButton
          name="chevronRight"
          label={`Move block ${blockNumber} up`}
          disabled={index === 0}
          rotate={-90}
          onClick={onMoveUp}
        />
        <IconButton
          name="chevronRight"
          label={`Move block ${blockNumber} down`}
          disabled={index === total - 1}
          rotate={90}
          onClick={onMoveDown}
        />
        <IconButton
          name="close"
          label={`Remove block ${blockNumber}`}
          danger
          onClick={onRemove}
        />
      </div>
    </article>
  );
}

// -- Insert composer ---------------------------------------------------------

function AddBlock({ onAdd }: { onAdd: (kind: BlockKind, text: string) => void }) {
  const [kind, setKind] = useState<BlockKind>("paragraph");
  const [text, setText] = useState("");
  const code = kind === "code";

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onAdd(kind, text);
    setText("");
  };

  return (
    <form
      onSubmit={submit}
      style={{
        marginTop: 18,
        border: `1px dashed ${color.borderStrong}`,
        borderRadius: radius.lg,
        background: color.sidebar,
        padding: 15,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 9, minWidth: 0 }}>
        <SectionLabel>Insert Block</SectionLabel>
        <div
          role="group"
          aria-label="Block kind"
          style={{
            display: "flex",
            gap: 4,
            padding: 3,
            borderRadius: radius.sm,
            background: color.sunken,
            marginLeft: "auto",
          }}
        >
          {KINDS.map((option) => {
            const active = option === kind;
            return (
              <button
                key={option}
                type="button"
                aria-label={`Insert ${option} block`}
                aria-pressed={active}
                onClick={() => setKind(option)}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  padding: "5px 10px",
                  borderRadius: 6,
                  background: active ? color.paper : "transparent",
                  color: active ? color.ink : color.muted2,
                  font: `600 11px ${font.sans}`,
                }}
              >
                {option}
              </button>
            );
          })}
        </div>
      </div>

      <FieldLabel htmlFor="document-new-block-text" hidden>
        New block text
      </FieldLabel>
      <textarea
        id="document-new-block-text"
        value={text}
        onChange={(event) => setText(event.target.value)}
        placeholder={`New ${KIND_META[kind].editLabel} block`}
        rows={code ? 5 : 3}
        spellCheck={!code}
        style={{
          ...inputStyle,
          marginTop: 12,
          minHeight: code ? 118 : 76,
          resize: "vertical",
          font: code ? `400 12.5px/1.5 ${font.mono}` : `400 13px/1.5 ${font.sans}`,
        }}
      />
      <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 10 }}>
        <button
          type="submit"
          style={{
            all: "unset",
            cursor: "pointer",
            display: "inline-flex",
            alignItems: "center",
            gap: 7,
            borderRadius: 8,
            background: accentVar,
            color: "#fff",
            padding: "8px 13px",
            font: `600 12px ${font.sans}`,
          }}
        >
          <Icon name="plus" size={13} strokeWidth={1.9} />
          Insert block
        </button>
      </div>
    </form>
  );
}

// -- Enumerated document rail (folder/document tree) -------------------------

function DocRail({
  docIds,
  activeDoc,
  openId,
  newId,
  setOpenId,
  setNewId,
  onOpen,
  onCreate,
  onRefresh,
  openDoc,
}: {
  docIds: string[];
  activeDoc: string | null;
  openId: string;
  newId: string;
  setOpenId: (id: string) => void;
  setNewId: (id: string) => void;
  onOpen: (event: FormEvent) => void;
  onCreate: (event: FormEvent) => void;
  onRefresh: () => void;
  openDoc: (id: string) => void;
}) {
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const tree = useMemo(() => buildDocTree(docIds), [docIds]);

  const toggle = (path: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });

  return (
    <aside
      style={{
        width: 292,
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
          <Icon name="document" size={14} strokeWidth={1.7} />
        </span>
        <div style={{ minWidth: 0 }}>
          <div style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>Documents</div>
          <div style={{ marginTop: 1, font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
            node index
          </div>
        </div>
        <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
          {docIds.length}
        </div>
      </div>

      <div style={{ padding: "14px", borderBottom: `1px solid ${color.borderSoft}` }}>
        <DocIdForm
          id="document-create-id"
          label="Create document id"
          placeholder="projects/release-notes"
          value={newId}
          setValue={setNewId}
          submitLabel="Create document"
          icon="plus"
          onSubmit={onCreate}
        />
      </div>

      <div style={{ padding: "14px", borderBottom: `1px solid ${color.borderSoft}` }}>
        <DocIdForm
          id="document-open-id"
          label="Open document id"
          placeholder="existing/doc-id"
          value={openId}
          setValue={setOpenId}
          submitLabel="Open document"
          icon="chevronRight"
          onSubmit={onOpen}
        />
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "13px 0" }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            padding: "0 14px 8px",
          }}
        >
          <SectionLabel>Files</SectionLabel>
          <button
            type="button"
            aria-label="Refresh documents"
            title="Refresh documents"
            onClick={onRefresh}
            style={{
              all: "unset",
              cursor: "pointer",
              marginLeft: "auto",
              width: 24,
              height: 24,
              borderRadius: 6,
              border: `1px solid ${color.border}`,
              background: color.paper,
              color: color.muted3,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Icon name="refresh" size={13} strokeWidth={1.7} />
          </button>
        </div>

        {tree.length === 0 ? (
          <div
            style={{
              margin: "7px 14px",
              padding: "13px 12px",
              border: `1px dashed ${color.borderStrong}`,
              borderRadius: radius.md,
              background: color.paper,
              font: `400 12px/1.45 ${font.sans}`,
              color: color.muted2,
            }}
          >
            No documents on this node yet. Create one above to start the tree.
          </div>
        ) : (
          tree.map((node) => (
            <TreeItem
              key={node.path}
              node={node}
              depth={0}
              activeDoc={activeDoc}
              collapsed={collapsed}
              onToggle={toggle}
              openDoc={openDoc}
            />
          ))
        )}
      </div>

      <div
        style={{
          padding: "12px 14px 14px",
          borderTop: `1px solid ${color.borderSoft}`,
          font: `400 11.5px/1.45 ${font.sans}`,
          color: color.muted2,
        }}
      >
        Enumerated from the node&apos;s document index. Slashes in an id become folders.
      </div>
    </aside>
  );
}

// -- The view ----------------------------------------------------------------

export function DocumentView() {
  const { state, actions } = useDucktape();
  const [openId, setOpenId] = useState("");
  const [newId, setNewId] = useState("");

  const blocks = state.activeDocBlocks;

  // Enumerate the node's document index on mount so the tree reflects every
  // doc, not just ones opened this session. `actions` is a stable facade, so
  // this fires once; the rail's refresh control re-runs it on demand, and every
  // committed block event re-enumerates through the store's refresh.
  useEffect(() => {
    actions.listDocs();
  }, [actions]);

  const open = (event: FormEvent) => {
    event.preventDefault();
    if (!openId.trim()) return;
    actions.openDoc(openId);
    setOpenId("");
  };

  const create = (event: FormEvent) => {
    event.preventDefault();
    if (!newId.trim()) return;
    actions.createDoc(newId);
    setNewId("");
  };

  const addBlock = (kind: BlockKind, text: string) => {
    const after = blocks.length > 0 ? blocks[blocks.length - 1].id : null;
    actions.insertBlock({ after, kind, text });
  };

  const moveUp = (index: number) => {
    const after = index >= 2 ? blocks[index - 2].id : null;
    actions.moveBlock({ blockId: blocks[index].id, after });
  };

  const moveDown = (index: number) => {
    const after = blocks[index + 1].id;
    actions.moveBlock({ blockId: blocks[index].id, after });
  };

  return (
    <div
      data-screen-label="Documents"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        background: color.paper,
      }}
    >
      <DocRail
        docIds={state.docIds}
        activeDoc={state.activeDoc}
        openId={openId}
        newId={newId}
        setOpenId={setOpenId}
        setNewId={setNewId}
        onOpen={open}
        onCreate={create}
        onRefresh={actions.listDocs}
        openDoc={actions.openDoc}
      />

      <main style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}>
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
          <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Documents</div>
          {state.activeDoc ? (
            <>
              <StatusPill
                label={ACTIVE_PILL.label}
                text={ACTIVE_PILL.text}
                bg={ACTIVE_PILL.bg}
                border={ACTIVE_PILL.border}
              />
              <div
                title={state.activeDoc}
                style={{
                  marginLeft: 2,
                  minWidth: 0,
                  font: `500 12px ${font.mono}`,
                  color: color.muted2,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {state.activeDoc}
              </div>
              <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
                {blocks.length} {blocks.length === 1 ? "block" : "blocks"}
              </div>
            </>
          ) : (
            <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
              no active doc
            </div>
          )}
        </header>

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            background: color.sidebar,
            padding: state.activeDoc ? "22px 26px" : 0,
          }}
        >
          {!state.activeDoc ? (
            <EmptyState
              title="No document open"
              body="Pick a document from the tree, or create one to load its blocks."
            />
          ) : (
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
              <div
                style={{
                  padding: "28px 34px 24px",
                  borderBottom: blocks.length > 0 ? `1px solid ${color.borderSoft}` : undefined,
                }}
              >
                <div style={{ display: "flex", alignItems: "flex-start", gap: 14 }}>
                  <div
                    style={{
                      width: 38,
                      height: 38,
                      borderRadius: radius.lg,
                      background: color.dark,
                      color: color.onDark,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      flexShrink: 0,
                    }}
                  >
                    <Icon name="document" size={17} />
                  </div>
                  <div style={{ minWidth: 0 }}>
                    <div
                      style={{
                        font: `650 28px/1.16 ${font.sans}`,
                        color: color.dark,
                        letterSpacing: 0,
                        overflowWrap: "anywhere",
                      }}
                    >
                      {state.activeDoc}
                    </div>
                    <div
                      style={{
                        marginTop: 6,
                        display: "flex",
                        alignItems: "center",
                        gap: 8,
                        flexWrap: "wrap",
                      }}
                    >
                      <span style={{ font: `400 12.5px ${font.sans}`, color: color.muted }}>
                        Ordered block document
                      </span>
                      <span style={{ width: 3, height: 3, borderRadius: "50%", background: color.chip }} />
                      <span style={{ font: `500 11px ${font.mono}`, color: color.muted2 }}>
                        {blocks.length} {blocks.length === 1 ? "block" : "blocks"}
                      </span>
                    </div>
                  </div>
                </div>
              </div>

              <div style={{ padding: "0 34px 34px" }}>
                {blocks.length === 0 ? (
                  <EmptyState
                    title="This document is empty"
                    body="Insert a heading, paragraph, or code block to start shaping the page."
                    icon="hash"
                  />
                ) : (
                  <div>
                    {blocks.map((block, index) => (
                      <BlockRow
                        key={block.id}
                        block={block}
                        index={index}
                        total={blocks.length}
                        op={
                          state.activeDoc
                            ? state.ops[opKey.docBlock(state.activeDoc, block.id)]
                            : undefined
                        }
                        onUpdate={(text) => actions.updateBlock({ blockId: block.id, text })}
                        onRemove={() => actions.removeBlock(block.id)}
                        onMoveUp={() => moveUp(index)}
                        onMoveDown={() => moveDown(index)}
                      />
                    ))}
                  </div>
                )}
                <AddBlock onAdd={addBlock} />
              </div>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
