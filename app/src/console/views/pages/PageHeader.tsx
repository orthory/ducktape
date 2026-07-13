// The document header: the TRUE breadcrumb, plus the page comment affordance.
//
// It used to print a hardcoded "Pages / <title>" — a page nested three deep
// looked exactly like a top-level one, and no segment went anywhere. The chain
// comes from PageMeta.parent (page-tree.ancestorChain); every ancestor is a
// button that opens that page.

import type { CSSProperties } from "react";

import type { PageMeta } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";
import type { CommentAnchor } from "./CommentCard";

export function PageHeader({
  chain,
  onOpen,
  onComment,
}: {
  /** Root-first ancestry INCLUDING the open page, or empty with no page open. */
  chain: PageMeta[];
  onOpen: (pageId: string) => void;
  onComment: (anchor: CommentAnchor) => void;
}) {
  return (
    <header
      style={{
        height: 56,
        flexShrink: 0,
        display: "flex",
        alignItems: "center",
        gap: 6,
        padding: "0 22px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.paper,
      }}
    >
      <nav
        aria-label="Breadcrumb"
        style={{ minWidth: 0, display: "flex", alignItems: "center", gap: 6 }}
      >
        <div style={{ font: `600 15px ${font.sans}`, color: color.dark, flexShrink: 0 }}>Pages</div>
        {chain.map((page, i) => {
          const current = i === chain.length - 1;
          return (
            <div
              key={page.id}
              style={{ minWidth: 0, display: "flex", alignItems: "center", gap: 6 }}
            >
              <span style={{ color: color.muted2 }}>/</span>
              {/* no aria-label: the accessible name IS the title, which keeps
                  it distinct from the rail's "Open <title>" buttons. */}
              <button
                type="button"
                aria-current={current ? "page" : undefined}
                disabled={current}
                onClick={() => onOpen(page.id)}
                style={{
                  all: "unset",
                  cursor: current ? "default" : "pointer",
                  minWidth: 0,
                  maxWidth: 220,
                  font: `500 13px ${font.sans}`,
                  color: current ? color.ink : color.muted3,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {page.title || "Untitled"}
              </button>
            </div>
          );
        })}
      </nav>

      {chain.length > 0 ? (
        <div style={{ marginLeft: "auto", display: "flex", gap: 8, flexShrink: 0 }}>
          <button
            type="button"
            aria-label="Comment on page"
            onClick={(event) => {
              const rect = event.currentTarget.getBoundingClientRect();
              onComment({ x: rect.left, y: rect.bottom });
            }}
            style={headerBtn}
          >
            <Icon name="chat" size={15} strokeWidth={1.9} /> Comment
          </button>
        </div>
      ) : (
        <div
          style={{ marginLeft: "auto", font: `500 11px ${font.mono}`, color: color.muted2 }}
        >
          no page open
        </div>
      )}
    </header>
  );
}

const headerBtn: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  gap: 5,
  minHeight: 30,
  padding: "4px 11px",
  borderRadius: radius.sm,
  border: `1px solid ${color.border}`,
  background: color.paper,
  color: color.muted3,
  font: `600 12px ${font.sans}`,
};
