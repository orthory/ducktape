//! the shared harness: a module over an in-memory store, plus the ctx
//! constructors the behaviour tests drive it with.
//!
//! `identity` answers "no account" by default, so an external key resolves to
//! `Party::Key(key)` — the non-account principal every test signs as unless it
//! deliberately registers an account. `tasks` answers with whatever job the
//! test installs, so attempt fencing is exercised against a real reply shape.

#![allow(dead_code)]

use collaboration::{
    encode_msg, encode_query, CollaborationMsg, CollaborationQuery, CollaborationReply,
    Collaboration, MessageId, MessageKind, ProtectedRead, Role, SendRequest,
};
use sdk::{Cause, Env, Error, Module, Msg, Origin};
use sdk_testkit::{MemStore, TestCtx};

pub const MODULE: &str = "collaboration";
pub const IDENTITY: &str = "identity";
pub const TASKS: &str = "tasks";
/// the height-lane ceiling every test composes with, so a deadline reads in
/// the same units the assertions use.
pub const MAX_TTL: u64 = collaboration::HEIGHT_LANE_MAX_DELIVERY_TTL;

pub fn module() -> Collaboration {
    Collaboration::new(
        MODULE,
        IDENTITY,
        TASKS,
        Box::new(MemStore::new()),
        MAX_TTL,
    )
}

/// an authenticated external signer: 32 bytes of one value, the shape a real
/// ed25519 origin has.
pub fn key(byte: u8) -> Vec<u8> {
    vec![byte; 32]
}

/// a ctx at `now`, dispatching as the external key `signer`.
pub fn at(now: u64, origin: Origin) -> TestCtx {
    with_job(now, origin, None)
}

/// a ctx whose `tasks` sibling answers with `job` for every job query — the
/// attempt fence's input.
pub fn with_job(now: u64, origin: Origin, job: Option<tasks::Job>) -> TestCtx {
    TestCtx::with_env(Env {
        height: now,
        consensus_time: now,
        origin,
        me: MODULE.into(),
        cause: Cause::Direct,
    })
    .on_query(IDENTITY, |_| {
        Ok(identity::encode_reply(&identity::IdentityReply::Account(
            None,
        )))
    })
    .on_query(TASKS, move |_| {
        Ok(tasks::encode_job_reply(&tasks::JobsReply::Job(job.clone())))
    })
}

/// a job record with `attempt`, enough for the attempt fence to read.
pub fn job(job_id: &str, attempt: u64) -> tasks::Job {
    tasks::Job {
        job_id: job_id.into(),
        kind: "review".into(),
        spec: "{}".into(),
        submitter: tasks::Party::System,
        status: tasks::JobStatus::Processing,
        attempt,
        claim: None,
        result: None,
        comments: Vec::new(),
        created_at_revision: 1,
        created_at_height: 1,
        updated_at_height: 1,
    }
}

pub fn msg(payload: CollaborationMsg) -> Msg {
    Msg {
        target: MODULE.into(),
        payload: encode_msg(&payload),
    }
}

/// run one op and commit the block, the way a host does on success.
pub async fn apply(
    module: &mut Collaboration,
    ctx: &mut TestCtx,
    payload: CollaborationMsg,
) -> Result<(), Error> {
    let result = module.execute(ctx, &msg(payload)).await;
    if result.is_ok() {
        module.commit_block().await?;
    } else {
        module.abort_block().await?;
    }
    result
}

/// apply and unwrap — the happy-path setup steps.
pub async fn ok(module: &mut Collaboration, ctx: &mut TestCtx, payload: CollaborationMsg) {
    apply(module, ctx, payload)
        .await
        .unwrap_or_else(|e| panic!("expected the op to be admitted: {e:?}"));
}

pub async fn read(
    module: &Collaboration,
    ctx: &TestCtx,
    participant_id: &str,
    via: Option<&str>,
    read: ProtectedRead,
) -> CollaborationReply {
    let request = encode_query(&CollaborationQuery::Read {
        participant_id: participant_id.into(),
        via: via.map(str::to_string),
        read,
    });
    let bytes = module
        .query_with(ctx, &request)
        .await
        .expect("a read answers, refusals included");
    collaboration::decode_reply(&bytes).expect("a reply decodes")
}

