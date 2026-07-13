// The account-first navigation contract (ADR 2026-07-14 account-node-access-
// model, A5/A6): the operator rail exists only while node control is
// available; network-data surfaces (members, governance, explorer) live on
// the account rail; a direct remote client sees no node chrome at all.

import { describe, expect, it } from "vitest";

import type { Workspace } from "../domain/workspace-client";
import {
  DEFAULT_OPERATOR_SCREEN,
  nodeControlAvailable,
} from "../console/store/state";

const ws = { id: "w" } as unknown as Workspace;

describe("nodeControlAvailable (ADR A5, interim form)", () => {
  it("a managed local workspace is controllable", () => {
    expect(nodeControlAvailable({ workspace: ws, managed: true })).toBe(true);
  });
  it("a direct remote client is not", () => {
    expect(nodeControlAvailable({ workspace: null, managed: false })).toBe(false);
  });
  it("a workspace without a managed daemon is not", () => {
    expect(nodeControlAvailable({ workspace: ws, managed: false })).toBe(false);
  });
});

describe("operator rail default", () => {
  it("is the node console", () => {
    expect(DEFAULT_OPERATOR_SCREEN).toBe("status");
  });
});
