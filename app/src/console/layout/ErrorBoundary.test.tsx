import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ErrorBoundary } from "./ErrorBoundary";

function Boom(): never {
  throw new Error("kaboom in render");
}

describe("ErrorBoundary", () => {
  it("renders a fallback instead of a blank window when a child throws", () => {
    // React logs the caught error; silence it for a clean test run.
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Something crashed")).toBeTruthy();
    // the message shows in the reason line AND again in the stack trace
    expect(screen.getAllByText(/kaboom in render/).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Reload" })).toBeTruthy();
    spy.mockRestore();
  });

  it("renders its children when nothing throws", () => {
    render(
      <ErrorBoundary>
        <div>healthy content</div>
      </ErrorBoundary>,
    );
    expect(screen.getByText("healthy content")).toBeTruthy();
  });
});
