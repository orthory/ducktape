// A shared auto-growing composer used for both the main channel lane and the
// thread panel. Each caller owns its own `value`/`onChange` (draft state
// stays local to whichever <Composer> instance renders it — the thread
// panel's reply box never touches the main channel's in-progress draft,
// since they're just two separate component instances).

import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";

import { Icon } from "../../components/Icon";
import { accentVar, color, font, radius } from "../../theme/tokens";

const DEFAULT_MAX_HEIGHT = 168;

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

  // Auto-grow: reset to `auto` first so shrinking (deleting text) is picked
  // up too, then clamp to `maxHeight` — past that the textarea scrolls
  // internally instead of pushing the send button off-screen.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, maxHeight)}px`;
  }, [value, maxHeight]);

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
            <b style={{ fontWeight: 600, color: color.muted }}>Enter</b> to send ·{" "}
            <b style={{ fontWeight: 600, color: color.muted }}>Shift+Enter</b> for a new line
          </span>
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
