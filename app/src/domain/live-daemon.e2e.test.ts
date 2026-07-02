// @vitest-environment node
//
// Live-daemon e2e: the app's REAL domain layer — remoteTransport + the typed
// chat/tasks clients — against a REAL spawned `ducktape-noded` process. The
// unit suites pin the TS side of the wire against hand-copied literals; this
// suite closes the loop against the daemon's actual serde output, so a
// Rust-side wire change breaks HERE instead of corrupting blocks in the field.
//
// The daemon binary comes from DUCKTAPE_NODED_BIN or the workspace's
// target/{debug,release}. Without a built binary the suite SKIPS (visibly):
// CI builds the daemon first, so the skip path is local-dev-only.

import { spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  authorName,
  blocksText,
  createChannel,
  channels,
  latestMessages,
  postMessage,
  thread,
} from "./chat-client";
import { createTask, listTasks, updateStatus } from "./tasks-client";
import { remoteTransport } from "./transport";
import type { BlockEvent, NodeTransport } from "./transport";

const binaryPath = (): string | null => {
  const fromEnv = process.env.DUCKTAPE_NODED_BIN;
  if (fromEnv) return existsSync(fromEnv) ? fromEnv : null;
  const here = dirname(fileURLToPath(import.meta.url)); // app/src/domain
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

const bin = binaryPath();
if (!bin) {
  console.warn(
    "[live-daemon.e2e] ducktape-noded not built — skipping (cargo build -p noded, or set DUCKTAPE_NODED_BIN)",
  );
}

describe.skipIf(!bin)("app domain layer against a live daemon", () => {
  let daemon: ChildProcess;
  let base: string;
  let transport: NodeTransport;

  beforeAll(async () => {
    const port = await freePort();
    base = `http://127.0.0.1:${port}`;
    daemon = spawn(bin!, ["--listen", `127.0.0.1:${port}`], {
      stdio: "ignore",
    });
    // readiness = a status answer (the daemon prints before binding, and
    // status only answers once genesis is done) — same rule as the app's
    // own bootstrap probe.
    const deadline = Date.now() + 30_000;
    for (;;) {
      try {
        const res = await fetch(`${base}/v1/status`);
        if (res.ok) break;
      } catch {
        // not up yet
      }
      if (Date.now() > deadline) throw new Error("daemon never became ready");
      await new Promise((r) => setTimeout(r, 150));
    }
    transport = remoteTransport(base);
  }, 45_000);

  afterAll(async () => {
    // retire it the way a real client does — through the wire; the kill is
    // only the backstop for a daemon too wedged to honor its own shutdown.
    try {
      await fetch(`${base}/v1/shutdown`, { method: "POST" });
    } catch {
      // already gone
    }
    daemon?.kill();
  });

  it("reports status for every genesis module", async () => {
    const status = await transport.status();
    expect(status.height).toBeGreaterThanOrEqual(0);
    expect(status.appHash).toMatch(/^[0-9a-f]{64}$/);
    expect(status.modules.map((m) => m.id)).toEqual([
      "chat",
      "tasks",
      "document",
      "forge",
    ]);
  });

  it("drives the chat flow end to end: channel, post, read, thread", async () => {
    await createChannel(transport, {
      channelId: "general",
      name: "General",
      origin: "eddy",
    });
    const posted = await postMessage(transport, {
      channelId: "general",
      messageId: "m1",
      text: "hello from the app domain layer",
      origin: "eddy",
    });
    expect(posted.height).toBeGreaterThan(0);
    expect(posted.appHash).toMatch(/^[0-9a-f]{64}$/);

    const all = await channels(transport);
    expect(all.map((c) => c.id)).toContain("general");

    const messages = await latestMessages(transport, "general", 16);
    expect(messages).toHaveLength(1);
    const view = messages[0];
    expect(blocksText(view.head.blocks)).toBe("hello from the app domain layer");
    // authorship flows from the submit origin through Origin::External into
    // AuthorRef::User — the exact seam a casing drift would corrupt.
    expect(authorName(view.head.author)).toBe("eddy");

    await postMessage(transport, {
      channelId: "general",
      messageId: "m2",
      text: "a threaded reply",
      origin: "jess",
      thread: view.seq,
    });
    const t = await thread(transport, {
      channelId: "general",
      rootSeq: view.seq,
    });
    expect(t).not.toBeNull();
    expect(t!.replies).toHaveLength(1);
    expect(blocksText(t!.replies[0].head.blocks)).toBe("a threaded reply");
    expect(authorName(t!.replies[0].head.author)).toBe("jess");
  });

  it("drives the tasks flow end to end: create, list, update", async () => {
    await createTask(transport, { taskId: "t1", title: "ship the e2e suite" });
    let tasks = await listTasks(transport);
    const created = tasks.find((t) => t.id === "t1");
    expect(created?.title).toBe("ship the e2e suite");
    expect(created?.status).toBe("Open");

    await updateStatus(transport, { taskId: "t1", status: "Done" });
    tasks = await listTasks(transport);
    expect(tasks.find((t) => t.id === "t1")?.status).toBe("Done");
  });

  it("streams committed blocks over the websocket", async () => {
    const seen: BlockEvent[] = [];
    const heard = new Promise<BlockEvent>((done) => {
      const unsubscribe = transport.onBlock((block) => {
        seen.push(block);
        unsubscribe();
        done(block);
      });
    });
    // give the shared socket a beat to connect before committing the block.
    await new Promise((r) => setTimeout(r, 300));
    const committed = await postMessage(transport, {
      channelId: "general",
      messageId: "m3",
      text: "block event probe",
      origin: "eddy",
    });
    const event = await heard;
    expect(event.height).toBeGreaterThanOrEqual(committed.height);
    expect(event.appHash).toMatch(/^[0-9a-f]{64}$/);
  }, 15_000);
});
