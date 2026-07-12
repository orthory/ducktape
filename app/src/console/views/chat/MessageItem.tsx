// A single row in the message stream: Slack renders every message identically
// — avatar + compacted header + hover action bar + reactions + inline thread
// indicator, left-aligned in one column. There is no right-aligned "mine"
// bubble and no color-coding by author; only consecutive-same-author
// grouping (via `groupStart`) distinguishes rows.

import { useContext, useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";

import { authorName } from "../../../domain/chat-client";
import type { AuthorNames, AuthorRef, MessageView } from "../../../domain/chat-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { ConsoleContext } from "../../store/context";
import type { OpRecord } from "../../store/finalization";
import { AskAgentButton } from "./AskAgentButton";
import { wallClockMillisOf } from "../../../domain/wire";
import { authorKey, hasReacted, isAgentAuthor } from "./chat-helpers";
import { blocksToInput } from "./chat-input";
import { EmojiPicker } from "./EmojiPicker";
import { HoverButton } from "./HoverButton";
import { RichText } from "./rich-text";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const QUICK_REACTS = ["👍", "✅", "👀"];

// The message content (avatar + body + hover affordances) is capped to a
// readable measure and left-aligned, while the row's hover highlight spans the
// full pane. This keeps long lines legible on a wide pane AND anchors the hover
// bar / menu / picker to the content's right edge instead of the far pane edge.
// 880 is the width the console already used for the column — the earlier hover
// complaint was the highlight stopping short of the edge, not the text measure.
const CONTENT_MAX = 880;

// `created_at` is the node's consensus_time — millis, legacy seconds, or a
// height counter depending on the lane (see domain/wire.ts). Empty when the
// stamp isn't real wall-clock — better to show no time than a fake "09:21 AM".
const timeOf = (createdAt: number): string => {
  const ms = wallClockMillisOf(createdAt);
  return ms === null ? "" : new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
};

// ── Tiny local glyphs (Icon.tsx isn't ours to extend in this task) ─────

function ThreadGlyph({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 7a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-6l-4 3.5V14H7a2 2 0 0 1-2-2z" />
    </svg>
  );
}

function MoreGlyph({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
      <circle cx="5" cy="12" r="1.7" />
      <circle cx="12" cy="12" r="1.7" />
      <circle cx="19" cy="12" r="1.7" />
    </svg>
  );
}

function LinkGlyph({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <path d="M9.5 14.5l5-5" />
      <path d="M11 8l1.3-1.3a3 3 0 0 1 4.3 4.3L15 12.5" />
      <path d="M13 16l-1.3 1.3a3 3 0 0 1-4.3-4.3L9 11.5" />
    </svg>
  );
}

function RefGlyph({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <path d="M8 4.5h8a2 2 0 0 1 2 2v13l-6-3-6 3v-13a2 2 0 0 1 2-2z" />
    </svg>
  );
}

function ExplorerGlyph({ size = 13 }: { size?: number }) {
  // A block/cube — "open this message's block in the explorer".
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3.4l7 4v9l-7 4-7-4v-9z" />
      <path d="M12 3.4v17M5 7.4l7 4 7-4" />
    </svg>
  );
}

function PencilGlyph({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 20h4l10.5-10.5a2 2 0 0 0-2.8-2.8L5 17v3z" />
      <path d="M13.5 6.5l4 4" />
    </svg>
  );
}

function TrashGlyph({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 7h14M10 7V5.5a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1V7M6.5 7l.8 11a1.5 1.5 0 0 0 1.5 1.4h6.4a1.5 1.5 0 0 0 1.5-1.4l.8-11" />
    </svg>
  );
}

function AddReactGlyph({ size = 15 }: { size?: number }) {
  // A smiley (open at the top-right for a "+") — the standard "add reaction"
  // mark. Eyes are drawn as short thick strokes so they read at ~13-15px.
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <path d="M20.4 11.3a8.5 8.5 0 1 1-7.7-7.7" />
      <path d="M8.8 10.3v.6M14 10.3v.6" strokeWidth={2.1} />
      <path d="M8.7 14.3a3.4 3.4 0 0 0 5.6 0" />
      <path d="M19 2.8v4.6M16.7 5.1h4.6" />
    </svg>
  );
}

// ── Avatar ───────────────────────────────────────────────

