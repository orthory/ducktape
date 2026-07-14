// @vitest-environment node
//
// The conformance lane: one op script through the REAL remoteTransport against
// BOTH nodes — a spawned `ducktape-noded` and a `ducktape-simnode --auto` —
// diffing the normalized observable traces. App-hash parity is impossible BY
// DESIGN (noded stamps wall-clock consensus_time, and modules commit
// timestamps into their roots), so hashes are shape-checked, never compared.
// What must match is the wire itself: receipt shape, query replies minus
// time-valued fields, the status module list, and block rows minus the
// time-tainted commit digest. A wire change on either side fails HERE, which
// is what keeps the sim honest as a stand-in for the daemon.
//
// Skips (visibly) unless BOTH binaries are built:
// cargo build -p noded -p simnode.

import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { remoteTransport } from "../domain/transport";
import type { NodeTransport, SubmitReceipt } from "../domain/transport";
import { simnodeBinary, spawnSimnode, type SimNode } from "./simnode-harness";

// ── noded discovery (live-daemon.e2e's exact contract) ──

const nodedBinary = (): string | null => {
  const fromEnv = process.env.DUCKTAPE_NODED_BIN;
  if (fromEnv) {
    // an explicit path is a promise the caller made — a broken one must
    // FAIL, never skip.
    if (!existsSync(fromEnv)) {
      throw new Error(`DUCKTAPE_NODED_BIN is set but does not exist: ${fromEnv}`);
    }
    return fromEnv;
  }
  const here = dirname(fileURLToPath(import.meta.url)); // app/src/test
  for (const profile of ["debug", "release"]) {
    const candidate = resolve(here, "../../..", "target", profile, "ducktape-noded");
    if (existsSync(candidate)) return candidate;
  }
  return null;
};

const freePort = (): Promise<number> =>
  new Promise((done, fail) => {
    const probe = createServer();
    probe.once("error", fail);
    probe.listen(0, "127.0.0.1", () => {
      const { port } = probe.address() as { port: number };
      probe.close(() => done(port));
    });
  });

const nodedBin = nodedBinary();
const simBin = simnodeBinary();
if (!nodedBin || !simBin) {
  console.warn(
    "[sim-parity] needs both binaries — skipping (cargo build -p noded -p simnode)",
  );
}

// ── Normalization ───────────────────────────────────────
//
// The one honest divergence between the nodes is time: noded's consensus_time
// is wall clock, the sim's is a logical clock, and modules commit it into
// rows (sent_at, created_at, …). Time-valued fields are therefore MASKED —
// not removed, so a side that stops sending one still fails the diff.

const HEX64 = /^[0-9a-f]{64}$/;
const TIME_KEY = /_at$|_ms$|_time$/;

const normalize = (value: unknown, mask: ReadonlySet<string>): unknown => {
  if (Array.isArray(value)) return value.map((entry) => normalize(entry, mask));
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>).map(([key, entry]) => {
      if (TIME_KEY.test(key)) return [key, "«time»"];
      if (mask.has(key)) {
        // masked digests still owe the wire their shape.
        expect(entry, `${key} must stay 64-hex`).toMatch(HEX64);
        return [key, "«hex64»"];
      }
      return [key, normalize(entry, mask)];
    }),
  );
};

const NO_MASK = new Set<string>();

// ── The script — identical against both nodes ───────────

const SCRIPT: { target: string; payload: unknown; origin: string }[] = [
  {
    target: "chat",
    payload: {
      create_channel: { channel_id: "general", name: "General", post_policy: "open" },
    },
    origin: "parity-owner",
  },
  {
    target: "chat",
    payload: {
      post_message: {
        channel_id: "general",
        message_id: "m-1",
        blocks: [{ paragraph: [{ text: "hello parity", marks: [] }] }],
        thread: null,
        as_agent: null,
      },
    },
    origin: "parity-eddy",
  },
  {
    target: "tasks",
    payload: { create_task: { task_id: "t-1", title: "parity" } },
    origin: "parity-owner",
  },
];

const QUERIES: { target: string; query: unknown }[] = [
  { target: "chat", query: "channels" },
  { target: "chat", query: { messages_latest: { channel_id: "general", limit: 16 } } },
  { target: "tasks", query: "list" },
];

