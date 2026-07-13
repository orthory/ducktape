import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "./actions";
import { openDuckRef } from "./open-duck-ref";

const mockActions = () =>
  ({
    openPage: vi.fn(),
    setScreen: vi.fn(),
    openFiles: vi.fn(),
    openForgeItem: vi.fn(),
    selectChannel: vi.fn(),
    focusMessage: vi.fn(),
  }) as unknown as ConsoleActions;

describe("openDuckRef — the deep-link adapter", () => {
  it("opens a page and lands on the pages screen", () => {
    const a = mockActions();
    openDuckRef({ page: { id: "pg-1", label: "x" } }, a);
    expect(a.openPage).toHaveBeenCalledWith("pg-1");
    expect(a.setScreen).toHaveBeenCalledWith("pages");
  });

  it("opens a file in the files browser", () => {
    const a = mockActions();
    openDuckRef({ file: { path: "/shared/attachments/u/d.pdf", name: "d.pdf", embed: false } }, a);
    expect(a.openFiles).toHaveBeenCalledWith("/shared/attachments/u/d.pdf");
  });

  it("jumps to a forge item, with an optional discussion anchor", () => {
    const a = mockActions();
    openDuckRef({ forge: { repo: "ducktape", number: 58, seq: 12 } }, a);
    expect(a.openForgeItem).toHaveBeenCalledWith({ repo: "ducktape", number: 58, messageSeq: 12 });
    openDuckRef({ forge: { repo: "ducktape", number: null } }, a);
    expect(a.openForgeItem).toHaveBeenCalledWith({ repo: "ducktape", number: null });
  });

  it("selects a channel, focuses an anchored message, reroutes forge:* ids", () => {
    const a = mockActions();
    openDuckRef({ channel: { id: "general" } }, a);
    expect(a.setScreen).toHaveBeenCalledWith("chat");
    expect(a.selectChannel).toHaveBeenCalledWith("general");
    openDuckRef({ channel: { id: "general", seq: 42 } }, a);
    expect(a.focusMessage).toHaveBeenCalledWith("general", 42);
    openDuckRef({ channel: { id: "forge:ducktape:58", seq: 3 } }, a);
    expect(a.openForgeItem).toHaveBeenCalledWith({ repo: "ducktape", number: 58, messageSeq: 3 });
  });
});
