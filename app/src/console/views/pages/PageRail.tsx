// The Docs rail: the "Pages" header, the New page button, and the nested
// page tree. Owns the tree's collapse state (a pure view preference,
// persisted to localStorage) so the view above stays out of it.

import { useMemo, useState } from "react";
import type { CSSProperties } from "react";

import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";
import { buildForest } from "./page-tree";
import { PageTree } from "./PageTree";

const TREE_COLLAPSE_KEY = "ducktape.docTreeCollapsed";

const sectionLabelStyle: CSSProperties = {
  font: `600 9px ${font.mono}`,
  letterSpacing: ".11em",
  color: color.muted2,
  textTransform: "uppercase",
};

const loadTreeCollapsed = (): Set<string> => {
  try {
    const raw = localStorage.getItem(TREE_COLLAPSE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(parsed) ? parsed.filter((x) => typeof x === "string") : []);
  } catch {
    return new Set();
  }
};
const saveTreeCollapsed = (set: ReadonlySet<string>): void => {
  try {
    localStorage.setItem(TREE_COLLAPSE_KEY, JSON.stringify([...set]));
  } catch {
    // best-effort
  }
};

export function PageRail({
  pages,
  activePage,
  onNewPage,
  onAddChild,
  onOpen,
  onDelete,
  onMove,
  onRefresh,
}: {
  pages: { id: string; title: string; parent: string | null }[];
  activePage: string | null;
  onNewPage: () => void;
  onAddChild: (id: string) => void;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  onMove: (id: string, parent: string | null) => void;
  onRefresh: () => void;
}) {
  const forest = useMemo(() => buildForest(pages), [pages]);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(loadTreeCollapsed);
  const toggleCollapse = (id: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      saveTreeCollapsed(next);
      return next;
    });
  return (
    <aside
      style={{
        width: 272,
        flexShrink: 0,
        borderRight: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        display: "flex",
        flexDirection: "column",
        color: color.muted3,
      }}
    >
      <div
        style={{
          height: 56,
          flexShrink: 0,
          padding: "0 15px",
          display: "flex",
          alignItems: "center",
          gap: 9,
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span
          style={{
            width: 26,
            height: 26,
            borderRadius: 8,
            background: color.dark,
            color: color.onDark,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="pages" size={14} strokeWidth={1.7} />
        </span>
        <div style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>Pages</div>
        <button
          type="button"
          aria-label="Refresh pages"
          title="Refresh pages"
          onClick={onRefresh}
          style={{
            all: "unset",
            cursor: "pointer",
            marginLeft: "auto",
            width: 26,
            height: 26,
            borderRadius: 6,
            border: `1px solid ${color.border}`,
            background: color.paper,
            color: color.muted3,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="refresh" size={13} strokeWidth={1.7} />
        </button>
      </div>

      <button
        type="button"
        aria-label="New page"
        onClick={onNewPage}
        style={{
          all: "unset",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          gap: 8,
          margin: "12px 12px 6px",
          padding: "8px 10px",
          borderRadius: radius.sm,
          background: color.dark,
          color: color.onDark,
          font: `600 12.5px ${font.sans}`,
        }}
      >
        <Icon name="plus" size={14} strokeWidth={1.9} /> New page
      </button>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "6px 0 13px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "0 14px 8px" }}>
          <div style={sectionLabelStyle}>All pages</div>
        </div>
        {pages.length === 0 ? (
          <div
            style={{
              margin: "7px 14px",
              padding: "13px 12px",
              border: `1px dashed ${color.borderStrong}`,
              borderRadius: radius.md,
              background: color.paper,
              font: `400 12px/1.45 ${font.sans}`,
              color: color.muted2,
            }}
          >
            No pages on this node yet. Create one above to start writing.
          </div>
        ) : (
          <PageTree
            nodes={forest}
            activeId={activePage}
            collapsed={collapsed}
            onOpen={onOpen}
            onToggle={toggleCollapse}
            onAddChild={onAddChild}
            onDelete={onDelete}
            onMove={onMove}
          />
        )}
      </div>
    </aside>
  );
}
