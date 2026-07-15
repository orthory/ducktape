// Terminal surface: the create -> subscribe -> render -> input bridge between
// the node transport and xterm.js. xterm is mocked (jsdom has no canvas
// renderer); the point is the wiring, not the glyphs — a session is created for
// codex, its topic is subscribed, base64 output chunks decode onto the terminal,
// and keystrokes encode back as termInput.

import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConsoleContext } from "../../console/store/context";
import type { ConsoleContextValue } from "../../console/store/context";
import {
  decodeTermChunk,
  termCommandMsg,
  termInputMsg,
  termResizeMsg,
} from "../../domain/term-client";
import type { TopicHandlers } from "../../domain/transport";
import { makeTransportStub } from "../transport-stub";

const xterm = vi.hoisted(() => ({
  writes: [] as (Uint8Array | string)[],
  dataCb: null as null | ((d: string) => void),
}));

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    write(d: Uint8Array | string) {
      xterm.writes.push(d);
    }
    onData(cb: (d: string) => void) {
      xterm.dataCb = cb;
      return { dispose() {} };
    }
    onResize() {
      return { dispose() {} };
    }
    loadAddon() {}
    open() {}
    focus() {}
    dispose() {
      // a disposed terminal has no data callback — lets a test observe that the
      // raw keystroke path is left UNWIRED after a mode switch to shared.
      xterm.dataCb = null;
    }
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
    activate() {}
    dispose() {}
  },
}));

import { TerminalView } from "../../console/views/terminal/TerminalView";

afterEach(() => {
  xterm.writes.length = 0;
  xterm.dataCb = null;
  vi.restoreAllMocks();
});

describe("term-client wire helpers", () => {
  it("round-trips bytes through base64 and shapes the ops", () => {
    // termInput UTF-8-encodes then base64s; decodeTermChunk is its inverse.
    const encoded = termInputMsg("s1", "héllo").data;
    expect(new TextDecoder().decode(decodeTermChunk(encoded))).toBe("héllo");
    expect(termInputMsg("s1", "x")).toEqual({ op: "termInput", session: "s1", data: btoa("x") });
    expect(termResizeMsg("s1", 80, 24)).toEqual({
      op: "termResize",
      session: "s1",
      cols: 80,
      rows: 24,
    });
    // a shared-session command carries plain text (not base64) + the author.
    expect(termCommandMsg("s1", "ls -la", "ext:me")).toEqual({
      op: "termCommand",
      session: "s1",
      text: "ls -la",
      origin: "ext:me",
    });
  });
});

describe("terminal surface", () => {
  it("creates a codex session, subscribes, renders output, and sends input", async () => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        disconnect() {}
      },
    );
    let handlers: TopicHandlers | null = null;
    const subscribe = vi.fn((_topics: string[], h: TopicHandlers) => {
      handlers = h;
      return () => {};
    });
    const createTermSession = vi.fn().mockResolvedValue({ sessionId: "s1", topic: "term:s1" });
    const sendTerm = vi.fn();
    const transport = makeTransportStub({ subscribe, createTermSession, sendTerm });

    await act(async () => {
      render(
        <ConsoleContext.Provider value={{ transport } as unknown as ConsoleContextValue}>
          <TerminalView />
        </ConsoleContext.Provider>,
      );
    });

    await waitFor(() => expect(createTermSession).toHaveBeenCalledWith("codex", "single"));
    await waitFor(() => expect(subscribe).toHaveBeenCalled());
    expect(subscribe.mock.calls[0][0]).toEqual(["term:s1"]);
    // an initial resize rides after fit(), at the terminal's geometry.
    expect(sendTerm).toHaveBeenCalledWith(termResizeMsg("s1", 80, 24));

    // server output: a base64 chunk decodes onto the terminal.
    await act(async () => {
      handlers!.onTermChunk!(btoa("hi"));
    });
    const lastWrite = xterm.writes[xterm.writes.length - 1] as Uint8Array;
    expect(new TextDecoder().decode(lastWrite)).toBe("hi");

    // keystroke: xterm.onData -> termInput with the bytes base64-encoded.
    await act(async () => {
      xterm.dataCb!("x");
    });
    expect(sendTerm).toHaveBeenCalledWith(termInputMsg("s1", "x"));
  });

  it("shared mode: command lane in, ordered log out, no raw input", async () => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        disconnect() {}
      },
    );
    const subscribed: string[][] = [];
    let handlers: TopicHandlers | null = null;
    const subscribe = vi.fn((topics: string[], h: TopicHandlers) => {
      subscribed.push(topics);
      handlers = h;
      return () => {};
    });
    const createTermSession = vi.fn().mockResolvedValue({ sessionId: "s1", topic: "term:s1" });
    const sendTerm = vi.fn();
    const transport = makeTransportStub({ subscribe, createTermSession, sendTerm });
    const state = { author: "ext:me" };

    render(
      <ConsoleContext.Provider
        value={{ transport, state } as unknown as ConsoleContextValue}
      >
        <TerminalView />
      </ConsoleContext.Provider>,
    );

    // starts single (default); wait until the raw keystroke path is wired so the
    // switch-to-shared genuinely tears it back down.
    await waitFor(() => expect(createTermSession).toHaveBeenCalledWith("codex", "single"));
    await waitFor(() => expect(xterm.dataCb).not.toBeNull());

    // flip the header toggle to shared.
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /shared/i }));
    });
    await waitFor(() => expect(createTermSession).toHaveBeenCalledWith("codex", "shared"));

    // shared subscribes BOTH the output topic and the command-log topic.
    await waitFor(() =>
      expect(subscribed[subscribed.length - 1]).toEqual(["term:s1", "term-cmd:s1"]),
    );
    // and leaves raw keystrokes UNWIRED (node refuses raw_input_on_shared).
    expect(xterm.dataCb).toBeNull();

    // the command box sends a termCommand — plain text + the author origin.
    const box = screen.getByRole("textbox", { name: /command/i });
    await act(async () => {
      fireEvent.change(box, { target: { value: "ls -la" } });
      fireEvent.submit(box.closest("form")!);
    });
    expect(sendTerm).toHaveBeenCalledWith(termCommandMsg("s1", "ls -la", "ext:me"));

    // the ordered command log renders seq · origin · text rows.
    await act(async () => {
      handlers!.onTermCommandLog!(1, "ext:alice", "whoami");
    });
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("ext:alice")).toBeInTheDocument();
    expect(screen.getByText("whoami")).toBeInTheDocument();
  });
});
