//! the tagging plane — the cross-module engagement router.
//!
//! generalizes "content in module A names an entity of module B, and B gets
//! engaged": a content module reports each content item as a [`TagEvent`] in
//! the same block as the content, and this module sends an [`EngagementEvent`]
//! to every explicitly named, genesis-configured entity-owner module. Scope
//! subscribers are added to that recipient set for policies that also engage
//! on untagged content.
//!
//! ## router only
//!
//! the plane holds subscription state and routes — nothing else. engagement
//! POLICY (mention-gating, assignment, round-robin) lives in each recipient
//! module; the plane delivers tagged content directly to tag owners and every
//! user-authored content event to scope subscribers. The recipient decides
//! what engages.
//!
//! ## origin-keyed trust (spoof-proof by construction)
//!
//! every op is MODULE-ORIGIN ONLY, and the trusted party is always the
//! dispatch origin, never a payload field: a [`TagEvent`]'s source is the
//! emitting module, a subscription's subscriber is the subscribing module.
//! an external submitter has no surface here at all.
//!
//! ## the loop rule (generic, stated once)
//!
//! an engaged entity's reply is itself content and would re-fire engagement.
//! the plane drops every non-[`Author::User`] tag event: only external users
//! open engagement, entity-/module-/system-authored content never does —
//! chat's agent-answers-agent loop prevention, stated module-agnostically.
//!
//! ## the no-fail tag intake
//!
//! [`TaggingMsg::Tag`] rides the same block as the content that produced it
//! (the discipline chat hooks established): an `Err` here would abort the
//! user's post, so a malformed report — undecodable, oversized ids, an
//! overlong tag list — is a staged no-op (plus an observability note) or a
//! deterministic truncation, NEVER an error. [`TaggingMsg::Subscribe`] /
//! [`TaggingMsg::Unsubscribe`] are the opposite on purpose: they ride the
//! SUBSCRIBER's own admin block, where a validation error aborting the whole
//! registration is exactly the atomicity the caller wants.
//!
//! `root()` folds in every scope and subscriber, so any transition moves the
//! app-hash. a joiner rebuilds this module from a peer via
//! [`TaggingModule::snapshot`] / [`TaggingModule::install`]: the snapshot
//! ships the committed map in the exact canonical encoding `root()` hashes,
//! and install re-derives the root from the decoded temporaries before
//! adopting them.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::{BTreeMap, BTreeSet};

use sdk::codec;
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::{Digest as _, Sha256};

/// the field separator inside composite scope keys. rejected inside
/// caller-chosen ids so a crafted container can never forge another scope.
const SEP: char = '\x1f';

/// the composite subscription key: scopes are namespaced per source module.
fn scope_key(source: &str, container: &str) -> String {
    format!("{source}{SEP}{container}")
}

// ---- state ---------------------------------------------------------------------

/// everything `root()` commits to: subscription scopes → subscriber sets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Committed {
    subscriptions: BTreeMap<String, BTreeSet<ModuleId>>,
}

// ---- canonical encoding ----------------------------------------------------------

/// canonical byte encoding of the committed state: u64-le counts, sorted-key
/// order, u64-le length prefixes (via [`sdk::codec`]). this is the exact
/// preimage `root()` hashes AND the snapshot format.
fn encode_committed(c: &Committed) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(c.subscriptions.len() as u64).to_le_bytes());
    for (key, subscribers) in &c.subscriptions {
        codec::push_bytes(&mut out, key.as_bytes());
        out.extend_from_slice(&(subscribers.len() as u64).to_le_bytes());
        for subscriber in subscribers {
            codec::push_bytes(&mut out, subscriber.as_bytes());
        }
    }
    out
}

fn committed_root(c: &Committed) -> StateRoot {
    StateRoot(Sha256::digest(encode_committed(c)).into())
}

// ---- strict decode (untrusted snapshot bytes) -------------------------------------
// over the shared `sdk::codec::Cursor`: every accessor bounds-checks before it
// reads, and `bound` rejects a forged count the remaining bytes cannot hold.

