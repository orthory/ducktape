// Terminal surface: the create -> subscribe -> render -> input bridge between
// the node transport and xterm.js. xterm is mocked (jsdom has no canvas
// renderer); the point is the wiring, not the glyphs — a session is created for
// codex, its topic is subscribed, base64 output chunks decode onto the terminal,
// and keystrokes encode back as termInput.

import { act, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConsoleContext } from "../../console/store/context";
import type { ConsoleContextValue } from "../../console/store/context";
import { decodeTermChunk, termInputMsg, termResizeMsg } from "../../domain/term-client";
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
    dispose() {}
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

    await waitFor(() => expect(createTermSession).toHaveBeenCalledWith("codex"));
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
});
