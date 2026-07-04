// A shared auto-growing composer used for both the main channel lane and the
// thread panel. Each caller owns its own `value`/`onChange` (draft state
// stays local to whichever <Composer> instance renders it — the thread
// panel's reply box never touches the main channel's in-progress draft,
// since they're just two separate component instances).

import { useEffect, useRef, useState } from "react";
import type { CSSProperties, KeyboardEvent, ReactNode } from "react";

import { Icon } from "../../components/Icon";
import { HoverButton } from "./HoverButton";
import { accentVar, color, font, radius } from "../../theme/tokens";

const DEFAULT_MAX_HEIGHT = 168;

function FmtButton({ label, title, onClick }: { label: ReactNode; title: string; onClick: () => void }) {
  const style: CSSProperties = {
    minWidth: 22,
    height: 22,
    padding: "0 5px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 5,
    color: color.muted2,
    font: `600 12px ${font.sans}`,
  };
  return (
    <HoverButton
      title={title}
      onMouseDown={(event) => event.preventDefault()}
      onClick={onClick}
      style={style}
      hoverStyle={{ background: color.hover, color: color.ink }}
    >
      {label}
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

  const canSend = value.trim().length > 0;

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
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
        <textarea
          ref={ref}
          autoFocus={autoFocus}
          rows={1}
          value={value}
          onChange={(event) => onChange(event.target.value)}
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
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10, minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 1, minWidth: 0 }}>
            <FmtButton title="Bold  **text**" label="B" onClick={() => wrap("**", "**", "bold")} />
            <FmtButton
              title="Italic  *text*"
              label={<span style={{ fontStyle: "italic" }}>I</span>}
              onClick={() => wrap("*", "*", "italic")}
            />
            <FmtButton
              title="Code block  ```"
              label={<span style={{ font: `600 11px ${font.mono}` }}>{"</>"}</span>}
              onClick={() => wrap("```\n", "\n```", "code")}
            />
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
