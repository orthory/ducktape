// The library's pure half: what a SKILL.md's frontmatter says about a skill, and
// what the picker's search does with a list of them. A document with no
// frontmatter is a skill too — it just has no description.

import { describe, expect, it } from "vitest";

import {
  filterLibrary,
  inLibrary,
  LIBRARY_ROOT,
  libraryPrefix,
  librarySkill,
  parseSkillMeta,
  type LibrarySkill,
} from "./skill-library";

const doc = (body: string) => body.replace(/^\n/, "");

describe("parseSkillMeta", () => {
  it("reads the name and description the assembler's index reads", () => {
    expect(
      parseSkillMeta(
        doc(`
---
name: release
description: Cut a release, tag it, and write the notes.
---

# release
`),
      ),
    ).toEqual({ name: "release", description: "Cut a release, tag it, and write the notes." });
  });

  it("degrades to nulls for a document with no frontmatter — never an error", () => {
    expect(parseSkillMeta("# release\n\njust prose\n")).toEqual({
      name: null,
      description: null,
    });
    expect(parseSkillMeta("")).toEqual({ name: null, description: null });
  });

  it("degrades key-by-key: a half-filled block still yields what it has", () => {
    expect(parseSkillMeta("---\nname: triage\n---\n")).toEqual({
      name: "triage",
      description: null,
    });
    expect(parseSkillMeta("---\ndescription:\nname: triage\n---\n")).toEqual({
      name: "triage",
      description: null,
    });
  });

  it("unquotes values and ignores keys past the closing fence", () => {
    expect(
      parseSkillMeta('---\nname: "release"\ndescription: \'ship it\'\n---\ndescription: body\n'),
    ).toEqual({ name: "release", description: "ship it" });
  });

  it("ignores a body that merely contains dashes, and tolerates CRLF + BOM", () => {
    expect(parseSkillMeta("# release\n---\nname: nope\n")).toEqual({
      name: null,
      description: null,
    });
    expect(parseSkillMeta("﻿---\r\nname: release\r\n---\r\n")).toEqual({
      name: "release",
      description: null,
    });
  });
});

describe("librarySkill", () => {
  it("names a skill from its frontmatter", () => {
    expect(
      librarySkill("/shared/skills/release", "---\nname: Release\ndescription: Ship it.\n---\n"),
    ).toEqual({ prefix: "/shared/skills/release", name: "Release", description: "Ship it." });
  });

  it("falls back to the folder name when the document is missing or bare", () => {
    expect(librarySkill("/shared/skills/triage", null)).toEqual({
      prefix: "/shared/skills/triage",
      name: "triage",
      description: null,
    });
    expect(librarySkill("/shared/skills/triage", "# triage\n").name).toBe("triage");
  });
});

describe("library paths", () => {
  it("publishes a skill as one slug segment under the shared root", () => {
    expect(libraryPrefix("Release Notes")).toBe(`${LIBRARY_ROOT}/release-notes`);
  });

  it("knows a library prefix from an agent-private one", () => {
    expect(inLibrary("/shared/skills/release")).toBe(true);
    expect(inLibrary("/shared/agents/bot/persona")).toBe(false);
    // The root itself is not a skill.
    expect(inLibrary(LIBRARY_ROOT)).toBe(false);
  });
});

describe("filterLibrary", () => {
  const skills: LibrarySkill[] = [
    { prefix: "/shared/skills/release", name: "release", description: "Cut a release." },
    { prefix: "/shared/skills/triage", name: "triage", description: "Sort incoming bugs." },
    { prefix: "/shared/skills/qa", name: "qa", description: null },
  ];

  it("keeps everything for a blank query, in library order", () => {
    expect(filterLibrary(skills, "   ").map((skill) => skill.name)).toEqual([
      "release",
      "triage",
      "qa",
    ]);
  });

  it("matches name, description, or path — case-insensitively", () => {
    expect(filterLibrary(skills, "RELEASE").map((skill) => skill.name)).toEqual(["release"]);
    expect(filterLibrary(skills, "bugs").map((skill) => skill.name)).toEqual(["triage"]);
    expect(filterLibrary(skills, "/shared/skills/qa").map((skill) => skill.name)).toEqual(["qa"]);
  });

  it("requires every term, and never ranks above matching", () => {
    expect(filterLibrary(skills, "sort bugs").map((skill) => skill.name)).toEqual(["triage"]);
    expect(filterLibrary(skills, "sort release")).toEqual([]);
  });

  it("does not crash on a description-less row", () => {
    expect(filterLibrary(skills, "qa").map((skill) => skill.name)).toEqual(["qa"]);
  });
});
