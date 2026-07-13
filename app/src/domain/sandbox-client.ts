// Typed mirror of the desktop shell's `sandbox_preflight` command
// (app/src-tauri/src/sandbox.rs). A read-only host probe for the Node view's
// sandbox onboarding section: it reports the active workspace's serving opt-in
// (from node.toml) plus the local backend's readiness. Only reachable in the
// desktop build against a locally managed node — the Sandbox tab is what gates
// the call; here we just degrade to `null` off Tauri so the view falls back to
// its honest "run preflight on the node host" state.

import { invoke } from "@tauri-apps/api/core";

import { isTauri } from "./node-bootstrap";

export interface ProbeResult {
  ok: boolean;
  detail: string;
}

export interface SandboxPreflight {
  /** Host OS bucket: "linux" | "macos" | "other". */
  os: string;
  /** Platform sandbox backend binary: "podman" | "tart" | "". */
  backend: string;
  /** Resolved base image (node.toml override, else the podman default). */
  image: string;
  /** node.toml announce_capabilities — the serving opt-in. */
  announceCapabilities: boolean;
  /** node.toml sandbox mode: "direct" | "podman" | "tart" | "" (unset). */
  mode: string;
  /** Backend binary presence; null when no backend applies to this OS. */
  backendBinary: ProbeResult | null;
  /** Base image pulled; null off the podman path. */
  baseImage: ProbeResult | null;
  /** cgroup v2 cpu+memory delegation; Linux only, else null. */
  cgroupDelegation: ProbeResult | null;
}

export type SandboxApplyMode = "off" | "direct" | "podman" | "tart";

/** Probe the local host for one workspace. Resolves `null` off the desktop
 *  build (nothing to probe), so callers render the unknown/guidance state. */
export const sandboxPreflight = (id: string): Promise<SandboxPreflight | null> =>
  isTauri() ? invoke<SandboxPreflight>("sandbox_preflight", { id }) : Promise.resolve(null);

/** Persist a sandbox choice and restart the managed workspace node. The Rust
 * command rolls the config back when the new node fails to boot. */
export const sandboxApply = (id: string, mode: SandboxApplyMode): Promise<void> =>
  invoke<void>("workspace_sandbox_apply", { id, mode });
