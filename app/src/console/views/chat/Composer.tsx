// A shared auto-growing composer used for both the main channel lane and the
// thread panel. Each caller owns its own `value`/`onChange` (draft state
// stays local to whichever <Composer> instance renders it — the thread
// panel's reply box never touches the main channel's in-progress draft,
// since they're just two separate component instances).

import { useContext, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, KeyboardEvent, ReactNode } from "react";

import { Icon } from "../../components/Icon";
import type { IconName } from "../../components/Icon";
import { ConsoleContext } from "../../store/context";
import { HoverButton } from "./HoverButton";
import { MentionMenu } from "./MentionMenu";
import {
  insertMention,
  mentionCandidateToken,
  mentionableUsers,
  mentionCandidatesAll,
  mentionTokenAt,
} from "./mention";
import { accentVar, color, font, radius } from "../../theme/tokens";

const DEFAULT_MAX_HEIGHT = 168;
const EMPTY_NODE_USERS: Record<string, { userKey: string; name: string | null }> = {};

function FmtButton({
  label,
  icon,
  title,
  onClick,
}: {
  label?: ReactNode;
  icon?: IconName;
  title: string;
  onClick: () => void;
}) {
  const style: CSSProperties = {
    width: 24,
    height: 24,
    padding: 0,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    flexShrink: 0,
    borderRadius: 5,
    color: color.muted2,
    font: `650 12px ${font.sans}`,
    lineHeight: 1,
  };
  return (
    <HoverButton
      title={title}
      onMouseDown={(event) => event.preventDefault()}
      onClick={onClick}
      style={style}
      hoverStyle={{ background: color.hover, color: color.ink }}
    >
      {icon ? <Icon name={icon} size={14} strokeWidth={1.9} /> : label}
    </HoverButton>
  );
}

