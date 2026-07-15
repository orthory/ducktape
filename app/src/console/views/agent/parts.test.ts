// The agent id the console derives is submitted verbatim to consensus, which
// admits only DNS labels (`validate_agent_id`, crates/apps/agent/src/lib.rs) —
// the id IS the local part of `<agent_id>@agents.duck`. An id this client can
// produce but the node rejects is an opaque failed-op toast, so `slug` must be
// TOTAL: every output is a legal label, or nothing.

import { describe, expect, it } from "vitest";

import { repoFile } from "../../../test/repo-file";
import { MAX_AGENT_ID_LEN, parseCapList, slug } from "./parts";

/** The consensus rule, restated: lowercase [a-z0-9-], 1..=63, no edge hyphen. */
const isLabel = (id: string) =>
  id.length >= 1 && id.length <= MAX_AGENT_ID_LEN && /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(id);

describe("agent id derivation", () => {
  it("only ever yields a legal label or nothing", () => {
    const cases = [
      "Triage Agent",
      "quackbot",
      "  -Lead-  ",
      "QA/Luna",
      "under_score",
      "dot.ted",
      "日本語",
      "!!!",
      "",
      " ",
      "x".repeat(MAX_AGENT_ID_LEN),
      "x".repeat(MAX_AGENT_ID_LEN + 1),
      "x".repeat(200),
      // truncation lands mid-separator: the cut must not leave a trailing hyphen
      `${"a".repeat(MAX_AGENT_ID_LEN - 1)} b c`,
      `${"a".repeat(MAX_AGENT_ID_LEN)} b`,
    ];
    for (const raw of cases) {
      const id = slug(raw);
      expect(id === "" || isLabel(id), `slug(${JSON.stringify(raw)}) = ${id}`).toBe(true);
    }
  });

  it("pins the boundaries", () => {
    expect(slug("x".repeat(MAX_AGENT_ID_LEN))).toHaveLength(MAX_AGENT_ID_LEN);
    expect(slug("x".repeat(MAX_AGENT_ID_LEN + 1))).toHaveLength(MAX_AGENT_ID_LEN);
    expect(slug(`${"a".repeat(MAX_AGENT_ID_LEN - 1)} b c`)).toBe("a".repeat(MAX_AGENT_ID_LEN - 1));
    expect(slug("  -Lead-  ")).toBe("lead");
    expect(slug("Triage Agent")).toBe("triage-agent");
    expect(slug("日本語")).toBe("");
    expect(slug("")).toBe("");
  });

  // The length cap is the one number shared with the node. Read the Rust const
  // so raising it on one side only turns this red.
  it("mirrors the consensus agent-id length cap", () => {
    const lib = repoFile("crates/apps/agent/src/lib.rs");
    const cap = /MAX_AGENT_ID_LEN: usize = (\d+)/.exec(lib);
    expect(cap, "MAX_AGENT_ID_LEN not found in the agent crate").not.toBeNull();
    expect(MAX_AGENT_ID_LEN).toBe(Number(cap![1]));
  });
});

it("parses comma and whitespace separated capability grants", () => {
  expect(parseCapList("alpha, beta\n/shared/data  *")).toEqual([
    "alpha",
    "beta",
    "/shared/data",
    "*",
  ]);
});
