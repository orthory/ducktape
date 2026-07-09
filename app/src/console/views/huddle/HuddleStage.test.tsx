// Render-level verification of the expanded huddle stage: gallery renders a tile
// per participant (self as "You"), the gallery/spotlight toggle flips the view,
// the control bar drives mute/leave, and collapse calls back. Media itself never
// runs here (getCallSession is a stubbed action → bind* no-op); we verify the
// layout + wiring with avatar placeholders, which is what a decode-less box shows.

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { keyHex } from "../../../domain/chat-client";
import type { Channel } from "../../../domain/chat-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { HuddleStage } from "./HuddleStage";

const selfNode = [9];
const selfHex = keyHex(selfNode);
const bytes = (t: string) => Array.from(new TextEncoder().encode(t));

const channel = (): Channel =>
  ({
    id: "ch",
    name: "general",
    huddle: [
      { user: bytes("me"), node: selfNode, joined_at: 0 },
      { user: bytes("bob"), node: [1], joined_at: 1 },
    ],
  }) as Channel;

const renderStage = (voicePatch: Partial<ConsoleState["voice"]> = {}) => {
  const base = createInitialState();
  const state: ConsoleState = {
    ...base,
    status: { publicKey: selfHex } as ConsoleState["status"],
    channels: [channel()],
    authorNames: {},
    videoCapability: { canEncode: false, canDecode: true, canScreenShare: false },
    voice: {
      ...base.voice,
      channelId: "ch",
      status: "live",
      muted: false,
      sessionStartMs: Date.now(),
      speaking: false,
      ...voicePatch,
    },
  };
  const spies: Record<string, ReturnType<typeof vi.fn>> = {};
  const actions = new Proxy({}, { get: (_t, k: string) => (spies[k] ??= vi.fn()) }) as ConsoleActions;
  const onCollapse = vi.fn();
  render(
    <ConsoleContext.Provider value={{ state, actions }}>
      <HuddleStage onCollapse={onCollapse} />
    </ConsoleContext.Provider>,
  );
  return { spies, onCollapse };
};

describe("HuddleStage", () => {
  it("renders the channel header and a gallery tile per participant", () => {
    renderStage();
    expect(screen.getByText("#general")).toBeInTheDocument();
    expect(screen.getByText("2 in call")).toBeInTheDocument();
    expect(screen.getByText("You")).toBeInTheDocument(); // self tile
    expect(screen.getByText("bob")).toBeInTheDocument(); // peer tile
  });

  it("toggles between gallery and spotlight", () => {
    renderStage();
    // Starts in gallery → the toggle offers "Spotlight".
    const toggle = screen.getByRole("button", { name: /spotlight/i });
    fireEvent.click(toggle);
    // Now in spotlight → the toggle offers "Gallery".
    expect(screen.getByRole("button", { name: /gallery/i })).toBeInTheDocument();
  });

  it("drives mute and leave from the control bar", () => {
    const { spies } = renderStage();
    fireEvent.click(screen.getByRole("button", { name: /^mic$/i }));
    expect(spies.setHuddleMuted).toHaveBeenCalledWith(true);
    fireEvent.click(screen.getByRole("button", { name: /leave/i }));
    expect(spies.leaveHuddle).toHaveBeenCalled();
  });

  it("hides the camera control when the runtime cannot encode", () => {
    renderStage(); // canEncode:false
    expect(screen.queryByRole("button", { name: /camera/i })).toBeNull();
  });

  it("collapses back to the dock", () => {
    const { onCollapse } = renderStage();
    fireEvent.click(screen.getByRole("button", { name: /collapse/i }));
    expect(onCollapse).toHaveBeenCalled();
  });
});
