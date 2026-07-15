//! governance authorizes a module CODE swap the same way it authorizes a node
//! upgrade: a member-gated proposal + simple-majority tally, and on passing,
//! `handle_execute` emits the modreg op as a host-drained follow-up.
//!
//! unlike the upgrade twin (which pins against a stub), these tests register the
//! REAL code registry: what is pinned is the whole authorization chain the live
//! network runs — governance emits, the host stamps `Origin::Module("governance")`,
//! modreg's origin gate accepts, and the pending swap LANDS in consensus state.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use futures::executor::block_on;
use governance::Governance;
use governance::{
    GovAction, GovMsg, GovQuery, GovReply, ProposalStatus, decode_reply as gov_decode,
    encode_msg as gov_encode, encode_query as gov_query,
};
use host::{BlockContext, Host, SubmitError};
use modreg::{
    ModregMsg, ModregQuery, ModregReply, Modreg, decode_reply as modreg_decode,
    encode_msg as modreg_encode, encode_query as modreg_query,
};
use sdk::{Error, Module as _, Msg, Origin};
use valset::Valset;

fn member_key(seed: u8) -> Vec<u8> {
    let seed = [seed; 32];
    PrivateKey::decode(&seed[..])
        .expect("any 32 bytes is a valid seed")
        .public_key()
        .as_ref()
        .to_vec()
}

fn hash(seed: u8) -> Vec<u8> {
    vec![seed; modreg::CODE_HASH_LEN]
}

/// a host with valset (members 1,2) + governance wired to the REAL code
/// registry, and the `hello` module pre-registered (active code = hash(1)).
async fn gov_host_with_modreg() -> Host {
    let mut valset = Valset::new("valset");
    valset.insert(member_key(1));
    valset.insert(member_key(2));
    let mut host = Host::genesis(vec![
        Box::new(valset),
        Box::new(Governance::new("governance", "valset", "upgrade", "identity").with_modreg("modreg")),
        Box::new(Modreg::new("modreg", "valset")),
    ])
    .expect("genesis");
    // genesis-bootstrap the swappable module's initial code (System origin).
    host.submit_at(
        BlockContext {
            protocol_version: 0,
            height: 0,
            consensus_time: 0,
            origin: Origin::System,
        },
        Msg {
            target: "modreg".into(),
            payload: modreg_encode(&ModregMsg::Register {
                module_id: "hello".into(),
                code_hash: hash(1),
            }),
        },
    )
    .await
    .expect("register hello");
    host
}

async fn submit_as(
    host: &mut Host,
    who: &[u8],
    at: u64,
    payload: Vec<u8>,
) -> Result<(), SubmitError> {
    host.submit_at(
        BlockContext {
            protocol_version: 0,
            height: at,
            consensus_time: at,
            origin: Origin::External(who.to_vec()),
        },
        Msg {
            target: "governance".into(),
            payload,
        },
    )
    .await
    .map(|_| ())
}

async fn proposal_status(host: &Host, id: &str) -> Option<ProposalStatus> {
    let reply = host
        .query(
            "governance",
            &gov_query(&GovQuery::Proposal {
                proposal_id: id.into(),
            }),
        )
        .await
        .expect("gov query");
    match gov_decode(&reply).expect("decode") {
        GovReply::Proposal(p) => p.map(|v| v.status),
        _ => None,
    }
}

async fn hello_code(host: &Host) -> modreg::ModuleCode {
    let reply = host
        .query("modreg", &modreg_query(&ModregQuery::Status))
        .await
        .expect("modreg status");
    match modreg_decode(&reply).expect("decode") {
        ModregReply::Status { modules } => modules
            .into_iter()
            .find(|m| m.module_id == "hello")
            .expect("hello entry"),
        other => panic!("expected Status, got {other:?}"),
    }
}

