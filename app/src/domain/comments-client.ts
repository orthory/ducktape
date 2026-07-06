// Typed client for the node's `comments` module — the TS mirror of
// `crates/apps/comments`. A thread anchors to {module, target} (a pages block
// or page id); authorship is derived by the module from the submit origin, so
// it appears only in replies. Pure functions over an injected NodeTransport,
// same contract as pages-client/chat-client.

import type { NodeTransport, SubmitReceipt } from "./transport";
import type { AuthorRef } from "./chat-client";
import { replyVariant } from "./wire";

export type { AuthorRef };

export interface Anchor {
  module: string;
  target: string;
}

export interface Comment {
  id: string;
  thread_id: string;
  author: AuthorRef;
  text: string;
  created_at: number;
  edited_at: number | null;
  deleted: boolean;
}

export interface Thread {
  id: string;
  anchor: Anchor;
  opener: AuthorRef;
  created_at: number;
  resolved: boolean;
  resolved_by: AuthorRef | null;
  comment_ids: string[];
}

export interface ThreadView {
  thread: Thread;
  comments: Comment[];
}

export interface AnchorThreads {
  target: string;
  threads: ThreadView[];
}

const TARGET = "comments";

export const addComment = (
  transport: NodeTransport,
  params: { threadId: string; commentId: string; anchor: Anchor; text: string },
): Promise<SubmitReceipt> =>
  transport.submit(TARGET, {
    add_comment: {
      thread_id: params.threadId,
      comment_id: params.commentId,
      anchor: params.anchor,
      text: params.text,
    },
  });

export const editComment = (
  transport: NodeTransport,
  params: { commentId: string; text: string },
): Promise<SubmitReceipt> =>
  transport.submit(TARGET, {
    edit_comment: { comment_id: params.commentId, text: params.text },
  });

export const deleteComment = (
  transport: NodeTransport,
  commentId: string,
): Promise<SubmitReceipt> =>
  transport.submit(TARGET, { delete_comment: { comment_id: commentId } });

export const resolveThread = (
  transport: NodeTransport,
  params: { threadId: string; resolved: boolean },
): Promise<SubmitReceipt> =>
  transport.submit(TARGET, {
    resolve_thread: { thread_id: params.threadId, resolved: params.resolved },
  });

export const threadsForAnchors = (
  transport: NodeTransport,
  params: { module: string; targets: string[] },
): Promise<AnchorThreads[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        threads_for_anchors: { module: params.module, targets: params.targets },
      }),
    )
    .then((reply) => replyVariant<AnchorThreads[]>(reply, "anchored"));

export const getThread = (
  transport: NodeTransport,
  threadId: string,
): Promise<ThreadView | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { thread: { thread_id: threadId } }))
    .then((reply) => replyVariant<ThreadView | null>(reply, "thread"));
