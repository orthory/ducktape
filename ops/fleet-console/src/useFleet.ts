import { useEffect, useState } from "react";
import type { Fleet } from "./types";

export interface FleetState {
  fleet: Fleet | null;
  error: string | null;
  loading: boolean;
}

// Poll /fleet.json (written by ops/fleet.sh into the served dir). Layout-agnostic
// on purpose: the grid today and a future xyflow graph consume the same state.
export function useFleet(intervalMs = 5000): FleetState {
  const [state, setState] = useState<FleetState>({
    fleet: null,
    error: null,
    loading: true,
  });

  useEffect(() => {
    let live = true;
    const tick = async () => {
      try {
        const res = await fetch("./fleet.json", { cache: "no-store" });
        if (!res.ok) throw new Error(`fleet.json ${res.status}`);
        const fleet = (await res.json()) as Fleet;
        if (live) setState({ fleet, error: null, loading: false });
      } catch (err) {
        if (live)
          setState((s) => ({
            fleet: s.fleet,
            error: err instanceof Error ? err.message : String(err),
            loading: false,
          }));
      }
    };
    tick();
    const iv = setInterval(tick, intervalMs);
    return () => {
      live = false;
      clearInterval(iv);
    };
  }, [intervalMs]);

  return state;
}
