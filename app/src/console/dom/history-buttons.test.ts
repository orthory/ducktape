// history-buttons: pointer back/forward buttons and Alt+Arrow drive injected
// history traversal; editables keep Alt+Arrow (macOS word-caret movement); the
// engine's own traversal is cancelled so nothing ever double-navigates.

import { afterEach, describe, expect, it, vi } from "vitest";

import { installHistoryButtons } from "./history-buttons";

const nav = () => ({ back: vi.fn(), forward: vi.fn() });

let uninstall: (() => void) | null = null;

afterEach(() => {
  uninstall?.();
  uninstall = null;
  document.body.innerHTML = "";
});

describe("pointer buttons", () => {
  it("button 3 goes back, button 4 goes forward, defaults cancelled", () => {
    const history = nav();
    uninstall = installHistoryButtons(history);

    const back = new MouseEvent("mouseup", { button: 3, cancelable: true });
    window.dispatchEvent(back);
    expect(history.back).toHaveBeenCalledTimes(1);
    expect(back.defaultPrevented).toBe(true);

    const forward = new MouseEvent("mouseup", { button: 4, cancelable: true });
    window.dispatchEvent(forward);
    expect(history.forward).toHaveBeenCalledTimes(1);
    expect(forward.defaultPrevented).toBe(true);
  });

  it("ignores primary/middle/secondary buttons", () => {
    const history = nav();
    uninstall = installHistoryButtons(history);
    for (const button of [0, 1, 2]) {
      window.dispatchEvent(new MouseEvent("mouseup", { button, cancelable: true }));
    }
    expect(history.back).not.toHaveBeenCalled();
    expect(history.forward).not.toHaveBeenCalled();
  });
});

describe("Alt+Arrow", () => {
  it("Alt+ArrowLeft goes back, Alt+ArrowRight goes forward", () => {
    const history = nav();
    uninstall = installHistoryButtons(history);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", altKey: true, cancelable: true }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", altKey: true, cancelable: true }));
    expect(history.back).toHaveBeenCalledTimes(1);
    expect(history.forward).toHaveBeenCalledTimes(1);
  });

  it("stays out of editables — the caret keeps Option/Alt+Arrow", () => {
    const history = nav();
    uninstall = installHistoryButtons(history);
    const input = document.createElement("input");
    document.body.appendChild(input);
    const event = new KeyboardEvent("keydown", {
      key: "ArrowLeft",
      altKey: true,
      bubbles: true,
      cancelable: true,
    });
    input.dispatchEvent(event);
    expect(history.back).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });

  it("requires a bare Alt chord (no Ctrl/Meta/Shift) and arrow keys only", () => {
    const history = nav();
    uninstall = installHistoryButtons(history);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", altKey: true, shiftKey: true }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "a", altKey: true }));
    expect(history.back).not.toHaveBeenCalled();
  });
});

describe("uninstall", () => {
  it("removes both listeners", () => {
    const history = nav();
    const remove = installHistoryButtons(history);
    remove();
    window.dispatchEvent(new MouseEvent("mouseup", { button: 3, cancelable: true }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", altKey: true }));
    expect(history.back).not.toHaveBeenCalled();
  });
});
