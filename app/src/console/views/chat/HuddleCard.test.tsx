// Render-level verification of the huddle card's new roster affordances: the
// per-member mute glyph + stale "remove" (SweepHuddle) control that make mute
// and eviction reachable in an audio-only huddle, the "+N more" tail, and the
// "you're muted while talking" banner. Presentational + prop-driven, so it
// renders in jsdom without a store or media.

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { HuddleParticipant } from "../../store/huddle-roster";
import { HuddleCard } from "./HuddleCard";

const participant = (over: Partial<HuddleParticipant> & { name: string }): HuddleParticipant => ({
  key: over.name,
  muted: false,
  stale: false,
  isSelf: false,
  speaking: false,
  user: Array.from(new TextEncoder().encode(over.name)),
  ...over, // supplies name (required), and overrides key/user when given
});

const baseProps = {
  channelName: "general",
  status: "live" as const,
  error: null,
  muted: false,
  onSetMuted: vi.fn(),
  onLeave: vi.fn(),
  onRetry: vi.fn(),
};

describe("HuddleCard roster", () => {
  it("renders a row per member with the self marker", () => {
    render(
      <HuddleCard
        {...baseProps}
        participants={[participant({ name: "me", isSelf: true }), participant({ name: "bob" })]}
      />,
    );
    expect(screen.getByText("me")).toBeTruthy();
    expect(screen.getByText("bob")).toBeTruthy();
    expect(screen.getByText(/you/i)).toBeTruthy(); // self " · you" marker
  });

  it("offers a 'remove' control on a stale member and fires onSweep with the user bytes", () => {
    const onSweep = vi.fn();
    const bobBytes = Array.from(new TextEncoder().encode("bob"));
    render(
      <HuddleCard
        {...baseProps}
        onSweep={onSweep}
        participants={[
          participant({ name: "me", isSelf: true }),
          participant({ name: "bob", stale: true, user: bobBytes }),
        ]}
      />,
    );
    const remove = screen.getByText("remove");
    fireEvent.click(remove);
    expect(onSweep).toHaveBeenCalledWith(bobBytes);
  });

  it("does not offer 'remove' when onSweep is absent (e.g. read-only surface)", () => {
    render(
      <HuddleCard
        {...baseProps}
        participants={[participant({ name: "bob", stale: true })]}
      />,
    );
    expect(screen.queryByText("remove")).toBeNull();
  });

  it("shows a '+N more' tail past maxRows", () => {
    const many = Array.from({ length: 7 }, (_, i) => participant({ name: `u${i}` }));
    render(<HuddleCard {...baseProps} maxRows={4} participants={many} />);
    expect(screen.getByText("+3 more")).toBeTruthy();
  });

  it("keeps a stale member's 'remove' reachable even when it falls past maxRows", () => {
    const onSweep = vi.fn();
    const deadBytes = Array.from(new TextEncoder().encode("dead"));
    // 6 members, cap 2, the stale one is last (index 5) — would be hidden by a
    // naive slice, but its remove control must still render.
    const roster = [
      ...Array.from({ length: 5 }, (_, i) => participant({ name: `u${i}` })),
      participant({ name: "dead", stale: true, user: deadBytes }),
    ];
    render(<HuddleCard {...baseProps} maxRows={2} onSweep={onSweep} participants={roster} />);
    fireEvent.click(screen.getByText("remove"));
    expect(onSweep).toHaveBeenCalledWith(deadBytes);
    expect(screen.getByText("+3 more")).toBeTruthy(); // the 3 non-stale hidden rows
  });

  it("shows the 'You're muted' banner only when the self member is muted AND speaking", () => {
    const { rerender } = render(
      <HuddleCard
        {...baseProps}
        muted
        participants={[participant({ name: "me", isSelf: true, muted: true, speaking: true })]}
      />,
    );
    expect(screen.getByText(/you.re muted/i)).toBeTruthy();

    // Muted but silent → no banner.
    rerender(
      <HuddleCard
        {...baseProps}
        muted
        participants={[participant({ name: "me", isSelf: true, muted: true, speaking: false })]}
      />,
    );
    expect(screen.queryByText(/you.re muted/i)).toBeNull();
  });

  it("shows the error copy and a Retry (not mute) on failure", () => {
    render(
      <HuddleCard
        {...baseProps}
        status="error"
        error="mic-denied"
        participants={[]}
      />,
    );
    expect(screen.getByText(/system settings/i)).toBeTruthy();
    expect(screen.getByText("Retry")).toBeTruthy();
  });
});
