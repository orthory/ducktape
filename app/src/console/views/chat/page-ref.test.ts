// The `[[page:<id>]]` grammar must accept exactly what the runs module accepts
// — a chip the console shows but an agent's injected context skips (or the
// reverse) misrepresents what the agent actually read. Every rejection case
// here is lifted from crates/apps/runs/src/inject.rs's own tests.

import { describe, expect, it } from "vitest";

import type { PageMeta } from "../../../domain/pages-client";
import {
  insertPageRef,
  pageRefCandidates,
  pageRefTokenAt,
  parsePageRefs,
  splitPageRefs,
} from "./page-ref";

describe("parsePageRefs — parity with the runs module", () => {
  it("parses refs in first-seen order, deduped", () => {
    expect(parsePageRefs("see [[page:plan]] and [[page:spec]] and [[page:plan]] again")).toEqual([
      "plan",
      "spec",
    ]);
  });

  // inject.rs: malformed_page_refs_are_skipped_never_a_failure
  it.each([
    ["empty id", "[[page:]]"],
    ["whitespace in the id", "[[page:has space]]"],
    ["unterminated", "[[page:unterminated"],
    ["a bracket inside the id", "[[page:a]b]]"],
    ["the wrong opener", "[page:not-a-ref]]"],
  ])("skips %s", (_why, text) => {
    expect(parsePageRefs(text)).toEqual([]);
  });

  it("keeps scanning past a malformed ref", () => {
    expect(parsePageRefs("[[page:a]b]] trailing [[page:ok]] ok")).toEqual(["ok"]);
  });

  it("stops at an unterminated open — nothing after it can close", () => {
    expect(parsePageRefs("[[page:unterminated [[page:ok]]")).toEqual([]);
  });
});

describe("splitPageRefs", () => {
  it("interleaves literal runs and refs, in order", () => {
    expect(splitPageRefs("see [[page:plan]] now")).toEqual([
      { text: "see " },
      { pageId: "plan" },
      { text: " now" },
    ]);
  });

  it("leaves a malformed ref verbatim in the literal run", () => {
    expect(splitPageRefs("a [[page:]] b")).toEqual([{ text: "a [[page:]] b" }]);
  });

  it("round-trips the source text", () => {
    const source = "x [[page:a]] y [[page:bad id]] z [[page:b]]";
    const rebuilt = splitPageRefs(source)
      .map((segment) => ("pageId" in segment ? `[[page:${segment.pageId}]]` : segment.text))
      .join("");
    expect(rebuilt).toBe(source);
  });

  it("has no literal segments to render for a bare ref", () => {
    expect(splitPageRefs("[[page:solo]]")).toEqual([{ pageId: "solo" }]);
  });
});

describe("pageRefTokenAt", () => {
  it("opens on `[[` and takes everything after it as the query", () => {
    expect(pageRefTokenAt("link [[", 7)).toEqual({ start: 5, query: "" });
    expect(pageRefTokenAt("link [[pla", 10)).toEqual({ start: 5, query: "pla" });
  });

  // The whole reason this isn't `mentionTokenAt`: a page title has spaces.
  it("does NOT close on whitespace — titles have spaces in them", () => {
    expect(pageRefTokenAt("[[launch plan", 13)).toEqual({ start: 0, query: "launch plan" });
  });

  it("closes on `]` (the ref is already complete) and on a newline", () => {
    expect(pageRefTokenAt("[[page:plan]] and", 17)).toBeNull();
    expect(pageRefTokenAt("[[plan\nnext", 11)).toBeNull();
  });

  it("takes the nearest opener and reads only up to the caret", () => {
    expect(pageRefTokenAt("[[a and [[b", 11)).toEqual({ start: 8, query: "b" });
    expect(pageRefTokenAt("[[plan", 4)).toEqual({ start: 0, query: "pl" });
  });

  it("is null with no `[[` at all", () => {
    expect(pageRefTokenAt("just text", 9)).toBeNull();
  });
});

describe("insertPageRef", () => {
  it("replaces the typed fragment with the full ref and lands the caret after it", () => {
    const token = pageRefTokenAt("see [[launch", 12)!;
    expect(insertPageRef("see [[launch", token, 12, "pg-7")).toEqual({
      text: "see [[page:pg-7]] ",
      caret: 18,
    });
  });

  it("keeps the text after the caret", () => {
    const token = pageRefTokenAt("see [[la", 8)!;
    expect(insertPageRef("see [[la and more", token, 8, "pg-7").text).toBe(
      "see [[page:pg-7]]  and more",
    );
  });
});

describe("pageRefCandidates", () => {
  const page = (id: string, title: string): PageMeta => ({ id, title, parent: null });
  const PAGES = [page("p1", "Launch plan"), page("p2", "Roadmap"), page("p3", "Plan B")];

  it("lists everything on a bare `[[`", () => {
    expect(pageRefCandidates(PAGES, "").map((p) => p.id)).toEqual(["p1", "p3", "p2"]);
  });

  it("matches on title, case-insensitively, prefix first", () => {
    expect(pageRefCandidates(PAGES, "plan").map((p) => p.id)).toEqual(["p3", "p1"]);
  });

  it("matches on id too — the id is what the ref actually carries", () => {
    expect(pageRefCandidates(PAGES, "p2").map((p) => p.id)).toEqual(["p2"]);
  });
});
