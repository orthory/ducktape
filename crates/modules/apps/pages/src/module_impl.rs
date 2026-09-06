use super::{
    Ctx, Error, Module, ModuleId, Msg, PageQuery, PageReply, Pages, Party, ResolverSyncTarget,
    StateRoot, StateSyncHandle, decode_msg, decode_query, encode_reply,
};
use attribution::{Actor, AttributionMsg, AttributionUpdate, ObjectRef, Reason, Relation};
use sdk::Origin;
use std::collections::{BTreeMap, BTreeSet};

/// A module-wide clock gives every changed source a newer revision, including
/// a deleted object whose client-minted id is later reused. Authorship lives in
/// the source record, so a large subtree needs no extra per-object store reads.
const SOURCE_REVISION_KEY: &[u8] = b"\0attribution-revision";

fn actor_of(party: &Party) -> Actor {
    match party {
        Party::Account(account) => Actor::Account(*account),
        Party::Key(key) => Actor::Key(key.clone()),
        Party::Module(module) => Actor::Module(module.clone()),
        Party::System => Actor::System,
    }
}

fn authored_relations(author: &Party) -> Vec<Relation> {
    match author {
        Party::Account(account) => vec![Relation {
            recipient: *account,
            reason: Reason::Authorship,
            detail: Vec::new(),
        }],
        Party::Key(_) | Party::Module(_) | Party::System => Vec::new(),
    }
}

impl Pages {
    async fn identity_account(
        &self,
        ctx: &dyn Ctx,
        query: identity::IdentityQuery,
    ) -> Result<Option<u64>, Error> {
        let Some(identity) = &self.identity else {
            return Ok(None);
        };
        let bytes = ctx.query(identity, &identity::encode_query(&query)).await?;
        let identity::IdentityReply::Account(account) =
            identity::decode_reply(&bytes).map_err(Error::Module)?
        else {
            return Err(Error::Module("pages: unexpected identity reply".into()));
        };
        Ok(account.map(|account| account.number))
    }

    async fn party_of_origin(&self, ctx: &dyn Ctx) -> Result<Party, Error> {
        match &ctx.env().origin {
            Origin::External(key) => {
                if key.is_empty() {
                    return Err(Error::Module(super::PageError::EmptyOrigin.to_string()));
                }
                if key.len() > super::MAX_COMMENT_AUTHOR_BYTES {
                    return Err(Error::Module(super::PageError::AuthorTooLarge.to_string()));
                }
                let account = self
                    .identity_account(ctx, identity::IdentityQuery::OfKey { key: key.clone() })
                    .await?;
                Ok(match account {
                    Some(account) => Party::Account(account),
                    None => Party::Key(key.clone()),
                })
            }
            Origin::Program(account) => Ok(Party::Account(*account)),
            Origin::Module(module) => {
                if module.len() > super::MAX_COMMENT_AUTHOR_BYTES {
                    return Err(Error::Module(super::PageError::AuthorTooLarge.to_string()));
                }
                Ok(Party::Module(module.clone()))
            }
            Origin::System => Ok(Party::System),
        }
    }

    async fn validate_mentions(&self, ctx: &dyn Ctx, msg: &super::PageMsg) -> Result<(), Error> {
        use super::{InlineMark, PageMsg};
        let accounts: BTreeSet<u64> = match msg {
            PageMsg::AddComment { mentions, .. } | PageMsg::EditComment { mentions, .. } => {
                mentions.iter().copied().collect()
            }
            PageMsg::InsertBlock { block, .. } => block
                .marks
                .iter()
                .filter_map(|mark| match mark.kind {
                    InlineMark::Mention(account) => Some(account),
                    _ => None,
                })
                .collect(),
            PageMsg::UpdateText {
                marks: Some(marks), ..
            } => marks
                .iter()
                .filter_map(|mark| match mark.kind {
                    InlineMark::Mention(account) => Some(account),
                    _ => None,
                })
                .collect(),
            PageMsg::SetSpanMark {
                kind: InlineMark::Mention(account),
                active: true,
                ..
            } => BTreeSet::from([*account]),
            _ => BTreeSet::new(),
        };
        for number in accounts {
            let exists = number != 0
                && self
                    .identity_account(ctx, identity::IdentityQuery::Get { number })
                    .await?
                    .is_some();
            if !exists {
                return Err(Error::Module(format!(
                    "pages: mention names no account: {number}"
                )));
            }
        }
        Ok(())
    }