/// propose an action at `base`, both members vote yes, Execute — the early
/// strict-majority path settles Passed in the same round.
async fn pass(host: &mut Host, base: u64, id: &str, action: GovAction) {
    let (m1, m2) = (member_key(1), member_key(2));
    submit_as(
        host,
        &m1,
        base,
        gov_encode(&GovMsg::Propose {
            proposal_id: id.into(),
            action,
            voting_period: 100,
        }),
    )
    .await
    .expect("propose");
    for (who, at) in [(&m1, base + 1), (&m2, base + 2)] {
        submit_as(
            host,
            who,
            at,
            gov_encode(&GovMsg::Vote {
                proposal_id: id.into(),
                approve: true,
            }),
        )
        .await
        .expect("vote");
    }
    submit_as(
        host,
        &m2,
        base + 3,
        gov_encode(&GovMsg::Execute {
            proposal_id: id.into(),
        }),
    )
    .await
    .expect("execute");
}

/// the headline: a passing UpdateModule lands the pending swap in the REAL
/// registry — the whole chain from ballot to consensus code state.
#[test]
fn a_passing_update_module_lands_a_pending_swap_in_the_registry() {
    block_on(async {
        let mut host = gov_host_with_modreg().await;
        pass(
            &mut host,
            1,
            "mod-1",
            GovAction::UpdateModule {
                name: "hello-v2".into(),
                module_id: "hello".into(),
                activation_height: 500,
                code_hash: hash(2),
            },
        )
        .await;

        assert_eq!(
            proposal_status(&host, "mod-1").await,
            Some(ProposalStatus::Passed)
        );
        let code = hello_code(&host).await;
        assert_eq!(code.active_code_hash, hash(1), "active untouched until H");
        let pending = code.pending.expect("pending swap landed");
        assert_eq!(pending.name, "hello-v2");
        assert_eq!(pending.activation_height, 500);
        assert_eq!(pending.code_hash, hash(2));
    });
}

/// a passing CancelModuleUpdate clears the pending swap before its boundary.
#[test]
fn a_passing_cancel_module_update_clears_the_pending_swap() {
    block_on(async {
        let mut host = gov_host_with_modreg().await;
        pass(
            &mut host,
            1,
            "mod-1",
            GovAction::UpdateModule {
                name: "hello-v2".into(),
                module_id: "hello".into(),
                activation_height: 500,
                code_hash: hash(2),
            },
        )
        .await;
        assert!(hello_code(&host).await.pending.is_some());

        pass(
            &mut host,
            10,
            "mod-cancel",
            GovAction::CancelModuleUpdate {
                name: "hello-v2".into(),
                module_id: "hello".into(),
            },
        )
        .await;
        let code = hello_code(&host).await;
        assert!(code.pending.is_none(), "cancel cleared the pending swap");
        assert_eq!(code.active_code_hash, hash(1), "active untouched");
    });
}

/// a follow-up the registry REFUSES (unregistered module) fails the Execute op
/// atomically — the staged Passed transition rolls back, the proposal stays Open.
#[test]
fn a_refused_schedule_fails_execute_atomically() {
    block_on(async {
        let mut host = gov_host_with_modreg().await;
        let (m1, m2) = (member_key(1), member_key(2));
        submit_as(
            &mut host,
            &m1,
            1,
            gov_encode(&GovMsg::Propose {
                proposal_id: "mod-ghost".into(),
                action: GovAction::UpdateModule {
                    name: "v2".into(),
                    module_id: "ghost".into(), // never registered
                    activation_height: 500,
                    code_hash: hash(2),
                },
                voting_period: 100,
            }),
        )
        .await
        .expect("propose");
        for (who, at) in [(&m1, 2u64), (&m2, 3u64)] {
            submit_as(
                &mut host,
                who,
                at,
                gov_encode(&GovMsg::Vote {
                    proposal_id: "mod-ghost".into(),
                    approve: true,
                }),
            )
            .await
            .expect("vote");
        }
        let err = submit_as(
            &mut host,
            &m2,
            4,
            gov_encode(&GovMsg::Execute {
                proposal_id: "mod-ghost".into(),
            }),
        )
        .await
        .expect_err("modreg refuses a swap for an unregistered module");
        assert!(matches!(err, SubmitError::Rejected(_)), "got {err:?}");
        assert_eq!(
            proposal_status(&host, "mod-ghost").await,
            Some(ProposalStatus::Open),
            "no partial state: the proposal remains Open"
        );
    });
}

