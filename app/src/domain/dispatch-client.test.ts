// The dispatch client mirrors the dispatch module's single-dispatch read. The
// only field this feature consumes is `assignee` (the saga lease holder), so
// the tests pin the query address and the hex projection.

import { describe, expect, it, vi } from "vitest";

import { assigneeHex, dispatch, type DispatchView } from "./dispatch-client";
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

describe("dispatch", () => {
  it("addresses the dispatch under receiver 'runs' by default", async () => {
    const view = { dispatch_id: "d1", assignee: [1, 2] };
    const transport = stubTransport({ dispatch: view });
    await expect(dispatch(transport, { dispatchId: "d1" })).resolves.toEqual(view);
    expect(transport.query).toHaveBeenCalledWith("dispatch", {
      dispatch: { receiver: "runs", dispatch_id: "d1" },
    });
  });

  it("returns null when the dispatch is unknown", async () => {
    const transport = stubTransport({ dispatch: null });
    await expect(dispatch(transport, { dispatchId: "gone" })).resolves.toBeNull();
  });
});

describe("assigneeHex", () => {
  it("hex-encodes the assignee bytes", () => {
    expect(assigneeHex({ assignee: [1, 2] } as DispatchView)).toBe("0102");
  });

  it("is null with no assignee or no view", () => {
    expect(assigneeHex({ assignee: null } as DispatchView)).toBeNull();
    expect(assigneeHex(null)).toBeNull();
  });
});
