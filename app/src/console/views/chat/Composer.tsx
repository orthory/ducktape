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
        margin: 13,
        borderRadius: radius.md,
        border: `1px solid ${focused ? color.borderStrong : color.borderSoft}`,
        background: color.paper,
        transition: "border-color .12s ease",
      }}
    >
      <div style={{ display: "flex", alignItems: "flex-end", gap: 8, padding: "8px 8px 6px 13px" }}>
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
            flex: 1,
            minWidth: 0,
            resize: "none",
            font: `400 13px ${font.sans}`,
            color: color.ink,
            lineHeight: 1.5,
            padding: "3px 0",
            maxHeight,
            overflowY: "auto",
          }}
        />
        <button
          type="button"
          onClick={() => canSend && onSend()}
          disabled={!canSend}
          title="Send"
          style={{
            all: "unset",
            cursor: canSend ? "pointer" : "not-allowed",
            width: 28,
            height: 28,
            borderRadius: 7,
            flexShrink: 0,
            background: canSend ? accentVar : color.chip,
            color: canSend ? "#fff" : color.muted2,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="chevronRight" size={15} />
        </button>
      </div>
      <div style={{ padding: "0 13px 7px", font: `400 10px ${font.mono}`, color: color.muted2 }}>
        Enter to send · Shift+Enter for a new line
      </div>
    </div>
  );
}
