// Typed client for the desktop shell's ~/.ducktape workspace registry — the TS
// mirror of app/src-tauri/src/workspaces.rs. Every call is a Tauri `invoke`,
// so these ONLY work in the desktop build (guard with isDesktop()); the web
// build has no local registry and dials a single configured node instead.
//
// Wire casing is the Rust command's: the registry structs serialize camelCase
// (chainId, httpUrl), so the TS shapes match verbatim — no remapping.

import { invoke } from "@tauri-apps/api/core";

import { isTauri } from "./node-bootstrap";

// ── Wire types (verbatim from workspaces.rs) ────────────

export interface WorkspacePorts {
  listen: number;
  http: number;
  rpc: number;
}

export interface Workspace {
  id: string;
  name: string;
  chainId: string;
  /** This workspace's own identity pubkey, hex. */
  pubkey: string;
  /** Founded the network (sole genesis validator) vs joined one. */
  founder: boolean;
  /** Already in the validator set — boots straight to a validator. False means
   *  it PARKS until a member admits it (surfaced by workspacePhase). */
  member: boolean;
  ports: WorkspacePorts;
}

export interface WorkspaceSelection {
  id: string;
  /** The http url the webview should dial for this workspace's node. */
  httpUrl: string;
}

/** The joiner onboarding phases, read back from the node log. "ready" is not
 *  one of these — the caller derives it from a successful /v1/status. */
export type OnboardingPhase =
  | "starting"
  | "parked"
  | "admitted"
  | "synced"
  | "promoted"
  | "fatal";

export interface PhaseReport {
  phase: OnboardingPhase;
  /** The trailing text of the latest marker line, for a live status string. */
  detail: string | null;
}

// ── Guard ───────────────────────────────────────────────

/** The registry is desktop-only; the web build never calls these. */
export const isDesktop = (): boolean => isTauri();

// Joining a RUNNING network (post-genesis, network shape) is blocked at the
// node/consensus layer, not here: config.rs rejects an un-admitted network-shape
// key ("live admission is not built yet"), and an un-admitted key can't even
// connect to the mesh to be admitted. The park→promote flow only works in the
// dev-seed shape, where every joiner is pre-listed in peer_seeds on all nodes.
// The join/admit code below is complete and correct FOR WHEN that node feature
// lands; until then the UI gates it behind this flag. Founding a network (the
// create flow) is unaffected and fully works.
export const LIVE_JOIN_SUPPORTED = false;

// ── Reads ───────────────────────────────────────────────

export const listWorkspaces = (): Promise<Workspace[]> =>
  invoke<Workspace[]>("workspace_list");

export const activeWorkspace = (): Promise<Workspace | null> =>
  invoke<Workspace | null>("workspace_active");

export const workspacePhase = (id: string): Promise<PhaseReport> =>
  invoke<PhaseReport>("workspace_phase", { id });

export const inviteBlob = (id: string): Promise<string> =>
  invoke<string>("workspace_invite_blob", { id });

// ── Writes ──────────────────────────────────────────────

/** Found a new network; returns the recorded (now active) workspace. */
export const createWorkspace = (name: string): Promise<Workspace> =>
  invoke<Workspace>("workspace_create", { name });

/** Join an existing network from an invite blob; returns the recorded (now
 *  active) workspace. A non-member workspace parks when started. */
export const joinWorkspace = (name: string, blob: string): Promise<Workspace> =>
  invoke<Workspace>("workspace_join", { name, blob });

/** Admit a joiner by pubkey through this running member node's governance. */
export const admitMember = (id: string, pubkey: string): Promise<void> =>
  invoke<void>("workspace_admit", { id, pubkey });

/** Make a workspace active and ensure its node is running; returns the http
 *  url to dial. Idempotent — adopts an already-listening node. */
export const selectWorkspace = (id: string): Promise<WorkspaceSelection> =>
  invoke<WorkspaceSelection>("workspace_select", { id });
