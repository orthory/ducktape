import { useState } from "react";
import { useFleet } from "./useFleet";
import { GridView } from "./components/GridView";
import { Drawer } from "./components/Drawer";

export function App() {
  const { fleet, error, loading } = useFleet();
  const [openId, setOpenId] = useState<string | null>(null);

  const nodes = fleet?.worktrees ?? [];
  const live = nodes.filter((n) => n.status === "up").length;
  // Re-derive from the latest poll so an open drawer tracks status changes.
  const openNode = openId ? (nodes.find((n) => n.id === openId) ?? null) : null;

  return (
    <div className="app">
      <header className="topbar">
        <span className="logo">🦆 Fleet QA</span>
        <span className="count">
          {live}/{nodes.length} live
        </span>
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
      ) : (
        <GridView nodes={nodes} onOpen={(n) => setOpenId(n.id)} />
      )}
      {openNode && <Drawer node={openNode} onClose={() => setOpenId(null)} />}
    </div>
  );
}