export function Composer({
  value,
  onChange,
  onSend,
  placeholder,
  maxHeight = DEFAULT_MAX_HEIGHT,
  autoFocus,
}: {
  value: string;
  onChange: (next: string) => void;
  onSend: () => void;
  placeholder: string;
  /** Thread panel composer caps shorter than the main lane's (narrower panel). */
  maxHeight?: number;
  autoFocus?: boolean;
}) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const [focused, setFocused] = useState(false);
  // Caret range to restore after a toolbar edit re-renders the controlled value.
  const pendingSelection = useRef<[number, number] | null>(null);

  // ── @mention typeahead ──
  // The composer is shared (main lane + thread panel), so mention candidates
  // come from context here rather than threading props through both callers.
  // Context may be absent in bare component tests — then no menu, no crash.
  const store = useContext(ConsoleContext);
  const agents = store?.state.agents ?? [];
  const nodeUsers = store?.state.nodeUsers ?? EMPTY_NODE_USERS;
  const users = useMemo(() => mentionableUsers(nodeUsers, agents), [nodeUsers, agents]);
  const [caret, setCaret] = useState(0);
  const [mentionIndex, setMentionIndex] = useState(0);
  // The token.start the user Escaped out of — that token stays dismissed
  // until it's gone (mirrors the Pages slash menu's `slashDismissed`).
  const [mentionDismissedAt, setMentionDismissedAt] = useState<number | null>(null);

  const mentionToken = mentionTokenAt(value, caret);
  const mentionStart = mentionToken?.start ?? null;
  const mentionQuery = mentionToken?.query ?? "";
  const menuCandidates = useMemo(
    () =>
      mentionStart !== null && mentionStart !== mentionDismissedAt
        ? mentionCandidatesAll(agents, users, mentionQuery)
        : [],
    [agents, mentionDismissedAt, mentionQuery, mentionStart, users],
  );
  const menuOpen = menuCandidates.length > 0;

  const handleChange = (next: string, nextCaret: number) => {
    setCaret(nextCaret);
    const nextToken = mentionTokenAt(next, nextCaret);
    if (nextToken) setMentionIndex(0);
    if (!nextToken || nextToken.start !== mentionDismissedAt) setMentionDismissedAt(null);
    onChange(next);
  };

  const pickMention = (token: string) => {
    if (!mentionToken) return;
    const el = ref.current;
    const next = insertMention(value, mentionToken, el?.selectionStart ?? caret, token);
    pendingSelection.current = [next.caret, next.caret];
    setCaret(next.caret);
    setMentionIndex(0);
    onChange(next.text);
  };

  // Auto-grow: reset to `auto` first so shrinking (deleting text) is picked
  // up too, then clamp to `maxHeight` — past that the textarea scrolls
  // internally instead of pushing the send button off-screen.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, maxHeight)}px`;
    if (pendingSelection.current) {
      const [start, end] = pendingSelection.current;
      pendingSelection.current = null;
      el.focus();
      el.setSelectionRange(start, end);
    }
  }, [value, maxHeight]);

  // Wrap the current selection (or a placeholder) in markdown the composer's
  // own parser understands (**bold**, *italic*, ```code```), then re-select the
  // wrapped text so the user can keep typing or toggle it off.
  const wrap = (before: string, after: string, placeholder: string) => {
    const el = ref.current;
    if (!el) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const selected = value.slice(start, end) || placeholder;
    pendingSelection.current = [start + before.length, start + before.length + selected.length];
    onChange(value.slice(0, start) + before + selected + after + value.slice(end));
  };

  const insertLink = () => {
    const el = ref.current;
    if (!el) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const selected = value.slice(start, end);
    const selectedUrl = /^https?:\/\/\S+$/i.test(selected.trim()) ? selected.trim() : null;
    const label = selected && !selectedUrl ? selected : "link";
    const href = selectedUrl ?? "https://example.com";
    const inserted = `[${label}](${href})`;
    const labelStart = start + 1;
    const hrefStart = start + label.length + 3;
    pendingSelection.current = selected
      ? [hrefStart, hrefStart + href.length]
      : [labelStart, labelStart + label.length];
    onChange(value.slice(0, start) + inserted + value.slice(end));
  };

  const quote = () => {
    const el = ref.current;
    if (!el) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const selected = value.slice(start, end) || "quote";
    const inserted = selected
      .split("\n")
      .map((line) => (line.startsWith(">") ? line : `> ${line}`))
      .join("\n");
    pendingSelection.current = value.slice(start, end)
      ? [start, start + inserted.length]
      : [start + 2, start + 2 + selected.length];
    onChange(value.slice(0, start) + inserted + value.slice(end));
  };

  const insertDivider = () => {
    const el = ref.current;
    if (!el) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const before = value.slice(0, start);
    const after = value.slice(end);
    const prefix = before && !before.endsWith("\n") ? "\n" : "";
    const inserted = `${prefix}---\n`;
    pendingSelection.current = [before.length + inserted.length, before.length + inserted.length];
    onChange(before + inserted + after);
  };

  const canSend = value.trim().length > 0;

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    // An open mention menu owns the navigation keys; Enter/Tab pick instead of
    // sending. IME guard as below — committing a candidate must not pick.
    if (menuOpen && !event.nativeEvent.isComposing) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setMentionIndex((i) => (i + 1) % menuCandidates.length);
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setMentionIndex((i) => (i - 1 + menuCandidates.length) % menuCandidates.length);
        return;
      }
      if (event.key === "Enter" || event.key === "Tab") {
        event.preventDefault();
        pickMention(
          mentionCandidateToken(menuCandidates[Math.min(mentionIndex, menuCandidates.length - 1)]!),
        );
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        // stopPropagation: the ThreadPanel closes on a bubbled Escape — with
        // the menu open, Escape means "dismiss the menu", not "close the thread".
        event.stopPropagation();
        setMentionDismissedAt(mentionToken?.start ?? null);
        return;
      }
    }
    // Ignore Enter while an IME composition is active — pressing Enter to commit
    // a Korean / Japanese / Chinese candidate must NOT also send the message.
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      if (canSend) onSend();
    }
    // Shift+Enter: fall through — the textarea inserts its own newline.
  };

  return (
    <div
      style={{
        padding: "12px 16px 14px",
        borderTop: `1px solid ${color.borderSoft}`,
        background: color.paper,
        flexShrink: 0,
        minWidth: 0,
      }}
    >
      <div
        style={{
          borderRadius: radius.lg,
          border: `1px solid ${focused ? color.borderStrong : color.border}`,
          background: color.paper,
          display: "flex",
          flexDirection: "column",
          gap: 7,
          padding: "10px 12px 8px",
          transition: "border-color .12s ease",
          minWidth: 0,
        }}
      >
        <div style={{ position: "relative", minWidth: 0 }}>
          {menuOpen && (
            <MentionMenu
              candidates={menuCandidates}
              activeIndex={Math.min(mentionIndex, menuCandidates.length - 1)}
              onPick={pickMention}
            />
          )}
          <textarea
            ref={ref}
            autoFocus={autoFocus}
            rows={1}
            value={value}
            onChange={(event) => handleChange(event.target.value, event.target.selectionStart)}
            onSelect={(event) => setCaret(event.currentTarget.selectionStart)}
            onKeyDown={handleKeyDown}
            onFocus={() => setFocused(true)}
            onBlur={() => setFocused(false)}
            placeholder={placeholder}
            style={{
              width: "100%",
              minWidth: 0,
              resize: "none",
              display: "block",
              font: `400 13.5px ${font.sans}`,
              color: color.ink,
              lineHeight: 1.5,
              padding: 0,
              maxHeight,
              overflowY: "auto",
              overflowWrap: "break-word",
            }}
          />
        </div>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10, minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 1, minWidth: 0 }}>
            <FmtButton title="Bold  **text**" label="B" onClick={() => wrap("**", "**", "bold")} />
            <FmtButton
              title="Italic  *text*"
              label={<span style={{ fontStyle: "italic" }}>I</span>}
              onClick={() => wrap("*", "*", "italic")}
            />
            <FmtButton title="Link  [text](https://...)" icon="link" onClick={insertLink} />
            <FmtButton title="Quote  > text" icon="quote" onClick={quote} />
            <FmtButton
              title="Code block  ```"
              icon="code"
              onClick={() => wrap("```\n", "\n```", "code")}
            />
            <FmtButton title="Divider  ---" icon="divider" onClick={insertDivider} />
            <div style={{ width: 1, height: 14, background: color.borderSoft, margin: "0 5px" }} />
            <span
              style={{
                font: `500 10px ${font.mono}`,
                color: color.muted2,
                userSelect: "none",
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              <b style={{ fontWeight: 600, color: color.muted }}>Enter</b> to send
            </span>
          </div>
          <button
            type="button"
            onClick={() => canSend && onSend()}
            title="Send"
            aria-label="Send message"
            aria-disabled={!canSend}
            style={{
              all: "unset",
              cursor: canSend ? "pointer" : "not-allowed",
              width: 31,
              height: 31,
              borderRadius: 8,
              flexShrink: 0,
              background: canSend ? accentVar : color.borderSoft,
              color: canSend ? color.onDark : color.muted2,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              transition: "background .12s ease, color .12s ease",
            }}
          >
            <Icon name="chevronRight" size={15} />
          </button>
        </div>
      </div>
    </div>
  );
}
