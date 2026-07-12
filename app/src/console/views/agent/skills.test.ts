// The soul's pure half: where an agent's documents live, what a fresh SKILL.md
// says, and what the form is allowed to submit.

import { describe, expect, it } from "vitest";

import {
  cleanPrefix,
  cleanSkills,
  newSkill,
  personaPrefix,
  skillDocPath,
  skillsSummary,
  skillTemplate,
} from "./skills";

describe("skill paths", () => {
  it("puts an agent's persona under its own duckfs folder", () => {
    expect(personaPrefix("triage-agent")).toBe("/shared/agents/triage-agent/persona");
  });

  it("resolves a prefix to the SKILL.md the assembler reads", () => {
    expect(skillDocPath("/shared/skills/release")).toBe("/shared/skills/release/SKILL.md");
  });

  it("normalizes an operator-typed prefix: absolute, no trailing slash", () => {
    expect(cleanPrefix(" shared/skills/release/ ")).toBe("/shared/skills/release");
    expect(cleanPrefix("   ")).toBe("");
  });
});

describe("newSkill", () => {
  it("seeds the persona as an always-loaded skill", () => {
    expect(newSkill("bot", "always")).toEqual({
      name: "persona",
      source_prefix: "/shared/agents/bot/persona",
      load: "always",
    });
  });

  it("defaults a fresh row to on-demand — the cheap mode", () => {
    expect(newSkill("bot").load).toBe("on_demand");
  });
});

describe("skillTemplate", () => {
  it("carries frontmatter the on-demand index can describe the skill from", () => {
    const doc = skillTemplate({ name: "release", displayName: "Bot", load: "on_demand" });
    expect(doc.startsWith("---\nname: release\ndescription: ")).toBe(true);
    expect(doc).toContain("# release");
  });

  it("tells the operator an always-loaded doc reaches every run", () => {
    const doc = skillTemplate({ name: "persona", displayName: "Bot", load: "always" });
    expect(doc).toContain("loaded into every run");
  });
});

describe("cleanSkills", () => {
  it("drops half-typed rows and normalizes what survives", () => {
    expect(
      cleanSkills([
        { name: " persona ", source_prefix: "shared/agents/bot/persona/", load: "always" },
        { name: "", source_prefix: "/shared/agents/bot/", load: "on_demand" },
        { name: "orphan", source_prefix: "  ", load: "on_demand" },
      ]),
    ).toEqual([
      { name: "persona", source_prefix: "/shared/agents/bot/persona", load: "always" },
    ]);
  });

  it("preserves curation order and each row's load mode", () => {
    const curated = cleanSkills([
      { name: "persona", source_prefix: "/a", load: "always" },
      { name: "release", source_prefix: "/b", load: "on_demand" },
    ]);
    expect(curated.map((skill) => [skill.name, skill.load])).toEqual([
      ["persona", "always"],
      ["release", "on_demand"],
    ]);
  });
});

describe("skillsSummary", () => {
  it("says in one line what the agent always carries", () => {
    expect(skillsSummary([])).toBe("none");
    expect(
      skillsSummary([
        { name: "persona", source_prefix: "/a", load: "always" },
        { name: "release", source_prefix: "/b", load: "on_demand" },
        { name: "triage", source_prefix: "/c", load: "on_demand" },
      ]),
    ).toBe("1 always · 2 on demand");
  });
});
