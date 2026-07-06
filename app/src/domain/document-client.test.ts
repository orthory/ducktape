// The document client mirrors document-interface: DocMsg encoding (no origin,
// snake_case fields) + DocReply decoding for the GetDoc / GetBlock queries,
// including the null (absent doc/block) case.

import { describe, expect, it, vi } from "vitest";

import {
  createDoc,
  getBlock,
  getDoc,
  insertBlock,
  moveBlock,
  removeBlock,
  updateBlock,
} from "./document-client";
import type { Block } from "./document-client";
import type { NodeTransport } from "./transport";

const stubTransport = (reply?: unknown): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  query: vi.fn().mockResolvedValue(reply),
  view: vi.fn(),
  putBlob: vi.fn(),
  getBlob: vi.fn(),
  status: vi.fn(),
  telemetry: vi.fn(),
  blocks: vi.fn(),
  onBlock: vi.fn(),
  onTelemetry: vi.fn(),
});

describe("document msgs", () => {
  it("encodes CreateDoc with no origin arg", async () => {
    const transport = stubTransport();
    await createDoc(transport, { docId: "notes" });
    expect(transport.submit).toHaveBeenCalledWith("document", {
      create_doc: { doc_id: "notes" },
    });
  });

  it("encodes InsertBlock, keeping after:null (a front insert) on the wire", async () => {
    const transport = stubTransport();
    const block: Block = { id: "b1", kind: "paragraph", text: "hi" };
    await insertBlock(transport, { docId: "notes", after: null, block });
    expect(transport.submit).toHaveBeenCalledWith("document", {
      insert_block: { doc_id: "notes", after: null, block },
    });
  });

  it("encodes UpdateBlock / RemoveBlock / MoveBlock verbatim", async () => {
    const transport = stubTransport();

    await updateBlock(transport, { docId: "notes", blockId: "b1", text: "next" });
    expect(transport.submit).toHaveBeenCalledWith("document", {
      update_block: { doc_id: "notes", block_id: "b1", text: "next" },
    });

    await removeBlock(transport, { docId: "notes", blockId: "b1" });
    expect(transport.submit).toHaveBeenCalledWith("document", {
      remove_block: { doc_id: "notes", block_id: "b1" },
    });

    await moveBlock(transport, { docId: "notes", blockId: "b1", after: "b0" });
    expect(transport.submit).toHaveBeenCalledWith("document", {
      move_block: { doc_id: "notes", block_id: "b1", after: "b0" },
    });
  });
});

describe("document queries", () => {
  it("sends GetDoc and decodes Doc into ordered blocks", async () => {
    const blocks: Block[] = [{ id: "b1", kind: "heading", text: "Title" }];
    const transport = stubTransport({ doc: blocks });
    await expect(getDoc(transport, "notes")).resolves.toEqual(blocks);
    expect(transport.query).toHaveBeenCalledWith("document", {
      get_doc: { doc_id: "notes" },
    });
  });

  it("decodes doc:null as an absent doc", async () => {
    const transport = stubTransport({ doc: null });
    await expect(getDoc(transport, "ghost")).resolves.toBeNull();
  });

  it("sends GetBlock and decodes Block", async () => {
    const block: Block = { id: "b1", kind: "code", text: "x = 1" };
    const transport = stubTransport({ block: block });
    await expect(
      getBlock(transport, { docId: "notes", blockId: "b1" }),
    ).resolves.toEqual(block);
    expect(transport.query).toHaveBeenCalledWith("document", {
      get_block: { doc_id: "notes", block_id: "b1" },
    });
  });
});
