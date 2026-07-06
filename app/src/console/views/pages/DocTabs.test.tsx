import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { DocTabs } from "./DocTabs";

describe("DocTabs", () => {
  it("renders a tab per open page and fires select/close", () => {
    const onSelect = vi.fn();
    const onClose = vi.fn();
    const { getByRole } = render(
      <DocTabs
        open={["p1", "p2"]}
        active="p1"
        titleOf={(id) => (id === "p1" ? "Alpha" : "")}
        onSelect={onSelect}
        onClose={onClose}
      />,
    );
    getByRole("tab", { name: /alpha/i }).click();
    expect(onSelect).toHaveBeenCalledWith("p1");
    // p2 has an empty title → "Untitled"
    getByRole("button", { name: /close untitled/i }).click();
    expect(onClose).toHaveBeenCalledWith("p2");
  });

  it("renders nothing when no tabs are open", () => {
    const { container } = render(
      <DocTabs open={[]} active={null} titleOf={() => ""} onSelect={vi.fn()} onClose={vi.fn()} />,
    );
    expect(container.firstChild).toBeNull();
  });
});
