//! the action-owner contract: the shared Probe/Apply validation against
//! pages state, the NO-FAIL `Apply` arm that translates to `PageMsg`
//! follow-ups, and the committed failure rows (see the crate docs on the
//! sibling-action caveat).

use package::PackageActionMsg;
use pages::{
    PageMsg, PageQuery, PageReply, ThreadView, encode_msg as pages_encode_msg,
    encode_query as pages_encode_query,
};
use sdk::{Ctx, Msg};
use sha2::{Digest, Sha256};

use crate::state::Phase;
use crate::{
    ACTION_BLOCK_UPDATE_TEXT, ACTION_COMMENT_ADD, ACTION_THREAD_RESOLVE, BlockUpdateTextPayload,
    CommentAddPayload, DocsHarness, FailureRow, MAX_ACTION_ID_BYTES, MAX_BLOCK_TEXT_BYTES,
    MAX_COMMENT_TEXT_BYTES, MAX_FAILURE_ROWS, RunContext, ThreadResolvePayload, minted_comment_id,
    minted_thread_id,
};

/// what one probe/apply validated — shared so the probe verdict and the
/// apply-time re-check cannot drift (the tasks-module idiom).
pub(crate) enum ValidatedAction {
    CommentAdd {
        thread_id: String,
        comment_id: String,
        target: String,
        text: String,
    },
    UpdateText {
        block_id: String,
        text: String,
    },
    ResolveThread {
        thread_id: String,
        resolved: bool,
    },
}

impl DocsHarness {
    // ---- the action-owner contract ---------------------------------------------