describe.skipIf(!nodedBin || !simBin)("noded ↔ simnode wire conformance", () => {
  let daemon: ChildProcess;
  let daemonStorage: string;
  let nodedBase: string;
  let noded: NodeTransport;
  let sim: SimNode;
  let simTransport: NodeTransport;
  let nodedReceipts: SubmitReceipt[];
  let simReceipts: SubmitReceipt[];

  beforeAll(async () => {
    const port = await freePort();
    nodedBase = `http://127.0.0.1:${port}`;
    daemonStorage = mkdtempSync(join(tmpdir(), "ducktape-parity-noded-"));
    daemon = spawn(nodedBin!, ["--listen", `127.0.0.1:${port}`, "--storage", daemonStorage], {
      // stderr stays visible so a startup failure reads as itself, not as a
      // readiness timeout.
      stdio: ["ignore", "ignore", "inherit"],
    });
    const deadline = Date.now() + 30_000;
    for (;;) {
      try {
        const res = await fetch(`${nodedBase}/v1/status`);
        if (res.ok) break;
      } catch {
        // not up yet
      }
      if (Date.now() > deadline) throw new Error("noded never became ready");
      await new Promise((r) => setTimeout(r, 150));
    }
    noded = remoteTransport(nodedBase);

    // --auto: every submit commits inline, i.e. noded's behavior — the whole
    // point here is the two nodes answering the same script the same way.
    sim = await spawnSimnode({ auto: true });
    simTransport = remoteTransport(sim.base);

    const run = async (transport: NodeTransport): Promise<SubmitReceipt[]> => {
      const receipts: SubmitReceipt[] = [];
      for (const op of SCRIPT) {
        receipts.push(await transport.submit(op.target, op.payload, op.origin));
      }
      return receipts;
    };
    nodedReceipts = await run(noded);
    simReceipts = await run(simTransport);
  }, 60_000);

  afterAll(async () => {
    try {
      await fetch(`${nodedBase}/v1/admin/shutdown`, { method: "POST" });
    } catch {
      // already gone
    }
    daemon?.kill();
    if (daemonStorage) rmSync(daemonStorage, { recursive: true, force: true });
    await sim?.stop();
  });

  it("submit receipts match: same fields, same heights, same content address", () => {
    expect(simReceipts.length).toBe(nodedReceipts.length);
    nodedReceipts.forEach((fromNoded, index) => {
      const fromSim = simReceipts[index];
      // shape: the exact field set, on both sides.
      expect(Object.keys(fromSim).sort()).toEqual(Object.keys(fromNoded).sort());
      expect(fromSim.height).toBe(fromNoded.height);
      expect(fromNoded.appHash).toMatch(HEX64);
      expect(fromSim.appHash).toMatch(HEX64);
      // the content address is sha256 of the committed payload bytes — the
      // SAME bytes on both nodes, so this one digest must be EQUAL, not just
      // shaped: it is what makes op hashes portable between sim and daemon.
      expect(fromSim.opHash).toBe(fromNoded.opHash);
      expect(fromNoded.opHash).toMatch(HEX64);
    });
  });

  it("query replies match once time-valued fields are masked", async () => {
    for (const { target, query } of QUERIES) {
      const fromNoded = await noded.query(target, query);
      const fromSim = await simTransport.query(target, query);
      expect(normalize(fromSim, NO_MASK)).toEqual(normalize(fromNoded, NO_MASK));
    }
  });

  it("status serves the same module list and height", async () => {
    const fromNoded = await noded.status();
    const fromSim = await simTransport.status();
    expect(fromSim.height).toBe(fromNoded.height);
    expect(fromSim.version).toBe(fromNoded.version);
    expect(fromSim.modules.map((m) => m.id)).toEqual(fromNoded.modules.map((m) => m.id));
    expect(fromSim.modules.map((m) => m.category)).toEqual(
      fromNoded.modules.map((m) => m.category),
    );
    // roots commit module timestamps, so they can only be shape-checked.
    [...fromNoded.modules, ...fromSim.modules].forEach((module) =>
      expect(module.root).toMatch(HEX64),
    );
    expect(fromNoded.appHash).toMatch(HEX64);
    expect(fromSim.appHash).toMatch(HEX64);
  });

  it("block rows match minus the time-tainted commit digest", async () => {
    const fromNoded = await noded.blocks();
    const fromSim = await simTransport.blocks();
    const mask = new Set(["commitHash"]);
    expect(normalize(fromSim, mask)).toEqual(normalize(fromNoded, mask));
  });
});
