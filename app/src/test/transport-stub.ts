import { vi } from "vitest";

import type { NodeTransport, SubmitReceipt } from "../domain/transport";

export const makeTransportStub = (
  overrides: Partial<NodeTransport> = {},
): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) } satisfies SubmitReceipt),
  submitControl: vi
    .fn()
    .mockResolvedValue({ height: 1, appHash: "aa".repeat(32) } satisfies SubmitReceipt),
  query: vi.fn().mockResolvedValue({}),
  view: vi.fn().mockResolvedValue({ hits: [] }),
  putBlob: vi.fn().mockResolvedValue("ab".repeat(32)),
  getBlob: vi.fn().mockResolvedValue(new Uint8Array()),
  status: vi.fn().mockResolvedValue({
    version: "0.1.0",
    appHash: "aa".repeat(32),
    height: 1,
    modules: [],
  }),
  blocks: vi.fn().mockResolvedValue([]),
  filesStage: vi.fn(),
  filesCommit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  filesStat: vi.fn(),
  filesLs: vi.fn(),
  filesRead: vi.fn(),
  filesHistory: vi.fn(),
  subscribe: vi.fn(() => () => {}),
  onStream: vi.fn(() => () => {}),
  ...overrides,
});
