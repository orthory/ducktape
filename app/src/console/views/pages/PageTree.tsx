import { useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { Icon } from "../../components/Icon";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";
import { flattenVisible, subtreeIds } from "./page-tree";
import type { TreeNode } from "./page-tree";

const ROW_INDENT = 14;

/** The nested page tree in the Docs rail: collapsible folders, per-row add
 *  child / delete / move-to. */
export function PageTree({
  nodes,
  activeId,
  collapsed,
  onOpen,
  onToggle,
  onAddChild,
  onDelete,
  onMove,
}: {
  nodes: TreeNode[];
  activeId: string | null;
  collapsed: ReadonlySet<string>;
  onOpen: (id: string) => void;
  onToggle: (id: string) => void;
  onAddChild: (id: string) => void;
  onDelete: (id: string) => void;
  onMove: (id: string, parent: string | null) => void;
}) {
  const rows = flattenVisible(nodes, collapsed);
  const [menuFor, setMenuFor] = useState<string | null>(null);
  const [hoverId, setHoverId] = useState<string | null>(null);

  // The open row's own subtree — the pages it may NOT move under. It was
  // recomputed (a full DFS) inside a filter callback, i.e. once per candidate
  // row, so opening one menu cost O(N²) walks of the whole forest. It is one
  // Set, and only the open menu needs it.
  const forbidden = useMemo(
    () => (menuFor ? subtreeIds(nodes, menuFor) : null),
    [nodes, menuFor],
  );

  return (
    <div role="tree" aria-label="Pages">
      {rows.map((row) => {
        const active = row.id === activeId;
        const isCollapsed = collapsed.has(row.id);
        const menuOpen = menuFor === row.id;
        const moveTargets =
          menuOpen && forbidden ? rows.filter((r) => !forbidden.has(r.id)) : [];
        const rowStyle: CSSProperties = {
          display: "flex",
          alignItems: "center",
          gap: 2,
          boxSizing: "border-box",
          width: "calc(100% - 12px)",
          margin: "1px 6px",
          paddingLeft: 4 + row.depth * ROW_INDENT,
          paddingRight: 4,
          borderRadius: radius.sm,
          background: active ? color.hover : "transparent",
          position: "relative",
        };
        return (
          <div
            key={row.id}
            role="treeitem"
            aria-expanded={row.hasChildren ? !isCollapsed : undefined}
            aria-selected={active}
            style={rowStyle}
            onMouseEnter={() => setHoverId(row.id)}
            onMouseLeave={() => {
              setHoverId((h) => (h === row.id ? null : h));
              setMenuFor((m) => (m === row.id ? null : m));
            }}
          >
            {row.hasChildren ? (
              <button
                type="button"
                aria-label={`${isCollapsed ? "Expand" : "Collapse"} ${row.title}`}
                onClick={() => onToggle(row.id)}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  width: 16,
                  height: 22,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  color: color.muted3,
                  flexShrink: 0,
                }}
              >
                <Icon
                  name="chevronRight"
                  size={12}
                  strokeWidth={2}
                  style={{ transform: `rotate(${isCollapsed ? 0 : 90}deg)` }}
                />
              </button>
            ) : (
              <span style={{ width: 16, flexShrink: 0 }} />
            )}
            <button
              type="button"
              aria-label={`Open ${row.title}`}
              onClick={() => onOpen(row.id)}
              style={{
                all: "unset",
                cursor: "pointer",
                flex: 1,
                minWidth: 0,
                display: "flex",
                alignItems: "center",
                gap: 7,
                padding: "5px 0",
                color: active ? color.ink : color.inkSofter,
              }}
            >
              <Icon
                name="pages"
                size={13}
                strokeWidth={1.7}
                style={{ flexShrink: 0, color: active ? accentVar : color.muted2 }}
              />
              <span
                style={{
                  flex: 1,
                  minWidth: 0,
                  font: active ? `600 12.5px ${font.sans}` : `500 12.5px ${font.sans}`,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {row.title}
              </span>
            </button>
            {hoverId === row.id || menuFor === row.id ? (
              <>
                <button
                  type="button"
                  aria-label={`Add page under ${row.title}`}
                  title="Add subpage"
                  onClick={() => onAddChild(row.id)}
                  style={hoverBtn}
                >
                  <Icon name="plus" size={13} strokeWidth={1.9} />
                </button>
                <button
                  type="button"
                  aria-label={`More actions for ${row.title}`}
                  onClick={() => setMenuFor((m) => (m === row.id ? null : row.id))}
                  style={{ ...hoverBtn, font: `700 13px ${font.sans}` }}
                >
                  ⋯
                </button>
              </>
            ) : null}
            {menuFor === row.id ? (
              <div
                role="menu"
                style={{
                  position: "absolute",
                  zIndex: 30,
                  top: "100%",
                  right: 4,
                  minWidth: 170,
                  maxHeight: 260,
                  overflowY: "auto",
                  border: `1px solid ${color.border}`,
                  borderRadius: radius.md,
                  background: color.paper,
                  boxShadow: shadow.card,
                  padding: 4,
                }}
              >
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    setMenuFor(null);
                    onDelete(row.id);
                  }}
                  style={menuItem}
                >
                  Delete
                </button>
                <div style={{ height: 1, background: color.borderSoft, margin: "4px 2px" }} />
                <div style={{ ...menuItem, cursor: "default", color: color.muted2, font: `600 9px ${font.mono}`, letterSpacing: ".1em", textTransform: "uppercase" }}>
                  Move to
                </div>
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    setMenuFor(null);
                    onMove(row.id, null);
                  }}
                  style={menuItem}
                >
                  Top level
                </button>
                {/* indented by depth: a FLAT list of every title said nothing
                    about where a page would actually land, and two pages with
                    the same name were indistinguishable. */}
                {moveTargets.map((t) => (
                  <button
                    key={t.id}
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setMenuFor(null);
                      onMove(row.id, t.id);
                    }}
                    style={{ ...menuItem, paddingLeft: 9 + t.depth * 11 }}
                  >
                    {t.title}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

const hoverBtn: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  width: 22,
  height: 22,
  borderRadius: 5,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  color: color.muted2,
  flexShrink: 0,
};

const menuItem: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  display: "block",
  width: "100%",
  boxSizing: "border-box",
  padding: "6px 9px",
  borderRadius: radius.sm,
  font: `500 12px ${font.sans}`,
  color: color.ink,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};
