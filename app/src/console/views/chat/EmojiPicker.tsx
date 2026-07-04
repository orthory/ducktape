// A small reaction emoji picker popover. The node accepts any emoji string on
// AddReaction, so this is purely a client-side convenience grid of the common
// ones — no full unicode set (and no fake search over emoji we have no keyword
// data for), just a useful curated spread.

import { useEffect } from "react";

import { HoverButton } from "./HoverButton";
import { color, font, radius, shadow } from "../../theme/tokens";

const GROUPS: { label: string; emojis: string[] }[] = [
  { label: "Reactions", emojis: ["👍", "👎", "❤️", "🎉", "🙌", "👀", "✅", "❌", "🔥", "💯", "🚀", "👏"] },
  { label: "Faces", emojis: ["😂", "😄", "🙂", "😅", "😍", "🤔", "😮", "😢", "😡", "🥳", "😴", "🤯"] },
  { label: "Work", emojis: ["✨", "⭐", "💡", "⚡", "🐛", "🎯", "📌", "🔒", "🧪", "🛠️", "📈", "🧠"] },
  { label: "Misc", emojis: ["🙏", "💪", "🤝", "👌", "☕", "🍕", "🦆", "🎈", "💤", "❓", "❗", "🫡"] },
];

export function EmojiPicker({ onPick, onClose }: { onPick: (emoji: string) => void; onClose: () => void }) {
  // Escape + outside-click dismiss, attached one tick late so the click that
  // OPENED the picker doesn't immediately close it (mirrors OverflowMenu).
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
      onClick={(event) => event.stopPropagation()}
      style={{
        position: "absolute",
        top: 18,
        right: 8,
        width: 236,
        zIndex: 4,
        background: color.paper,
        border: `1px solid ${color.borderSoft}`,
        borderRadius: radius.md,
        boxShadow: shadow.pop,
        padding: 8,
      }}
    >
      <div style={{ maxHeight: 232, overflowY: "auto" }}>
        {GROUPS.map((group) => (
          <div key={group.label} style={{ marginBottom: 6 }}>
            <div
              style={{
                font: `600 9px ${font.mono}`,
                letterSpacing: ".08em",
                color: color.muted2,
                padding: "2px 3px 4px",
              }}
            >
              {group.label.toUpperCase()}
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(6, 1fr)", gap: 2 }}>
              {group.emojis.map((emoji) => (
                <HoverButton
                  key={emoji}
                  title={`React ${emoji}`}
                  onClick={() => {
                    onPick(emoji);
                    onClose();
                  }}
                  style={{
                    height: 30,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    borderRadius: radius.sm,
                    font: "17px/1 'Apple Color Emoji', 'Segoe UI Emoji', sans-serif",
                  }}
                  hoverStyle={{ background: color.hover }}
                >
                  {emoji}
                </HoverButton>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
