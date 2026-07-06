// The capability client mirrors the capability module's read surface: the `All`
// query (bare-string unit variant) and its node->tags reply, flattened to the
// distinct executor tags the "Runs on" picker offers. Announcing is host
// policy, never a client act, so there is no msg half to prove.

import { describe, expect, it, vi } from "vitest";

import { capabilities } from "./capability-client";
import type { NodeTransport } from "./transport";

const stubTransport = (reply?: unknown): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  query: vi.fn().mockResolvedValue(reply),
  view: vi.fn(),
  putBlob: vi.fn().mockResolvedValue("ab".repeat(32)),
  getBlob: vi.fn().mockResolvedValue(new Uint8Array()),
  status: vi.fn(),
  metrics: vi.fn(),
  blocks: vi.fn(),
  onBlock: vi.fn(),
});

describe("capabilities", () => {
  it("sends the bare string All and flattens node->tags to a sorted, deduped list", async () => {
    const transport = stubTransport({
      all: [
        [[1, 2], ["codex", "claude"]],
        [[3, 4], ["claude", "ollama"]],
      ],
    });
    await expect(capabilities(transport)).resolves.toEqual(["claude", "codex", "ollama"]);
    expect(transport.query).toHaveBeenCalledWith("capability", "all");
  });

  it("reads an empty registry as no announced executors", async () => {
    const transport = stubTransport({ all: [] });
    await expect(capabilities(transport)).resolves.toEqual([]);
  });

  it("throws loudly on a reply that is not the All variant", async () => {
    const transport = stubTransport({ providers: [] });
    await expect(capabilities(transport)).rejects.toThrow(/wanted all/);
  });
});
