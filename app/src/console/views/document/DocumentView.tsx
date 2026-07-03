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
import type { FormEvent } from "react";

import type { Block, BlockKind } from "../../../domain/document-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const KINDS: BlockKind[] = ["Paragraph", "Heading", "Code"];

const KIND_TINT: Record<BlockKind, string> = {
  Paragraph: color.blue,
  Heading: color.purple,
  Code: color.green,
};

const fieldStyle = {
  padding: "6px 9px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12px ${font.sans}`,
  color: color.ink,
  width: "100%",
} as const;

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

  return (
    <div
      style={{
        display: "flex",
        gap: 9,
        padding: 11,
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
        boxShadow: shadow.card,
        animation: "ik-fade .16s ease-out",
      }}
    >
      <span
        title={block.kind}
        style={{
          flexShrink: 0,
          height: "fit-content",
          padding: "2px 7px",
          borderRadius: radius.sm,
          background: color.sunken,
          border: `1px solid ${color.border}`,
          font: `600 9.5px ${font.mono}`,
          letterSpacing: ".04em",
          color: KIND_TINT[block.kind],
        }}
      >
        {block.kind.slice(0, 4).toUpperCase()}
      </span>

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
        rows={code ? 4 : 2}
        placeholder="empty block"
        style={{
          flex: 1,
          minWidth: 0,
          resize: "vertical",
          border: "none",
          outline: "none",
          background: "transparent",
          color: color.ink,
          font: heading
            ? `600 14px ${font.sans}`
            : code
              ? `400 12px ${font.mono}`
              : `400 12.5px ${font.sans}`,
        }}
      />

      <div style={{ flexShrink: 0, display: "flex", flexDirection: "column", gap: 3 }}>
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
        <IconButton name="close" title="Delete block" onClick={onRemove} />
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
}: {
  name: "chevronRight" | "close";
  title: string;
  onClick: () => void;
  disabled?: boolean;
  rotate?: number;
}) {
  return (
    <button
      title={title}
      disabled={disabled}
      onClick={onClick}
      style={{
        all: "unset",
        cursor: disabled ? "default" : "pointer",
        width: 20,
        height: 20,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        borderRadius: 5,
        color: disabled ? color.iconIdle : color.muted2,
      }}
    >
      <Icon name={name} size={13} style={{ transform: `rotate(${rotate}deg)` }} />
    </button>
  );
}

// ── Append composer ─────────────────────────────────────

function AddBlock({ onAdd }: { onAdd: (kind: BlockKind, text: string) => void }) {
  const [kind, setKind] = useState<BlockKind>("Paragraph");
  const [text, setText] = useState("");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onAdd(kind, text);
    setText("");
  };

  return (
    <form
      onSubmit={submit}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: 11,
        borderRadius: radius.md,
        border: `1px dashed ${color.borderStrong}`,
        background: color.sunken,
      }}
    >
      <div style={{ display: "flex", gap: 5 }}>
        {KINDS.map((option) => {
          const on = option === kind;
          return (
            <button
              key={option}
              type="button"
              onClick={() => setKind(option)}
              style={{
                all: "unset",
                cursor: "pointer",
                padding: "3px 9px",
                borderRadius: radius.sm,
                border: `1px solid ${on ? accentVar : color.border}`,
                background: on ? accentVar : color.paper,
                color: on ? "#fff" : color.muted3,
                font: `600 10.5px ${font.sans}`,
              }}
            >
              {option}
            </button>
          );
        })}
      </div>
      <div style={{ display: "flex", gap: 7 }}>
        <input
          value={text}
          onChange={(event) => setText(event.target.value)}
          placeholder={`New ${kind.toLowerCase()} block`}
          style={fieldStyle}
        />
        <button
          type="submit"
          title="Add block"
          style={{
            all: "unset",
            cursor: "pointer",
            flexShrink: 0,
            width: 28,
            height: 28,
            borderRadius: 7,
            background: accentVar,
            color: "#fff",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="plus" size={14} />
        </button>
      </div>
    </form>
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
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 7,
          padding: "11px 17px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <Icon name="document" size={15} color={color.muted} />
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>Docs</span>
        {state.activeDoc && (
          <span
            title={state.activeDoc}
            style={{
              marginLeft: 4,
              padding: "2px 8px",
              borderRadius: radius.sm,
              background: color.chip,
              font: `500 11px ${font.mono}`,
              color: color.muted3,
            }}
          >
            {state.activeDoc}
          </span>
        )}
      </div>

      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        {/* ── Registry rail: known doc-ids + open / new affordances ── */}
        <div
          style={{
            width: 220,
            flexShrink: 0,
            display: "flex",
            flexDirection: "column",
            gap: 10,
            padding: 13,
            borderRight: `1px solid ${color.borderSoft}`,
            background: color.sidebar,
            overflowY: "auto",
          }}
        >
          <span
            style={{
              font: `600 10px ${font.sans}`,
              color: color.muted,
              letterSpacing: ".06em",
            }}
          >
            DOCUMENTS
          </span>
          <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
            {state.docIds.length === 0 ? (
              <span style={{ font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
                No known docs yet — create one or open by id.
              </span>
            ) : (
              state.docIds.map((id) => {
                const on = id === state.activeDoc;
                return (
                  <button
                    key={id}
                    onClick={() => actions.openDoc(id)}
                    style={{
                      all: "unset",
                      cursor: "pointer",
                      padding: "6px 9px",
                      borderRadius: radius.sm,
                      background: on ? color.hover : "transparent",
                      font: `${on ? 600 : 400} 12px ${font.mono}`,
                      color: on ? color.ink : color.inkSofter,
                      wordBreak: "break-all",
                    }}
                  >
                    {id}
                  </button>
                );
              })
            )}
          </div>

          <form onSubmit={open} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <input
              value={openId}
              onChange={(event) => setOpenId(event.target.value)}
              placeholder="open by id"
              style={fieldStyle}
            />
          </form>

          <form onSubmit={create} style={{ display: "flex", gap: 6 }}>
            <input
              value={newId}
              onChange={(event) => setNewId(event.target.value)}
              placeholder="new doc id"
              style={fieldStyle}
            />
            <button
              type="submit"
              title="Create doc"
              style={{
                all: "unset",
                cursor: "pointer",
                flexShrink: 0,
                width: 28,
                height: 28,
                borderRadius: 7,
                background: newId.trim() ? accentVar : color.chip,
                color: "#fff",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Icon name="plus" size={14} />
            </button>
          </form>
        </div>

        {/* ── Block editor for the open doc ── */}
        <div style={{ flex: 1, minWidth: 0, overflowY: "auto", padding: 17 }}>
          {!state.activeDoc ? (
            <div
              style={{
                height: "100%",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                textAlign: "center",
                font: `400 12.5px ${font.sans}`,
                color: color.muted2,
              }}
            >
              Open a document or create one to start editing its blocks.
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {blocks.length === 0 && (
                <span
                  style={{
                    font: `400 12px ${font.sans}`,
                    color: color.muted2,
                    fontStyle: "italic",
                  }}
                >
                  No blocks yet — add one below.
                </span>
              )}
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
              <AddBlock onAdd={addBlock} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
