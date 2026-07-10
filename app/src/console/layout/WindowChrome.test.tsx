// The in-app window chrome only exists where the shell drops native
// decorations: a tauri desktop that is not macOS. The web build (no tauri
// marker) must render nothing; jsdom's UA has no "Mac", so marking tauri
// yields exactly the Linux/Windows shape.

import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ResizeEdges, WindowControls } from "./WindowChrome";

const win = vi.hoisted(() => ({
  minimize: vi.fn(() => Promise.resolve()),
  toggleMaximize: vi.fn(() => Promise.resolve()),
  close: vi.fn(() => Promise.resolve()),
  startResizeDragging: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => win }));

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("window controls", () => {
  it("render nothing on the web build", () => {
    render(<WindowControls />);
    expect(screen.queryByLabelText("Close window")).toBeNull();
  });

  it("render nothing on a mac desktop (native traffic lights own the chrome)", () => {
    markTauri();
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
    });
    const { container } = render(
      <>
        <WindowControls />
        <ResizeEdges />
      </>,
    );
    expect(screen.queryByLabelText("Close window")).toBeNull();
    expect(container.querySelector("[data-resize-dir]")).toBeNull();
  });

  it("drive the native window on a non-mac desktop", () => {
    markTauri();
    render(<WindowControls />);
    fireEvent.click(screen.getByLabelText("Minimize"));
    fireEvent.click(screen.getByLabelText("Maximize"));
    fireEvent.click(screen.getByLabelText("Close window"));
    expect(win.minimize).toHaveBeenCalled();
    expect(win.toggleMaximize).toHaveBeenCalled();
    expect(win.close).toHaveBeenCalled();
  });
});

describe("resize edges", () => {
  it("render nothing on the web build", () => {
    const { container } = render(<ResizeEdges />);
    expect(container.querySelector("[data-resize-dir]")).toBeNull();
  });

  it("start a WM resize drag for their direction", () => {
    markTauri();
    const { container } = render(<ResizeEdges />);
    const east = container.querySelector('[data-resize-dir="East"]');
    expect(east).not.toBeNull();
    fireEvent.mouseDown(east!, { button: 0 });
    expect(win.startResizeDragging).toHaveBeenCalledWith("East");
  });
});
