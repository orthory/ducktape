import { describe, expect, it } from "vitest";
import { addComment, resolveThread, threadsForAnchors } from "./comments-client";
import type { NodeTransport } from "./transport";

function fake(sink: unknown[], reply: unknown = {}): NodeTransport {
  return {
    submit: (target: string, payload: unknown) => {
      sink.push({ target, payload });
      return Promise.resolve({} as never);
    },
    query: (target: string, payload: unknown) => {
      sink.push({ target, payload });
      return Promise.resolve(reply as never);
    },
    view: () => Promise.resolve({} as never),
  } as unknown as NodeTransport;
}

describe("comments-client", () => {
  it("addComment wire shape", async () => {
    const sink: { target: string; payload: unknown }[] = [];
    await addComment(fake(sink), {
      threadId: "t1",
      commentId: "c1",
      anchor: { module: "pages", target: "b1" },
      text: "hi",
    });
    expect(sink[0]).toEqual({
      target: "comments",
      payload: {
        add_comment: {
          thread_id: "t1",
          comment_id: "c1",
          anchor: { module: "pages", target: "b1" },
          text: "hi",
        },
      },
    });
  });
  it("resolveThread wire shape", async () => {
    const sink: { target: string; payload: unknown }[] = [];
    await resolveThread(fake(sink), { threadId: "t1", resolved: true });
    expect((sink[0] as { payload: unknown }).payload).toEqual({
      resolve_thread: { thread_id: "t1", resolved: true },
    });
  });
  it("threadsForAnchors decodes the anchored reply", async () => {
    const sink: unknown[] = [];
    const reply = { anchored: [{ target: "b1", threads: [] }] };
    const out = await threadsForAnchors(fake(sink, reply), { module: "pages", targets: ["b1"] });
    expect(out).toEqual([{ target: "b1", threads: [] }]);
  });
});
