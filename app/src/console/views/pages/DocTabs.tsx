import type { CSSProperties } from "react";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";

/** The document tab strip: one tab per open page, click to switch, middle-click
 *  or the × to close. `activePage` names the active tab. */
export function DocTabs({
  open,
  active,
  titleOf,
  onSelect,
  onClose,
}: {
  open: string[];
  active: string | null;
  titleOf: (id: string) => string;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
}) {
  if (open.length === 0) return null;
  return (
    <div
      role="tablist"
      aria-label="Open pages"
      // `no-scrollbar` keeps the strip scrollable (many tabs overflow and can be
      // wheeled through) while hiding the global 10px scrollbar chrome.
      className="no-scrollbar"
      style={{
        display: "flex",
        alignItems: "stretch",
        gap: 2,
        height: 34,
        flexShrink: 0,
        padding: "0 10px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        overflowX: "auto",
      }}
    >
      {open.map((id) => {
        const isActive = id === active;
        const label = titleOf(id) || "Untitled";
        const tabStyle: CSSProperties = {
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "0 8px 0 10px",
          maxWidth: 200,
          cursor: "pointer",
          borderTopLeftRadius: radius.sm,
          borderTopRightRadius: radius.sm,
          borderBottom: isActive ? `2px solid ${color.dark}` : "2px solid transparent",
          background: isActive ? color.paper : "transparent",
          color: isActive ? color.ink : color.inkSofter,
          font: `${isActive ? 600 : 500} 11.5px ${font.sans}`,
        };
        return (
          <div
            key={id}
            role="tab"
            aria-selected={isActive}
            aria-label={label}
            tabIndex={0}
            onClick={() => onSelect(id)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onSelect(id);
            }}
            onAuxClick={(e) => {
              if (e.button === 1) onClose(id);
            }}
            style={tabStyle}
          >
            <span
              style={{
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {label}
            </span>
            <button
              type="button"
              aria-label={`Close ${label}`}
              onClick={(e) => {
                e.stopPropagation();
                onClose(id);
              }}
              style={{
                all: "unset",
                cursor: "pointer",
                width: 16,
                height: 16,
                borderRadius: 4,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                color: color.muted2,
              }}
            >
              <Icon name="close" size={10} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
