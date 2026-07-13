// The shared per-suite harness behind the simnode.*.scenario suites: the REAL
// DucktapeProvider over the REAL remoteTransport (http + ws) against a spawned
// deterministic sim node (bin/simnode), with block production under test
// control. Each suite calls useSimScenario() once inside its describe block;
// the helper registers the ws stub and per-test teardown, and hands back
// boot/state/actions accessors. The captured probe lives at module scope, but
// vitest isolates modules per test FILE, so suites never share a probe.
//
// Extracted from simnode.scenario.test.tsx when the scenario lane grew from
// one suite (the provider core) to one per module surface.

import { cleanup, render, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterAll, afterEach, beforeAll, expect, vi } from "vitest";
import { WebSocket as WsWebSocket } from "ws";

import { DucktapeProvider } from "../console/store/DucktapeProvider";
import { useDucktape } from "../console/store/use-ducktape";
import { remoteTransport } from "../domain/transport";
import type { NodeTransport } from "../domain/transport";
import {
  spawnSimnode,
  type SimControl,
  type SimNode,
  type SimSpawnOptions,
} from "./simnode-harness";

type Captured = ReturnType<typeof useDucktape>;

let capturedState: Captured["state"] | null = null;
let capturedActions: Captured["actions"] | null = null;

function Probe() {
  const { state, actions } = useDucktape();
  capturedState = state;
  capturedActions = actions;
  return null;
}

export interface SimScenario {
  /** Spawn a fresh sim + provider; resolves once the store is connected. The
   *  `base` is the sim's http origin — a few scenarios re-dial it (connectRemote)
   *  to exercise a store path that needs a resolved node url. */
  boot(
    options?: SimSpawnOptions,
    ui?: ReactNode,
  ): Promise<{ sim: SimControl; transport: NodeTransport; base: string }>;
  /** The live store state as of the LAST render — re-read after every await. */
  state(): Captured["state"];
  actions(): Captured["actions"];
}

/** Call once at the top of a describe block. */
export const useSimScenario = (): SimScenario => {
  let live: SimNode | null = null;

  // vitest's jsdom env leaves undici's WebSocket as the global while Event is
  // jsdom's — undici's own dispatchEvent then brand-fails across realms
  // (uncaught "must be an instance of Event"). The `ws` client is
  // callback-based (no EventTarget dispatch), so it is realm-safe here, and
  // remoteTransport only assigns onmessage/onclose/onerror, which ws supports.
  beforeAll(() => {
    vi.stubGlobal("WebSocket", WsWebSocket as unknown as typeof WebSocket);
  });
  afterAll(() => {
    vi.unstubAllGlobals();
  });

  afterEach(async () => {
    cleanup();
    vi.useRealTimers();
    capturedState = null;
    capturedActions = null;
    await live?.stop();
    live = null;
  });

  const boot = async (options: SimSpawnOptions = {}, ui: ReactNode = null) => {
    live = await spawnSimnode(options);
    const transport = remoteTransport(live.base);
    render(
      <DucktapeProvider transport={transport}>
        <Probe />
        {ui}
      </DucktapeProvider>,
    );
    await waitFor(() => expect(capturedState?.connected).toBe(true), {
      timeout: 15_000,
    });
    return { sim: live.sim, transport, base: live.base };
  };

  return {
    boot,
    state: () => {
      expect(capturedState).not.toBeNull();
      return capturedState!;
    },
    actions: () => {
      expect(capturedActions).not.toBeNull();
      return capturedActions!;
    },
  };
};
