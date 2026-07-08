// The capability client mirrors the capability module's read surface: the `All`
// query (bare-string unit variant) and its node->tags reply, flattened to the
// distinct executor tags the "Runs on" picker offers. Announcing is host
// policy, never a client act, so there is no msg half to prove.

import { describe, expect, it, vi } from "vitest";

import { capabilities, capabilitiesByNode, providersOf } from "./capability-client";
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
  filesStage: vi.fn(),
  filesCommit: vi.fn(),
  filesStat: vi.fn(),
  filesLs: vi.fn(),
  filesRead: vi.fn(),
  filesHistory: vi.fn(),
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

describe("capabilitiesByNode", () => {
  it("keeps the node key: hex(node) -> its announced tags", async () => {
    const transport = stubTransport({
      all: [
        [[1, 2], ["codex", "claude"]],
        [[3, 4], ["ollama"]],
      ],
    });
    const map = await capabilitiesByNode(transport);
    expect(map.get("0102")).toEqual(["codex", "claude"]);
    expect(map.get("0304")).toEqual(["ollama"]);
    expect(transport.query).toHaveBeenCalledWith("capability", "all");
  });

  it("reads an empty registry as an empty map", async () => {
    const transport = stubTransport({ all: [] });
    expect((await capabilitiesByNode(transport)).size).toBe(0);
  });
});

describe("providersOf", () => {
  it("collapses provider_model_effort tags to one title-cased entry per provider", () => {
    const groups = providersOf([
      "claude",
      "claude_opus_high",
      "claude_fable_low",
      "codex",
      "codex_gpt-5.5_high",
    ]);
    expect(groups.map((g) => g.provider)).toEqual(["claude", "codex"]);
    expect(groups.map((g) => g.label)).toEqual(["Claude", "Codex"]);
    expect(groups[0].tags).toEqual(["claude", "claude_opus_high", "claude_fable_low"]);
    expect(groups[1].tags).toEqual(["codex", "codex_gpt-5.5_high"]);
  });

  it("names distinct models with the effort dropped, not the raw combos", () => {
    const groups = providersOf([
      "codex",
      "codex_gpt-5.5_low",
      "codex_gpt-5.5_xhigh",
      "codex_gpt-5.4-mini_high",
      "codex_gpt-5.3-codex-spark_medium",
    ]);
    // The bare `codex` tag names no model; each model appears once, no effort.
    // Models keep their internal hyphens/dots (only the trailing _effort drops).
    expect(groups[0].models).toEqual(["gpt-5.5", "gpt-5.4-mini", "gpt-5.3-codex-spark"]);
  });

  it("preserves first-seen provider order and dedupes repeated tags", () => {
    const groups = providersOf(["codex", "claude_opus_high", "codex", "claude_opus_high"]);
    expect(groups.map((g) => g.provider)).toEqual(["codex", "claude"]);
    expect(groups[0].tags).toEqual(["codex"]);
    expect(groups[1].models).toEqual(["opus"]);
  });

  it("treats an underscore-free tag as its own model-less provider", () => {
    const groups = providersOf(["gpu", "oracle"]);
    expect(groups.map((g) => g.provider)).toEqual(["gpu", "oracle"]);
    expect(groups.every((g) => g.models.length === 0)).toBe(true);
  });

  it("returns nothing for a node that announced no executors", () => {
    expect(providersOf([])).toEqual([]);
  });
});