/// the conversation event sequence an admission took. derived, never
/// hardcoded: every committed change consumes a sequence, so a test that
/// counts them by hand breaks the moment a setup step changes.
pub async fn admitted_seq(
    module: &Collaboration,
    ctx: &TestCtx,
    participant_id: &str,
    generation: u64,
    sequence: u64,
) -> u64 {
    let CollaborationReply::SendState(collaboration::SendState::Admitted { seq, .. }) = read(
        module,
        ctx,
        participant_id,
        None,
        ProtectedRead::SendState {
            generation,
            sequence,
        },
    )
    .await
    else {
        panic!("the send was admitted under {generation}/{sequence}");
    };
    seq
}

/// the credential a participant's current binding holds. Drawn from the
/// participant's own allocator, so it depends on how many credentials that
/// participant has been issued — derive it rather than guessing.
pub async fn credential_of(
    module: &Collaboration,
    ctx: &TestCtx,
    participant_id: &str,
    conversation_id: &str,
) -> u64 {
    let CollaborationReply::Binding(Some(view)) = read(
        module,
        ctx,
        participant_id,
        None,
        ProtectedRead::Binding {
            conversation_id: conversation_id.into(),
        },
    )
    .await
    else {
        panic!("{participant_id} holds a binding on {conversation_id}");
    };
    view.credential
}

// ---- op builders ----------------------------------------------------------

pub fn register(participant_id: &str) -> CollaborationMsg {
    CollaborationMsg::RegisterParticipant {
        participant_id: participant_id.into(),
        display_name: participant_id.into(),
        agent_account: None,
    }
}

pub fn conversation(conversation_id: &str) -> CollaborationMsg {
    CollaborationMsg::CreateConversation {
        conversation_id: conversation_id.into(),
        topic: "review".into(),
    }
}

pub fn seat(conversation_id: &str, participant_id: &str, role: Option<Role>) -> CollaborationMsg {
    CollaborationMsg::SetRoster {
        conversation_id: conversation_id.into(),
        participant_id: participant_id.into(),
        role,
    }
}

pub fn bind(
    conversation_id: &str,
    participant_id: &str,
    service_key: Vec<u8>,
    expected_credential: u64,
) -> CollaborationMsg {
    CollaborationMsg::Bind {
        conversation_id: conversation_id.into(),
        participant_id: participant_id.into(),
        device: "laptop".into(),
        service_key,
        expected_credential,
    }
}

/// a minimal notice from `sender` to `recipient` under credential `generation`
/// at `sequence`, expiring at `expires_at`.
pub fn note(
    conversation_id: &str,
    sender: &str,
    recipient: &str,
    generation: u64,
    sequence: u64,
    expires_at: u64,
) -> SendRequest {
    SendRequest {
        conversation_id: conversation_id.into(),
        sender_participant_id: sender.into(),
        message_id: MessageId {
            generation,
            sequence,
        },
        recipient_participant_id: recipient.into(),
        kind: MessageKind::Notice,
        reply_to: None,
        task: None,
        body: "ping".into(),
        references: Vec::new(),
        expires_at,
    }
}

/// the whole setup every messaging test starts from: two participants owned by
/// two different keys, both seated as members on one conversation owned by the
/// first.
pub struct Scene {
    pub module: Collaboration,
    pub owner_a: Vec<u8>,
    pub owner_b: Vec<u8>,
}

pub async fn scene(conversation_id: &str) -> Scene {
    let mut module = module();
    let owner_a = key(1);
    let owner_b = key(2);

    let mut ctx = at(1, Origin::External(owner_a.clone()));
    ok(&mut module, &mut ctx, register("alice")).await;
    ok(&mut module, &mut ctx, conversation(conversation_id)).await;

    let mut ctx_b = at(1, Origin::External(owner_b.clone()));
    ok(&mut module, &mut ctx_b, register("bob")).await;

    ok(
        &mut module,
        &mut ctx,
        seat(conversation_id, "alice", Some(Role::Member)),
    )
    .await;
    ok(
        &mut module,
        &mut ctx,
        seat(conversation_id, "bob", Some(Role::Member)),
    )
    .await;

    Scene {
        module,
        owner_a,
        owner_b,
    }
}
