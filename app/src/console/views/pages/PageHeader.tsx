// The document header: the true, navigable breadcrumb and live presence.
//
// It used to print a hardcoded "Pages / <title>" — a page nested three deep
// looked exactly like a top-level one, and no segment went anywhere. The chain
// comes from PageMeta.parent (page-tree.ancestorChain); every ancestor is a
// button that opens that page.

import type { PageMeta } from "../../../domain/pages-client";
import { color, font } from "../../theme/tokens";
import { PagePresenceBar, type PagePresencePeer } from "./PagePresence";

export function PageHeader({
  chain,
  presence,
  onOpen,
}: {
  /** Root-first ancestry INCLUDING the open page, or empty with no page open. */
  chain: PageMeta[];
  presence: PagePresencePeer[];
  onOpen: (pageId: string) => void;
}) {
  return (
    <header
      style={{
        height: 52,
        flexShrink: 0,
        display: "flex",
        alignItems: "center",
        gap: 6,
        padding: "0 24px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.paper,
      }}
    >
      <nav
        aria-label="Breadcrumb"
        style={{ minWidth: 0, display: "flex", alignItems: "center", gap: 6 }}
      >
        {chain.length === 0 ? (
          <div style={{ font: `600 13px ${font.sans}`, color: color.muted3 }}>Pages</div>
        ) : null}
        {chain.map((page, i) => {
          const current = i === chain.length - 1;
          return (
            <div
              key={page.id}
              style={{ minWidth: 0, display: "flex", alignItems: "center", gap: 6 }}
            >
              {i > 0 ? <span style={{ color: color.muted2 }}>/</span> : null}
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
                  font: `${current ? 600 : 500} 13px ${font.sans}`,
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
        <div style={{ marginLeft: "auto", display: "flex", flexShrink: 0 }}>
          <PagePresenceBar peers={presence} />
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
