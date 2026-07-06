import { describe, expect, it } from "vitest";
import { createPage, setPageParent, deletePage } from "./pages-client";
import type { NodeTransport } from "./transport";

function fakeTransport(sink: unknown[]): NodeTransport {
  return {
    submit: (target: string, payload: unknown) => {
      sink.push({ target, payload });
      return Promise.resolve({ height: 1, opHash: "x" } as never);
    },
    query: () => Promise.resolve({} as never),
    view: () => Promise.resolve({} as never),
  } as unknown as NodeTransport;
}

describe("pages-client nesting", () => {
  it("createPage carries snake_case parent", async () => {
    const sink: { target: string; payload: unknown }[] = [];
    await createPage(fakeTransport(sink), { pageId: "p2", title: "c", parent: "p1" });
    expect(sink[0].payload).toEqual({ create_page: { page_id: "p2", title: "c", parent: "p1" } });
  });
  it("createPage without parent sends null", async () => {
    const sink: { target: string; payload: unknown }[] = [];
    await createPage(fakeTransport(sink), { pageId: "p1", title: "r" });
    expect(sink[0].payload).toEqual({ create_page: { page_id: "p1", title: "r", parent: null } });
  });
  it("setPageParent + deletePage shapes", async () => {
    const sink: { target: string; payload: unknown }[] = [];
    await setPageParent(fakeTransport(sink), { pageId: "p2", parent: null });
    await deletePage(fakeTransport(sink), "p2");
    expect(sink[0].payload).toEqual({ set_page_parent: { page_id: "p2", parent: null } });
    expect(sink[1].payload).toEqual({ delete_page: { page_id: "p2" } });
  });
});
