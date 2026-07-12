import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { CallTiles } from "./CallTiles";
import { color } from "../../theme/tokens";
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
  peers: { nab: { muted: false, cameraOn: true, sharing: false, atMs: 0 } },
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

  it("labels a screen-sharing peer's tile and still paints its video", () => {
    const sharingPeers = { nab: { muted: false, cameraOn: false, sharing: true, atMs: 0 } };
    const { container, getByTitle } = render(
      <CallTiles layout="gallery" participants={[p({ key: "ab" })]} {...common} peers={sharingPeers} />,
    );
    // sharing counts as an active video lane → canvas painted (decode-capable).
    expect(container.querySelector("canvas")).not.toBeNull();
    expect(getByTitle("Sharing screen")).toBeTruthy();
  });

  it("labels our own screen share on the self tile", () => {
    const { getByTitle } = render(
      <CallTiles
        layout="gallery"
        participants={[p({ key: "self", isSelf: true })]}
        {...common}
        selfCameraOn={false}
        selfSharing
      />,
    );
    expect(getByTitle("Sharing screen")).toBeTruthy();
  });

  // A video tile is theme-INVARIANT: the picture looks the same in light and
  // dark, so the letterbox behind it and the name chip on top of it must stay
  // dark-with-light-text in BOTH themes. `color.dark`/`color.onDark` are
  // --c-filled/--c-on-filled, which INVERT — in dark mode that letterboxed video
  // in near-white and turned the participant's name dark-on-dark (invisible).
  // Hence the scrim tokens. jsdom can't resolve CSS vars or compute contrast, so
  // this asserts the property that actually broke: these two colors must be
  // concrete literals, never a `var(--c-*)` that a theme flip can invert.
  it("paints the video frame and name chip with theme-invariant scrim colors", () => {
    const { container, getByText } = render(
      <CallTiles layout="gallery" participants={[p({ key: "self", isSelf: true })]} {...common} />,
    );
    const frame = container.querySelector("video")!.parentElement!;
    const chip = getByText("You").parentElement!;

    // A reverted token renders as `var(--c-filled, …)`, which jsdom either keeps
    // verbatim or drops as an invalid color — both fail these two assertions.
    for (const [what, value] of [
      ["frame background", frame.style.background || frame.style.backgroundColor],
      ["chip background", chip.style.background || chip.style.backgroundColor],
      ["chip text", chip.style.color],
    ] as const) {
      expect(value, `${what} must be a concrete color`).not.toBe("");
      expect(value, `${what} must not be a theme-inverting var()`).not.toContain("var(");
    }
  });

  it("keeps the scrim swatch pair readable (WCAG AA) in both themes", () => {
    // jsdom can't evaluate color-mix/vars, so contrast is checked on the literal
    // scrim tokens — which is sound precisely BECAUSE they don't vary by theme.
    const channels = (hex: string) =>
      [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16) / 255);
    const luminance = (rgb: number[]) => {
      const [r, g, b] = rgb.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const l1 = luminance(channels(color.onScrim));
    const l2 = luminance(channels(color.scrim));
    const contrast = (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
    expect(contrast, "onScrim on scrim").toBeGreaterThanOrEqual(4.5);
  });

  it("caps a strip and surfaces the overflow tail", () => {
    const many = Array.from({ length: 10 }, (_, i) => p({ key: `u${i}` }));
    const { getByText } = render(
      <CallTiles layout="strip" participants={many} {...common} maxTiles={4} />,
    );
    expect(getByText(/\+6 more not shown/i)).toBeTruthy();
  });
});
