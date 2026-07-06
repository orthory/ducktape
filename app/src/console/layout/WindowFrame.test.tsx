// The title bar's search affordance is a console control: it opens the ⌘K
// palette over a connected workspace. With no workspace chosen (the onboarding
// gate) or mid-join (the waiting room) there is nothing to search, so the bar
// must not render.

import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DucktapeProvider } from "../store/DucktapeProvider";
import { WindowFrame } from "./WindowFrame";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.unstubAllGlobals();
  invokeMock.mockReset();
  localStorage.clear();
});

describe("window frame search affordance", () => {
  it("is hidden while the onboarding gate is up (no workspace chosen)", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) =>
      cmd === "workspace_list" ? Promise.resolve([]) : Promise.resolve(null),
    );

    render(
      <DucktapeProvider>
        <WindowFrame>
          <div />
        </WindowFrame>
      </DucktapeProvider>,
    );

    // boot settles on the raised gate; the search affordance must not appear.
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("workspace_list"));
    expect(screen.queryByLabelText("Search")).toBeNull();
  });

  it("shows once a node is connected (web build resolves one directly)", async () => {
    // no tauri marker: the web build dials its configured node — no onboarding.
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(JSON.stringify({ channels: [] }), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
        ),
      ),
    );

    render(
      <DucktapeProvider>
        <WindowFrame>
          <div />
        </WindowFrame>
      </DucktapeProvider>,
    );

    await waitFor(() => expect(screen.getByLabelText("Search")).toBeTruthy());
  });
});
