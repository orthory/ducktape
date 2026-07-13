import { describe, expect, it, vi } from "vitest";
import {
  addComment,
  createPage,
  deletePage,
  editComment,
  moveCommentThread,
  resolveThread,
  setSpanMark,
  setPageParent,
  threadsForTargets,
} from "./pages-client";
import type { NodeTransport } from "./transport";
import { makeTransportStub } from "../test/transport-stub";

function fakeTransport(sink: unknown[], reply: unknown = {}): NodeTransport {
  return makeTransportStub({
    submit: vi.fn((target: string, payload: unknown) => {
      sink.push({ target, payload });
      return Promise.resolve({ height: 1, opHash: "x" } as never);
    }),
    query: vi.fn((target: string, payload: unknown) => {
      sink.push({ target, payload });
      return Promise.resolve(reply as never);
    }),
    view: vi.fn(() => Promise.resolve({} as never)),
  });
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

describe("pages-client comments", () => {
  it("addComment targets the pages module with a bare target id", async () => {
    const sink: { target: string; payload: unknown }[] = [];
    await addComment(fakeTransport(sink), { threadId: "t1", commentId: "c1", target: "b1", text: "hi" });
    expect(sink[0]).toEqual({
      target: "pages",
      payload: {
        add_comment: {
          thread_id: "t1",
          comment_id: "c1",
          target: "b1",
          text: "hi",
          mentions: [],
        },
      },
    });
  });
  it("persists exact UTF-16 span marks and comment anchors", async () => {
    const sink: { target: string; payload: unknown }[] = [];
    const transport = fakeTransport(sink);
    await setSpanMark(transport, {
      blockId: "b1", start: 1, end: 3, kind: "bold", active: true,
    });
    await addComment(transport, {
      threadId: "t1", commentId: "c1", target: "b1", text: "note", anchor: { start: 1, end: 3 },
    });
    await moveCommentThread(transport, {
      threadId: "t1", target: "b2", anchor: { start: 0, end: 2 },
    });
    expect(sink[0].payload).toEqual({
      set_span_mark: { block_id: "b1", start: 1, end: 3, kind: "bold", active: true },
    });
    expect(sink[1].payload).toEqual({
      add_comment: {
        thread_id: "t1", comment_id: "c1", target: "b1", text: "note",
        anchor: { start: 1, end: 3 }, mentions: [],
      },
    });
    expect(sink[2].payload).toEqual({
      move_comment_thread: {
        thread_id: "t1", target: "b2", anchor: { start: 0, end: 2 },
      },
    });
  });
  it("resolveThread wire shape", async () => {
    const sink: { target: string; payload: unknown }[] = [];
    await resolveThread(fakeTransport(sink), { threadId: "t1", resolved: true });
    expect(sink[0].payload).toEqual({ resolve_thread: { thread_id: "t1", resolved: true } });
  });
  it("editComment carries newly introduced structured mentions", async () => {
    const sink: { target: string; payload: unknown }[] = [];
    await editComment(fakeTransport(sink), {
      commentId: "c1",
      text: "hello @alice",
      mentions: [{ user: [1, 2] }],
    });
    expect(sink[0]).toEqual({
      target: "pages",
      payload: {
        edit_comment: {
          comment_id: "c1",
          text: "hello @alice",
          mentions: [{ user: [1, 2] }],
        },
      },
    });
  });
  it("threadsForTargets decodes the comment_threads reply", async () => {
    const sink: unknown[] = [];
    const reply = { comment_threads: [{ target: "b1", threads: [] }] };
    const out = await threadsForTargets(fakeTransport(sink, reply), { targets: ["b1"] });
    expect(out).toEqual([{ target: "b1", threads: [] }]);
  });
});
