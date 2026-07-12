// Pure logic for the Node view's sandbox onboarding section: mapping a host
// preflight probe bundle to a detection checklist, and generating the guided
// node.toml the operator pastes to opt in/out of serving agent work.
//
// The app has no node.toml write path (only the node binary writes it, via its
// init/join verbs), so onboarding degrades to copy-paste guidance rather than a
// live config write. Serving on/off + backend mode are therefore rendered as
// exact TOML lines with a copy button — that is the deliberate degradation, not
// a missing feature.

import type { SandboxPreflight, ProbeResult } from "../../../domain/sandbox-client";

export type SandboxMode = "direct" | "podman" | "tart";
export type CheckState = "ok" | "fail" | "unknown";

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
        (pf.os === "macos" ? "tart uses VM base images (phase 2)" : UNKNOWN_DETAIL),
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

/** The exact node.toml lines to serve agent work in `mode`. Direct announces
 *  tags with no metered capacity (never matches a demand-carrying job);
 *  sandboxed modes carry cores/mem so demand-carrying jobs can match. */
export function servingTomlLines(mode: SandboxMode, image = DEFAULT_SANDBOX_IMAGE): string {
  const lines = ["announce_capabilities = true", `sandbox = "${mode}"`];
  if (mode !== "direct") {
    lines.push(`sandbox_image = "${image}"`, "sandbox_cores = 2", "sandbox_mem_gb = 4");
  }
  return lines.join("\n");
}

/** node.toml line to stop serving (announce the empty set → leave registry). */
export const SERVING_OFF_TOML = "announce_capabilities = false";

/** The canned "set up with an agent" run prompt (spec §6): one prewritten
 *  instruction through the existing run pipeline, no new infrastructure. */
export function setupPrompt(mode: SandboxMode, image = DEFAULT_SANDBOX_IMAGE): string {
  if (mode === "tart") {
    return "Install and configure the tart sandbox backend for this Ducktape node: install tart (Apple Silicon), create/pull the base VM image, verify `tart run` starts a guest, report results.";
  }
  return `Install and configure the podman sandbox backend for this Ducktape node: install rootless podman, pull ${image}, verify cgroup v2 cpu delegation, report results.`;
}
