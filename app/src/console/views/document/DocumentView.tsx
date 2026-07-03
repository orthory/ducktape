// The docs surface over the node's `document` module: a block-based store where
// a document IS exactly an ordered list of blocks keyed by doc_id.
//
// The module has no "list docs" query, so known doc-ids come from a client-side
// registry the store keeps per node (see DucktapeProvider). This view offers an
// "open by id" input and a "new doc" button to grow that registry, then edits
// the open doc block-by-block: inline text edits (UpdateBlock), append with a
// kind selector (InsertBlock), reorder (MoveBlock), and delete (RemoveBlock).
// Kind is fixed at insert time — the module has no change-kind op. No optimistic
// state: every write goes through the store's submit-then-refresh.

import { useEffect, useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";

import type { Block, BlockKind } from "../../../domain/document-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius } from "../../theme/tokens";

const KINDS: BlockKind[] = ["Paragraph", "Heading", "Code"];

const KIND_PILL: Record<
  BlockKind,
  { label: string; text: string; bg: string; border: string }
> = {
  Paragraph: { label: "TEXT", text: color.blue, bg: "#f1f4f8", border: "#d7e0eb" },
  Heading: { label: "HEAD", text: color.purple, bg: "#f1edf5", border: "#ddd2e6" },
  Code: { label: "CODE", text: color.green, bg: "#eef5f0", border: "#cfe3d7" },
};

const ACTIVE_PILL = {
  label: "OPEN",
  text: color.purple,
  bg: "#f1edf5",
  border: "#ddd2e6",
};

const fieldStyle: CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "8px 10px",
  borderRadius: radius.sm,
  border: `1px solid ${color.border}`,
  background: color.paper,
  font: `400 12px ${font.sans}`,
  color: color.ink,
  outline: "none",
};

const monoId: CSSProperties = {
  font: `500 11px ${font.mono}`,
  color: color.muted3,
  wordBreak: "break-all",
};

