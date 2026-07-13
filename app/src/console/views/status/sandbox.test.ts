import { describe, expect, it } from "vitest";

import type { SandboxPreflight } from "../../../domain/sandbox-client";
import {
  DEFAULT_SANDBOX_IMAGE,
  currentSandboxMode,
  modeOptionsFor,
  preflightChecklist,
  setupPrompt,
} from "./sandbox";

describe("modeOptionsFor", () => {
  it("offers both sandbox backends on macOS", () => {
    expect(modeOptionsFor(true).map((mode) => mode.id)).toEqual(["off", "direct", "podman", "tart"]);
  });

  it("keeps Tart off non-macOS hosts", () => {
    expect(modeOptionsFor(false).map((mode) => mode.id)).toEqual(["off", "direct", "podman"]);
  });
});

describe("currentSandboxMode", () => {
  it("uses the effective serving state instead of stale backend config", () => {
    expect(currentSandboxMode(linuxPreflight({ announceCapabilities: false, mode: "podman" }))).toBe("off");
    expect(currentSandboxMode(linuxPreflight({ announceCapabilities: true, mode: "" }))).toBe("direct");
  });
});

const linuxPreflight = (over: Partial<SandboxPreflight> = {}): SandboxPreflight => ({
  os: "linux",
  backend: "podman",
  image: DEFAULT_SANDBOX_IMAGE,
  announceCapabilities: false,
  mode: "",
  backendBinary: { ok: true, detail: "podman version 4.9.3" },
  baseImage: { ok: true, detail: `${DEFAULT_SANDBOX_IMAGE} present` },
  cgroupDelegation: { ok: true, detail: "cpu + memory delegated (cpu memory pids)" },
  ...over,
});

describe("preflightChecklist", () => {
  it("null bundle → three unknown items (honest, no host reached)", () => {
    const items = preflightChecklist(null);
    expect(items.map((i) => i.id)).toEqual(["backend", "image", "cgroup"]);
    expect(items.every((i) => i.state === "unknown")).toBe(true);
    expect(items.every((i) => !i.fixable)).toBe(true);
    expect(items[0].detail).toContain("node host");
  });

  it("all probes green → all ok, nothing fixable", () => {
    const items = preflightChecklist(linuxPreflight());
    expect(items.map((i) => i.state)).toEqual(["ok", "ok", "ok"]);
    expect(items.some((i) => i.fixable)).toBe(false);
  });

  it("missing binary + image → fail + fixable; detail passed through", () => {
    const items = preflightChecklist(
      linuxPreflight({
        backendBinary: { ok: false, detail: "podman not found on PATH" },
        baseImage: { ok: false, detail: "not pulled" },
      }),
    );
    const backend = items.find((i) => i.id === "backend")!;
    const image = items.find((i) => i.id === "image")!;
    expect(backend.state).toBe("fail");
    expect(backend.fixable).toBe(true);
    expect(backend.detail).toBe("podman not found on PATH");
    expect(image.fixable).toBe(true);
  });

  it("cgroup failure is never fixable-by-agent (host/systemd matter)", () => {
    const items = preflightChecklist(
      linuxPreflight({ cgroupDelegation: { ok: false, detail: "cpu not delegated" } }),
    );
    const cgroup = items.find((i) => i.id === "cgroup")!;
    expect(cgroup.state).toBe("fail");
    expect(cgroup.fixable).toBe(false);
  });

  it("macOS: null image/cgroup probes read as unknown with tart-shaped detail", () => {
    const items = preflightChecklist(
      linuxPreflight({ os: "macos", backend: "tart", baseImage: null, cgroupDelegation: null }),
    );
    const image = items.find((i) => i.id === "image")!;
    const cgroup = items.find((i) => i.id === "cgroup")!;
    expect(image.state).toBe("unknown");
    expect(image.detail).toContain("tart");
    expect(cgroup.detail).toContain("not applicable");
  });
});

describe("setupPrompt", () => {
  it("podman prompt names the image + cgroup verification", () => {
    const prompt = setupPrompt("podman");
    expect(prompt).toContain("rootless podman");
    expect(prompt).toContain(DEFAULT_SANDBOX_IMAGE);
    expect(prompt).toContain("cgroup");
  });

  it("tart prompt targets the macOS backend", () => {
    expect(setupPrompt("tart")).toContain("sshpass");
  });
});
