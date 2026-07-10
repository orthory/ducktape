// The Notion-style "Subpages" section of an open doc: one row per child page,
// rendered between the title and the block rows. Purely presentational — the
// parent relationship lives in PageMeta.parent, so this needs nothing beyond
// the page enumeration the view already holds. Renders nothing for a leaf.

import type { PageMeta } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";

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
    <section aria-label="Subpages" style={{ margin: "2px 0 14px" }}>
      <div
        style={{
          padding: "2px 0 4px",
          font: `600 10.5px ${font.mono}`,
          letterSpacing: "0.07em",
          textTransform: "uppercase",
          color: color.muted2,
        }}
      >
        Subpages
      </div>
      {children.map((page) => (
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
            gap: 7,
            width: "100%",
            boxSizing: "border-box",
            padding: "4px 6px",
            borderRadius: radius.sm,
            color: color.ink,
            font: `500 13.5px ${font.sans}`,
          }}
          onMouseEnter={(event) => {
            event.currentTarget.style.background = color.hover;
          }}
          onMouseLeave={(event) => {
            event.currentTarget.style.background = "transparent";
          }}
        >
          <Icon name="pages" size={13} strokeWidth={1.7} style={{ color: color.muted3 }} />
          <span
            style={{
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              borderBottom: `1px solid ${color.borderStrong}`,
            }}
          >
            {page.title || "Untitled"}
          </span>
        </button>
      ))}
    </section>
  );
}
