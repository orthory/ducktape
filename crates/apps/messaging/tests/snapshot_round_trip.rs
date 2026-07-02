//! snapshot/install round-trip for the in-memory messaging module. state-sync
//! peers are untrusted, so the expected root is the trust anchor: install must
//! strictly decode canonical committed-state bytes, recompute the root before
//! mutating, and leave both committed state and pending overlays untouched on
//! rejection.

use messaging::Messaging;
use messaging_interface::{
    ChatMessage, MessagingMsg, MessagingQuery, MessagingReply, decode_reply, encode_msg,
    encode_query,
};
use sdk::{Ctx, Error, Module, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};

struct TestCtx {
    env: sdk::Env,
}

impl TestCtx {
    fn at(consensus_time: u64) -> Self {
        Self {
            env: sdk::Env {
                height: 0,
                consensus_time,
                origin: Origin::System,
                me: "messaging".into(),
            },
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }

    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, _ev: sdk::Event) {}
    fn request_effect(&mut self, _eff: sdk::Effect) {}
}

fn run<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

fn module_msg(payload: MessagingMsg) -> Msg {
    Msg {
        target: "messaging".into(),
        payload: encode_msg(&payload),
    }
}

fn create(module: &mut Messaging, channel_id: &str, name: &str, at: u64) {
    run(module.execute(
        &mut TestCtx::at(at),
        &module_msg(MessagingMsg::CreateChannel {
            channel_id: channel_id.into(),
            name: name.into(),
        }),
    ))
    .unwrap();
}

fn post(
    module: &mut Messaging,
    channel_id: &str,
    message_id: &str,
    author: &str,
    body: &str,
    at: u64,
) {
    run(module.execute(
        &mut TestCtx::at(at),
        &module_msg(MessagingMsg::PostMessage {
            channel_id: channel_id.into(),
            message_id: message_id.into(),
            author: author.into(),
            body: body.into(),
        }),
    ))
    .unwrap();
}

fn messages(module: &Messaging, channel_id: &str) -> Vec<ChatMessage> {
    let reply = run(module.query(&encode_query(&MessagingQuery::Messages {
        channel_id: channel_id.into(),
    })))
    .unwrap();
    match decode_reply(&reply).unwrap() {
        MessagingReply::Messages(messages) => messages,
        other => panic!("unexpected reply: {other:?}"),
    }
}

fn committed_source() -> Messaging {
    let mut src = Messaging::new("messaging");
    create(&mut src, "general", "General", 10);
    create(&mut src, "random", "Random", 11);
    run(src.commit_block()).unwrap();

    post(&mut src, "general", "m1", "alice", "hello", 20);
    post(&mut src, "general", "m2", "bob", "hi", 21);
    post(&mut src, "random", "m3", "alice", "aside", 22);
    run(src.commit_block()).unwrap();
    src
}

#[test]
fn install_reconstructs_source_root_and_history() {
    let src = committed_source();
    let src_root = src.root();
    let bytes = src.snapshot();

    let mut h = Sha256::new();
    h.update(&bytes);
    assert_eq!(
        StateRoot(h.finalize().into()),
        src_root,
        "sha256(snapshot()) == root()"
    );

    let mut dst = Messaging::new("messaging");
    create(&mut dst, "stale", "Stale", 30);
    run(dst.commit_block()).unwrap();
    post(&mut dst, "stale", "pending", "mallory", "drop me", 31);

    dst.install(&bytes, src_root).unwrap();

    assert_eq!(dst.root(), src_root, "installed root equals source root");
    assert_eq!(messages(&dst, "general"), messages(&src, "general"));
    assert_eq!(messages(&dst, "random"), messages(&src, "random"));
    assert!(
        messages(&dst, "stale").is_empty(),
        "install replaces state and clears pending"
    );
}

#[test]
fn tampered_snapshot_is_rejected_and_leaves_target_untouched() {
    let src = committed_source();
    let src_root = src.root();
    let mut bytes = src.snapshot();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;

    let mut dst = Messaging::new("messaging");
    create(&mut dst, "local", "Local", 40);
    run(dst.commit_block()).unwrap();
    post(&mut dst, "local", "pending", "alice", "not committed", 41);
    let before_root = dst.root();
    let before_view = messages(&dst, "local");

    let err = dst.install(&bytes, src_root).unwrap_err();
    assert!(matches!(err, Error::Module(_)));
    assert_eq!(
        dst.root(),
        before_root,
        "failed install must not move committed root"
    );
    assert_eq!(
        messages(&dst, "local"),
        before_view,
        "failed install must preserve pending overlay"
    );
}

#[test]
fn truncated_and_padded_snapshots_are_rejected() {
    let src = committed_source();
    let src_root = src.root();
    let bytes = src.snapshot();

    let mut dst = Messaging::new("messaging");
    create(&mut dst, "local", "Local", 50);
    run(dst.commit_block()).unwrap();
    post(&mut dst, "local", "pending", "alice", "not committed", 51);
    let before_root = dst.root();
    let before_view = messages(&dst, "local");

    assert!(dst.install(&bytes[..bytes.len() - 1], src_root).is_err());

    let mut padded = bytes.clone();
    padded.push(0);
    assert!(dst.install(&padded, src_root).is_err());

    assert_eq!(dst.root(), before_root);
    assert_eq!(messages(&dst, "local"), before_view);
}

#[test]
fn empty_snapshot_installs_as_zero_state() {
    let src = Messaging::new("messaging");
    let bytes = src.snapshot();

    let mut dst = committed_source();
    dst.install(&bytes, StateRoot::ZERO).unwrap();

    assert_eq!(dst.root(), StateRoot::ZERO);
    assert!(messages(&dst, "general").is_empty());
}
