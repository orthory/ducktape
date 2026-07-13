import { afterEach, describe, expect, it, vi } from "vitest";

import { createPagePresenceSession, parsePeerCursor } from "./page-presence";

class FakeSocket {
  static instances: FakeSocket[] = [];
  readyState = 0;
  sent: string[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;

  constructor(readonly url: string) {
    FakeSocket.instances.push(this);
  }
  send(value: string) {
    this.sent.push(value);
  }
  open() {
    this.readyState = 1;
    this.onopen?.();
  }
  receive(value: unknown) {
    this.onmessage?.({ data: value });
  }
  close() {
    this.readyState = 3;
    this.onclose?.();
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
  FakeSocket.instances = [];
});

describe("Pages presence wire", () => {
  it("accepts one bounded peer cursor and rejects malformed controls", () => {
    const peer = "ab".repeat(32);
    expect(
      parsePeerCursor(
        { type: "peerCursor", peer, blockId: "b1", anchor: 2, head: 5 },
        123,
      ),
    ).toEqual({ peer, blockId: "b1", anchor: 2, head: 5, atMs: 123 });
    expect(parsePeerCursor({ type: "peerCursor", peer: "nope", blockId: null, anchor: 0, head: 0 })).toBeNull();
    expect(parsePeerCursor({ type: "peerCursor", peer, blockId: "", anchor: 0, head: 0 })).toBeNull();
    expect(parsePeerCursor({ type: "peerCursor", peer, blockId: null, anchor: -1, head: 0 })).toBeNull();
  });

  it("replays recipients/current cursor on open and surfaces peer beacons", () => {
    vi.stubGlobal("WebSocket", FakeSocket);
    const seen = vi.fn();
    const session = createPagePresenceSession(seen);
    session.setRecipients(["AA".repeat(32), "aa".repeat(32)]);
    session.setCursor({ blockId: "b1", anchor: 3, head: 7 });
    session.start("ws://node/v1/presence/ws?page=p1");

    const socket = FakeSocket.instances[0]!;
    socket.open();
    expect(socket.sent.map((value) => JSON.parse(value))).toEqual([
      { type: "recipients", peers: ["aa".repeat(32)] },
      { type: "cursor", blockId: "b1", anchor: 3, head: 7 },
    ]);

    socket.receive(
      JSON.stringify({
        type: "peerCursor",
        peer: "bb".repeat(32),
        blockId: "b2",
        anchor: 1,
        head: 1,
      }),
    );
    expect(seen).toHaveBeenCalledWith(
      expect.objectContaining({ peer: "bb".repeat(32), blockId: "b2", head: 1 }),
    );
    session.stop();
  });
});
