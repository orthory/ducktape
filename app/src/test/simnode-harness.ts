// Spawn/drive harness for `ducktape-simnode` — the deterministic scenario
// node (bin/simnode). It serves noded's real /v1 http+ws wire, but blocks
// commit only when this harness says so: submits park until `sim.step()`,
// `sim.peerBlock()` lands a concurrent writer's block, personas reshape the
// wire between the local daemon (receipts carry opHash, empty ring) and the
// networked validator (height-only receipts, populated ring).
//
// The binary comes from DUCKTAPE_SIMNODE_BIN or the workspace's
// target/{debug,release} (cargo build -p simnode). Without one, suites should
// skip visibly via `simnodeBinary()` — the same contract as live-daemon.e2e.

import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// ── Types ───────────────────────────────────────────────

export type SimPersona = "local" | "networked";

export interface SimSnapshot {
  height: number;
  held: number;
  oracleQueued: number;
  auto: boolean;
  persona: SimPersona;
}

export interface SimCommitted {
  height: number;
  appHash: string;
  opHash: string;
  target: string;
  kind: "held" | "oracle" | "peer";
}

export type StepReport = SimSnapshot & { committed: SimCommitted | null };

export interface SimControl {
  /** Commit exactly one queued op (oracle follow-ups drain before held submits). */
  step(): Promise<StepReport>;
  setAuto(enabled: boolean): Promise<SimSnapshot>;
  setPersona(persona: SimPersona): Promise<SimSnapshot>;
  /** Commit a concurrent writer's block, independent of the held queue. */
  peerBlock(target: string, payload: unknown, origin?: string): Promise<SimCommitted>;
  state(): Promise<SimSnapshot>;
}

export interface SimNode {
  /** http base of the /v1 wire — hand it to remoteTransport(). */
  base: string;
  sim: SimControl;
  stop(): Promise<void>;
}

export interface SimSpawnOptions {
  persona?: SimPersona;
  auto?: boolean;
  echoOracle?: boolean;
  /** Fabricate a mesh identity: `status.publicKey` serves this 64-hex ed25519
   *  key (no mesh behind it). Required by consensus-op scenarios that name a
   *  node key (huddle membership, account writes keyed off `status.publicKey`). */
  nodeKey?: string;
}

// ── Binary discovery ────────────────────────────────────

export const simnodeBinary = (): string | null => {
  const fromEnv = process.env.DUCKTAPE_SIMNODE_BIN;
  if (fromEnv) {
    // an explicit path is a promise the caller made — a broken one must
    // FAIL, never skip.
    if (!existsSync(fromEnv)) {
      throw new Error(`DUCKTAPE_SIMNODE_BIN is set but does not exist: ${fromEnv}`);
    }
    return fromEnv;
  }
  const here = dirname(fileURLToPath(import.meta.url)); // app/src/test
  for (const profile of ["debug", "release"]) {
    const candidate = resolve(here, "../../..", "target", profile, "ducktape-simnode");
    if (existsSync(candidate)) return candidate;
  }
  return null;
};

// ── Spawn ───────────────────────────────────────────────

const freePort = (): Promise<number> =>
  new Promise((done, fail) => {
    const probe = createServer();
    probe.once("error", fail);
    probe.listen(0, "127.0.0.1", () => {
      const { port } = probe.address() as { port: number };
      probe.close(() => done(port));
    });
  });

const controlPost = <T>(base: string, path: string, body?: unknown): Promise<T> =>
  Promise.resolve()
    .then(() =>
      fetch(`${base}${path}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: body === undefined ? undefined : JSON.stringify(body),
      }),
    )
    .then(async (res) => {
      if (!res.ok) throw new Error(`sim ${path} replied ${res.status}: ${await res.text()}`);
      return res.json() as Promise<T>;
    });

const controlOf = (base: string): SimControl => ({
  step: () => controlPost<StepReport>(base, "/sim/step"),
  setAuto: (enabled) => controlPost<SimSnapshot>(base, "/sim/auto", { enabled }),
  setPersona: (persona) => controlPost<SimSnapshot>(base, "/sim/persona", { persona }),
  peerBlock: (target, payload, origin) =>
    controlPost<SimCommitted>(base, "/sim/peer-block", { target, payload, origin }),
  state: () =>
    Promise.resolve()
      .then(() => fetch(`${base}/sim/state`))
      .then((res) => res.json() as Promise<SimSnapshot>),
});

/** Spawn a fresh sim on a free port with its own storage dir; resolves once
 *  /v1/status answers (genesis done). Callers own stop(). */
export const spawnSimnode = (options: SimSpawnOptions = {}): Promise<SimNode> => {
  const bin = simnodeBinary();
  if (!bin) {
    return Promise.reject(
      new Error("ducktape-simnode not built — cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN"),
    );
  }
  let child: ChildProcess;
  let storage: string;
  return Promise.resolve()
    .then(freePort)
    .then((port) => {
      const base = `http://127.0.0.1:${port}`;
      // explicit storage: the sim has no height-resume watermark, so a fresh
      // dir per spawn is part of its determinism contract.
      storage = mkdtempSync(join(tmpdir(), "ducktape-simnode-"));
      const args = ["--listen", `127.0.0.1:${port}`, "--storage", storage];
      if (options.auto) args.push("--auto");
      if (options.persona) args.push("--persona", options.persona);
      if (options.echoOracle) args.push("--echo-oracle");
      if (options.nodeKey) args.push("--node-key", options.nodeKey);
      child = spawn(bin, args, {
        // stderr stays visible so a startup failure reads as itself, not as
        // a readiness timeout.
        stdio: ["ignore", "ignore", "inherit"],
      });
      return awaitReady(base).then(() => base);
    })
    .then((base) => ({
      base,
      sim: controlOf(base),
      stop: () =>
        Promise.resolve()
          .then(() => fetch(`${base}/v1/admin/shutdown`, { method: "POST" }))
          .catch(() => undefined) // already gone
          .then(() => {
            child.kill();
            rmSync(storage, { recursive: true, force: true });
          }),
    }));
};

const awaitReady = (base: string): Promise<void> => {
  const deadline = Date.now() + 30_000;
  const probe = (): Promise<void> =>
    Promise.resolve()
      .then(() => fetch(`${base}/v1/status`))
      .then((res) => {
        if (res.ok) return;
        throw new Error(`status ${res.status}`);
      })
      .catch(() => {
        if (Date.now() > deadline) throw new Error("simnode never became ready");
        return new Promise<void>((r) => setTimeout(r, 100)).then(probe);
      });
  return probe();
};
