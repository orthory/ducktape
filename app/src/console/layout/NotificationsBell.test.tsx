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

const markNative = () => {
  (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__ = vi.fn(() => Promise.resolve());
};

const item = (patch: Partial<NotifyItem> = {}): NotifyItem => ({
  category: "mention",
  title: "Ping",
  body: "hey there",
  channelId: "general",
  messageId: null,
  at: Date.now(),
  ...patch,
});

const renderBell = () => {
  const setScreen = vi.fn();
  const selectChannel = vi.fn();
  const openForgeItem = vi.fn();
  const actions = { setScreen, selectChannel, openForgeItem } as unknown as ConsoleActions;
  render(
    <ConsoleContext.Provider value={{ state: createInitialState(), actions }}>
      <NotificationsBell />
    </ConsoleContext.Provider>,
  );
  return { setScreen, selectChannel, openForgeItem };
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__;
  vi.clearAllMocks();
});

describe("NotificationsBell", () => {
  it("renders nothing on web", () => {
    renderBell();
    expect(screen.queryByLabelText("Notifications")).toBeNull();
  });

  it("shows the unread badge, marks seen on open, and navigates on item click", async () => {
    markNative();
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
    markNative();
    notifyMocks.recent.mockResolvedValueOnce({
      unread: 3,
      items: [
        item({ title: "Run done", category: "run", channelId: null }),
        item({
          title: "PR mention",
          channelId: "forge:repo:3",
          messageId: "mention-3",
        }),
        item({
          title: "PR reply",
          category: "reply",
          channelId: "forge:repo:4",
          messageId: "reply-4",
        }),
      ],
    });

    const { setScreen, selectChannel, openForgeItem } = renderBell();
    // The boot snapshot carries unread — the badge must show it even though
    // no unread event ever fired (the engine badges before the webview mounts).
    await waitFor(() =>
      expect(screen.getByLabelText("Notifications")).toHaveTextContent("3"),
    );
    fireEvent.click(screen.getByLabelText("Notifications"));

    fireEvent.click(await screen.findByRole("button", { name: "Open notification: Run done" }));
    expect(setScreen).toHaveBeenCalledWith("agent");
    expect(selectChannel).not.toHaveBeenCalled();

    fireEvent.click(screen.getByLabelText("Notifications"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Open notification: PR mention" }),
    );
    expect(openForgeItem).toHaveBeenCalledWith({
      repo: "repo",
      number: 3,
      messageId: "mention-3",
    });
    expect(selectChannel).not.toHaveBeenCalled();

    fireEvent.click(screen.getByLabelText("Notifications"));
    const prReply = await screen.findByRole("button", {
      name: "Open notification: PR reply",
    });
    fireEvent.click(prReply);
    // A forge-item channel jumps to the ITEM, not the repo list.
    expect(openForgeItem).toHaveBeenCalledWith({
      repo: "repo",
      number: 4,
      messageId: "reply-4",
    });
    expect(selectChannel).not.toHaveBeenCalled();
  });

  it("keeps a live item that lands before the boot snapshot resolves", async () => {
    markNative();
    let resolveSnapshot: (value: unknown) => void = () => {};
    notifyMocks.recent.mockImplementationOnce(
      () => new Promise((resolve) => (resolveSnapshot = resolve)),
    );
    let pushItem: (entry: NotifyItem) => void = () => {};
    notifyMocks.onItem.mockImplementation(async (cb: (entry: NotifyItem) => void) => {
      pushItem = cb;
      return () => {};
    });

    renderBell();
    await waitFor(() => expect(notifyMocks.onItem).toHaveBeenCalled());

    // A notification arrives while recent()'s IPC is still in flight...
    const live = item({ title: "Landed live", at: 2000 });
    await act(async () => pushItem(live));
    // ...then the stale snapshot (captured before it existed) resolves.
    await act(async () =>
      resolveSnapshot({ unread: 1, items: [item({ title: "From boot", at: 1000 })] }),
    );

    fireEvent.click(screen.getByLabelText("Notifications"));
    // Both share #general, so they land in one stack — expand it to see them.
    fireEvent.click(await screen.findByText("#general"));
    // Both survive — the merge keeps the live prepend above the boot items.
    expect(screen.getByText("Landed live")).toBeInTheDocument();
    expect(screen.getByText("From boot")).toBeInTheDocument();
  });

  it("stacks items sharing a channel and expands them on click", async () => {
    markNative();
    notifyMocks.recent.mockResolvedValueOnce({
      unread: 3,
      items: [
        item({ title: "Casey mentioned you", at: 3000 }),
        item({ title: "Ana mentioned you", at: 2000 }),
        item({ title: "Run done", category: "run", channelId: null, at: 1000 }),
      ],
    });

    const { selectChannel, setScreen } = renderBell();
    fireEvent.click(await screen.findByLabelText("Notifications"));

    // The two #general mentions collapse into one row; the lone run stays flat.
    const stack = await screen.findByText("#general");
    expect(screen.getByText("2 messages")).toBeInTheDocument();
    expect(screen.queryByText("Casey mentioned you")).toBeNull();
    expect(screen.getByText("Run done")).toBeInTheDocument();

    fireEvent.click(stack);
    expect(screen.getByText("Casey mentioned you")).toBeInTheDocument();
    expect(screen.getByText("Ana mentioned you")).toBeInTheDocument();

    // An expanded item still navigates, and collapsing hides them again.
    fireEvent.click(screen.getByText("Ana mentioned you"));
    expect(setScreen).toHaveBeenCalledWith("chat");
    expect(selectChannel).toHaveBeenCalledWith("general");
  });

  it("prepends live items and shows the empty state before any arrive", async () => {
    markNative();
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