function shortId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 6)}…${id.slice(-4)}` : id;
}

function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        font: `600 9px ${font.mono}`,
        letterSpacing: ".12em",
        color: color.muted2,
      }}
    >
      {children}
    </div>
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
        minHeight: 280,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        textAlign: "center",
        color: color.muted2,
      }}
    >
      <div style={{ maxWidth: 310, padding: 22 }}>
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
  title,
  onClick,
  disabled = false,
  rotate = 0,
  danger = false,
}: {
  name: "chevronRight" | "close";
  title: string;
  onClick: () => void;
  disabled?: boolean;
  rotate?: number;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      style={{
        all: "unset",
        cursor: disabled ? "default" : "pointer",
        width: 26,
        height: 26,
        borderRadius: 7,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: disabled ? color.iconIdle : danger ? color.red : color.muted3,
        background: disabled ? "transparent" : color.sunken,
      }}
    >
      <Icon name={name} size={13} style={{ transform: `rotate(${rotate}deg)` }} />
    </button>
  );
}

function RailSubmitButton({
  title,
  disabled,
  children,
}: {
  title: string;
  disabled: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="submit"
      title={title}
      style={{
        all: "unset",
        cursor: "pointer",
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

// ── One editable block ──────────────────────────────────

function BlockRow({
  block,
  index,
  total,
  onUpdate,
  onRemove,
  onMoveUp,
  onMoveDown,
}: {
  block: Block;
  index: number;
  total: number;
  onUpdate: (text: string) => void;
  onRemove: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}) {
  const [draft, setDraft] = useState(block.text);
  // Re-sync when the committed text changes under us (a refresh landed) — there
  // is no optimistic state, so the queried block is the source of truth.
  useEffect(() => setDraft(block.text), [block.text]);

  const commit = () => {
    if (draft !== block.text) onUpdate(draft);
  };

  const code = block.kind === "Code";
  const heading = block.kind === "Heading";
  const pill = KIND_PILL[block.kind];

  return (
    <article
      style={{
        display: "grid",
        gridTemplateColumns: "82px minmax(0, 1fr) 32px",
        gap: 14,
        padding: "21px 0",
        borderBottom: `1px solid ${color.borderSoft}`,
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
        <StatusPill label={pill.label} text={pill.text} bg={pill.bg} border={pill.border} />
        <span style={{ font: `500 10px ${font.mono}`, color: color.muted2 }}>
          {String(index + 1).padStart(2, "0")}
        </span>
      </div>

      <div style={{ minWidth: 0 }}>
        <textarea
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={commit}
          onKeyDown={(event) => {
            // Cmd/Ctrl+Enter commits without leaving the block; plain Enter keeps
            // adding lines (paragraphs and code are multi-line).
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
              event.preventDefault();
              commit();
            }
          }}
          rows={code ? 7 : heading ? 2 : 4}
          placeholder="empty block"
          style={{
            width: "100%",
            boxSizing: "border-box",
            minHeight: heading ? 56 : code ? 160 : 104,
            resize: "vertical",
            border: code ? `1px solid ${color.border}` : "none",
            borderRadius: code ? radius.md : 0,
            outline: "none",
            background: code ? color.sunken : "transparent",
            color: color.ink,
            padding: code ? 13 : 0,
            font: heading
              ? `650 24px/1.22 ${font.sans}`
              : code
                ? `400 12.5px/1.55 ${font.mono}`
                : `400 14px/1.62 ${font.sans}`,
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
        </div>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 6, alignItems: "flex-end" }}>
        <IconButton
          name="chevronRight"
          title="Move up"
          disabled={index === 0}
          rotate={-90}
          onClick={onMoveUp}
        />
        <IconButton
          name="chevronRight"
          title="Move down"
          disabled={index === total - 1}
          rotate={90}
          onClick={onMoveDown}
        />
        <IconButton name="close" title="Delete block" danger onClick={onRemove} />
      </div>
    </article>
  );
}

// ── Append composer ─────────────────────────────────────

function AddBlock({ onAdd }: { onAdd: (kind: BlockKind, text: string) => void }) {
  const [kind, setKind] = useState<BlockKind>("Paragraph");
  const [text, setText] = useState("");
  const code = kind === "Code";

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
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionLabel>INSERT</SectionLabel>
        <div
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
      <textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        placeholder={`New ${kind.toLowerCase()} block`}
        rows={code ? 5 : 3}
        style={{
          ...fieldStyle,
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
          <Icon name="plus" size={13} />
          Insert block
        </button>
      </div>
    </form>
  );
}

// ── Registry rail ──────────────────────────────────────

function DocRail({
  docIds,
  activeDoc,
  openId,
  newId,
  setOpenId,
  setNewId,
  onOpen,
  onCreate,
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
  openDoc: (id: string) => void;
}) {
  return (
    <aside
      style={{
        width: 266,
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
        <Icon name="document" size={15} color={color.muted} />
        <div style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>Documents</div>
        <div style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}>
          {docIds.length}
        </div>
      </div>

      <div style={{ padding: "14px 14px 10px", borderBottom: `1px solid ${color.borderSoft}` }}>
        <SectionLabel>NEW DOCUMENT</SectionLabel>
        <form onSubmit={onCreate} style={{ marginTop: 9, display: "flex", gap: 7 }}>
          <input
            value={newId}
            onChange={(event) => setNewId(event.target.value)}
            placeholder="new doc id"
            style={{ ...fieldStyle, font: `400 12px ${font.mono}` }}
          />
          <RailSubmitButton title="Create doc" disabled={!newId.trim()}>
            <Icon name="plus" size={14} />
          </RailSubmitButton>
        </form>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "13px 8px" }}>
        <div style={{ padding: "0 8px 8px" }}>
          <SectionLabel>KNOWN DOCS</SectionLabel>
        </div>
        {docIds.length === 0 ? (
          <div
            style={{
              margin: "7px 8px",
              padding: "13px 12px",
              border: `1px dashed ${color.borderStrong}`,
              borderRadius: radius.md,
              background: color.paper,
              font: `400 12px/1.45 ${font.sans}`,
              color: color.muted2,
            }}
          >
            No known docs yet. Create one or open an existing id.
          </div>
        ) : (
          docIds.map((id) => {
            const active = id === activeDoc;
            return (
              <button
                key={id}
                type="button"
                onClick={() => openDoc(id)}
                title={id}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  boxSizing: "border-box",
                  width: "100%",
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  margin: "1px 0",
                  padding: "9px 10px",
                  borderRadius: radius.sm,
                  background: active ? color.hover : "transparent",
                  color: active ? color.ink : color.inkSofter,
                }}
              >
                <Icon name="hash" size={12} color={active ? accentVar : color.muted2} />
                <span style={{ ...monoId, color: active ? color.ink : color.inkSofter }}>
                  {id}
                </span>
              </button>
            );
          })
        )}
      </div>

      <div style={{ padding: 14, borderTop: `1px solid ${color.borderSoft}` }}>
        <SectionLabel>OPEN BY ID</SectionLabel>
        <form onSubmit={onOpen} style={{ marginTop: 9, display: "flex", gap: 7 }}>
          <input
            value={openId}
            onChange={(event) => setOpenId(event.target.value)}
            placeholder="existing doc id"
            style={{ ...fieldStyle, font: `400 12px ${font.mono}` }}
          />
          <RailSubmitButton title="Open doc" disabled={!openId.trim()}>
            <Icon name="chevronRight" size={14} />
          </RailSubmitButton>
        </form>
      </div>
    </aside>
  );
}

// ── The view ────────────────────────────────────────────

export function DocumentView() {
  const { state, actions } = useDucktape();
  const [openId, setOpenId] = useState("");
  const [newId, setNewId] = useState("");

  const blocks = state.activeDocBlocks;

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
    // Append: after the last block, or null (front) when the doc is empty.
    const after = blocks.length > 0 ? blocks[blocks.length - 1].id : null;
    actions.insertBlock({ after, kind, text });
  };

  const moveUp = (index: number) => {
    // Land after the block two positions up, or null (front) from index 1.
    const after = index >= 2 ? blocks[index - 2].id : null;
    actions.moveBlock({ blockId: blocks[index].id, after });
  };

  const moveDown = (index: number) => {
    // Land after the block currently one position below.
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
          <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Docs</div>
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

        <div style={{ flex: 1, minHeight: 0, overflowY: "auto", background: color.paper }}>
          {!state.activeDoc ? (
            <EmptyState
              title="No document open"
              body="Create a document from the rail or open a known id to load its block list."
            />
          ) : (
            <div
              style={{
                maxWidth: 820,
                margin: "0 auto",
                padding: "34px 38px 46px",
              }}
            >
              <div
                style={{
                  paddingBottom: 20,
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
                        {blocks.length} blocks
                      </span>
                    </div>
                  </div>
                </div>
              </div>

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
          )}
        </div>
      </main>
    </div>
  );
}
