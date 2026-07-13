// The composer's `[[` page typeahead. Detection (`pageRefTokenAt`) and ranking
// (`pageRefCandidates`) are unchanged; insertion now emits the unified markdown
// ref `[title](duck://page/<id>)` (grammar + tokenizer tests live in
// duck-ref.test.ts).

import { describe, expect, it } from "vitest";

import type { PageMeta } from "../../../domain/pages-client";
import { insertPageRef, pageRefCandidates, pageRefTokenAt } from "./page-ref";

describe("pageRefTokenAt", () => {
  it("opens on `[[` and takes everything after it as the query", () => {
    expect(pageRefTokenAt("link [[", 7)).toEqual({ start: 5, query: "" });
    expect(pageRefTokenAt("link [[pla", 10)).toEqual({ start: 5, query: "pla" });
  });

  // The whole reason this isn't `mentionTokenAt`: a page title has spaces.
  it("does NOT close on whitespace — titles have spaces in them", () => {
    expect(pageRefTokenAt("[[launch plan", 13)).toEqual({ start: 0, query: "launch plan" });
  });

  it("closes on `]` and on a newline", () => {
    expect(pageRefTokenAt("[[a]] and", 9)).toBeNull();
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

describe("insertPageRef — emits the markdown page ref", () => {
  it("replaces the typed fragment with `[title](duck://page/<id>)` and lands the caret after it", () => {
    const token = pageRefTokenAt("see [[launch", 12)!;
    const out = insertPageRef("see [[launch", token, 12, { id: "pg-7", title: "Launch plan" });
    expect(out.text).toBe("see [Launch plan](duck://page/pg-7) ");
    expect(out.caret).toBe(out.text.length);
  });

  it("keeps the text after the caret", () => {
    const token = pageRefTokenAt("see [[la", 8)!;
    expect(
      insertPageRef("see [[la and more", token, 8, { id: "pg-7", title: "Spec" }).text,
    ).toBe("see [Spec](duck://page/pg-7)  and more");
  });

  it("falls back to the id when the title is empty", () => {
    const token = pageRefTokenAt("[[", 2)!;
    expect(insertPageRef("[[", token, 2, { id: "pg-9", title: "" }).text).toBe(
      "[pg-9](duck://page/pg-9) ",
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
