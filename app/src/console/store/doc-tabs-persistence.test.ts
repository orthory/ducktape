import { beforeEach, describe, expect, it } from "vitest";

import {
  docTabsScope,
  loadDocTabs,
  saveDocTabs,
} from "./state";

describe("workspace-scoped document tabs", () => {
  beforeEach(() => localStorage.clear());

  it("keeps local workspaces and remote nodes independent", () => {
    const team = docTabsScope("team", null);
    const lab = docTabsScope("lab", null);
    const remote = docTabsScope(null, "http://127.0.0.1:8844");

    saveDocTabs(team, ["team-plan"]);
    saveDocTabs(lab, ["lab-notes"]);
    saveDocTabs(remote, ["remote-runbook"]);

    expect(loadDocTabs(team)).toEqual(["team-plan"]);
    expect(loadDocTabs(lab)).toEqual(["lab-notes"]);
    expect(loadDocTabs(remote)).toEqual(["remote-runbook"]);
  });

  it("does not assign the legacy global tab list to an arbitrary workspace", () => {
    localStorage.setItem("ducktape.docTabs", JSON.stringify(["old-page"]));
    expect(loadDocTabs(docTabsScope("fresh", null))).toEqual([]);
  });
});