/// door checks: a bad hash length and an unwired registry are refused at
/// Propose — the proposal is never created.
#[test]
fn door_checks_refuse_bad_hash_and_unwired_registry() {
    block_on(async {
        // bad hash length on a wired host.
        let mut host = gov_host_with_modreg().await;
        let m1 = member_key(1);
        let err = submit_as(
            &mut host,
            &m1,
            1,
            gov_encode(&GovMsg::Propose {
                proposal_id: "mod-short".into(),
                action: GovAction::UpdateModule {
                    name: "v2".into(),
                    module_id: "hello".into(),
                    activation_height: 500,
                    code_hash: vec![1, 2, 3],
                },
                voting_period: 100,
            }),
        )
        .await
        .expect_err("a 3-byte code_hash must be refused at the door");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("code_hash")),
            "got {err:?}"
        );
        assert_eq!(proposal_status(&host, "mod-short").await, None);

        // an UNWIRED registry (Governance::new without with_modreg).
        let mut valset = Valset::new("valset");
        valset.insert(member_key(1));
        let mut unwired = Host::genesis(vec![
            Box::new(valset),
            Box::new(Governance::new("governance", "valset", "upgrade", "identity")),
        ])
        .expect("genesis");
        let err = submit_as(
            &mut unwired,
            &m1,
            1,
            gov_encode(&GovMsg::Propose {
                proposal_id: "mod-unwired".into(),
                action: GovAction::UpdateModule {
                    name: "v2".into(),
                    module_id: "hello".into(),
                    activation_height: 500,
                    code_hash: hash(2),
                },
                voting_period: 100,
            }),
        )
        .await
        .expect_err("no code registry wired: refused at the door");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("registry")),
            "got {err:?}"
        );
    });
}

/// tags 7 (UpdateModule) and 8 (CancelModuleUpdate) round-trip through
/// encode/decode_state — snapshot bytes reconstruct the exact action fields.
#[test]
fn snapshot_install_round_trips_module_update_proposals() {
    block_on(async {
        let mut host = gov_host_with_modreg().await;
        let m1 = member_key(1);
        for (id, action) in [
            (
                "mod-snap-update",
                GovAction::UpdateModule {
                    name: "hello-v2".into(),
                    module_id: "hello".into(),
                    activation_height: 900,
                    code_hash: hash(3),
                },
            ),
            (
                "mod-snap-cancel",
                GovAction::CancelModuleUpdate {
                    name: "hello-v2".into(),
                    module_id: "hello".into(),
                },
            ),
        ] {
            submit_as(
                &mut host,
                &m1,
                1,
                gov_encode(&GovMsg::Propose {
                    proposal_id: id.into(),
                    action,
                    voting_period: 100,
                }),
            )
            .await
            .expect("propose");
        }

        let root = host.module_root("governance").expect("gov root");
        let sdk::StateSyncHandle::SnapshotBytes(bytes) = ({
            let finalized = host::FinalizedBlock {
                height: 1,
                app_hash: host.app_hash(),
            };
            host.capture_finalized_snapshot(finalized)
                .expect("capture")
                .module("governance")
                .expect("gov entry")
                .state_sync
                .clone()
        }) else {
            panic!("governance must advertise snapshot bytes");
        };

        let mut rebuilt =
            Governance::new("governance", "valset", "upgrade", "identity").with_modreg("modreg");
        rebuilt.install(&bytes, root).expect("install");
        assert_eq!(rebuilt.root(), root, "installed root equals the source root");

        for (id, want) in [
            (
                "mod-snap-update",
                GovAction::UpdateModule {
                    name: "hello-v2".into(),
                    module_id: "hello".into(),
                    activation_height: 900,
                    code_hash: hash(3),
                },
            ),
            (
                "mod-snap-cancel",
                GovAction::CancelModuleUpdate {
                    name: "hello-v2".into(),
                    module_id: "hello".into(),
                },
            ),
        ] {
            let reply = rebuilt
                .query(&gov_query(&GovQuery::Proposal {
                    proposal_id: id.into(),
                }))
                .await
                .expect("query");
            let GovReply::Proposal(Some(view)) = gov_decode(&reply).expect("decode") else {
                panic!("proposal {id} must be present after install");
            };
            assert_eq!(view.action, want);
        }
    });
}