    async fn attribution_reports(
        &mut self,
        actor: &Party,
        before: &BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    ) -> Result<Vec<Msg>, Error> {
        let Some(attribution) = self.attribution.clone() else {
            return Ok(Vec::new());
        };
        let changed: Vec<_> = self
            .staged
            .staged_writes()
            .iter()
            .filter(|(key, value)| {
                let source_record = !key.starts_with(b"\0") || key.starts_with(b"\0cc:");
                let changed = before.get(*key) != Some(*value);
                source_record && changed
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        if changed.is_empty() {
            return Ok(Vec::new());
        }
        let mut updates = Vec::new();
        let revision = self
            .staged
            .get(SOURCE_REVISION_KEY)
            .await?
            .map(|bytes| sdk::wire::decode::<u64>(&bytes))
            .transpose()
            .map_err(Error::Module)?
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| Error::Module("pages: attribution revision exhausted".into()))?;
        for (key, value) in changed {
            let key = String::from_utf8(key)
                .map_err(|_| Error::Module("pages: corrupt logical key".into()))?;
            let (kind, id) = match key.strip_prefix("\0cc:") {
                Some(id) => ("comment", id),
                None if !key.starts_with('\0') => ("block", key.as_str()),
                None => continue,
            };
            let relations = match (kind, value) {
                ("comment", Some(bytes)) => {
                    let comment: super::Comment =
                        sdk::wire::decode(&bytes).map_err(Error::Module)?;
                    if comment.deleted {
                        Vec::new()
                    } else {
                        let mut relations = authored_relations(&comment.author);
                        let mentions: BTreeSet<_> = comment.mentions.into_iter().collect();
                        relations.extend(mentions.into_iter().map(|recipient| Relation {
                            recipient,
                            reason: Reason::Mention,
                            detail: Vec::new(),
                        }));
                        relations
                    }
                }
                ("block", Some(bytes)) => {
                    let block: super::Block = sdk::wire::decode(&bytes).map_err(Error::Module)?;
                    let mut relations = authored_relations(&block.author);
                    let mentions: BTreeSet<_> = block
                        .marks
                        .iter()
                        .filter_map(|mark| match mark.kind {
                            super::InlineMark::Mention(account) => Some(account),
                            _ => None,
                        })
                        .collect();
                    relations.extend(mentions.into_iter().map(|recipient| Relation {
                        recipient,
                        reason: Reason::Mention,
                        detail: Vec::new(),
                    }));
                    let is_page = block.kind == super::BlockKind::Page;
                    if is_page && let Party::Account(recipient) = block.author {
                        relations.push(Relation {
                            recipient,
                            reason: Reason::Ownership,
                            detail: Vec::new(),
                        });
                    }
                    relations
                }
                (_, None) => Vec::new(),
                _ => unreachable!("source kinds are closed above"),
            };
            updates.push(AttributionUpdate {
                object: ObjectRef {
                    kind: kind.into(),
                    object: id.into(),
                },
                revision,
                actor: actor_of(actor),
                relations,
                transfers: Vec::new(),
            });
        }
        if updates.is_empty() {
            return Ok(Vec::new());
        }
        self.staged
            .stage(SOURCE_REVISION_KEY.to_vec(), sdk::wire::encode(&revision));
        Ok(vec![Msg {
            target: attribution,
            payload: attribution::encode_msg(&AttributionMsg::AttributeBatch { updates }),
        }])
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Pages {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the store's REAL merkle root over all blocks, as a 32-byte state root.
    fn root(&self) -> StateRoot {
        self.staged.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        self.staged.state_sync_handle()
    }

    /// the network state-sync serve lane: answers the shared qmdb wire
    /// requests from committed state. read-only.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.staged.serve_sync(req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        self.staged.sync_target().await
    }

    /// Resolve the authenticated actor and mention accounts, then apply the
    /// source operation and relation reports to one reversible staged unit.
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let m = decode_msg(&msg.payload).map_err(Error::Module)?;
        let actor = self.party_of_origin(ctx).await?;
        self.validate_mentions(ctx, &m).await?;
        let output = match &m {
            super::PageMsg::CreatePage { page_id, .. } => sdk::wire::encode(page_id),
            super::PageMsg::InsertBlock { block, .. } => sdk::wire::encode(&block.id),
            super::PageMsg::AddComment { comment_id, .. } => sdk::wire::encode(comment_id),
            _ => sdk::wire::encode(&()),
        };
        let checkpoint = self.staged.checkpoint();
        let now = ctx.env().consensus_time;
        let reports = async {
            let authority = super::Authority {
                actor: actor.clone(),
                origin: ctx.env().origin.clone(),
            };
            self.apply(m, &authority, now)
                .await
                .map_err(|error| Error::Module(error.to_string()))?;
            self.attribution_reports(&actor, &checkpoint).await
        }
        .await;
        let reports = match reports {
            Ok(reports) => reports,
            Err(error) => {
                self.staged.restore(checkpoint);
                return Err(error);
            }
        };
        ctx.set_assigned(super::encode_assigned(&super::PageAssigned { actor }));
        ctx.set_output(output);
        for report in reports {
            ctx.emit_msg(report);
        }
        Ok(())
    }

    /// real async read of own store state, serving STAGED-over-committed via
    /// the overlay, so reads within a block observe this block's writes. the
    /// reserved sentinel reads as absence (it is not a block).
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            PageQuery::GetPage {
                page_id,
                after,
                limit,
            } => {
                let page = if page_id.starts_with('\0') {
                    None
                } else {
                    self.load_page_page(&page_id, after, limit).await?
                };
                Ok(encode_reply(&PageReply::Page(page)))
            }
            PageQuery::GetBlock { block_id } => {
                let block = if block_id.starts_with('\0') {
                    None
                } else {
                    self.load_block(&block_id).await?
                };
                Ok(encode_reply(&PageReply::Block(block)))
            }
            PageQuery::CommentThread { thread_id } => {
                let view = self
                    .thread_view(&thread_id)
                    .await
                    .map_err(|error| Error::Module(error.to_string()))?;
                Ok(encode_reply(&PageReply::CommentThread(view)))
            }
            PageQuery::GetComment { comment_id } => {
                let comment = self
                    .load_comment(&comment_id)
                    .await
                    .map_err(|e| Error::Module(e.to_string()))?;
                Ok(encode_reply(&PageReply::Comment(comment)))
            }
            PageQuery::TargetThreadCount { target } => {
                let ids = self
                    .load_target_index(&target)
                    .await
                    .map_err(|e| Error::Module(e.to_string()))?;
                Ok(encode_reply(
                    &PageReply::TargetThreadCount(ids.len() as u64),
                ))
            }
        }
    }

    /// publish the block-height's staged records in ONE store batch: writes
    /// AND deletes (a `None` value drops a key). no-op (and no root movement)
    /// if nothing was staged. BTreeMap iteration keeps the write order
    /// deterministic across validators.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    /// discard the staged records — nothing reached the store, so `root()` is
    /// unchanged.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}
