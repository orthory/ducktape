// W6 account-profile propagation: the reconcile-on-connect pass is idempotent
// (no-op when converged, pushes only dirty fields) and the panel's direct write
// clears explicitly. Pure over a stubbed transport — no node, no React.

import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AccountView } from "../../domain/identity-client";
import { makeTransportStub } from "../transport-stub";
import { saveAccountProfile } from "../../console/store/account-profile";
import {
  pushProfileEdit,
  reconcileProfile,
} from "../../console/store/profile-reconcile";

const NODE = "aa".repeat(32);

const account = (patch: Partial<AccountView> = {}): AccountView => ({
  account_id: [1, 2, 3],
  display_name: null,
  avatar: null,
  bio: null,
  nonce: 1,
  member_keys: [{ pubkey: [1, 2, 3], kind: "ed25519", label: null, added_at: 1 }],
  nodes: [[0xaa]],
  updated_at: 1,
  ...patch,
});

/** A transport whose identity `of_node` reply is `onChain`, files refs are an
 *  empty tree, and every submit/commit is a spy. */
const transportFor = (onChain: AccountView | null) => {
  const query = vi.fn((target: string, q: Record<string, unknown>) => {
    if (target === "files" && "refs" in q) return Promise.resolve({ refs: { head: null } });
    return Promise.resolve({ account: onChain });
  });
  return makeTransportStub({ query: query as never });
};

/** The op name of every submit(target, payload) call, e.g. "set_profile". */
const submittedOps = (transport: ReturnType<typeof makeTransportStub>): string[] =>
  (transport.submit as ReturnType<typeof vi.fn>).mock.calls.map(
    (c) => Object.keys(c[1] as object)[0],
  );

const PNG =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

beforeEach(() => localStorage.clear());

describe("reconcileProfile", () => {
  it("reports unbound and pushes nothing when the node has no account", async () => {
    saveAccountProfile({ name: "Kim", bio: "hi" });
    const t = transportFor(null);
    expect(await reconcileProfile(t, { nodePub: NODE })).toBe("unbound");
    expect(t.submit).not.toHaveBeenCalled();
  });

  it("no-ops when local profile already matches on-chain", async () => {
    saveAccountProfile({ name: "Kim", bio: "hi" });
    const t = transportFor(account({ display_name: "Kim", bio: "hi" }));
    expect(await reconcileProfile(t, { nodePub: NODE })).toBe("reconciled");
    expect(t.submit).not.toHaveBeenCalled();
  });

  it("pushes name and bio when they differ", async () => {
    saveAccountProfile({ name: "Kim", bio: "hi" });
    const t = transportFor(account({ display_name: null, bio: null }));
    await reconcileProfile(t, { nodePub: NODE });
    expect(submittedOps(t)).toEqual(
      expect.arrayContaining(["set_account_name", "set_profile"]),
    );
  });

  it("uploads a new avatar once and references it in set_profile", async () => {
    saveAccountProfile({ avatar: PNG });
    const t = transportFor(account());
    await reconcileProfile(t, { nodePub: NODE });
    expect(t.filesCommit).toHaveBeenCalledTimes(1);
    const profileCall = (t.submit as ReturnType<typeof vi.fn>).mock.calls.find(
      (c) => "set_profile" in (c[1] as object),
    );
    const avatar = (profileCall?.[1] as { set_profile: { avatar: string } }).set_profile.avatar;
    expect(avatar).toMatch(/^\/shared\/attachments\/avatars\/[0-9a-f]{16}\.png$/);
  });

  it("skips the avatar upload when the on-chain ref already matches (idempotent)", async () => {
    saveAccountProfile({ avatar: PNG });
    // first pass computes the content path; feed it back as the on-chain state.
    const first = transportFor(account());
    await reconcileProfile(first, { nodePub: NODE });
    const path = (
      (first.submit as ReturnType<typeof vi.fn>).mock.calls.find(
        (c) => "set_profile" in (c[1] as object),
      )?.[1] as { set_profile: { avatar: string } }
    ).set_profile.avatar;

    const second = transportFor(account({ avatar: path }));
    expect(await reconcileProfile(second, { nodePub: NODE })).toBe("reconciled");
    expect(second.filesCommit).not.toHaveBeenCalled();
    expect(second.submit).not.toHaveBeenCalled();
  });

  it("adopts an on-chain name into an empty local store instead of clearing it", async () => {
    const t = transportFor(account({ display_name: "Rae" }));
    await reconcileProfile(t, { nodePub: NODE });
    expect(t.submit).not.toHaveBeenCalled(); // never wipes the account
    const { loadAccountProfile } = await import("../../console/store/account-profile");
    expect(loadAccountProfile().name).toBe("Rae");
  });
});

describe("pushProfileEdit", () => {
  it("clears the bio with an explicit null when the user empties it", async () => {
    const t = transportFor(account({ bio: "old" }));
    await pushProfileEdit(t, { nodePub: NODE, bio: "   ", avatar: undefined });
    const call = (t.submit as ReturnType<typeof vi.fn>).mock.calls[0];
    expect((call[1] as { set_profile: { bio: null } }).set_profile.bio).toBeNull();
  });

  it("rejects when the node isn't linked to an account", async () => {
    const t = transportFor(null);
    await expect(
      pushProfileEdit(t, { nodePub: NODE, bio: "x", avatar: undefined }),
    ).rejects.toThrow(/isn't linked/);
  });
});
