// The duckfs client mirrors files-interface (crates/apps/files/src/wire.rs):
// upload chunks large files at CHUNK_SIZE, staging each via filesStage and
// referencing the returned digests in a Commit; small files ride inline. Reads
// page (readAll loops read to eof); refs + diff take the generic query lane.
// Everything is a pure function over an injected NodeTransport.

import { describe, expect, it, vi } from "vitest";

import * as files from "./files-client";
import { NodeError } from "./transport";
import type { FileReadRange, FileSnapshot, NodeTransport } from "./transport";
import { makeTransportStub } from "../test/transport-stub";

const b64 = (...bytes: number[]): string => btoa(String.fromCharCode(...bytes));

// House stub pattern, extended to the duckfs plane. `query` defaults to an empty
// filesystem (head null) so an upload's base-snapshot resolves without a stub.
const fakeTransport = (overrides: Partial<NodeTransport> = {}): NodeTransport => ({
  ...makeTransportStub({
    query: vi.fn().mockResolvedValue({ refs: { head: null, pins: {}, window_len: 0 } }),
    ...overrides,
  }),
});

describe("uploadFile", () => {
  it("commits a small file inline, without staging any chunk", async () => {
    const filesCommit = vi.fn().mockResolvedValue({ height: 7, appHash: "bb".repeat(32) });
    const filesStage = vi.fn();
    const transport = fakeTransport({ filesCommit, filesStage });

    const block = await files.uploadFile(transport, {
      path: "/shared/hi.bin",
      bytes: new Uint8Array([1, 2, 3, 4]),
    });

    expect(filesStage).not.toHaveBeenCalled();
    expect(block).toEqual({ height: 7, appHash: "bb".repeat(32) });

    const body = filesCommit.mock.calls[0][0];
    expect(body.base_snapshot).toBeNull(); // empty-fs head
    expect(body.changes).toHaveLength(1);
    const put = body.changes[0].put;
    expect(put.path).toBe("/shared/hi.bin");
    expect(put.content).toEqual({ inline: { b64: b64(1, 2, 3, 4) } });
  });

  it("stages one chunk per 1 MiB and references the digests in order", async () => {
    const digests = ["11".repeat(32), "22".repeat(32), "33".repeat(32)];
    let served = 0;
    const filesStage = vi
      .fn()
      .mockImplementation(() => Promise.resolve({ digest: digests[served++] }));
    const filesCommit = vi.fn().mockResolvedValue({ height: 9, appHash: "cc".repeat(32) });
    const transport = fakeTransport({ filesStage, filesCommit });

    // 2.5 MiB → three chunks: 1 MiB, 1 MiB, 0.5 MiB.
    const size = files.CHUNK_SIZE * 2 + files.CHUNK_SIZE / 2;
    const progress: number[] = [];

    await files.uploadFile(transport, {
      path: "/shared/big.bin",
      bytes: new Uint8Array(size),
      onProgress: (staged, total) => progress.push(staged / total),
    });

    expect(filesStage).toHaveBeenCalledTimes(3);
    for (const [chunk] of filesStage.mock.calls) {
      expect((chunk as Uint8Array).length).toBeLessThanOrEqual(files.CHUNK_SIZE);
    }
    // last chunk is the 0.5 MiB remainder.
    expect((filesStage.mock.calls[2][0] as Uint8Array).length).toBe(files.CHUNK_SIZE / 2);

    const content = filesCommit.mock.calls[0][0].changes[0].put.content;
    expect(content.chunks.size).toBe(size);
    expect(content.chunks.chunks).toEqual(digests); // ordered, as staged
    expect(progress).toEqual([1 / 3, 2 / 3, 1]);
  });

  it("uses the live head as the commit base for CAS", async () => {
    const filesCommit = vi.fn().mockResolvedValue({ height: 4, appHash: "dd".repeat(32) });
    const query = vi
      .fn()
      .mockResolvedValue({ refs: { head: "ee".repeat(32), pins: {}, window_len: 2 } });
    const transport = fakeTransport({ filesCommit, query });

    await files.uploadFile(transport, { path: "/shared/x", bytes: new Uint8Array([9]) });

    expect(filesCommit.mock.calls[0][0].base_snapshot).toBe("ee".repeat(32));
  });

  it("surfaces a per-path CAS conflict rejection verbatim", async () => {
    const filesCommit = vi
      .fn()
      .mockRejectedValue(
        new NodeError("httpError", "files: conflict: /shared/x changed since base", 400),
      );
    const transport = fakeTransport({ filesCommit });

    await expect(
      files.uploadFile(transport, { path: "/shared/x", bytes: new Uint8Array([1]) }),
    ).rejects.toThrow(/files: conflict: \/shared\/x changed since base/);
  });
});

