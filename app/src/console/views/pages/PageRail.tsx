// The Docs rail: the "Pages" header, the New page button, and the nested
// page tree. Owns the tree's collapse state (a pure view preference,
// persisted to localStorage) so the view above stays out of it.

import { useMemo, useState } from "react";
import type { CSSProperties } from "react";

import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";
import { ancestorChain, buildForest } from "./page-tree";
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
  const [query, setQuery] = useState("");
  const visiblePages = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) return pages;
    const visible = new Set(
      pages
        .filter((page) => page.title.toLocaleLowerCase().includes(needle))
        .flatMap((page) => ancestorChain(pages, page.id).map((ancestor) => ancestor.id)),
    );
    return pages.filter((page) => visible.has(page.id));
  }, [pages, query]);
  const forest = useMemo(() => buildForest(visiblePages), [visiblePages]);
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
        width: 224,
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
          height: 52,
          flexShrink: 0,
          padding: "0 14px",
          display: "flex",
          alignItems: "center",
          gap: 9,
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span
          style={{
            width: 24,
            height: 24,
            borderRadius: 7,
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
        <div style={{ font: `600 13px ${font.sans}`, color: color.ink }}>Pages</div>
        <button
          type="button"
          aria-label="New page"
          title="New page"
          onClick={onNewPage}
          style={{
            all: "unset",
            cursor: "pointer",
            marginLeft: "auto",
            width: 26,
            height: 26,
            borderRadius: 6,
            color: color.muted3,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="plus" size={14} strokeWidth={1.9} />
        </button>
        <button
          type="button"
          aria-label="Refresh pages"
          title="Refresh pages"
          onClick={onRefresh}
          style={{
            all: "unset",
            cursor: "pointer",
            width: 26,
            height: 26,
            borderRadius: 6,
            background: "transparent",
            color: color.muted3,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="refresh" size={13} strokeWidth={1.7} />
        </button>
      </div>

      <label
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          height: 32,
          margin: "12px 14px 8px",
          padding: "0 10px",
          border: `1px solid ${color.border}`,
          borderRadius: radius.sm,
          background: color.sunken,
          color: color.muted2,
        }}
      >
        <Icon name="search" size={13} strokeWidth={1.7} />
        <input
          type="search"
          aria-label="Search pages"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search"
          style={{
            minWidth: 0,
            width: "100%",
            color: color.inkSofter,
            font: `400 12.5px ${font.sans}`,
          }}
        />
      </label>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "4px 0 13px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "0 14px 8px" }}>
          <div style={sectionLabelStyle}>Workspace</div>
        </div>
        {forest.length === 0 ? (
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
            {query ? "No pages match this search." : "No pages yet. Use + to start writing."}
          </div>
        ) : (
          <PageTree
            nodes={forest}
            activeId={activePage}
            collapsed={query ? new Set() : collapsed}
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
