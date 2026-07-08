import { afterEach, describe, expect, it } from "vitest";

import { installAutocompleteDefault } from "./suppress-autocomplete";

// MutationObserver delivers on a microtask/task boundary; flush both.
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

let dispose: (() => void) | undefined;

afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = "";
});

describe("installAutocompleteDefault", () => {
  it("stamps autocomplete=off on inputs and textareas already in the DOM", () => {
    document.body.innerHTML = `<input id="a" /><textarea id="b"></textarea>`;

    dispose = installAutocompleteDefault();

    expect(document.getElementById("a")).toHaveAttribute("autocomplete", "off");
    expect(document.getElementById("b")).toHaveAttribute("autocomplete", "off");
  });

  it("stamps autocorrect=off and autocapitalize=off (macOS WKWebView completion)", () => {
    document.body.innerHTML = `<input id="a" /><textarea id="b"></textarea>`;

    dispose = installAutocompleteDefault();

    for (const id of ["a", "b"]) {
      expect(document.getElementById(id)).toHaveAttribute("autocorrect", "off");
      expect(document.getElementById(id)).toHaveAttribute("autocapitalize", "off");
    }
  });

  it("preserves an explicit autocomplete value (deliberate opt-in)", () => {
    document.body.innerHTML = `<input id="pw" autocomplete="new-password" />`;

    dispose = installAutocompleteDefault();

    expect(document.getElementById("pw")).toHaveAttribute("autocomplete", "new-password");
  });

  it("preserves explicit autocorrect/autocapitalize values (prose opt-in)", () => {
    document.body.innerHTML = `<textarea id="prose" autocorrect="on" autocapitalize="sentences"></textarea>`;

    dispose = installAutocompleteDefault();

    expect(document.getElementById("prose")).toHaveAttribute("autocorrect", "on");
    expect(document.getElementById("prose")).toHaveAttribute("autocapitalize", "sentences");
  });

  it("never touches spellcheck — squiggles stay a per-field prose choice", () => {
    document.body.innerHTML = `<textarea id="plain"></textarea>`;

    dispose = installAutocompleteDefault();

    expect(document.getElementById("plain")).not.toHaveAttribute("spellcheck");
  });

  it("stamps inputs mounted after install", async () => {
    dispose = installAutocompleteDefault();

    const host = document.createElement("div");
    host.innerHTML = `<span><input id="late" /></span>`;
    document.body.appendChild(host);
    await flush();

    expect(document.getElementById("late")).toHaveAttribute("autocomplete", "off");
  });

  it("stamps an input added as a bare node after install", async () => {
    dispose = installAutocompleteDefault();

    const input = document.createElement("input");
    input.id = "bare";
    document.body.appendChild(input);
    await flush();

    expect(document.getElementById("bare")).toHaveAttribute("autocomplete", "off");
  });

  it("leaves a late input's explicit value untouched", async () => {
    dispose = installAutocompleteDefault();

    const input = document.createElement("input");
    input.id = "current";
    input.setAttribute("autocomplete", "current-password");
    document.body.appendChild(input);
    await flush();

    expect(document.getElementById("current")).toHaveAttribute("autocomplete", "current-password");
  });

  it("stops stamping after the disposer runs", async () => {
    installAutocompleteDefault()();

    const input = document.createElement("input");
    input.id = "post-dispose";
    document.body.appendChild(input);
    await flush();

    expect(document.getElementById("post-dispose")).not.toHaveAttribute("autocomplete");
  });
});
