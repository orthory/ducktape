// Pure logic for the Sandbox page: map a host preflight bundle to its detection
// checklist and effective mode.

import type { SandboxPreflight, ProbeResult } from "../../../domain/sandbox-client";

export type SandboxMode = "direct" | "podman" | "tart";
export type CheckState = "ok" | "fail" | "unknown";

export const MODE_OPTIONS: { id: "off" | SandboxMode; label: string; blurb: string }[] = [
  { id: "off", label: "Off", blurb: "Serve no agent work — leave the capability registry." },
  { id: "direct", label: "Direct", blurb: "Unsandboxed spawn. Tags only, no metered capacity." },
  { id: "podman", label: "Podman", blurb: "Rootless podman with per-run cpu/memory caps." },
  { id: "tart", label: "Tart", blurb: "Apple-Silicon VM per run." },
];

export const modeOptionsFor = (macos: boolean) => MODE_OPTIONS.filter((m) => m.id !== "tart" || macos);

export const currentSandboxMode = (pf: SandboxPreflight | null): "off" | SandboxMode => {
  if (!pf?.announceCapabilities) return "off";
  return pf.mode === "podman" || pf.mode === "tart" ? pf.mode : "direct";
};

export interface ChecklistItem {
  id: "backend" | "image" | "cgroup";
  label: string;
  state: CheckState;
  detail: string;
  /** Whether a red item can be repaired by an agent setup run (install/pull). */
  fixable: boolean;
}

/** Kept in sync with the Rust `DEFAULT_SANDBOX_IMAGE`. */
export const DEFAULT_SANDBOX_IMAGE = "docker.io/library/node:22-slim";

const UNKNOWN_DETAIL = "run preflight on the node host";

const probeState = (probe: ProbeResult | null): CheckState =>
  probe === null ? "unknown" : probe.ok ? "ok" : "fail";

/** Map a preflight bundle (or its absence) to the detection checklist. A null
 *  bundle — web build, remote node, or the command not yet present — yields an
 *  all-unknown list: we can only truthfully probe the local managed host. */
export function preflightChecklist(pf: SandboxPreflight | null): ChecklistItem[] {
  if (!pf) {
    return [
      { id: "backend", label: "podman binary installed", state: "unknown", detail: UNKNOWN_DETAIL, fixable: false },
      { id: "image", label: `base image ${DEFAULT_SANDBOX_IMAGE} pulled`, state: "unknown", detail: UNKNOWN_DETAIL, fixable: false },
      { id: "cgroup", label: "cgroup v2 cpu + memory delegation", state: "unknown", detail: UNKNOWN_DETAIL, fixable: false },
    ];
  }

  const backend = probeState(pf.backendBinary);
  const image = probeState(pf.baseImage);
  const cgroup = probeState(pf.cgroupDelegation);

  return [
    {
      id: "backend",
      label: `${pf.backend || "podman"} binary installed`,
      state: backend,
      detail: pf.backendBinary?.detail ?? UNKNOWN_DETAIL,
      fixable: backend === "fail",
    },
    {
      id: "image",
      label: `base image ${pf.image} pulled`,
      state: image,
      detail:
        pf.baseImage?.detail ??
        (pf.os === "macos" ? "tart uses VM base images" : UNKNOWN_DETAIL),
      fixable: image === "fail",
    },
    {
      id: "cgroup",
      label: "cgroup v2 cpu + memory delegation",
      state: cgroup,
      detail:
        pf.cgroupDelegation?.detail ??
        (pf.os === "linux" ? UNKNOWN_DETAIL : "not applicable on this OS"),
      // Delegation is a systemd/host matter (may need a drop-in + root), not a
      // pure install an agent run resolves — the agent prompt still verifies it.
      fixable: false,
    },
  ];
}

/** The canned "set up with an agent" run prompt (spec §6): one prewritten
 *  instruction through the existing run pipeline, no new infrastructure. */
export function setupPrompt(mode: SandboxMode, image = DEFAULT_SANDBOX_IMAGE): string {
  if (mode === "tart") {
    return "Install and configure the tart sandbox backend for this Ducktape node: install tart and sshpass (Apple Silicon), create/pull the base VM image, verify `tart run` boots it and SSH reaches the guest, report results.";
  }
  return `Install and configure the podman sandbox backend for this Ducktape node: install rootless podman, pull ${image}, verify cgroup v2 cpu delegation, report results.`;
}