describe("uploadFiles", () => {
  it("commits a folder as one atomic change set with preserved relative paths", async () => {
    const digests = ["11".repeat(32), "22".repeat(32)];
    let served = 0;
    const filesStage = vi
      .fn()
      .mockImplementation(() => Promise.resolve({ digest: digests[served++] }));
    const filesCommit = vi.fn().mockResolvedValue({ height: 10, appHash: "44".repeat(32) });
    const query = vi
      .fn()
      .mockResolvedValue({ refs: { head: "55".repeat(32), pins: {}, window_len: 2 } });
    const transport = fakeTransport({ filesStage, filesCommit, query });

    await files.uploadFiles(transport, {
      targetDir: "/shared",
      entries: [
        {
          kind: "file",
          relativePath: "Project/readme.txt",
          bytes: new Uint8Array([1, 2, 3]),
          meta: { mime: "text/plain" },
        },
        {
          kind: "file",
          relativePath: "Project/docs/plan.md",
          bytes: new Uint8Array([4, 5]),
          meta: { mime: "text/markdown" },
        },
      ],
      message: "upload folder Project",
    });

    expect(filesStage).toHaveBeenCalledTimes(2);
    expect(filesStage.mock.calls.map(([chunk]) => Array.from(chunk as Uint8Array))).toEqual([
      [1, 2, 3],
      [4, 5],
    ]);

    const body = filesCommit.mock.calls[0][0];
    expect(body.base_snapshot).toBe("55".repeat(32));
    expect(body.message).toBe("upload folder Project");
    expect(body.changes).toEqual([
      {
        put: {
          path: "/shared/Project/readme.txt",
          exec: false,
          meta: { mime: "text/plain" },
          content: { chunks: { size: 3, chunks: [digests[0]] } },
        },
      },
      {
        put: {
          path: "/shared/Project/docs/plan.md",
          exec: false,
          meta: { mime: "text/markdown" },
          content: { chunks: { size: 2, chunks: [digests[1]] } },
        },
      },
    ]);
  });

  it("can include an empty directory and an empty file in the same folder commit", async () => {
    const filesStage = vi.fn();
    const filesCommit = vi.fn().mockResolvedValue({ height: 11, appHash: "66".repeat(32) });
    const transport = fakeTransport({ filesStage, filesCommit });

    await files.uploadFiles(transport, {
      targetDir: "/shared",
      entries: [
        { kind: "dir", relativePath: "EmptyFolder" },
        { kind: "file", relativePath: "EmptyFolder/.keep", bytes: new Uint8Array() },
      ],
    });

    expect(filesStage).not.toHaveBeenCalled();
    expect(filesCommit.mock.calls[0][0].changes).toEqual([
      { mkdir: { path: "/shared/EmptyFolder" } },
      {
        put: {
          path: "/shared/EmptyFolder/.keep",
          exec: false,
          meta: {},
          content: { chunks: { size: 0, chunks: [] } },
        },
      },
    ]);
  });

  it("rejects duplicate target paths before staging any chunks", async () => {
    const filesStage = vi.fn();
    const filesCommit = vi.fn();
    const transport = fakeTransport({ filesStage, filesCommit });

    await expect(
      files.uploadFiles(transport, {
        targetDir: "/shared",
        entries: [
          { kind: "file", relativePath: "Project/readme.txt", bytes: new Uint8Array([1]) },
          { kind: "file", relativePath: "Project/readme.txt", bytes: new Uint8Array([2]) },
        ],
      }),
    ).rejects.toThrow(/duplicate upload path: \/shared\/Project\/readme\.txt/);

    expect(filesStage).not.toHaveBeenCalled();
    expect(filesCommit).not.toHaveBeenCalled();
  });

  it("rejects an over-cap full target path before staging any chunks", async () => {
    const filesStage = vi.fn();
    const filesCommit = vi.fn();
    const transport = fakeTransport({ filesStage, filesCommit });

    await expect(
      files.uploadFiles(transport, {
        targetDir: `/${"a".repeat(files.MAX_PATH_BYTES)}`,
        entries: [{ kind: "file", relativePath: "x.txt", bytes: new Uint8Array([1]) }],
      }),
    ).rejects.toThrow(/upload target path exceeds the byte cap/);

    expect(filesStage).not.toHaveBeenCalled();
    expect(filesCommit).not.toHaveBeenCalled();
  });
});

