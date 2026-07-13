// The drag math is the contract: pointer delta resizes the panel through the
// CSS var (clamped), release persists, double-click resets, arrow keys resize
// without a pointer. Events target the handle itself — pointer capture routes
// the whole drag through it.

import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { PanelResizer } from "./PanelResizer";

const varOf = (name: string) => document.documentElement.style.getPropertyValue(name);

const mount = (side: "left" | "right", varName: string) => {
  render(
    <div style={{ position: "relative" }}>
      <PanelResizer varName={varName} defaultWidth={200} min={160} max={340} side={side} />
    </div>,
  );
  const handle = screen.getByRole("separator");
  // jsdom has no layout — pin the panel's live width the handle reads.
  Object.defineProperty(handle.parentElement!, "offsetWidth", { value: 200, configurable: true });
  return handle;
};

afterEach(() => {
  localStorage.clear();
  for (const name of ["--t-right", "--t-left", "--t-reset", "--t-keys"]) {
    document.documentElement.style.removeProperty(name);
  }
});

describe("PanelResizer", () => {
  it("drags a right-edge handle: +x grows, release persists, clamp holds", () => {
    const handle = mount("right", "--t-right");

    fireEvent.pointerDown(handle, { clientX: 10 });
    fireEvent.pointerMove(handle, { clientX: 60 });
    expect(varOf("--t-right")).toBe("250px");

    // clamp: a huge drag stops at max
    fireEvent.pointerMove(handle, { clientX: 900 });
    expect(varOf("--t-right")).toBe("340px");

    fireEvent.pointerUp(handle, { clientX: 60 });
    expect(localStorage.getItem("--t-right")).toBe("250");
    expect(handle.getAttribute("aria-valuenow")).toBe("250");

    // the drag ended — a later stray move must not resize
    fireEvent.pointerMove(handle, { clientX: 500 });
    expect(varOf("--t-right")).toBe("250px");
  });

  it("drags a left-edge handle: -x grows (right-docked panel)", () => {
    const handle = mount("left", "--t-left");

    fireEvent.pointerDown(handle, { clientX: 100 });
    fireEvent.pointerMove(handle, { clientX: 40 });
    expect(varOf("--t-left")).toBe("260px");
    fireEvent.pointerCancel(handle, { clientX: 40 });
    expect(varOf("--t-left")).toBe("260px");
  });

  it("resizes from the keyboard and persists", () => {
    const handle = mount("right", "--t-keys");

    fireEvent.keyDown(handle, { key: "ArrowRight" });
    expect(varOf("--t-keys")).toBe("216px");
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(varOf("--t-keys")).toBe("200px");
    expect(localStorage.getItem("--t-keys")).toBe("200");
  });

  it("double-click resets to the default and forgets the saved width", () => {
    localStorage.setItem("--t-reset", "300");
    const handle = mount("right", "--t-reset");
    // mount restored the saved width
    expect(varOf("--t-reset")).toBe("300px");

    fireEvent.doubleClick(handle);
    expect(varOf("--t-reset")).toBe("200px");
    expect(localStorage.getItem("--t-reset")).toBeNull();
  });
});
