// The dispatch client mirrors the dispatch module's single-dispatch read. The
// only field this feature consumes is `assignee` (the saga lease holder), so
// the tests pin the query address and the hex projection.

import { describe, expect, it, vi } from "vitest";

import { assigneeHex, dispatch, runLease, type DispatchView } from "./dispatch-client";
import { makeTransportStub } from "../test/transport-stub";

const stubTransport = (reply?: unknown) =>
  makeTransportStub({ query: vi.fn().mockResolvedValue(reply) });

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

it("projects the attempt fencing and lease metadata", () => {
  expect(
    runLease({
      assignee: [1, 2],
      attempt: 3,
      max_attempts: 5,
      lease_expires_at: 80,
      deadline: 100,
      lease_updated_at: 40,
      reassignable: true,
    } as DispatchView),
  ).toEqual({
    assigneeHex: "0102",
    attempt: 3,
    maxAttempts: 5,
    expiresAt: 80,
    deadline: 100,
    updatedAt: 40,
    reassignable: true,
  });
});