    /// read one block from the wired pages module (staged-over-committed).
    async fn block_of(
        &self,
        ctx: &dyn Ctx,
        block_id: &str,
    ) -> Result<Option<pages::Block>, String> {
        let reply = ctx
            .query(
                &self.pages,
                &pages_encode_query(&PageQuery::GetBlock {
                    block_id: block_id.into(),
                }),
            )
            .await
            .map_err(|e| format!("pages query failed: {e}"))?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::Block(block)) => Ok(block),
            Ok(other) => Err(format!("unexpected pages reply: {other:?}")),
            Err(e) => Err(e),
        }
    }

    /// read one comment thread from the wired pages module.
    async fn thread_of(
        &self,
        ctx: &dyn Ctx,
        thread_id: &str,
    ) -> Result<Option<ThreadView>, String> {
        let reply = ctx
            .query(
                &self.pages,
                &pages_encode_query(&PageQuery::CommentThread {
                    thread_id: thread_id.into(),
                }),
            )
            .await
            .map_err(|e| format!("pages query failed: {e}"))?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::CommentThread(view)) => Ok(view),
            Ok(other) => Err(format!("unexpected pages reply: {other:?}")),
            Err(e) => Err(e),
        }
    }

    /// read one comment by id from the wired pages module. tombstones read as
    /// `Some` — the id is taken either way.
    async fn comment_of(
        &self,
        ctx: &dyn Ctx,
        comment_id: &str,
    ) -> Result<Option<pages::Comment>, String> {
        let reply = ctx
            .query(
                &self.pages,
                &pages_encode_query(&PageQuery::GetComment {
                    comment_id: comment_id.into(),
                }),
            )
            .await
            .map_err(|e| format!("pages query failed: {e}"))?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::Comment(comment)) => Ok(comment),
            Ok(other) => Err(format!("unexpected pages reply: {other:?}")),
            Err(e) => Err(e),
        }
    }

    /// validate one owned action against STAGED-OR-COMMITTED pages state —
    /// the read-only half of `Probe` and the re-check `Apply` runs.
    pub(crate) async fn validate_action(
        &self,
        ctx: &dyn Ctx,
        action_id: &str,
        tag: &str,
        payload: &[u8],
        run_context: &[u8],
    ) -> Result<ValidatedAction, String> {
        let active = self
            .store()
            .installed
            .as_ref()
            .is_some_and(|i| i.phase == Phase::Active);
        if !active {
            return Err("the docs package is not active".into());
        }
        match tag {
            ACTION_COMMENT_ADD => {
                let p: CommentAddPayload = serde_json::from_slice(payload)
                    .map_err(|e| format!("malformed {tag} payload: {e}"))?;
                if p.text.is_empty() || p.text.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(format!("text must be 1..={MAX_COMMENT_TEXT_BYTES} bytes"));
                }
                if self.block_of(ctx, &p.target).await?.is_none() {
                    return Err(format!("unknown comment target: {}", p.target));
                }
                // the minted ids embed (run_id, action_id) — bound the parts.
                if action_id.is_empty() || action_id.len() > MAX_ACTION_ID_BYTES {
                    return Err(format!("action_id must be 1..={MAX_ACTION_ID_BYTES} bytes"));
                }
                let rc: RunContext = serde_json::from_slice(run_context)
                    .map_err(|e| format!("malformed run context: {e}"))?;
                // BOTH branches mint the comment id, and pages stores comment
                // ids globally — a squat in ANY thread would make the
                // AddComment follow-up reject and abort the delivery block,
                // so probe it away here (and again on the apply re-check).
                let comment_id = minted_comment_id(&rc.run_id, action_id);
                if self.comment_of(ctx, &comment_id).await?.is_some() {
                    return Err(format!("minted comment id already taken: {comment_id}"));
                }
                let thread_id = match &p.thread_id {
                    Some(thread_id) => {
                        let view = self
                            .thread_of(ctx, thread_id)
                            .await?
                            .ok_or_else(|| format!("unknown thread: {thread_id}"))?;
                        if view.thread.target != p.target {
                            return Err(format!(
                                "thread {thread_id} targets {:?}, not {:?}",
                                view.thread.target, p.target
                            ));
                        }
                        if view.thread.comment_ids.len() >= pages::MAX_COMMENTS_PER_THREAD {
                            return Err(format!("thread is full: {thread_id}"));
                        }
                        thread_id.clone()
                    }
                    None => {
                        // opening a new thread under a minted id: a squatted
                        // id would abort the delivery block (a comment write
                        // is not idempotent), so probe it away here.
                        let minted = minted_thread_id(&rc.run_id, action_id);
                        if self.thread_of(ctx, &minted).await?.is_some() {
                            return Err(format!("minted thread id already taken: {minted}"));
                        }
                        minted
                    }
                };
                Ok(ValidatedAction::CommentAdd {
                    thread_id,
                    comment_id,
                    target: p.target,
                    text: p.text,
                })
            }
            ACTION_BLOCK_UPDATE_TEXT => {
                let p: BlockUpdateTextPayload = serde_json::from_slice(payload)
                    .map_err(|e| format!("malformed {tag} payload: {e}"))?;
                if p.text.len() > MAX_BLOCK_TEXT_BYTES {
                    return Err(format!("text must be at most {MAX_BLOCK_TEXT_BYTES} bytes"));
                }
                let block = self
                    .block_of(ctx, &p.block_id)
                    .await?
                    .ok_or_else(|| format!("unknown block: {}", p.block_id))?;
                if let Some(expected) = &p.expected_hash {
                    let pin = parse_sha256_field(expected)
                        .ok_or_else(|| format!("malformed expected_hash: {expected}"))?;
                    let current: Vec<u8> = Sha256::digest(block.text.as_bytes()).to_vec();
                    if current != pin {
                        return Err(format!(
                            "expected_hash mismatch: block {} changed since the agent read it",
                            p.block_id
                        ));
                    }
                }
                Ok(ValidatedAction::UpdateText {
                    block_id: p.block_id,
                    text: p.text,
                })
            }
            ACTION_THREAD_RESOLVE => {
                let p: ThreadResolvePayload = serde_json::from_slice(payload)
                    .map_err(|e| format!("malformed {tag} payload: {e}"))?;
                if self.thread_of(ctx, &p.thread_id).await?.is_none() {
                    return Err(format!("unknown thread: {}", p.thread_id));
                }
                Ok(ValidatedAction::ResolveThread {
                    thread_id: p.thread_id,
                    resolved: p.resolved,
                })
            }
            other => Err(format!("docs-harness does not own action tag: {other}")),
        }
    }

    /// NO-FAIL: an accepted action's `Apply` rides the runs module's delivery
    /// block — decode-or-drop, re-validate against now-staged pages state,
    /// then translate to the `PageMsg` follow-up; a late conflict lands a
    /// committed error row + breadcrumb instead of a block abort.
    pub(crate) async fn apply_action(&mut self, ctx: &mut dyn Ctx, apply: &PackageActionMsg) {
        let PackageActionMsg::Apply {
            action_id,
            tag,
            payload,
            run_context,
        } = apply;
        let validated = match self
            .validate_action(&*ctx, action_id, tag, payload, run_context)
            .await
        {
            Ok(validated) => validated,
            Err(reason) => {
                self.record_failure(ctx, action_id, tag, reason);
                return;
            }
        };
        // the duplicated-action_id dedupe (see the sibling-action caveat in
        // the module docs): a same-block re-apply of one (run, action) key
        // would mint identical page ids and poison the delivery block.
        let dedupe = match serde_json::from_slice::<RunContext>(run_context) {
            Ok(rc) => format!("{}\u{1f}{action_id}", rc.run_id),
            Err(_) => format!("\u{1f}{action_id}"), // only reachable for tags that skip rc
        };
        if !self.applied_this_block.insert(dedupe) {
            self.record_failure(
                ctx,
                action_id,
                tag,
                "duplicate action_id in one delivery".into(),
            );
            return;
        }
        let follow_up = match validated {
            ValidatedAction::CommentAdd {
                thread_id,
                comment_id,
                target,
                text,
            } => PageMsg::AddComment {
                thread_id,
                comment_id,
                target,
                text,
            },
            ValidatedAction::UpdateText { block_id, text } => {
                PageMsg::UpdateText { block_id, text }
            }
            ValidatedAction::ResolveThread {
                thread_id,
                resolved,
            } => PageMsg::ResolveThread {
                thread_id,
                resolved,
            },
        };
        ctx.emit_msg(Msg {
            target: self.pages.clone(),
            payload: pages_encode_msg(&follow_up),
        });
    }

    /// land one error row (bounded, oldest evicted) + its breadcrumb — the
    /// committed half of "mutate nothing, record failure".
    ///
    /// STRICT-DECODER VS STAGEABLE-STATE INVARIANT: `tag` is stored VERBATIM
    /// here, un-revalidated at this module's boundary — an empty tag is
    /// representable in the `PackageActionMsg::Apply` wire shape (e.g. the
    /// `other => Err(...)` "does not own action tag" arm in
    /// `validate_action` would happily format one into a `FailureRow`) and
    /// would fail `Store::decode`'s stricter reject-empty-tag check
    /// (state.rs) on a snapshot round-trip. that state is never actually
    /// staged in a live network: `runs` validates every oracle-supplied
    /// action tag's shape (rejecting empty/malformed ones) before it is ever
    /// allowed to become an Apply this arm could see (crates/apps/runs/src/
    /// lib.rs, the open-action pipeline) — so this arm is composition-safe
    /// by upstream validation, not because it independently enforces what
    /// the decoder later demands.
    fn record_failure(&mut self, ctx: &mut dyn Ctx, action_id: &str, tag: &str, reason: String) {
        self.breadcrumb(ctx, format!("action {action_id} ({tag}) dropped: {reason}"));
        let failures = &mut self.store_mut().failures;
        if failures.len() >= MAX_FAILURE_ROWS {
            failures.remove(0);
        }
        failures.push(FailureRow {
            action_id: action_id.into(),
            tag: tag.into(),
            reason,
        });
    }
}

