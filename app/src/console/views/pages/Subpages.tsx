// Child pages, rendered as INLINE PAGE BLOCKS in the document's flow — the way
// Notion does it: an icon, the title, the same rhythm and left edge as a block
// row. It used to be a boxed-off "SUBPAGES" section in uppercase mono with
// underlined titles, which read as a widget bolted onto the page rather than
// part of it.
//
// Purely presentational: the parent relationship lives in PageMeta.parent, so
// this needs nothing beyond the page enumeration the view already holds.
// Renders nothing for a leaf.

import type { PageMeta } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";
import { splitTitleEmoji } from "./page-icon";
import { ROW_PAD_Y } from "./pages-style";

export function Subpages({
  pages,
  activePage,
  onOpen,
}: {
  pages: PageMeta[];
  activePage: string;
  onOpen: (id: string) => void;
}) {
  const children = pages.filter((p) => p.parent === activePage);
  if (children.length === 0) return null;
  return (
    <section aria-label="Subpages" style={{ margin: "2px 0 6px" }}>
      {children.map((page) => {
        // the page's icon is the leading emoji of its title (page-icon.ts), so
        // an inline page block shows it exactly as the page itself does.
        const { icon, title } = splitTitleEmoji(page.title);
        const label = title || "Untitled";
        return (
          <button
            key={page.id}
            type="button"
            aria-label={`Open subpage ${page.title || "Untitled"}`}
            onClick={() => onOpen(page.id)}
            style={{
              all: "unset",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 8,
              width: "100%",
              boxSizing: "border-box",
              padding: `${ROW_PAD_Y + 2}px 4px`,
              marginLeft: -4,
              borderRadius: radius.sm,
              color: color.ink,
              font: `500 14.5px/1.6 ${font.sans}`,
            }}
            onMouseEnter={(event) => {
              event.currentTarget.style.background = color.hover;
            }}
            onMouseLeave={(event) => {
              event.currentTarget.style.background = "transparent";
            }}
          >
            {icon ? (
              <span
                aria-hidden="true"
                style={{
                  width: 17,
                  textAlign: "center",
                  font: "14px/1 'Apple Color Emoji', 'Segoe UI Emoji', sans-serif",
                }}
              >
                {icon}
              </span>
            ) : (
              <Icon
                name="pages"
                size={15}
                strokeWidth={1.7}
                style={{ width: 17, flexShrink: 0, color: color.muted2 }}
              />
            )}
            <span
              style={{
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {label}
            </span>
          </button>
        );
      })}
    </section>
  );
}