function Avatar({ author, name, size }: { author: AuthorRef; name: string; size: number }) {
  const agent = isAgentAuthor(author);
  return (
    <span
      style={{
        width: size,
        height: size,
        borderRadius: agent ? 8 : "50%",
        background: agent ? color.dark : color.chip,
        color: agent ? color.onDark : color.muted3,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 ${size <= 22 ? 10 : 11}px ${font.sans}`,
        flexShrink: 0,
      }}
    >
      {name.slice(0, agent ? 2 : 1).toUpperCase()}
    </span>
  );
}

function AgentPill() {
  return (
    <span
      style={{
        font: `700 8.5px ${font.mono}`,
        letterSpacing: ".04em",
        color: color.onDark,
        background: color.dark,
        borderRadius: 4,
        padding: "1.5px 5px",
      }}
    >
      AGENT
    </span>
  );
}

// ── Reactions ────────────────────────────────────────────

function ReactionsRow({
  message,
  selfKey,
  onReact,
  onAddReaction,
}: {
  message: MessageView;
  selfKey: string;
  onReact: (emoji: string) => void;
  onAddReaction: () => void;
}) {
  if (message.reactions.length === 0) return null;
  return (
    <div style={{ display: "flex", flexWrap: "wrap", gap: 5, marginTop: 6, minWidth: 0, maxWidth: "100%" }}>
      {message.reactions.map((reaction) => {
        const mine = hasReacted(message, reaction.emoji, selfKey);
        return (
          <HoverButton
            key={reaction.emoji}
            onClick={() => onReact(reaction.emoji)}
            title={mine ? "Remove reaction" : "React"}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 5,
              padding: "2px 8px",
              borderRadius: 999,
              border: `1px solid ${mine ? accentVar : color.borderSoft}`,
              background: mine ? "rgba(160,90,60,.10)" : color.paper,
              font: `500 11px ${font.sans}`,
              color: mine ? accentVar : color.muted3,
            }}
            hoverStyle={{ background: mine ? "rgba(160,90,60,.16)" : color.hover }}
          >
            <span>{reaction.emoji}</span>
            <span>{reaction.reactors.length}</span>
          </HoverButton>
        );
      })}
      <HoverButton
        onClick={(event) => {
          event.stopPropagation();
          onAddReaction();
        }}
        title="Add a reaction"
        style={{
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          width: 26,
          padding: "2px 0",
          borderRadius: 999,
          border: `1px solid ${color.borderSoft}`,
          background: color.paper,
          color: color.muted2,
        }}
        hoverStyle={{ background: color.hover, color: color.muted3 }}
      >
        <AddReactGlyph size={13} />
      </HoverButton>
    </div>
  );
}

// ── Inline thread indicator ──────────────────────────────

function ThreadIndicator({
  replyCount,
  replyHint,
  onClick,
}: {
  replyCount: number;
  replyHint: string | null;
  onClick: () => void;
}) {
  return (
    <HoverButton
      onClick={onClick}
      title="View thread"
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        marginTop: 6,
        padding: "3px 9px 3px 7px",
        borderRadius: 8,
        border: `1px solid ${color.borderSoft}`,
        background: color.paper,
        maxWidth: "100%",
        minWidth: 0,
      }}
      hoverStyle={{ background: color.hover }}
    >
      <span style={{ color: accentVar, display: "flex" }}>
        <ThreadGlyph size={12} />
      </span>
      <span style={{ font: `500 11.5px ${font.sans}`, color: accentVar }}>
        {replyCount} {replyCount === 1 ? "reply" : "replies"}
      </span>
      {replyHint && (
        <span
          style={{
            font: `400 11px ${font.sans}`,
            color: color.muted2,
            minWidth: 0,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          · {replyHint}
        </span>
      )}
    </HoverButton>
  );
}

// ── Inline edit box + delete confirm ─────────────────────

function EditBox({
  draft,
  onChange,
  onSave,
  onCancel,
}: {
  draft: string;
  onChange: (value: string) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  return (
    <div style={{ marginTop: 2 }}>
      <textarea
        autoFocus
        value={draft}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape" && !event.nativeEvent.isComposing) {
            event.preventDefault();
            onCancel();
          } else if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault();
            onSave();
          }
        }}
        rows={Math.min(8, Math.max(1, draft.split("\n").length))}
        style={{
          width: "100%",
          boxSizing: "border-box",
          resize: "vertical",
          padding: "8px 10px",
          borderRadius: radius.sm,
          border: `1px solid ${accentVar}`,
          background: color.paper,
          font: `400 13.5px ${font.sans}`,
          color: color.ink,
          lineHeight: 1.5,
          outline: "none",
        }}
      />
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 6 }}>
        <button
          type="button"
          onClick={onSave}
          style={{
            all: "unset",
            cursor: "pointer",
            padding: "5px 12px",
            borderRadius: radius.sm,
            background: color.dark,
            color: color.onDark,
            font: `600 11.5px ${font.sans}`,
          }}
        >
          Save
        </button>
        <button
          type="button"
          onClick={onCancel}
          style={{
            all: "unset",
            cursor: "pointer",
            padding: "5px 10px",
            borderRadius: radius.sm,
            color: color.muted3,
            font: `500 11.5px ${font.sans}`,
          }}
        >
          Cancel
        </button>
        <span style={{ font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
          Enter to save · Esc to cancel
        </span>
      </div>
    </div>
  );
}

function DeleteConfirm({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        marginTop: 6,
        padding: "7px 11px",
        borderRadius: radius.sm,
        border: `1px solid ${color.dangerBorder}`,
        background: color.dangerSoft,
        maxWidth: "100%",
      }}
    >
      <span style={{ flex: 1, font: `400 12px ${font.sans}`, color: color.danger, minWidth: 0 }}>
        Delete this message? This can't be undone.
      </span>
      <button
        type="button"
        onClick={onConfirm}
        style={{
          all: "unset",
          cursor: "pointer",
          padding: "4px 11px",
          borderRadius: radius.sm,
          background: color.danger,
          color: "#fff",
          font: `600 11.5px ${font.sans}`,
          flexShrink: 0,
        }}
      >
        Delete
      </button>
      <button
        type="button"
        onClick={onCancel}
        style={{
          all: "unset",
          cursor: "pointer",
          padding: "4px 8px",
          borderRadius: radius.sm,
          color: color.muted3,
          font: `500 11.5px ${font.sans}`,
          flexShrink: 0,
        }}
      >
        Cancel
      </button>
    </div>
  );
}

// ── Hover action bar + overflow menu ─────────────────────

function MenuRow({
  label,
  icon,
  onClick,
}: {
  label: string;
  icon: ReactNode;
  onClick: () => void;
}) {
  return (
    <HoverButton
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        width: "100%",
        padding: "7px 9px",
        borderRadius: radius.sm,
        font: `400 12.5px ${font.sans}`,
        color: color.inkSoft,
      }}
      hoverStyle={{ background: color.hover }}
    >
      <span style={{ color: color.muted2, display: "flex" }}>{icon}</span>
      <span>{label}</span>
    </HoverButton>
  );
}

function CopyMenuRow({
  label,
  icon,
  value,
  onDone,
}: {
  label: string;
  icon: ReactNode;
  value: string;
  onDone: () => void;
}) {
  const [copied, setCopied] = useState(false);
  return (
    <HoverButton
      onClick={(event) => {
        event.stopPropagation();
        void navigator.clipboard
          .writeText(value)
          .then(() => {
            setCopied(true);
            setTimeout(onDone, 700);
          })
          .catch(onDone);
      }}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        width: "100%",
        padding: "7px 9px",
        borderRadius: radius.sm,
        font: `400 12.5px ${font.sans}`,
        color: color.inkSoft,
      }}
      hoverStyle={{ background: color.hover }}
    >
      <span style={{ color: color.muted2, display: "flex" }}>{icon}</span>
      <span>{copied ? "Copied" : label}</span>
    </HoverButton>
  );
}

function DangerMenuRow({ label, icon, onClick }: { label: string; icon: ReactNode; onClick: () => void }) {
  return (
    <HoverButton
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        width: "100%",
        padding: "7px 9px",
        borderRadius: radius.sm,
        font: `400 12.5px ${font.sans}`,
        color: color.danger,
      }}
      hoverStyle={{ background: color.dangerSoft }}
    >
      <span style={{ display: "flex" }}>{icon}</span>
      <span>{label}</span>
    </HoverButton>
  );
}

function OverflowMenu({
  onClose,
  onReplyInThread,
  copyLinkValue,
  copyRefValue,
  onOpenExplorer,
  threadable,
  canModify,
  onStartEdit,
  onStartDelete,
}: {
  onClose: () => void;
  onReplyInThread: () => void;
  copyLinkValue: string;
  copyRefValue: string;
  /** Present only when this row's op resolved a finalized inclusion height. */
  onOpenExplorer?: () => void;
  threadable: boolean;
  canModify: boolean;
  onStartEdit: () => void;
  onStartDelete: () => void;
}) {
  // Escape closes it; an outside click does too — but the listener attaches
  // one tick late so the click that OPENED the menu doesn't immediately
  // close it again. Dismissal always sets state directly to null (never
  // toggles), so a row's own click handler can't race it back open.
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
      style={{
        position: "absolute",
        top: 18,
        right: 8,
        width: 184,
        zIndex: 3,
        background: color.paper,
        border: `1px solid ${color.borderSoft}`,
        borderRadius: radius.md,
        boxShadow: shadow.pop,
        padding: 4,
      }}
    >
      {threadable && (
        <MenuRow
          label="Reply in thread"
          icon={<ThreadGlyph size={13} />}
          onClick={() => {
            onReplyInThread();
            onClose();
          }}
        />
      )}
      <CopyMenuRow label="Copy link" icon={<LinkGlyph size={13} />} value={copyLinkValue} onDone={onClose} />
      <CopyMenuRow label="Copy reference" icon={<RefGlyph size={13} />} value={copyRefValue} onDone={onClose} />
      {onOpenExplorer && (
        <MenuRow
          label="Open in explorer"
          icon={<ExplorerGlyph size={13} />}
          onClick={() => {
            onOpenExplorer();
            onClose();
          }}
        />
      )}
      {canModify && (
        <>
          <div style={{ height: 1, background: color.borderSoft, margin: "4px 6px" }} />
          <MenuRow
            label="Edit message"
            icon={<PencilGlyph size={13} />}
            onClick={() => {
              onStartEdit();
              onClose();
            }}
          />
          <DangerMenuRow
            label="Delete message"
            icon={<TrashGlyph size={13} />}
            onClick={() => {
              onStartDelete();
              onClose();
            }}
          />
        </>
      )}
    </div>
  );
}

function HoverBar({
  channelId,
  seq,
  onQuickReact,
  onOpenPicker,
  onOpenThread,
  onMoreToggle,
  threadable,
}: {
  channelId: string;
  seq: number;
  onQuickReact: (emoji: string) => void;
  onOpenPicker: () => void;
  onOpenThread: () => void;
  onMoreToggle: () => void;
  threadable: boolean;
}) {
  const btnStyle: CSSProperties = {
    width: 27,
    height: 25,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 6,
    color: color.muted3,
  };
  return (
    <div
      style={{
        position: "absolute",
        top: -13,
        right: 8,
        zIndex: 2,
        display: "flex",
        alignItems: "center",
        gap: 1,
        padding: 2,
        borderRadius: 8,
        background: color.paper,
        border: `1px solid ${color.borderSoft}`,
        boxShadow: shadow.card,
      }}
    >
      {QUICK_REACTS.map((emoji) => (
        <HoverButton
          key={emoji}
          title={`React ${emoji}`}
          onClick={() => onQuickReact(emoji)}
          style={{ ...btnStyle, font: "14px/1 sans-serif" }}
          hoverStyle={{ background: color.hover }}
        >
          {emoji}
        </HoverButton>
      ))}
      <HoverButton
        title="Pick a reaction"
        onClick={(event) => {
          event.stopPropagation();
          onOpenPicker();
        }}
        style={btnStyle}
        hoverStyle={{ background: color.hover }}
      >
        <AddReactGlyph size={15} />
      </HoverButton>
      <div style={{ width: 1, height: 16, background: color.borderSoft, margin: "0 2px" }} />
      <AskAgentButton channelId={channelId} seq={seq} style={btnStyle} />
      {threadable && (
        <HoverButton
          title="Reply in thread"
          onClick={onOpenThread}
          style={btnStyle}
          hoverStyle={{ background: color.hover }}
        >
          <ThreadGlyph size={14} />
        </HoverButton>
      )}
      <HoverButton title="More" onClick={onMoreToggle} style={btnStyle} hoverStyle={{ background: color.hover }}>
        <MoreGlyph size={14} />
      </HoverButton>
    </div>
  );
}

// ── The row itself ───────────────────────────────────────

export function MessageItem({
  message,
  names,
  groupStart,
  selfKey,
  hovered,
  menuOpen,
  replyHint,
  linkRef,
  refRef,
  onHover,
  onMenuToggle,
  onOpenThread,
  onReact,
  onEdit,
  onDelete,
  onTagClick,
  threadable = true,
  op,
}: {
  message: MessageView;
  names: AuthorNames;
  groupStart: boolean;
  selfKey: string;
  hovered: boolean;
  menuOpen: boolean;
  /** "· lastReplyAuthor" hint for the inline thread pill, or null. */
  replyHint: string | null;
  /** Precomputed "copy link" / "copy reference" clipboard values. */
  linkRef: string;
  refRef: string;
  onHover: (over: boolean) => void;
  onMenuToggle: (open: boolean) => void;
  onOpenThread: () => void;
  onReact: (emoji: string) => void;
  /** Replace this message's text (author-gated; only offered on own messages). */
  onEdit: (text: string) => void;
  /** Tombstone this message (author-gated; only offered on own messages). */
  onDelete: () => void;
  /** Make body #tags click-to-filter (tag passed WITHOUT the `#`). Absent —
   *  e.g. in the ThreadPanel — tags stay tinted but inert. */
  onTagClick?: (tag: string) => void;
  /** False inside the ThreadPanel — a thread reply can't itself spawn a
   *  nested thread, so the thread affordances are hidden there. */
  threadable?: boolean;
  /** This row's finalization record: pending dot → inline checkmark once the
   *  write that produced/touched it is included (hover: height + op hash). */
  op?: OpRecord;
}) {
  const author = authorName(message.head.author, names);
  // An unresolved key handle ("4c3a9460…") reads better in mono + muted so it
  // isn't mistaken for a chosen display name; a real name stays sans + ink.
  const unresolvedKey = /^[0-9a-f]{8}…$/.test(author);
  const deleted = message.head.deleted;
  const replyCount = message.head.reply_count;
  // A message we authored can be edited/deleted — the module rejects the write
  // from anyone else, so the affordances are hidden rather than shown-and-failing.
  const canModify = !deleted && authorKey(message.head.author) === selfKey;
  // A committed chat message needs no per-row "confirmed" badge — every message
  // is on-chain by definition, so a persistent checkmark is pure noise. Show the
  // mark ONLY while a write is in flight (the pulsing dot) or if it FAILED (the
  // cross); the settled state falls through to the plain gutter/header. The
  // explorer deep-link moves to the overflow menu instead of a tiny clickable ✓.
  const store = useContext(ConsoleContext);
  const liveOp = op && op.phase !== "finalized" ? op : undefined;
  const explorerHeight =
    store && op?.phase === "finalized" && op.height !== undefined ? op.height : null;
  const openInExplorer =
    explorerHeight === null ? undefined : () => store?.actions.openExplorerAt(explorerHeight);
  const [editing, setEditing] = useState(false);
  const [editDraft, setEditDraft] = useState("");
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);

  const startEdit = () => {
    setConfirmingDelete(false);
    setEditDraft(blocksToInput(message.head.blocks));
    setEditing(true);
  };
  const saveEdit = () => {
    const next = editDraft.trim();
    if (next) onEdit(next);
    setEditing(false);
  };
  // Hide the hover bar while editing/confirming so the row's action UI is the
  // sole focus and the floating bar can't overlap the textarea. An open menu or
  // emoji picker keeps the bar (and row highlight) up even as the pointer leaves.
  const active = hovered || menuOpen || pickerOpen;
  const showBar = active && !deleted && !editing && !confirmingDelete;

  return (
    <div
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      style={{
        borderRadius: 9,
        padding: `${groupStart ? 7 : 1}px 8px`,
        margin: `${groupStart ? 8 : 0}px -8px 0`,
        width: "calc(100% + 16px)",
        maxWidth: "calc(100% + 16px)",
        boxSizing: "border-box",
        minWidth: 0,
        background: active ? color.sunken : "transparent",
      }}
    >
      <div style={{ position: "relative", display: "flex", gap: 11, maxWidth: CONTENT_MAX, minWidth: 0 }}>
      <div style={{ width: 30, flexShrink: 0, display: "flex", justifyContent: "center", paddingTop: 1 }}>
        {groupStart ? (
          <Avatar author={message.head.author} name={author} size={30} />
        ) : liveOp ? (
          // a grouped row has no meta line — an in-flight/failed mark takes the
          // time gutter (and must stay hoverable, so it outranks the hover time)
          <span style={{ marginTop: 4 }}>
            <FinalizationMark op={liveOp} />
          </span>
        ) : hovered ? (
          <span style={{ font: `400 10px ${font.mono}`, color: color.muted2, marginTop: 4 }}>
            {timeOf(message.head.created_at)}
          </span>
        ) : null}
      </div>
      <div style={{ minWidth: 0, flex: 1, maxWidth: "100%" }}>
        {groupStart && (
          <div style={{ display: "flex", alignItems: "baseline", gap: 7, minWidth: 0, flexWrap: "wrap", rowGap: 2 }}>
            <span
              title={unresolvedKey ? "Member without a display name yet" : undefined}
              style={{
                font: unresolvedKey ? `500 12px ${font.mono}` : `600 13px ${font.sans}`,
                color: unresolvedKey ? color.muted3 : color.ink,
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                maxWidth: "min(360px, 70%)",
              }}
            >
              {author}
            </span>
            {isAgentAuthor(message.head.author) && <AgentPill />}
            <span style={{ font: `400 11px ${font.mono}`, color: color.muted2 }}>
              {timeOf(message.head.created_at)}
            </span>
            {message.head.edited_at !== null && (
              <span style={{ font: `400 10px ${font.sans}`, color: color.muted2 }}>(edited)</span>
            )}
            <FinalizationMark op={liveOp} />
          </div>
        )}
        <div
          data-chat-body
          style={{
            font: `400 13.5px ${font.sans}`,
            lineHeight: 1.55,
            marginTop: groupStart ? 2 : 0,
            color: deleted ? color.muted2 : color.inkSofter,
            minWidth: 0,
            maxWidth: "100%",
            overflowWrap: "anywhere",
            wordBreak: "break-word",
          }}
        >
          {deleted ? (
            <span style={{ fontStyle: "italic" }}>message deleted</span>
          ) : editing ? (
            <EditBox draft={editDraft} onChange={setEditDraft} onSave={saveEdit} onCancel={() => setEditing(false)} />
          ) : (
            <RichText blocks={message.head.blocks} names={names} onTagClick={onTagClick} />
          )}
        </div>
        {confirmingDelete && (
          <DeleteConfirm
            onConfirm={() => {
              onDelete();
              setConfirmingDelete(false);
            }}
            onCancel={() => setConfirmingDelete(false)}
          />
        )}
        {!deleted && !editing && (
          <ReactionsRow
            message={message}
            selfKey={selfKey}
            onReact={onReact}
            onAddReaction={() => {
              onMenuToggle(false);
              setPickerOpen(true);
            }}
          />
        )}
        {!deleted && !editing && threadable && replyCount > 0 && (
          <ThreadIndicator replyCount={replyCount} replyHint={replyHint} onClick={onOpenThread} />
        )}
      </div>
      {showBar && (
        <HoverBar
          channelId={message.channel_id}
          seq={message.seq}
          onQuickReact={onReact}
          onOpenPicker={() => {
            onMenuToggle(false);
            setPickerOpen(true);
          }}
          onOpenThread={onOpenThread}
          onMoreToggle={() => onMenuToggle(!menuOpen)}
          threadable={threadable}
        />
      )}
      {pickerOpen && (
        <EmojiPicker
          onPick={(emoji) => onReact(emoji)}
          onClose={() => setPickerOpen(false)}
        />
      )}
      {menuOpen && (
        <OverflowMenu
          onClose={() => onMenuToggle(false)}
          onReplyInThread={onOpenThread}
          copyLinkValue={linkRef}
          copyRefValue={refRef}
          onOpenExplorer={openInExplorer}
          threadable={threadable}
          canModify={canModify}
          onStartEdit={startEdit}
          onStartDelete={() => setConfirmingDelete(true)}
        />
      )}
      </div>
    </div>
  );
}