fn decode_committed(bytes: &[u8]) -> Result<Committed, Error> {
    let mut cur = codec::Cursor::new(bytes);
    let mut c = Committed::default();
    let scopes = cur.u64("scope count")?;
    // every scope costs at least its two fixed-width counts (8 + 8); a count
    // the input cannot possibly hold is rejected before the loop builds
    // anything.
    cur.bound(scopes, 8 + 8, "scope")?;
    for _ in 0..scopes {
        let key = cur.string("scope key")?;
        if let Some((last, _)) = c.subscriptions.iter().next_back()
            && last.as_str() >= key.as_str()
        {
            return Err(Error::Module("snapshot keys not strictly ascending".into()));
        }
        let count = cur.u64("subscriber count")?;
        cur.bound(count, 8, "subscriber")?;
        let mut subscribers = BTreeSet::new();
        for _ in 0..count {
            let subscriber = cur.string("subscriber")?;
            match subscribers.iter().next_back() {
                Some(last) if *last >= subscriber => {
                    return Err(Error::Module(
                        "snapshot subscribers not strictly ascending".into(),
                    ));
                }
                _ => {}
            }
            subscribers.insert(subscriber);
        }
        if subscribers.is_empty() {
            // committed state never holds an empty scope (unsubscribe of the
            // last subscriber removes the key), so a snapshot must not either.
            return Err(Error::Module("snapshot has an empty subscription scope".into()));
        }
        c.subscriptions.insert(key, subscribers);
    }
    cur.finish("snapshot")?;
    Ok(c)
}

// ---- the module -----------------------------------------------------------------

pub struct TaggingModule {
    id: ModuleId,
    /// Genesis-configured modules that may receive explicit entity mentions
    /// without a scope subscription. This is deliberately not payload-driven:
    /// an external author can construct a tag, so routing it to an arbitrary
    /// registered module could make that module reject the foreign event and
    /// abort the content block.
    direct_owners: BTreeSet<ModuleId>,
    /// committed state — what `root()` and the app-hash commit to.
    committed: Committed,
    /// this block's staged writes, read ahead of committed state
    /// (read-your-writes) but merged in only at `commit_block`. a staged
    /// empty set is a scope removal.
    staged: BTreeMap<String, BTreeSet<ModuleId>>,
}

