import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { Job } from "../../../domain/jobs-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { JobsView } from "./JobsView";

const jobs: Job[] = [
  {
    job_id: "job-pending-123456",
    kind: "render",
    spec: "render frame 42",
    submitter: "alice-submitter-key",
    status: "pending",
    attempt: 0,
    claim: null,
    result: null,
    created_at_height: 10,
    updated_at_height: 10,
  },
  {
    job_id: "job-processing-123456",
    kind: "transcode",
    spec: "transcode clip 7",
    submitter: "bob-submitter-key",
    status: "processing",
    attempt: 1,
    claim: { worker: "carol-worker-key", claimed_at_height: 11, lease_views: 32 },
    result: null,
    created_at_height: 5,
    updated_at_height: 11,
  },
];

const renderJobs = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    status: {
      version: "0.1.0",
      appHash: "aa".repeat(32),
      height: 8,
      modules: [{ id: "jobs", root: "bb".repeat(32) }],
    },
    jobs,
    jobCounts: { pending: 1, processing: 1, done: 0, failed: 0, cancelled: 0 },
    ...patch,
  };
  const spies: Record<string, (...args: unknown[]) => void> = {};
  const noop = vi.fn() as (...args: unknown[]) => void;

  function Harness() {
    const [state] = useState(initialState);
    const actions = new Proxy(
      {},
      {
        get: (_target, key: string) => {
          spies[key] ??= vi.fn() as (...args: unknown[]) => void;
          return spies[key] ?? noop;
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <JobsView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { spies };
};

describe("JobsView", () => {
  it("submits the kind composer and exposes a claim control on pending jobs", () => {
    const { spies } = renderJobs();

    const kindInput = screen.getByLabelText(/kind/i);
    fireEvent.change(kindInput, { target: { value: "render" } });
    const specInput = screen.getByLabelText(/spec/i);
    fireEvent.change(specInput, { target: { value: "render frame 99" } });
    fireEvent.submit(kindInput.closest("form")!);

    expect(spies.submitJob).toHaveBeenCalledWith({ kind: "render", spec: "render frame 99" });
    expect(kindInput).toHaveValue("");

    fireEvent.click(screen.getByRole("button", { name: /^claim$/i }));
    expect(spies.claimJob).toHaveBeenCalledWith({ jobId: "job-pending-123456", leaseViews: 32 });
  });

  it("is honest when the jobs module is not backed by the node", () => {
    renderJobs({
      jobs: [],
      jobCounts: null,
      status: {
        version: "0.1.0",
        appHash: "aa".repeat(32),
        height: 8,
        modules: [{ id: "chat", root: "bb".repeat(32) }],
      },
    });

    expect(screen.getByText(/jobs module is not available/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/kind/i)).toBeDisabled();
  });
});
