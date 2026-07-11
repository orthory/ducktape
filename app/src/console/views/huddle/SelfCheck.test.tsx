import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SelfCheck } from "./SelfCheck";
import type { SelfCheckProps } from "./SelfCheck";

const base: SelfCheckProps = {
  status: "live",
  cameraOn: false,
  sharing: false,
  canEncode: true,
  muted: false,
  level: 0,
  speaking: false,
  bindPreview: vi.fn(),
  onToggleCamera: vi.fn(),
};

describe("SelfCheck", () => {
  it("offers a 'Turn on camera' action when the camera is off and encode is possible", () => {
    render(<SelfCheck {...base} />);
    expect(screen.getByRole("button", { name: /turn on camera/i })).toBeTruthy();
  });

  it("says the camera is unavailable when the box cannot encode", () => {
    render(<SelfCheck {...base} canEncode={false} />);
    expect(screen.queryByRole("button", { name: /turn on camera/i })).toBeNull();
    expect(screen.getByText(/no camera encoder/i)).toBeTruthy();
  });

  it("shows the live self-preview instead of the prompt once the camera is on", () => {
    const { container } = render(<SelfCheck {...base} cameraOn />);
    expect(screen.queryByRole("button", { name: /turn on camera/i })).toBeNull();
    expect(container.querySelector("video")).toBeTruthy();
  });

  it("mic label: muted → still reacts; quiet → prompt; picking up when active", () => {
    const { rerender } = render(<SelfCheck {...base} muted />);
    expect(screen.getByText(/still reacts here/i)).toBeTruthy();

    rerender(<SelfCheck {...base} muted={false} level={0} speaking={false} />);
    expect(screen.getByText(/say something to test your mic/i)).toBeTruthy();

    rerender(<SelfCheck {...base} muted={false} speaking level={0.5} />);
    expect(screen.getByText(/picking you up/i)).toBeTruthy();
  });

  it("frames the whole thing as a self-check, not a dead 'connecting'", () => {
    render(<SelfCheck {...base} />);
    expect(screen.getByText(/only one here/i)).toBeTruthy();
    expect(screen.queryByText(/^connecting/i)).toBeNull();
  });
});