impl TaggingModule {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            direct_owners: BTreeSet::new(),
            committed: Committed::default(),
            staged: BTreeMap::new(),
        }
    }

    /// Allow one engagement-aware module to receive explicit entity tags
    /// directly. Genesis config, not committed state; every production host
    /// must wire the same set just like its module registry.
    pub fn with_direct_owner(mut self, owner: impl Into<ModuleId>) -> Self {
        self.direct_owners.insert(owner.into());
        self
    }

    // ---- staged-overlay reads ----------------------------------------------------

    fn subscribers(&self, key: &str) -> Option<&BTreeSet<ModuleId>> {
        match self.staged.get(key) {
            Some(set) if set.is_empty() => None,
            Some(set) => Some(set),
            None => self.committed.subscriptions.get(key),
        }
    }

    // ---- validation helpers --------------------------------------------------------

    fn validate_id(field: &str, value: &str) -> Result<(), Error> {
        if value.is_empty() {
            return Err(Error::Module(format!("{field} must be non-empty")));
        }
        if value.len() > MAX_ID_BYTES {
            return Err(Error::Module(format!(
                "{field} is {} bytes; the cap is {MAX_ID_BYTES}",
                value.len()
            )));
        }
        if value.contains(SEP) {
            return Err(Error::Module(format!(
                "{field} must not contain the reserved separator"
            )));
        }
        Ok(())
    }

    /// the module behind the current dispatch — every tagging op's acting
    /// party. externals and the system have no surface here.
    fn acting_module(origin: &Origin) -> Result<ModuleId, Error> {
        match origin {
            Origin::Module(module) => Ok(module.clone()),
            _ => Err(Error::Module(
                "tagging ops are module-origin only (subscriber and source derive from the \
                 emitting module)"
                    .into(),
            )),
        }
    }

    fn note(&self, ctx: &mut dyn Ctx, text: String) {
        ctx.emit_event(Event {
            source: self.id.clone(),
            payload: text.into_bytes(),
        });
    }

    // ---- the subscription arm (validating — rides the subscriber's block) -----------

    fn on_subscribe(
        &mut self,
        ctx: &mut dyn Ctx,
        source: String,
        container: String,
    ) -> Result<(), Error> {
        let subscriber = Self::acting_module(&ctx.env().origin)?;
        Self::validate_id("source", &source)?;
        Self::validate_id("container", &container)?;
        // the registry is genesis-fixed, so this existence check is
        // deterministic across every validator.
        if ctx.module_root(&source).is_none() {
            return Err(Error::Module(format!(
                "source {source:?} is not a registered module"
            )));
        }
        let key = scope_key(&source, &container);
        let mut set = self.subscribers(&key).cloned().unwrap_or_default();
        if set.contains(&subscriber) {
            // idempotent: re-subscribing stages nothing.
            return Ok(());
        }
        if set.len() >= MAX_SUBSCRIBERS_PER_SCOPE {
            return Err(Error::Module(format!(
                "scope has {MAX_SUBSCRIBERS_PER_SCOPE} subscribers already"
            )));
        }
        set.insert(subscriber);
        self.staged.insert(key, set);
        Ok(())
    }

    fn on_unsubscribe(
        &mut self,
        ctx: &mut dyn Ctx,
        source: String,
        container: String,
    ) -> Result<(), Error> {
        let subscriber = Self::acting_module(&ctx.env().origin)?;
        let key = scope_key(&source, &container);
        let Some(set) = self.subscribers(&key) else {
            // idempotent: unsubscribing an absent subscription stages nothing.
            return Ok(());
        };
        if !set.contains(&subscriber) {
            return Ok(());
        }
        let mut set = set.clone();
        set.remove(&subscriber);
        // a staged empty set removes the scope at commit.
        self.staged.insert(key, set);
        Ok(())
    }

    // ---- the tag intake (NO-FAIL — rides the content's block) -----------------------

    fn on_tag(&mut self, ctx: &mut dyn Ctx, event: TagEvent) -> Result<(), Error> {
        let source = Self::acting_module(&ctx.env().origin)?;
        let TagEvent {
            container,
            content_seq,
            author,
            mut tags,
        } = event;
        if Self::validate_id("container", &container).is_err() {
            self.note(ctx, "dropped tag event with a malformed container".into());
            return Ok(());
        }
        // THE LOOP RULE: only external users open engagement. an engaged
        // entity's reply is itself content — entity-, module-, and
        // system-authored events are dropped here, plane-wide, so no
        // subscriber can be re-fired into an entity-answers-entity loop.
        let crate::Author::User(_) = &author else {
            return Ok(());
        };
        // deterministic truncation, never rejection: this arm must not abort
        // the content's block over a chatty tag list.
        if tags.len() > MAX_TAGS_PER_EVENT {
            tags.truncate(MAX_TAGS_PER_EVENT);
        }
        let malformed = |t: &crate::EntityRef| {
            Self::validate_id("tag module", &t.module).is_err()
                || Self::validate_id("tag entity", &t.entity).is_err()
        };
        if tags.iter().any(malformed) {
            self.note(ctx, "dropped malformed tags from a tag event".into());
            tags.retain(|t| !malformed(t));
        }
        // Exact-scope subscriptions continue to drive All/Assigned/RR
        // policies. A structured entity mention additionally reaches the
        // entity's owning module directly, even when this source/container
        // has no pre-existing subscription (for example a fresh Pages
        // comment thread). Deduping keeps a watched chat mention single-shot.
        let mut recipients = self
            .subscribers(&scope_key(&source, &container))
            .cloned()
            .unwrap_or_default();
        for tag in &tags {
            // Registry membership is genesis-fixed, so malformed/unknown
            // owner ids are deterministic no-ops on this no-fail arm.
            if self.direct_owners.contains(&tag.module)
                && ctx.module_root(&tag.module).is_some()
            {
                recipients.insert(tag.module.clone());
            }
        }
        if recipients.is_empty() {
            return Ok(());
        }
        let delivery = EngagementEvent {
            source,
            container,
            content_seq,
            author,
            tags,
        };
        for subscriber in recipients {
            ctx.emit_msg(Msg {
                target: subscriber,
                payload: encode_event(&delivery),
            });
        }
        Ok(())
    }

    // ---- state-sync -----------------------------------------------------------------

    /// serialize the COMMITTED state (never the staged overlay) into the
    /// canonical encoding `root()` commits to.
    pub fn snapshot(&self) -> Vec<u8> {
        encode_committed(&self.committed)
    }

    /// adopt a peer's snapshot — only after the decoded temporaries re-derive
    /// `expected` via the exact `root()` algorithm. all-or-nothing.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let committed = decode_committed(bytes)?;
        sdk::verify_snapshot_root(committed_root(&committed), expected)?;
        self.committed = committed;
        self.staged.clear();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for TaggingModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        committed_root(&self.committed)
    }

    fn snapshot_bytes(&self) -> Option<Vec<u8>> {
        Some(self.snapshot())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let decoded = match decode_msg(&msg.payload) {
            Ok(decoded) => decoded,
            Err(_) if matches!(ctx.env().origin, Origin::Module(_)) => {
                // a module shipping undecodable bytes may be riding someone
                // else's block (the tag lane) — a staged no-op, never an
                // abort of a block this module doesn't own.
                self.note(ctx, "dropped an undecodable tagging op".into());
                return Ok(());
            }
            Err(e) => return Err(Error::Module(e)),
        };
        match decoded {
            TaggingMsg::Subscribe { source, container } => {
                self.on_subscribe(ctx, source, container)
            }
            TaggingMsg::Unsubscribe { source, container } => {
                self.on_unsubscribe(ctx, source, container)
            }
            TaggingMsg::Tag(event) => self.on_tag(ctx, event),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (key, set) in std::mem::take(&mut self.staged) {
            if set.is_empty() {
                self.committed.subscriptions.remove(&key);
            } else {
                self.committed.subscriptions.insert(key, set);
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Author, EntityRef, decode_event, encode_msg};
    use futures::executor::block_on;
    use sdk::Env;
    use sdk_testkit::TestCtx;

    /// module ids `module_root` reports as live — the genesis-fixed registry.
    /// tagging gates source-liveness and tag-target resolution on
    /// `module_root(target).is_some()`.
    const REGISTERED: [&str; 5] = ["chat", "agent", "pages", "runs", "tagging"];

    // build the ctx a host hands tagging: env at block 0 with a chosen origin,
    // `me = "tagging"`, and the registered modules live via `module_root`.
    fn ctx_with_origin(origin: Origin) -> TestCtx {
        REGISTERED.iter().fold(
            TestCtx::with_env(Env {
                height: 0,
                consensus_time: 0,
                origin,
                me: "tagging".into(),
                protocol_version: 0,
            }),
            |ctx, module| ctx.with_module_root(module, StateRoot::ZERO),
        )
    }

    fn module() -> TaggingModule {
        TaggingModule::new("tagging").with_direct_owner("runs")
    }
    fn exec(m: &mut TaggingModule, ctx: &mut TestCtx, payload: &TaggingMsg) -> Result<(), Error> {
        let msg = Msg {
            target: "tagging".into(),
            payload: encode_msg(payload),
        };
        block_on(m.execute(ctx, &msg))
    }
    fn commit(m: &mut TaggingModule) {
        block_on(m.commit_block()).unwrap();
    }
    fn from_module(id: &str) -> TestCtx {
        ctx_with_origin(Origin::Module(id.into()))
    }
    fn subscribe(source: &str, container: &str) -> TaggingMsg {
        TaggingMsg::Subscribe {
            source: source.into(),
            container: container.into(),
        }
    }
    fn user_tag(container: &str, seq: u64, tags: Vec<EntityRef>) -> TaggingMsg {
        TaggingMsg::Tag(TagEvent {
            container: container.into(),
            content_seq: seq,
            author: Author::User(b"human".to_vec()),
            tags,
        })
    }
    fn agent_ref(entity: &str) -> EntityRef {
        EntityRef {
            module: "agent".into(),
            entity: entity.into(),
        }
    }

    #[test]
    fn ops_are_module_origin_only() {
        let mut m = module();
        for origin in [Origin::External(b"user".to_vec()), Origin::System] {
            let mut ctx = ctx_with_origin(origin);
            assert!(exec(&mut m, &mut ctx, &subscribe("chat", "general")).is_err());
            assert!(exec(&mut m, &mut ctx, &user_tag("general", 1, vec![])).is_err());
        }
        // an EXTERNAL submitter's undecodable bytes error (their own block);
        // a MODULE's undecodable bytes are a staged no-op (someone else's).
        let garbage = Msg {
            target: "tagging".into(),
            payload: b"not json".to_vec(),
        };
        let mut ctx = ctx_with_origin(Origin::External(b"user".to_vec()));
        assert!(block_on(m.execute(&mut ctx, &garbage)).is_err());
        let mut ctx = from_module("chat");
        block_on(m.execute(&mut ctx, &garbage)).unwrap();
        assert!(ctx.msgs().is_empty());
    }

    #[test]
    fn subscribe_validates_and_is_idempotent() {
        let mut m = module();
        // unknown source module is a registration error.
        let mut ctx = from_module("agent");
        assert!(exec(&mut m, &mut ctx, &subscribe("nonexistent", "c")).is_err());
        // separator-carrying container can never forge a foreign scope.
        assert!(exec(&mut m, &mut ctx, &subscribe("chat", "a\u{1f}b")).is_err());

        exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        commit(&mut m);
        let root_once = m.root();
        // re-subscribing stages nothing: the root is unchanged.
        let mut ctx = from_module("agent");
        exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        commit(&mut m);
        assert_eq!(m.root(), root_once);
    }

    #[test]
    fn tag_fans_out_to_subscribers_same_block() {
        let mut m = module();
        let mut ctx = from_module("agent");
        exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        commit(&mut m);

        let mut ctx = from_module("chat");
        exec(
            &mut m,
            &mut ctx,
            &user_tag("general", 7, vec![agent_ref("bot")]),
        )
        .unwrap();
        assert_eq!(ctx.msgs().len(), 1);
        assert_eq!(ctx.msgs()[0].target, "agent");
        let event = decode_event(&ctx.msgs()[0].payload).unwrap();
        assert_eq!(event.source, "chat");
        assert_eq!(event.container, "general");
        assert_eq!(event.content_seq, 7);
        assert_eq!(event.tags, vec![agent_ref("bot")]);

        // an unsubscribed container delivers nothing.
        let mut ctx = from_module("chat");
        exec(&mut m, &mut ctx, &user_tag("other", 1, vec![])).unwrap();
        assert!(ctx.msgs().is_empty());

        // the SOURCE is the origin: the same container name under another
        // source module is a different scope.
        let mut ctx = from_module("pages");
        exec(&mut m, &mut ctx, &user_tag("general", 1, vec![])).unwrap();
        assert!(ctx.msgs().is_empty());
    }

    #[test]
    fn entity_mention_routes_to_owner_without_a_scope_subscription() {
        let mut m = module();
        let mut ctx = from_module("pages");
        let tag = EntityRef {
            module: "runs".into(),
            entity: "qa-luna".into(),
        };
        exec(
            &mut m,
            &mut ctx,
            &user_tag("thread-1", 1, vec![tag.clone()]),
        )
        .unwrap();
        assert_eq!(ctx.msgs().len(), 1);
        assert_eq!(ctx.msgs()[0].target, "runs");
        let event = decode_event(&ctx.msgs()[0].payload).unwrap();
        assert_eq!(event.source, "pages");
        assert_eq!(event.container, "thread-1");
        assert_eq!(event.tags, vec![tag]);

        // A crafted tag naming an arbitrary registered module is not a
        // routing capability: Pages would reject EngagementEvent bytes and
        // abort the user's content block if this were delivered.
        let mut ctx = from_module("pages");
        exec(
            &mut m,
            &mut ctx,
            &user_tag(
                "thread-2",
                1,
                vec![EntityRef {
                    module: "pages".into(),
                    entity: "not-an-agent".into(),
                }],
            ),
        )
        .unwrap();
        assert!(ctx.msgs().is_empty());
    }

    #[test]
    fn loop_rule_only_user_authors_fire() {
        let mut m = module();
        let mut ctx = from_module("agent");
        exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        commit(&mut m);

        for author in [
            Author::Entity(agent_ref("bot")),
            Author::Module("chat".into()),
            Author::System,
        ] {
            let mut ctx = from_module("chat");
            exec(
                &mut m,
                &mut ctx,
                &TaggingMsg::Tag(TagEvent {
                    container: "general".into(),
                    content_seq: 9,
                    author,
                    tags: vec![agent_ref("bot")],
                }),
            )
            .unwrap();
            assert!(ctx.msgs().is_empty(), "non-user authors must never fire");
        }
    }

    #[test]
    fn tag_intake_never_fails_on_malformed_content() {
        let mut m = module();
        let mut ctx = from_module("agent");
        exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        commit(&mut m);

        // malformed container: dropped, not an error (the content block lives).
        let mut ctx = from_module("chat");
        exec(&mut m, &mut ctx, &user_tag("", 1, vec![])).unwrap();
        assert!(ctx.msgs().is_empty());

        // an overlong tag list is truncated deterministically, not rejected.
        let many: Vec<EntityRef> = (0..MAX_TAGS_PER_EVENT + 4)
            .map(|i| agent_ref(&format!("bot{i}")))
            .collect();
        let mut ctx = from_module("chat");
        exec(&mut m, &mut ctx, &user_tag("general", 2, many)).unwrap();
        let event = decode_event(&ctx.msgs()[0].payload).unwrap();
        assert_eq!(event.tags.len(), MAX_TAGS_PER_EVENT);

        // malformed tags are filtered out; well-formed ones still deliver.
        let mut ctx = from_module("chat");
        exec(
            &mut m,
            &mut ctx,
            &user_tag(
                "general",
                3,
                vec![
                    EntityRef {
                        module: String::new(),
                        entity: "x".into(),
                    },
                    agent_ref("bot"),
                ],
            ),
        )
        .unwrap();
        let event = decode_event(&ctx.msgs()[0].payload).unwrap();
        assert_eq!(event.tags, vec![agent_ref("bot")]);
    }

    #[test]
    fn unsubscribe_removes_scope_and_is_idempotent() {
        let mut m = module();
        let mut ctx = from_module("agent");
        exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        commit(&mut m);
        let empty_root = TaggingModule::new("tagging").root();
        assert_ne!(m.root(), empty_root);

        let mut ctx = from_module("agent");
        exec(
            &mut m,
            &mut ctx,
            &TaggingMsg::Unsubscribe {
                source: "chat".into(),
                container: "general".into(),
            },
        )
        .unwrap();
        commit(&mut m);
        // the last subscriber's departure removes the scope entirely: the
        // root returns to the empty module's.
        assert_eq!(m.root(), empty_root);

        // unsubscribing again (or a never-subscribed scope) stages nothing.
        let mut ctx = from_module("agent");
        exec(
            &mut m,
            &mut ctx,
            &TaggingMsg::Unsubscribe {
                source: "chat".into(),
                container: "general".into(),
            },
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(m.root(), empty_root);
    }

    #[test]
    fn subscriber_cap_is_enforced() {
        let mut m = module();
        for i in 0..MAX_SUBSCRIBERS_PER_SCOPE {
            let mut ctx = from_module(Box::leak(format!("m{i}").into_boxed_str()));
            exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        }
        let mut ctx = from_module("one-too-many");
        assert!(exec(&mut m, &mut ctx, &subscribe("chat", "general")).is_err());
    }

    #[test]
    fn abort_discards_staged_writes() {
        let mut m = module();
        let mut ctx = from_module("agent");
        exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        let before = m.root();
        block_on(m.abort_block()).unwrap();
        assert_eq!(m.root(), before);
        // and the staged subscription is gone: a tag delivers nothing.
        let mut ctx = from_module("chat");
        exec(&mut m, &mut ctx, &user_tag("general", 1, vec![])).unwrap();
        assert!(ctx.msgs().is_empty());
    }

    #[test]
    fn snapshot_install_roundtrip_and_rejects() {
        let mut m = module();
        let mut ctx = from_module("agent");
        exec(&mut m, &mut ctx, &subscribe("chat", "general")).unwrap();
        exec(&mut m, &mut ctx, &subscribe("pages", "space-1")).unwrap();
        commit(&mut m);

        let mut fresh = module();
        fresh.install(&m.snapshot(), m.root()).unwrap();
        assert_eq!(fresh.root(), m.root());

        // a wrong expected root refuses to install.
        let mut fresh = module();
        assert!(fresh.install(&m.snapshot(), StateRoot::ZERO).is_err());
        // trailing bytes refuse to decode.
        let mut bytes = m.snapshot();
        bytes.push(0);
        let mut fresh = module();
        assert!(fresh.install(&bytes, m.root()).is_err());
    }
}
