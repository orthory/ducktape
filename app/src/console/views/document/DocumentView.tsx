// The docs surface over the node's `document` module: a block-based store where
// a document is exactly an ordered list of blocks keyed by doc_id.
//
// The module has no "list docs" query, so known doc ids come from a client-side
// registry the store keeps per node. This view keeps that limitation visible:
// the rail switches remembered docs, while explicit create/open forms let an
// operator add a known id without pretending the node can enumerate documents.

import { useEffect, useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";

import type { Block, BlockKind } from "../../../domain/document-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const KINDS: BlockKind[] = ["Paragraph", "Heading", "Code"];

const KIND_META: Record<
  BlockKind,
  { label: string; editLabel: string; text: string; bg: string; border: string }
> = {
  Paragraph: {
    label: "TEXT",
    editLabel: "paragraph",
    text: color.blue,
    bg: "#f1f4f8",
    border: "#d7e0eb",
  },
  Heading: {
    label: "HEAD",
    editLabel: "heading",
    text: color.purple,
    bg: "#f1edf5",
    border: "#ddd2e6",
  },
  Code: {
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

const monoId: CSSProperties = {
  font: `500 11px ${font.mono}`,
  color: color.muted3,
  wordBreak: "break-all",
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

// -- One editable block ------------------------------------------------------

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

  useEffect(() => setDraft(block.text), [block.text]);

  const commit = () => {
    if (draft !== block.text) onUpdate(draft);
  };

  const code = block.kind === "Code";
  const heading = block.kind === "Heading";
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

// -- Registry rail -----------------------------------------------------------

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
            local registry
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
          placeholder="release-notes"
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
          placeholder="existing-doc-id"
          value={openId}
          setValue={setOpenId}
          submitLabel="Open document"
          icon="chevronRight"
          onSubmit={onOpen}
        />
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "13px 8px" }}>
        <div style={{ padding: "0 8px 8px" }}>
          <SectionLabel>Known Documents</SectionLabel>
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
            No documents have been opened on this node yet.
          </div>
        ) : (
          docIds.map((id) => {
            const active = id === activeDoc;
            return (
              <button
                key={id}
                type="button"
                aria-label={`Open ${id}`}
                title={id}
                onClick={() => openDoc(id)}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  boxSizing: "border-box",
                  width: "100%",
                  display: "flex",
                  alignItems: "center",
                  gap: 9,
                  margin: "1px 0",
                  padding: "9px 10px",
                  borderRadius: radius.sm,
                  background: active ? color.hover : "transparent",
                  color: active ? color.ink : color.inkSofter,
                }}
              >
                <span
                  style={{
                    width: 24,
                    height: 24,
                    borderRadius: 7,
                    border: `1px solid ${active ? color.borderStrong : color.border}`,
                    background: active ? color.paper : color.sunken,
                    color: active ? accentVar : color.muted2,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    flexShrink: 0,
                  }}
                >
                  <Icon name="hash" size={12} strokeWidth={1.8} />
                </span>
                <span style={{ ...monoId, color: active ? color.ink : color.inkSofter }}>
                  {id}
                </span>
                {active ? (
                  <span
                    style={{
                      marginLeft: "auto",
                      font: `600 8.5px ${font.mono}`,
                      color: color.onDark,
                      background: color.dark,
                      borderRadius: 4,
                      padding: "2px 5px",
                      letterSpacing: ".05em",
                      flexShrink: 0,
                    }}
                  >
                    OPEN
                  </span>
                ) : null}
              </button>
            );
          })
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
        The node does not expose document discovery; opened ids are remembered locally.
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
              body="Create a document from the rail or open a known id to load its blocks."
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