/// the module lookup, admission flavor: any id, absent allowed.
async fn module_code(host: &Host, id: &str) -> Option<modreg::ModuleCode> {
    let reply = host
        .query("modreg", &modreg_query(&ModregQuery::Status))
        .await
        .expect("modreg status");
    match modreg_decode(&reply).expect("decode") {
        ModregReply::Status { modules } => modules.into_iter().find(|m| m.module_id == id),
        other => panic!("expected Status, got {other:?}"),
    }
}

/// a passing RegisterModule ADMITS a brand-new module: the entry lands with an
/// EMPTY active hash and the pending initial code — registered, not yet
/// running, gated by the same readiness/height machinery as a swap.
#[test]
fn a_passing_register_module_admits_a_new_pending_entry() {
    block_on(async {
        let mut host = gov_host_with_modreg().await;
        assert!(module_code(&host, "kanban").await.is_none());
        pass(
            &mut host,
            1,
            "adm-1",
            GovAction::RegisterModule {
                name: "kanban-v1".into(),
                module_id: "kanban".into(),
                activation_height: 500,
                code_hash: hash(7),
            },
        )
        .await;

        assert_eq!(
            proposal_status(&host, "adm-1").await,
            Some(ProposalStatus::Passed)
        );
        let code = module_code(&host, "kanban").await.expect("entry admitted");
        assert!(code.active_code_hash.is_empty(), "no active code until H");
        let pending = code.pending.expect("pending initial code landed");
        assert_eq!(pending.name, "kanban-v1");
        assert_eq!(pending.activation_height, 500);
        assert_eq!(pending.code_hash, hash(7));
    });
}

/// a passing CancelModuleUpdate on an ADMISSION removes the entry entirely —
/// modreg never claims a module that has no code.
#[test]
fn a_passing_cancel_removes_an_admission_entry_entirely() {
    block_on(async {
        let mut host = gov_host_with_modreg().await;
        pass(
            &mut host,
            1,
            "adm-1",
            GovAction::RegisterModule {
                name: "kanban-v1".into(),
                module_id: "kanban".into(),
                activation_height: 500,
                code_hash: hash(7),
            },
        )
        .await;
        assert!(module_code(&host, "kanban").await.is_some());

        pass(
            &mut host,
            10,
            "adm-cancel",
            GovAction::CancelModuleUpdate {
                name: "kanban-v1".into(),
                module_id: "kanban".into(),
            },
        )
        .await;
        assert!(
            module_code(&host, "kanban").await.is_none(),
            "cancelled admission leaves no registry entry"
        );
    });
}

/// a RegisterModule of an id that already exists is refused by the registry,
/// and the refusal fails the Execute op ATOMICALLY — the staged Passed
/// transition rolls back, the proposal stays Open, hello is untouched (the
/// exact contract `a_refused_schedule_fails_execute_atomically` pins for
/// swaps).
#[test]
fn register_module_of_an_existing_id_fails_execute_atomically() {
    block_on(async {
        let mut host = gov_host_with_modreg().await;
        let (m1, m2) = (member_key(1), member_key(2));
        submit_as(
            &mut host,
            &m1,
            1,
            gov_encode(&GovMsg::Propose {
                proposal_id: "adm-dup".into(),
                action: GovAction::RegisterModule {
                    name: "hello-again".into(),
                    module_id: "hello".into(),
                    activation_height: 500,
                    code_hash: hash(9),
                },
                voting_period: 100,
            }),
        )
        .await
        .expect("propose");
        for (who, at) in [(&m1, 2u64), (&m2, 3u64)] {
            submit_as(
                &mut host,
                who,
                at,
                gov_encode(&GovMsg::Vote {
                    proposal_id: "adm-dup".into(),
                    approve: true,
                }),
            )
            .await
            .expect("vote");
        }
        let refused = submit_as(
            &mut host,
            &m2,
            4,
            gov_encode(&GovMsg::Execute {
                proposal_id: "adm-dup".into(),
            }),
        )
        .await;
        assert!(refused.is_err(), "registry refusal fails the Execute op");
        assert_eq!(
            proposal_status(&host, "adm-dup").await,
            Some(ProposalStatus::Open),
            "staged Passed transition rolled back"
        );
        let code = module_code(&host, "hello").await.expect("hello persists");
        assert_eq!(code.active_code_hash, hash(1), "active code untouched");
        assert!(code.pending.is_none(), "no pending landed");
    });
}
