import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { HuddleControls } from "./HuddleControls";
import type { HuddleControlsProps } from "./HuddleControls";

const base: HuddleControlsProps = {
  size: "compact",
  status: "live",
  muted: false,
  cameraOn: false,
  canEncode: true,
  onToggleMute: vi.fn(),
  onToggleCamera: vi.fn(),
  onLeave: vi.fn(),
  onRetry: vi.fn(),
};

const order = () =>
  screen.getAllByRole("button").map((b) => b.getAttribute("title") ?? "");

describe("HuddleControls", () => {
  it("orders mic, camera, then Leave last with no screen/devices by default", () => {
    render(<HuddleControls {...base} />);
    const titles = order();
    const mic = titles.findIndex((t) => /mute/i.test(t));
    const cam = titles.findIndex((t) => /camera/i.test(t));
    const leave = titles.findIndex((t) => /leave/i.test(t));
    expect(mic).toBeGreaterThanOrEqual(0);
    expect(cam).toBeGreaterThan(mic);
    expect(leave).toBe(titles.length - 1);
    expect(titles.some((t) => /screen/i.test(t))).toBe(false);
    expect(titles.some((t) => /device/i.test(t))).toBe(false);
  });

  it("hides the camera control when the box cannot encode", () => {
    render(<HuddleControls {...base} canEncode={false} />);
    expect(screen.queryByRole("button", { name: /camera/i })).toBeNull();
  });

  it("reserves the screen slot between camera and devices when both are enabled", () => {
    render(
      <HuddleControls
        {...base}
        canScreenShare
        onToggleScreen={vi.fn()}
        onOpenDevices={vi.fn()}
      />,
    );
    const titles = order();
    const cam = titles.findIndex((t) => /camera/i.test(t));
    const screenIdx = titles.findIndex((t) => /screen/i.test(t));
    const dev = titles.findIndex((t) => /device/i.test(t));
    expect(cam).toBeLessThan(screenIdx);
    expect(screenIdx).toBeLessThan(dev);
  });

  it("shows Retry (not Mute) in the error state, Leave still last", () => {
    render(<HuddleControls {...base} status="error" />);
    expect(screen.getByRole("button", { name: /retry/i })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /mute/i })).toBeNull();
    const titles = order();
    expect(/leave/i.test(titles[titles.length - 1])).toBe(true);
  });

  it("disables mic and camera when not live", () => {
    render(<HuddleControls {...base} status="connecting" />);
    expect(screen.getByRole("button", { name: /mute/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /camera/i })).toBeDisabled();
  });

  it("disables the camera with the given reason as its tooltip even while live", () => {
    // comfortable size gives the camera button visible "Camera" text so it's
    // locatable by role-name even when its title carries the cap reason.
    render(
      <HuddleControls
        {...base}
        size="comfortable"
        cameraDisabledReason="Video is capped at 8 participants"
      />,
    );
    const cam = screen.getByRole("button", { name: /camera/i });
    expect(cam).toBeDisabled();
    expect(cam).toHaveAttribute("title", "Video is capped at 8 participants");
  });
});
