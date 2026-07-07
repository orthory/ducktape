// Typed client for the node's `forge` module — the TS mirror of
// `crates/apps/forge-interface`. forge is git-backed: one Commit msg == one git
// commit, so HEAD (and thus the module root) advances per write. On top of the
// repos sits a GitHub-shaped issue/PR/review tracker: items are per-repo
// numbered, each owning a HIDDEN chat channel (`forge:<repo>:<number>` — see
// chat-client's isModuleChannel) for its discussion. Same contract as
// chat-client/tasks-client: pure functions over an injected NodeTransport.

import type { AuthorRef } from "./chat-client";
import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

const TARGET = "forge";

// ── Wire types (verbatim serde shapes) ──────────────────

export type ForgeItemKind = "issue" | "pr";

/** An item's lifecycle: issues toggle open/closed; a PR additionally lands on
 *  `merged` (terminal — set by MergePr, never by SetItemState). */
export type ForgeItemState = "open" | "closed" | "merged";

export type ForgeReviewVerdict = "approve" | "request_changes" | "comment";

/** Which side of the diff an inline review comment anchors to. */
export type ForgeReviewSide = "old" | "new";

/** One ref of a repo: short branch name + its head commit oid (40-hex). */
export interface ForgeRefHead {
  name: string;
  head: string;
}

/** One row of a repo's item list. `author` is the same AuthorRef shape chat
 *  replies carry (derived from the submit origin). */
export interface ForgeItemSummary {
  number: number;
  kind: ForgeItemKind;
  title: string;
  state: ForgeItemState;
  author: AuthorRef;
  created_at: number;
  updated_at: number;
}

/** One inline comment of a review, anchored to a diff line. */
export interface ForgeReviewComment {
  path: string;
  line: number;
  side: ForgeReviewSide;
  body: string;
}

/** One submitted review on a PR, pinned to the source head it looked at. */
export interface ForgeReview {
  author: AuthorRef;
  verdict: ForgeReviewVerdict;
  body: string;
  commit_oid: string;
  comments: ForgeReviewComment[];
  created_at: number;
}

/** The full item: summary fields flattened + body, its hidden discussion
 *  channel, and the PR-only fields (null on an issue). */
export interface ForgeItemDetail extends ForgeItemSummary {
  body: string;
  channel_id: string;
  source_branch: string | null;
  target_branch: string | null;
  merge_oid: string | null;
  reviews: ForgeReview[];
}

// ── Msgs (writes — one commit per submit) ───────────────

export const commit = (
  transport: NodeTransport,
  params: { path: string; content: string; message: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      commit: {
        path: params.path,
        content: params.content,
        message: params.message,
      },
    },
    params.origin,
  );

// ── Item ops (the issue/PR tracker) ─────────────────────

/** Open an issue on `repo`. The module mints the item number and its hidden
 *  discussion channel; authorship comes from `origin`. */
export const openIssue = (
  transport: NodeTransport,
  params: { repo: string; title: string; body: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { open_issue: { repo: params.repo, title: params.title, body: params.body } },
    params.origin,
  );

/** Open a PR from `sourceBranch` into `targetBranch` (empty string → the
 *  repo's main). Numbered from the same per-repo sequence as issues. */
export const openPr = (
  transport: NodeTransport,
  params: {
    repo: string;
    title: string;
    body: string;
    sourceBranch: string;
    targetBranch: string;
    origin?: string;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      open_pr: {
        repo: params.repo,
        title: params.title,
        body: params.body,
        source_branch: params.sourceBranch,
        target_branch: params.targetBranch,
      },
    },
    params.origin,
  );

/** Replace an item's title and/or body; null leaves that field untouched. */
export const editItem = (
  transport: NodeTransport,
  params: {
    repo: string;
    number: number;
    title: string | null;
    body: string | null;
    origin?: string;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      edit_item: {
        repo: params.repo,
        number: params.number,
        title: params.title,
        body: params.body,
      },
    },
    params.origin,
  );

/** Close (open: false) or reopen (open: true) an issue or PR. A merged PR is
 *  terminal — the module refuses to reopen it. */
export const setItemState = (
  transport: NodeTransport,
  params: { repo: string; number: number; open: boolean; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { set_item_state: { repo: params.repo, number: params.number, open: params.open } },
    params.origin,
  );

/** Merge a PR. CAS-style against both heads: `prevTargetOid` is the target
 *  branch head the merge builds on, `expectedSourceOid` the reviewed source
 *  head; `mergeOid` names the merge commit inside the pack the caller staged
 *  via `uploadMergePack` (`packDigest` — sha256, 64-hex). */
export const mergePr = (
  transport: NodeTransport,
  params: {
    repo: string;
    number: number;
    prevTargetOid: string;
    expectedSourceOid: string;
    mergeOid: string;
    packDigest: string;
    origin?: string;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      merge_pr: {
        repo: params.repo,
        number: params.number,
        prev_target_oid: params.prevTargetOid,
        expected_source_oid: params.expectedSourceOid,
        merge_oid: params.mergeOid,
        pack_digest: params.packDigest,
      },
    },
    params.origin,
  );

/** Submit a review on a PR, pinned to the source head (`commitOid`) it looked
 *  at, with optional inline diff comments. */
export const submitReview = (
  transport: NodeTransport,
  params: {
    repo: string;
    number: number;
    verdict: ForgeReviewVerdict;
    body: string;
    commitOid: string;
    comments: ForgeReviewComment[];
    origin?: string;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      submit_review: {
        repo: params.repo,
        number: params.number,
        verdict: params.verdict,
        body: params.body,
        commit_oid: params.commitOid,
        comments: params.comments,
      },
    },
    params.origin,
  );

/** Stage a merge pack's raw bytes in the node's blob store and hand back the
 *  sha256 digest a MergePr op references as `pack_digest`. Nothing is committed
 *  by the upload itself — the digest is normalized to the wire's 64 lowercase
 *  hex, and the bytes are copied onto a plain ArrayBuffer (putBlob's contract). */
export const uploadMergePack = (
  transport: NodeTransport,
  bytes: Uint8Array,
): Promise<string> =>
  Promise.resolve()
    .then(() => transport.putBlob(new Uint8Array(bytes)))
    .then((digest) => digest.toLowerCase());

// ── Queries (reads over committed state) ────────────────

/** The current HEAD commit oid (40-char sha1 hex), or null on an unborn repo
 *  (no commits yet). This hex is the state root's preimage: forge's `root()` is
 *  sha256 of the oid's raw bytes. */
export const head = (transport: NodeTransport): Promise<string | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "head"))
    .then((reply) => replyVariant<string | null>(reply, "head"));

/** Every ref of `repo`: short branch names + their head oids. */
export const listRefs = (
  transport: NodeTransport,
  repo: string,
): Promise<ForgeRefHead[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { list_refs: { repo } }))
    .then((reply) => replyVariant<ForgeRefHead[]>(reply, "refs"));

/** Every issue/PR of `repo` as summaries. */
export const listItems = (
  transport: NodeTransport,
  repo: string,
): Promise<ForgeItemSummary[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { list_items: { repo } }))
    .then((reply) => replyVariant<ForgeItemSummary[]>(reply, "items"));

/** One item in full (body, reviews, PR branches), or null when the number
 *  names nothing in `repo`. */
export const getItem = (
  transport: NodeTransport,
  params: { repo: string; number: number },
): Promise<ForgeItemDetail | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, { get_item: { repo: params.repo, number: params.number } }),
    )
    .then((reply) => replyVariant<ForgeItemDetail | null>(reply, "item"));
