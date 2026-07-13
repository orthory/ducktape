// The members-only panel's two traps: posting is gated per NODE key, so an
// account's every device needs its own row (three rows reading "Jess" is how
// the wrong one gets added), and an add must show feedback from the click —
// the store's optimistic projection puts the member row (and its finalization
// mark) there before the block lands.

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { keyBytes } from "../../../domain/chat-client";
import type { Channel } from "../../../domain/chat-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { opKey } from "../../store/finalization";
import type { OpLedger } from "../../store/finalization";
import { createInitialState } from "../../store/state";
import { ChannelMembersButton } from "./ChannelMembers";

const channel: Channel = {
  id: "secret",
  name: "Secret",
  created_at: 0,
  head_seq: 0,
  post_policy: "members_only",
  hooks: [],
  pinned: [],
};

// Jess posts from two devices; only the first is a member. Self is the embedded
// daemon's origin bytes ("operator"), the identity selfAuthorBytes reports with
// no node pubkey on status.
const jessA = `a1b2c3${"0".repeat(58)}`;
const jessB = `d4e5f6${"0".repeat(58)}`;
const selfHex = "6f70657261746f72"; // utf8 "operator"

const openPanel = (over: { members?: string[]; ops?: OpLedger } = {}) => {
  const setChannelMembership = vi.fn();
  const members = over.members ?? [jessA];
  render(
    <ConsoleContext.Provider
      value={{
        state: {
          ...createInitialState(),
          author: "operator",
          activeChannel: channel.id,
          channelMembers: members.map(keyBytes),
          nodeUsers: {
            [jessA]: { accountId: "acct-jess", name: "Jess" },
            [jessB]: { accountId: "acct-jess", name: "Jess" },
            [selfHex]: { accountId: "acct-self", name: "Operator" },
          },
          authorNames: { [jessA]: "Jess", [jessB]: "Jess", [selfHex]: "Operator" },
          ops: over.ops ?? {},
        },
        actions: {
          setChannelMembership,
          refreshChannelMembers: vi.fn(),
        } as unknown as ConsoleActions,
      }}
    >
      <ChannelMembersButton channel={channel} />
    </ConsoleContext.Provider>,
  );
  fireEvent.click(screen.getByTitle("Manage channel members"));
  return setChannelMembership;
};

describe("ChannelMembersButton", () => {
  it("labels every device with its node tag, marks our own, and adds by node key", () => {
    const setChannelMembership = openPanel();

    // Both of Jess's devices are listed, told apart by their node tag — the
    // added one marked and inert, the unadmitted one addable.
    expect(screen.getByLabelText(`Jess ${jessA.slice(0, 6)} — added`)).toBeTruthy();
    const add = screen.getByTitle(`Add Jess's device ${jessB.slice(0, 6)}`);
    expect(screen.getAllByText("Jess").length).toBe(3); // member row + both devices
    expect(screen.getByText(/this device/)).toBeTruthy(); // our own node marked

    fireEvent.click(add);
    expect(setChannelMembership).toHaveBeenCalledWith(channel.id, keyBytes(jessB), true);
  });

  it("renders the in-flight mark on the pending member row the optimistic add projected", () => {
    openPanel({
      members: [jessA, jessB],
      ops: {
        [opKey.membership(channel.id, jessB)]: {
          seq: 1,
          phase: "pending",
          startedAt: Date.now(),
        },
      },
    });

    expect(screen.getByLabelText("sent — awaiting confirmation")).toBeTruthy();
  });
});
