import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import type { Workspace } from "../../../domain/workspace-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState } from "../../store/state";
import { SandboxView } from "../sandbox/SandboxView";

const workspace: Workspace = {
  id: "acme-research",
  name: "Acme Research",
  chainId: "acme#abcd1234",
  pubkey: "ab".repeat(32),
  founder: true,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe("SandboxTab apply flow", () => {
  it("confirms and applies instead of showing config to copy", async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    invokeMock.mockImplementation((command: string) => {
      if (command === "sandbox_preflight") {
        return Promise.resolve({
          os: "macos",
          backend: "tart",
          image: "ghcr.io/cirruslabs/macos-sonoma-base:latest",
          announceCapabilities: false,
          mode: "",
          backendBinary: { ok: true, detail: "tart present" },
          baseImage: null,
          cgroupDelegation: null,
        });
      }
      if (command === "workspace_sandbox_apply") return Promise.resolve();
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(
      <ConsoleContext.Provider
        value={{
          state: { ...createInitialState(), workspace, managed: true, connected: true },
          actions: {} as ConsoleActions,
        }}
      >
        <SandboxView />
      </ConsoleContext.Provider>,
    );

    const page = document.querySelector('[data-screen-label="Sandbox"]');
    expect(page).toHaveAttribute("data-sandbox-layout", "full-width");
    expect(page).toHaveStyle({ width: "100%", minWidth: "0" });

    fireEvent.click(await screen.findByRole("button", { name: "Podman" }));
    const dialog = screen.getByRole("dialog", { name: "Apply Podman?" });
    expect(invokeMock).not.toHaveBeenCalledWith("workspace_sandbox_apply", expect.anything());
    expect(screen.queryByText("COPY")).not.toBeInTheDocument();
    expect(screen.queryByText(/paste/i)).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", { name: "Apply and restart" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("workspace_sandbox_apply", {
        id: workspace.id,
        mode: "podman",
      }),
    );
    expect(screen.getByText(/Applied\. The node restarted/)).toBeInTheDocument();
  });
});
