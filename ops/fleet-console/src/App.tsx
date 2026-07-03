import { useCallback, useState } from "react";
import type { FleetNode } from "./types";
import { useFleet } from "./useFleet";
import { GridView } from "./components/GridView";
import { GraphView } from "./components/GraphView";
import { Drawer } from "./components/Drawer";

type ViewMode = "grid" | "graph";

const initialView = (): ViewMode =>
  typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).get("view") === "graph"
    ? "graph"
    : "grid";

export function App() {
  const { fleet, error, loading } = useFleet();
  const [openId, setOpenId] = useState<string | null>(null);
  const [view, setView] = useState<ViewMode>(initialView);

  // Stable so GraphView's memo / RFB tiles don't churn on every poll.
  const onOpen = useCallback((n: FleetNode) => setOpenId(n.id), []);

  const nodes = fleet?.worktrees ?? [];
  const base = fleet?.base ?? "dev";
  const live = nodes.filter((n) => n.status === "up").length;
  const openNode = openId ? (nodes.find((n) => n.id === openId) ?? null) : null;

  return (
    <div className="app">
      <header className="topbar">
        <span className="logo">🦆 Fleet QA</span>
        <span className="count">
          {live}/{nodes.length} live
        </span>
        <div className="toggle" role="tablist">
          <button
            role="tab"
            data-active={view === "grid"}
            onClick={() => setView("grid")}
          >
            Grid
          </button>
          <button
            role="tab"
            data-active={view === "graph"}
            onClick={() => setView("graph")}
          >
            Graph
          </button>
        </div>
        <span className="spacer" />
        {error && <span className="err">fleet.json: {error}</span>}
        {fleet && (
          <span className="ts">
            updated {new Date(fleet.generatedAt).toLocaleTimeString()}
          </span>
        )}
      </header>
      {loading && !fleet ? (
        <div className="empty">loading…</div>
      ) : view === "grid" ? (
        <GridView nodes={nodes} onOpen={onOpen} />
      ) : (
        <GraphView nodes={nodes} base={base} onOpen={onOpen} />
      )}
      {openNode && <Drawer node={openNode} onClose={() => setOpenId(null)} />}
    </div>
  );
}
