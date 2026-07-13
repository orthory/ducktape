import { render, screen, waitFor } from "@testing-library/react";
import { useRef } from "react";
import { describe, expect, it } from "vitest";

import { PagePresenceBar, RemoteCursors, type PagePresencePeer } from "./PagePresence";

const peer: PagePresencePeer = {
  peer: "ab".repeat(32),
  name: "Alice",
  blockId: "b1",
  anchor: 2,
  head: 5,
  atMs: 1,
};

function CursorHarness({ peers }: { peers: PagePresencePeer[] }) {
  const row = useRef<HTMLDivElement | null>(null);
  const area = useRef<HTMLTextAreaElement | null>(null);
  return (
    <div ref={row} style={{ position: "relative" }}>
      <textarea ref={area} defaultValue="hello world" />
      <RemoteCursors peers={peers} areaRef={area} rowRef={row} text="hello world" />
    </div>
  );
}

describe("Pages live presence", () => {
  it("shows the active editor roster by name", () => {
    render(<PagePresenceBar peers={[peer]} />);
    const roster = screen.getByLabelText("1 other editor here");
    expect(roster.getAttribute("title")).toBe("Alice");
  });

  it("renders the peer's named caret beside its text offset", async () => {
    const { container, rerender } = render(<CursorHarness peers={[]} />);
    rerender(<CursorHarness peers={[peer]} />);
    await waitFor(() => expect(screen.getByLabelText("Alice's cursor")).toBeTruthy());
    expect(container.querySelector(`[data-peer-cursor="${peer.peer}"]`)).toBeTruthy();
  });
});
