import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { CallTiles } from "./CallTiles";
import type { HuddleParticipant } from "../../store/huddle-roster";

const p = (over: Partial<HuddleParticipant> & { key: string }): HuddleParticipant => ({
  name: over.key.toUpperCase(),
  muted: false,
  stale: false,
  isSelf: false,
  speaking: false,
  user: [1],
  ...over,
});

const common = {
  memberNodes: { self: "nself", ab: "nab" } as Record<string, string>,
  peers: { nab: { muted: false, cameraOn: true, atMs: 0 } },
  canEncode: true,
  canDecode: true,
  selfCameraOn: true,
  bindPreview: vi.fn(),
  bindTile: vi.fn(),
};

describe("CallTiles", () => {
  it("renders the self preview <video> when the self camera is on and encode is available", () => {
    const { container } = render(
      <CallTiles layout="gallery" participants={[p({ key: "self", isSelf: true })]} {...common} />,
    );
    expect(container.querySelector("video")).not.toBeNull();
  });

  it("renders a peer <canvas> only when decode is available and the beacon says camera-on", () => {
    const { container, rerender } = render(
      <CallTiles layout="gallery" participants={[p({ key: "ab" })]} {...common} />,
    );
    expect(container.querySelector("canvas")).not.toBeNull();
    rerender(<CallTiles layout="gallery" participants={[p({ key: "ab" })]} {...common} canDecode={false} />);
    expect(container.querySelector("canvas")).toBeNull();
  });

  it("marks a muted participant's tile with a mute glyph (video surfaces have no roster)", () => {
    const { queryByTitle, rerender } = render(
      <CallTiles layout="gallery" participants={[p({ key: "ab", muted: true })]} {...common} />,
    );
    expect(queryByTitle("Muted")).not.toBeNull();
    rerender(<CallTiles layout="gallery" participants={[p({ key: "ab", muted: false })]} {...common} />);
    expect(queryByTitle("Muted")).toBeNull();
  });

  it("caps a strip and surfaces the overflow tail", () => {
    const many = Array.from({ length: 10 }, (_, i) => p({ key: `u${i}` }));
    const { getByText } = render(
      <CallTiles layout="strip" participants={many} {...common} maxTiles={4} />,
    );
    expect(getByText(/\+6 more not shown/i)).toBeTruthy();
  });
});
