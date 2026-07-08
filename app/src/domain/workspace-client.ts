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

/** A workspace's daemon.log path + tail — the real startup reason (bind
 *  conflict, config error, panic) the node wrote but nothing in the UI read.
 *  Powers the "Node failed to start" surface and its "Open daemon.log". */
export interface LogTail {
  path: string;
  tail: string;
}

/** The running node's operational identity for the Node → Logs tab. Every
 *  process field is best-effort: a node we adopted (didn't spawn) has no
 *  pidfile, so pid/alive/uptimeSecs come back null. Verbatim from workspaces.rs
 *  RuntimeFacts (camelCase). */
export interface RuntimeFacts {
  pid: number | null;
  alive: boolean | null;
  uptimeSecs: number | null;
  binaryPath: string | null;
  dataDir: string;
  logPath: string;
}

/** One pending join request a parked joiner delivered over the lobby channel.
 *  Snake_case: these rows pass through verbatim from the NODE's
 *  `join-requests` JSON (not the registry's camelCase structs). */
export interface JoinRequest {
  /** The key asking to join, hex. */
  joiner: string;
  /** The member whose invite token authorized the announce, hex. */
  issuer: string;
  first_seen_ms: number;
  last_seen_ms: number;
}

// ── Guard ───────────────────────────────────────────────

/** The registry is desktop-only; the web build never calls these. */
export const isDesktop = (): boolean => isTauri();

// ── User identity (Settings > Devices) ──────────────────

/** This machine's user identity (`~/.ducktape/user.key`) — a sibling of the
 *  workspace registry, not part of it: this key is shared across every
 *  workspace rather than scoped to one. Rejects with the underlying node
 *  verb's error string when the key file exists but is corrupt — that file is
 *  never overwritten silently, so the error is the operator's only signal
 *  (surfaced verbatim in Settings > Devices). */
export interface UserIdentity {
  pubkey: string;
}

export const userIdentityStatus = (): Promise<UserIdentity> =>
  invoke<UserIdentity>("user_identity_status");

// Live join/admit — joining a RUNNING network (post-genesis, network shape) and
// admitting a joiner into it. The node's live-admission path landed in PR #77:
// config.rs now resolves an un-admitted network-shape key as a pending joiner
// that parks on the mesh until governance admits it (park→invite-accept→promote),
// proven by bin/node/tests/live_admission_e2e.rs. So this UI is now live. The
// flag remains a single named toggle should the flow ever need re-gating.
export const LIVE_JOIN_SUPPORTED = true;

// ── Reads ───────────────────────────────────────────────

export const listWorkspaces = (): Promise<Workspace[]> =>
  invoke<Workspace[]>("workspace_list");

export const activeWorkspace = (): Promise<Workspace | null> =>
  invoke<Workspace | null>("workspace_active");

export const workspacePhase = (id: string): Promise<PhaseReport> =>
  invoke<PhaseReport>("workspace_phase", { id });

/** The daemon.log path + last 64 KB for a workspace — called when a managed
 *  node fails to answer, to surface the real reason and back "Open daemon.log". */
export const workspaceLogTail = (id: string): Promise<LogTail> =>
  invoke<LogTail>("workspace_log_tail", { id });

/** The running node's pid/uptime/binary/paths — polled by the Node → Logs tab's
 *  runtime-facts row. Best-effort per field (see RuntimeFacts). */
export const workspaceRuntimeFacts = (id: string): Promise<RuntimeFacts> =>
  invoke<RuntimeFacts>("workspace_runtime_facts", { id });

export const inviteBlob = (id: string): Promise<string> =>
  invoke<string>("workspace_invite_blob", { id });

/** The verified join requests parked joiners announced to this member's
 *  running node — what the Members view renders with an Approve button.
 *  Approving is admitMember (the normal governance ballot). */
export const joinRequests = (id: string): Promise<JoinRequest[]> =>
  invoke<JoinRequest[]>("workspace_join_requests", { id });

// ── Writes ──────────────────────────────────────────────

/** Found a new network; returns the recorded (now active) workspace. */
export const createWorkspace = (name: string): Promise<Workspace> =>
  invoke<Workspace>("workspace_create", { name });

/** Join an existing network from an invite blob; returns the recorded (now
 *  active) workspace. A non-member workspace parks when started. */
export const joinWorkspace = (name: string, blob: string): Promise<Workspace> =>
  invoke<Workspace>("workspace_join", { name, blob });

/** Admit a joiner by pubkey through this running member node's governance.
 *  Grants RESIDENT standing (mesh + statesync, no quorum seat) — the first
 *  step of staged admission; [`promoteMember`] seats it once warm. */
export const admitMember = (id: string, pubkey: string): Promise<void> =>
  invoke<void>("workspace_admit", { id, pubkey });

/** Promote a resident into the consensus quorum by pubkey through this
 *  running member node's governance — staged admission's second step. */
export const promoteMember = (id: string, pubkey: string): Promise<void> =>
  invoke<void>("workspace_promote", { id, pubkey });

/** Revoke resident standing by pubkey through this running member node's
 *  governance — the undo of [`admitMember`]; the key's node parks again. */
export const removeResident = (id: string, pubkey: string): Promise<void> =>
  invoke<void>("workspace_resident_remove", { id, pubkey });

/** Remove a validator by pubkey through this running member node's governance.
 *  Opens a RemoveValidator proposal and casts this node's yes-ballot; the
 *  removal only takes effect once a strict majority of members (n/2 + 1)
 *  approve, and the removed node drops out at the next epoch cutover. */
export const demoteMember = (id: string, pubkey: string): Promise<void> =>
  invoke<void>("workspace_demote", { id, pubkey });

/** Request to leave a network: drive this node's on-chain SELF-removal (a
 *  RemoveValidator proposal for our OWN key, casting our yes-ballot) through the
 *  running member node, and KEEP THE NODE RUNNING. In a set of two-or-more the
 *  removal stays pending until a strict majority of the remaining members
 *  approve — the node must stay up through its own pending removal or quorum
 *  cannot finalize it. A solo (n=1) node is refused (you can't remove the last
 *  validator); it forgets its workspace directly instead. Rejects on failure. */
export const requestLeaveWorkspace = (id: string): Promise<void> =>
  invoke<void>("workspace_request_leave", { id });

/** Forget a workspace: stop its node, delete its directory, and drop its
 *  registry entry. GUARDED — refused while this node is still a current
 *  validator of a set of two-or-more (tearing it down would halt quorum and
 *  strand its pending removal). Safe once removed (no longer in the valset) or
 *  for a solo network only this node runs. Resolves to the newly-active
 *  workspace the registry repointed to, or null when none remain.
 *
 *  `force` is the escape hatch for a workspace whose node can NEVER come up (a
 *  bricked recovery — the guard can't reach it to confirm anything, so it stays
 *  un-removable). It overrides ONLY the "can't confirm the node left" refusal;
 *  the backend still refuses to force-tear-down a node it can REACH and that
 *  proves it is a live multi-member validator. */
export const forgetWorkspace = (
  id: string,
  force = false,
): Promise<Workspace | null> =>
  invoke<Workspace | null>("workspace_forget", { id, force });

/** Make a workspace active and ensure its node is running; returns the http
 *  url to dial. Idempotent — adopts an already-listening node. */
export const selectWorkspace = (id: string): Promise<WorkspaceSelection> =>
  invoke<WorkspaceSelection>("workspace_select", { id });
