import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { NotifyItem } from "../../domain/notify-client";

const notifyMocks = vi.hoisted(() => ({
  recent: vi.fn(async (): Promise<unknown> => ({ unread: 0, items: [] })),
  onItem: vi.fn(async (_cb: (entry: unknown) => void) => () => {}),
  onUnread: vi.fn(async (_cb: (unread: number) => void) => () => {}),
  markSeen: vi.fn(async () => {}),
  configure: vi.fn(async () => {}),
}));

vi.mock("../../domain/notify-client", () => notifyMocks);

import { ConsoleContext } from "../store/context";
import type { ConsoleActions } from "../store/actions";
import { createInitialState } from "../store/state";
import { NotificationsBell } from "./NotificationsBell";

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

const item = (patch: Partial<NotifyItem> = {}): NotifyItem => ({
  category: "mention",
  title: "Ping",
  body: "hey there",
  channelId: "general",
  at: Date.now(),
  ...patch,
});

const renderBell = () => {
  const setScreen = vi.fn();
  const selectChannel = vi.fn();
  const actions = { setScreen, selectChannel } as unknown as ConsoleActions;
  render(
    <ConsoleContext.Provider value={{ state: createInitialState(), actions }}>
      <NotificationsBell />
    </ConsoleContext.Provider>,
  );
  return { setScreen, selectChannel };
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe("NotificationsBell", () => {
  it("renders nothing on web", () => {
    renderBell();
    expect(screen.queryByLabelText("Notifications")).toBeNull();
  });

  it("shows the unread badge, marks seen on open, and navigates on item click", async () => {
    markTauri();
    notifyMocks.recent.mockResolvedValueOnce({ unread: 0, items: [item()] });
    let pushUnread: (unread: number) => void = () => {};
    notifyMocks.onUnread.mockImplementation(async (cb) => {
      pushUnread = cb;
      return () => {};
    });

    const { setScreen, selectChannel } = renderBell();
    await waitFor(() => expect(notifyMocks.recent).toHaveBeenCalled());

    await act(async () => pushUnread(2));
    expect(screen.getByLabelText("Notifications")).toHaveTextContent("2");

    fireEvent.click(screen.getByLabelText("Notifications"));
    await waitFor(() => expect(notifyMocks.markSeen).toHaveBeenCalled());

    fireEvent.click(await screen.findByText("Ping"));
    expect(setScreen).toHaveBeenCalledWith("chat");
    expect(selectChannel).toHaveBeenCalledWith("general");
    // The dropdown closes on navigation.
    expect(screen.queryByText("Ping")).toBeNull();
  });

  it("falls back to the category screen without a channel, and reroutes forge-item channels", async () => {
    markTauri();
    notifyMocks.recent.mockResolvedValueOnce({
      unread: 2,
      items: [
        item({ title: "Run done", category: "run", channelId: null }),
        item({ title: "PR reply", category: "reply", channelId: "forge:repo:4" }),
      ],
    });

    const { setScreen, selectChannel } = renderBell();
    // The boot snapshot carries unread — the badge must show it even though
    // no unread event ever fired (the engine badges before the webview mounts).
    await waitFor(() =>
      expect(screen.getByLabelText("Notifications")).toHaveTextContent("2"),
    );
    fireEvent.click(screen.getByLabelText("Notifications"));

    fireEvent.click(await screen.findByText("Run done"));
    expect(setScreen).toHaveBeenCalledWith("agent");
    expect(selectChannel).not.toHaveBeenCalled();

    fireEvent.click(screen.getByLabelText("Notifications"));
    fireEvent.click(await screen.findByText("PR reply"));
    expect(setScreen).toHaveBeenCalledWith("forge");
    expect(selectChannel).not.toHaveBeenCalled();
  });

  it("prepends live items and shows the empty state before any arrive", async () => {
    markTauri();
    let pushItem: (entry: NotifyItem) => void = () => {};
    notifyMocks.onItem.mockImplementation(async (cb: (entry: NotifyItem) => void) => {
      pushItem = cb;
      return () => {};
    });

    renderBell();
    await waitFor(() => expect(notifyMocks.onItem).toHaveBeenCalled());

    fireEvent.click(screen.getByLabelText("Notifications"));
    expect(await screen.findByText("No notifications")).toBeInTheDocument();

    await act(async () => pushItem(item({ title: "Fresh" })));
    expect(screen.getByText("Fresh")).toBeInTheDocument();
  });
});
