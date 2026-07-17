import { afterEach, describe, expect, it, vi } from "vitest";

import { openExternal } from "./external-link";

// jsdom has no __DUCKTAPE_TEST_NATIVE_INVOKE__, so openExternal takes the web path and
// calls window.open — spy on it to assert which schemes are honored.

afterEach(() => {
  vi.restoreAllMocks();
});

describe("openExternal", () => {
  it("opens http(s) URLs in a new tab", () => {
    const open = vi.spyOn(window, "open").mockReturnValue(null);

    openExternal("https://example.com/x");
    openExternal("http://example.com/y");

    expect(open).toHaveBeenCalledTimes(2);
    expect(open).toHaveBeenCalledWith("https://example.com/x", "_blank", "noopener,noreferrer");
  });

  it("rejects non-http(s) schemes", () => {
    const open = vi.spyOn(window, "open").mockReturnValue(null);

    for (const url of ["file:///etc/passwd", "javascript:alert(1)", "mailto:a@b.c", "/relative"]) {
      openExternal(url);
    }

    expect(open).not.toHaveBeenCalled();
  });
});