/// parse a `"sha256:<64 lowercase hex>"` field into raw digest bytes; `None`
/// on any other shape (a malformed guard is a clean rejection).
fn parse_sha256_field(field: &str) -> Option<Vec<u8>> {
    let hex = field.strip_prefix("sha256:")?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

// ---- the action-owner contract tests ---------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use package::{
        HarnessMsg, PackageActionQuery, PackageActionReply, encode_action_query, encode_harness_msg,
    };
    use sdk::{Module, Origin};

    use crate::testutil::*;

    fn probe(m: &DocsHarness, tag: &str, payload: serde_json::Value) -> PackageActionReply {
        let ctx = TestCtx::at(Origin::Module("runs".into()));
        let req = encode_action_query(&PackageActionQuery::Probe {
            action_id: "a1".into(),
            tag: tag.into(),
            payload: serde_json::to_vec(&payload).unwrap(),
            run_context: RUN_CONTEXT.to_vec(),
        });
        package::decode_action_reply(&block_on(m.query_with(&ctx, &req)).unwrap()).unwrap()
    }

    fn rejects(reply: &PackageActionReply, needle: &str) -> bool {
        matches!(reply, PackageActionReply::Rejected { reason } if reason.contains(needle))
    }

    #[test]
    fn probe_validates_against_pages_state() {
        let mut m = module();
        installed(&mut m);

        // comment.add: an existing target accepts; open-a-new-thread mints.
        assert_eq!(
            probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": "done, see the edit"}),
            ),
            PackageActionReply::Accepted
        );
        // appending to an existing, matching thread accepts.
        assert_eq!(
            probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "thread_id": "t1", "text": "replying"}),
            ),
            PackageActionReply::Accepted
        );
        // an unknown target rejects.
        assert!(rejects(
            &probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "ghost", "text": "hi"}),
            ),
            "ghost"
        ));
        // an unknown thread rejects.
        assert!(rejects(
            &probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "thread_id": "t9", "text": "hi"}),
            ),
            "t9"
        ));
        // a thread anchored elsewhere rejects (t1 targets b1, not p1).
        assert!(rejects(
            &probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "p1", "thread_id": "t1", "text": "hi"}),
            ),
            "target"
        ));
        // empty text rejects.
        assert!(matches!(
            probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": ""}),
            ),
            PackageActionReply::Rejected { .. }
        ));

        // update_text: existing block accepts; the expected_hash guard bites.
        assert_eq!(
            probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "new text"}),
            ),
            PackageActionReply::Accepted
        );
        let good_hash = format!("sha256:{}", hex(&Sha256::digest(b"old text")));
        assert_eq!(
            probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "new text",
                                   "expected_hash": good_hash}),
            ),
            PackageActionReply::Accepted
        );
        let stale_hash = format!("sha256:{}", hex(&Sha256::digest(b"someone else's text")));
        assert!(rejects(
            &probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "new text",
                                   "expected_hash": stale_hash}),
            ),
            "expected_hash"
        ));
        assert!(rejects(
            &probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "x", "expected_hash": "md5:nope"}),
            ),
            "malformed"
        ));
        assert!(rejects(
            &probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "ghost", "text": "x"}),
            ),
            "ghost"
        ));

        // thread.resolve: existing thread accepts, unknown rejects.
        assert_eq!(
            probe(
                &m,
                ACTION_THREAD_RESOLVE,
                serde_json::json!({"thread_id": "t1", "resolved": true}),
            ),
            PackageActionReply::Accepted
        );
        assert!(rejects(
            &probe(
                &m,
                ACTION_THREAD_RESOLVE,
                serde_json::json!({"thread_id": "t9", "resolved": true}),
            ),
            "t9"
        ));

        // schema strictness: unknown fields reject; unknown tags reject.
        assert!(rejects(
            &probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "x", "page_id": "p1"}),
            ),
            "malformed"
        ));
        assert!(rejects(
            &probe(&m, "pages.block.delete", serde_json::json!({})),
            "does not own"
        ));

        // nothing above staged anything: probes are read-only.
        commit(&mut m);
        assert!(m.committed.failures.is_empty());
    }

    #[test]
    fn probe_rejects_while_suspended() {
        let mut m = module();
        installed(&mut m);
        let mut ctx = TestCtx::at(package_origin());
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::SuspendPackage {
                package: PKG.into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert!(matches!(
            probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": "hi"}),
            ),
            PackageActionReply::Rejected { .. }
        ));
    }

    #[test]
    fn apply_translates_to_page_msgs_and_is_no_fail() {
        let mut m = module();
        installed(&mut m);

        // comment.add without a thread: minted thread + comment ids.
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                "a1",
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": "done"}),
            ),
        )
        .unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert_eq!(ctx.emitted[0].target, "pages");
        assert_eq!(
            pages::decode_msg(&ctx.emitted[0].payload).unwrap(),
            PageMsg::AddComment {
                thread_id: minted_thread_id("r1", "a1"),
                comment_id: minted_comment_id("r1", "a1"),
                target: "b1".into(),
                text: "done".into(),
            }
        );
        commit(&mut m);

        // update_text and thread.resolve translate too.
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                "a2",
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "new text"}),
            ),
        )
        .unwrap();
        assert_eq!(
            pages::decode_msg(&ctx.emitted[0].payload).unwrap(),
            PageMsg::UpdateText {
                block_id: "b1".into(),
                text: "new text".into(),
            }
        );
        commit(&mut m);
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                "a3",
                ACTION_THREAD_RESOLVE,
                serde_json::json!({"thread_id": "t1", "resolved": true}),
            ),
        )
        .unwrap();
        assert_eq!(
            pages::decode_msg(&ctx.emitted[0].payload).unwrap(),
            PageMsg::ResolveThread {
                thread_id: "t1".into(),
                resolved: true,
            }
        );
        commit(&mut m);

        // a late conflict (stale expected_hash): error row + breadcrumb, Ok.
        let stale = format!("sha256:{}", hex(&Sha256::digest(b"stale")));
        let mut late = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut late,
            apply(
                "a4",
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "x", "expected_hash": stale}),
            ),
        )
        .expect("a late conflict must not abort the delivery block");
        assert!(late.emitted.is_empty(), "nothing mutated");
        assert!(late.events.iter().any(|e| e.contains("expected_hash")));
        commit(&mut m);
        assert_eq!(m.committed.failures.len(), 1);
        assert_eq!(m.committed.failures[0].action_id, "a4");

        // a malformed payload: error row + breadcrumb, Ok.
        let mut bad = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut bad,
            apply("a5", ACTION_COMMENT_ADD, serde_json::json!({"bogus": true})),
        )
        .expect("a malformed apply must not abort the delivery block");
        assert!(bad.emitted.is_empty());
        assert!(bad.events.iter().any(|e| e.contains("dropped")));
        commit(&mut m);
        assert_eq!(m.committed.failures.len(), 2);
    }

    #[test]
    fn a_duplicated_action_id_applies_once_per_block() {
        let mut m = module();
        installed(&mut m);

        // two applies with the SAME action_id in one block: identical minted
        // comment ids would abort the delivery block at pages — the second
        // must drop with an error row instead.
        let payload = apply(
            "a1",
            ACTION_COMMENT_ADD,
            serde_json::json!({"target": "b1", "text": "done"}),
        );
        let mut first = TestCtx::at(Origin::Module("runs".into()));
        exec(&mut m, &mut first, payload.clone()).unwrap();
        assert_eq!(first.emitted.len(), 1);
        let mut second = TestCtx::at(Origin::Module("runs".into()));
        exec(&mut m, &mut second, payload.clone()).unwrap();
        assert!(second.emitted.is_empty(), "the duplicate must not emit");
        assert!(second.events.iter().any(|e| e.contains("duplicate")));
        commit(&mut m);

        // the dedupe window is the block: a NEW block accepts the key again
        // (a real re-run never reuses (run_id, action_id) — see module docs).
        let mut next_block = TestCtx::at(Origin::Module("runs".into()));
        exec(&mut m, &mut next_block, payload).unwrap();
        assert_eq!(next_block.emitted.len(), 1);
    }

    #[test]
    fn a_squatted_minted_comment_id_is_probed_away_and_never_aborts() {
        let mut m = module();
        installed(&mut m);
        let squat = minted_comment_id("r1", "a1");

        // probe: BOTH branches (new thread AND append) reject while the
        // minted comment id is taken anywhere — comment ids are global.
        for payload in [
            serde_json::json!({"target": "b1", "text": "new thread"}),
            serde_json::json!({"target": "b1", "thread_id": "t1", "text": "append"}),
        ] {
            let mut ctx = TestCtx::at(Origin::Module("runs".into()));
            ctx.squatted_comment = Some(squat.clone());
            let req = encode_action_query(&PackageActionQuery::Probe {
                action_id: "a1".into(),
                tag: ACTION_COMMENT_ADD.into(),
                payload: serde_json::to_vec(&payload).unwrap(),
                run_context: RUN_CONTEXT.to_vec(),
            });
            let reply =
                package::decode_action_reply(&block_on(m.query_with(&ctx, &req)).unwrap()).unwrap();
            assert!(
                rejects(&reply, "already taken"),
                "the squat must reject ({payload}): {reply:?}"
            );
        }

        // apply (the re-check, a squat that landed after the probe): error
        // row + breadcrumb, Ok — the delivery block COMMITS, never aborts.
        let mut late = TestCtx::at(Origin::Module("runs".into()));
        late.squatted_comment = Some(squat);
        exec(
            &mut m,
            &mut late,
            apply(
                "a1",
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": "done"}),
            ),
        )
        .expect("a squatted minted id must not abort the delivery block");
        assert!(late.emitted.is_empty(), "nothing reaches pages");
        assert!(late.events.iter().any(|e| e.contains("already taken")));
        commit(&mut m);
        assert_eq!(m.committed.failures.len(), 1);
        assert_eq!(m.committed.failures[0].action_id, "a1");

        // with no squat, the identical action applies cleanly.
        let mut clean = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut clean,
            apply(
                "a1",
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": "done"}),
            ),
        )
        .unwrap();
        assert_eq!(clean.emitted.len(), 1);
    }

    #[test]
    fn the_failure_log_is_bounded() {
        let mut m = module();
        installed(&mut m);
        for i in 0..(MAX_FAILURE_ROWS + 3) {
            let mut ctx = TestCtx::at(Origin::Module("runs".into()));
            exec(
                &mut m,
                &mut ctx,
                apply(
                    &format!("a{i}"),
                    ACTION_COMMENT_ADD,
                    serde_json::json!({"target": "ghost", "text": "hi"}),
                ),
            )
            .unwrap();
        }
        commit(&mut m);
        assert_eq!(m.committed.failures.len(), MAX_FAILURE_ROWS);
        // the oldest rows were evicted: a0..a2 are gone, a3 leads.
        assert_eq!(m.committed.failures[0].action_id, "a3");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