describe("readAll", () => {
  it("pages read calls until eof and concatenates the bytes", async () => {
    const pages: FileReadRange[] = [
      { b64: b64(1, 2, 3), eof: false },
      { b64: b64(4, 5), eof: true },
    ];
    const offsets: number[] = [];
    const filesRead = vi.fn().mockImplementation((p: { offset?: number }) => {
      offsets.push(p.offset ?? 0);
      return Promise.resolve(pages.shift());
    });
    const transport = fakeTransport({ filesRead });

    const bytes = await files.readAll(transport, { path: "/a.bin" });

    expect(Array.from(bytes)).toEqual([1, 2, 3, 4, 5]);
    expect(filesRead).toHaveBeenCalledTimes(2);
    expect(offsets).toEqual([0, 3]); // the second read resumes past the first page
  });

  it("returns empty bytes for an immediately-eof empty file", async () => {
    const filesRead = vi.fn().mockResolvedValue({ b64: "", eof: true });
    const transport = fakeTransport({ filesRead });
    expect((await files.readAll(transport, { path: "/empty" })).length).toBe(0);
    expect(filesRead).toHaveBeenCalledTimes(1);
  });
});

describe("mutations", () => {
  it("deletePath commits an rm change against the live head", async () => {
    const filesCommit = vi.fn().mockResolvedValue({ height: 3, appHash: "ff".repeat(32) });
    const query = vi
      .fn()
      .mockResolvedValue({ refs: { head: "12".repeat(32), pins: {}, window_len: 1 } });
    const transport = fakeTransport({ filesCommit, query });

    await files.deletePath(transport, { path: "/shared/old.txt" });

    const body = filesCommit.mock.calls[0][0];
    expect(body.base_snapshot).toBe("12".repeat(32));
    expect(body.changes).toEqual([{ rm: { path: "/shared/old.txt" } }]);
  });

  it("mkdir commits a mkdir change", async () => {
    const filesCommit = vi.fn().mockResolvedValue({ height: 2, appHash: "34".repeat(32) });
    const transport = fakeTransport({ filesCommit });

    await files.mkdir(transport, { path: "/shared/docs" });

    expect(filesCommit.mock.calls[0][0].changes).toEqual([{ mkdir: { path: "/shared/docs" } }]);
  });
});

describe("reads over the query lane", () => {
  it("refs unwraps the refs reply variant", async () => {
    const query = vi
      .fn()
      .mockResolvedValue({ refs: { head: "ab".repeat(32), pins: { release: "cd".repeat(32) }, window_len: 3 } });
    const transport = fakeTransport({ query });

    const refs = await files.refs(transport);

    expect(query).toHaveBeenCalledWith("files", { refs: {} });
    expect(refs.head).toBe("ab".repeat(32));
    expect(refs.window_len).toBe(3);
  });

  it("diff unwraps the diff reply variant and passes the range", async () => {
    const query = vi.fn().mockResolvedValue({
      diff: [
        { path: "/a", kind: "added" },
        { path: "/b", kind: "modified" },
      ],
    });
    const transport = fakeTransport({ query });

    const entries = await files.diff(transport, { from: "aa".repeat(32), to: "bb".repeat(32) });

    expect(query).toHaveBeenCalledWith("files", {
      diff: { from: "aa".repeat(32), to: "bb".repeat(32), prefix: "" },
    });
    expect(entries).toHaveLength(2);
    expect(entries[1]).toEqual({ path: "/b", kind: "modified" });
  });
});

describe("history", () => {
  it("returns the snapshot window newest-first", async () => {
    const snapshots: FileSnapshot[] = [
      { id: "s2", parent: "s1", root_tree: "t2", author: "me", height: 2, consensus_time: 20, message: "b" },
      { id: "s1", parent: null, root_tree: "t1", author: "me", height: 1, consensus_time: 10, message: "a" },
    ];
    const transport = fakeTransport({ filesHistory: vi.fn().mockResolvedValue(snapshots) });

    expect(await files.history(transport)).toEqual(snapshots);
  });
});
