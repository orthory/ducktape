// The account-first navigation contract (ADR 2026-07-14 account-node-access-
// model, A5/A6): the operator rail exists only while node control is
// available; network-data surfaces (members, governance, explorer) live on
// the account rail; a direct remote client sees no node chrome at all.

import { describe, expect, it } from "vitest";

import type { Workspace } from "../domain/workspace-client";
import {
  defaultScreenForSection,
  moduleAvailable,
  modulesInSection,
} from "../console/modules/registry";
import {
  DEFAULT_OPERATOR_SCREEN,
  nodeControlAvailable,
  ownerControlUnreachable,
} from "../console/store/state";

const ws = { id: "w" } as unknown as Workspace;

describe("nodeControlAvailable (ADR A5)", () => {
  const base = { workspace: null, managed: false, owner: false, adminReachable: false };
  it("a managed local workspace is controllable (process plane)", () => {
    expect(nodeControlAvailable({ ...base, workspace: ws, managed: true })).toBe(true);
  });
  it("a direct remote non-owner client is not", () => {
    expect(nodeControlAvailable(base)).toBe(false);
  });
  it("a workspace without a managed daemon is not", () => {
    expect(nodeControlAvailable({ ...base, workspace: ws, managed: false })).toBe(false);
  });
  it("a remote owner whose admin surface is reachable IS controllable", () => {
    expect(nodeControlAvailable({ ...base, owner: true, adminReachable: true })).toBe(true);
  });
  it("a remote owner whose admin surface is unreachable is not", () => {
    expect(nodeControlAvailable({ ...base, owner: true, adminReachable: false })).toBe(false);
  });
});

describe("ownerControlUnreachable (ADR A5 hint)", () => {
  const base = { workspace: null, managed: false, owner: false, adminReachable: false };
  it("a remote owner with an unreachable admin surface shows the hint", () => {
    expect(ownerControlUnreachable({ ...base, owner: true })).toBe(true);
  });
  it("a non-owner never shows the hint", () => {
    expect(ownerControlUnreachable(base)).toBe(false);
  });
  it("a reachable owner shows no hint (they get full control instead)", () => {
    expect(ownerControlUnreachable({ ...base, owner: true, adminReachable: true })).toBe(false);
  });
  it("a local managed node shows no hint (process-plane control, not the remote case)", () => {
    expect(ownerControlUnreachable({ ...base, workspace: ws, managed: true, owner: true })).toBe(
      false,
    );
  });
});

describe("operator rail default", () => {
  it("is the node console", () => {
    expect(DEFAULT_OPERATOR_SCREEN).toBe("status");
  });
});

const owner = { nodeControl: true, clientMode: false };
const client = { nodeControl: false, clientMode: true };

describe("module availability (ADR A6)", () => {
  it("the operator rail exists only under node control", () => {
    expect(modulesInSection("operator", owner).map((m) => m.id)).toEqual([
      "status",
      "gateway",
      "modules",
      "sandbox",
      "terminal",
      "metrics",
    ]);
    expect(modulesInSection("operator", client)).toEqual([]);
  });

  it("network-data surfaces live on the account rail", () => {
    expect(modulesInSection("user", owner).map((m) => m.id)).toEqual([
      "chat",
      "pages",
      "files",
      "browser",
      "forge",
      "agent",
      "members",
      "governance",
      "explorer",
    ]);
  });

  it("a client keeps account surfaces except the A3-pending ones", () => {
    const ids = modulesInSection("user", client).map((m) => m.id);
    expect(ids).toContain("explorer");
    expect(ids).not.toContain("members");
    expect(ids).not.toContain("governance");
  });

  it("the operator rail defaults to the node console, else account fallback", () => {
    expect(defaultScreenForSection("operator", owner)).toBe(DEFAULT_OPERATOR_SCREEN);
    expect(defaultScreenForSection("operator", client)).toBe("chat");
  });

  it("unknown ids are unavailable", () => {
    expect(moduleAvailable("nope", owner)).toBe(false);
  });
});
