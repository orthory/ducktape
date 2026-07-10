//! governance authorizes a node upgrade the SAME way it authorizes membership:
//! a member-gated proposal + simple-majority tally, and on passing,
//! `handle_execute` emits the upgrade-module op as a host-drained follow-up.
//!
//! these tests drive real ops through `Host::submit_at` with `Origin::External`
//! (the shape the ordered lane hands the host after VERIFYING a frame
//! signature), and pin the follow-up against an in-test stub `upgrade` module
//! that records exactly what it received and from which origin — so what is
//! pinned is the authorization model the live network runs: governance is the
//! sole author, and the host stamps the follow-up `Origin::Module("governance")`.

use std::cell::RefCell;
use std::rc::Rc;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use futures::executor::block_on;
use governance::Governance;
use governance::{
    GovAction, GovMsg, GovQuery, GovReply, ProposalStatus, decode_reply as gov_decode,
    encode_msg as gov_encode, encode_query as gov_query,
};
use host::{BlockContext, Host, SubmitError};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use upgrade::{UpgradeMsg, decode_msg as upgrade_decode};
use valset::Valset;

fn member_key(seed: u8) -> Vec<u8> {
    let seed = [seed; 32];
    PrivateKey::decode(&seed[..])
        .expect("any 32 bytes is a valid seed")
        .public_key()
        .as_ref()
        .to_vec()
}

/// what an in-test `upgrade` stub observed: the decoded op and the origin the
/// host stamped it with. shared with the test via `Rc<RefCell<..>>` (everything
/// here is single-threaded `block_on`).
type Spy = Rc<RefCell<Vec<(UpgradeMsg, Origin)>>>;

/// a minimal stand-in for the real upgrade module (not registered until a later
/// phase): it records every op it is dispatched, and can be told to REJECT a
/// `Schedule` to exercise follow-up atomicity.
struct UpgradeStub {
    id: ModuleId,
    seen: Spy,
    reject_schedule: bool,
}

#[async_trait::async_trait(?Send)]
impl Module for UpgradeStub {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::Stateless)
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let op = upgrade_decode(&msg.payload).map_err(Error::Module)?;
        self.seen
            .borrow_mut()
            .push((op.clone(), ctx.env().origin.clone()));
        if self.reject_schedule && matches!(op, UpgradeMsg::Schedule { .. }) {
            return Err(Error::Module("stub rejects schedule".into()));
        }
        Ok(())
    }
}

/// a host with valset (members 1,2) + governance + an upgrade stub whose spy the
/// caller keeps a handle to.
fn gov_host_with_upgrade(reject_schedule: bool) -> (Host, Spy) {
    let seen: Spy = Rc::new(RefCell::new(Vec::new()));
    let mut valset = Valset::new("valset");
    valset.insert(member_key(1));
    valset.insert(member_key(2));
    let host = Host::genesis(vec![
        Box::new(valset),
        Box::new(Governance::new("governance", "valset", "upgrade")),
        Box::new(UpgradeStub {
            id: "upgrade".into(),
            seen: seen.clone(),
            reject_schedule,
        }),
    ])
    .expect("genesis");
    (host, seen)
}

async fn submit_as(
    host: &mut Host,
    who: &[u8],
    at: u64,
    target: &str,
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
            target: target.into(),
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

/// propose an action, have both members vote yes, then Execute — the early
/// strict-majority path settles Passed in the same round.
async fn pass(host: &mut Host, id: &str, action: GovAction) {
    let (m1, m2) = (member_key(1), member_key(2));
    submit_as(
        host,
        &m1,
        1,
        "governance",
        gov_encode(&GovMsg::Propose {
            proposal_id: id.into(),
            action,
            voting_period: 100,
        }),
    )
    .await
    .expect("propose");
    submit_as(
        host,
        &m1,
        2,
        "governance",
        gov_encode(&GovMsg::Vote {
            proposal_id: id.into(),
            approve: true,
        }),
    )
    .await
    .expect("vote m1");
    submit_as(
        host,
        &m2,
        3,
        "governance",
        gov_encode(&GovMsg::Vote {
            proposal_id: id.into(),
            approve: true,
        }),
    )
    .await
    .expect("vote m2");
    submit_as(
        host,
        &m2,
        4,
        "governance",
        gov_encode(&GovMsg::Execute {
            proposal_id: id.into(),
        }),
    )
    .await
    .expect("execute");
}

#[test]
fn a_passing_schedule_upgrade_emits_the_upgrade_followup() {
    block_on(async {
        let (mut host, seen) = gov_host_with_upgrade(false);
        pass(
            &mut host,
            "up-1",
            GovAction::ScheduleUpgrade {
                name: "forge-multi-repo".into(),
                activation_height: 500,
                to_version: 2,
            },
        )
        .await;

        assert_eq!(
            proposal_status(&host, "up-1").await,
            Some(ProposalStatus::Passed)
        );
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1, "exactly one follow-up drained");
        assert_eq!(
            seen[0].0,
            UpgradeMsg::Schedule {
                name: "forge-multi-repo".into(),
                activation_height: 500,
                to_version: 2,
            }
        );
        assert_eq!(
            seen[0].1,
            Origin::Module("governance".into()),
            "the host stamps the follow-up with the governance module origin"
        );
    });
}

#[test]
fn cancel_upgrade_emits_cancel_followup() {
    block_on(async {
        let (mut host, seen) = gov_host_with_upgrade(false);
        pass(
            &mut host,
            "cancel-1",
            GovAction::CancelUpgrade {
                name: "forge-multi-repo".into(),
            },
        )
        .await;

        assert_eq!(
            proposal_status(&host, "cancel-1").await,
            Some(ProposalStatus::Passed)
        );
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(
            seen[0].0,
            UpgradeMsg::Cancel {
                name: "forge-multi-repo".into(),
            }
        );
        assert_eq!(seen[0].1, Origin::Module("governance".into()));
    });
}

#[test]
fn outsider_cannot_propose_schedule_upgrade() {
    block_on(async {
        let (mut host, seen) = gov_host_with_upgrade(false);
        let outsider = member_key(7);
        let err = submit_as(
            &mut host,
            &outsider,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "up-x".into(),
                action: GovAction::ScheduleUpgrade {
                    name: "n".into(),
                    activation_height: 500,
                    to_version: 2,
                },
                voting_period: 100,
            }),
        )
        .await
        .expect_err("outsider propose must be refused");
        assert!(matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("member")));
        assert!(
            seen.borrow().is_empty(),
            "no follow-up on a refused propose"
        );
    });
}

#[test]
fn empty_upgrade_name_is_rejected_at_propose() {
    block_on(async {
        let (mut host, _seen) = gov_host_with_upgrade(false);
        let m1 = member_key(1);
        let err = submit_as(
            &mut host,
            &m1,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "up-empty".into(),
                action: GovAction::ScheduleUpgrade {
                    name: String::new(),
                    activation_height: 500,
                    to_version: 2,
                },
                voting_period: 100,
            }),
        )
        .await
        .expect_err("empty name must be refused at the door");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("name")),
            "got {err:?}"
        );
        // never created — the door-check fires before any pending write.
        assert_eq!(proposal_status(&host, "up-empty").await, None);
    });
}

#[test]
fn a_rejected_followup_fails_execute_atomically() {
    block_on(async {
        let (mut host, _seen) = gov_host_with_upgrade(true);
        let (m1, m2) = (member_key(1), member_key(2));
        submit_as(
            &mut host,
            &m1,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "up-bad".into(),
                action: GovAction::ScheduleUpgrade {
                    name: "n".into(),
                    activation_height: 500,
                    to_version: 2,
                },
                voting_period: 100,
            }),
        )
        .await
        .expect("propose");
        submit_as(
            &mut host,
            &m1,
            2,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "up-bad".into(),
                approve: true,
            }),
        )
        .await
        .expect("vote m1");
        submit_as(
            &mut host,
            &m2,
            3,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "up-bad".into(),
                approve: true,
            }),
        )
        .await
        .expect("vote m2");

        // the stub rejects the Schedule follow-up -> the whole Execute op fails,
        // the staged Passed transition is discarded, and the proposal stays Open.
        let err = submit_as(
            &mut host,
            &m2,
            4,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "up-bad".into(),
            }),
        )
        .await
        .expect_err("a failing follow-up must fail the Execute op");
        assert!(matches!(err, SubmitError::Rejected(_)), "got {err:?}");
        assert_eq!(
            proposal_status(&host, "up-bad").await,
            Some(ProposalStatus::Open),
            "no partial state: the proposal remains Open, ready to retry",
        );
    });
}

#[test]
fn snapshot_install_round_trips_a_schedule_upgrade_proposal() {
    block_on(async {
        let (mut host, _seen) = gov_host_with_upgrade(false);
        let m1 = member_key(1);
        // an OPEN proposal carrying ScheduleUpgrade (tag 3) plus one vote.
        submit_as(
            &mut host,
            &m1,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "up-snap".into(),
                action: GovAction::ScheduleUpgrade {
                    name: "forge-multi-repo".into(),
                    activation_height: 900,
                    to_version: 3,
                },
                voting_period: 100,
            }),
        )
        .await
        .expect("propose");
        submit_as(
            &mut host,
            &m1,
            2,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "up-snap".into(),
                approve: true,
            }),
        )
        .await
        .expect("vote");

        let root = host.module_root("governance").expect("gov root");
        let sdk::StateSyncHandle::SnapshotBytes(bytes) = ({
            let finalized = host::FinalizedBlock {
                height: 2,
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

        // a fresh instance rebuilt from the snapshot recomputes the same root,
        // so tag 3 (ScheduleUpgrade) round-trips through encode/decode_state.
        let mut rebuilt = Governance::new("governance", "valset", "upgrade");
        rebuilt.install(&bytes, root).expect("install");
        assert_eq!(
            rebuilt.root(),
            root,
            "installed root equals the source root"
        );
        assert_ne!(root, StateRoot::ZERO, "the proposal is actually present");

        // and the round-tripped view still carries the exact action fields.
        let reply = rebuilt
            .query(&gov_query(&GovQuery::Proposal {
                proposal_id: "up-snap".into(),
            }))
            .await
            .expect("query");
        let GovReply::Proposal(Some(view)) = gov_decode(&reply).expect("decode") else {
            panic!("proposal must be present after install");
        };
        assert_eq!(
            view.action,
            GovAction::ScheduleUpgrade {
                name: "forge-multi-repo".into(),
                activation_height: 900,
                to_version: 3,
            }
        );
    });
}

#[test]
fn a_rejected_schedule_upgrade_emits_no_followup() {
    block_on(async {
        let (mut host, seen) = gov_host_with_upgrade(false);
        let (m1, m2) = (member_key(1), member_key(2));
        // propose with a short window; the only ballot is a NO, so at the deadline
        // yes=0 < the 2-of-2 majority and the tally settles Rejected.
        submit_as(
            &mut host,
            &m1,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "up-rej".into(),
                action: GovAction::ScheduleUpgrade {
                    name: "n".into(),
                    activation_height: 500,
                    to_version: 2,
                },
                voting_period: 5,
            }),
        )
        .await
        .expect("propose");
        submit_as(
            &mut host,
            &m1,
            2,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "up-rej".into(),
                approve: false,
            }),
        )
        .await
        .expect("vote no");
        // Execute AFTER the deadline (created_at 1 + period 5 = 6): decidable, and
        // yes=0 < majority=2 -> Rejected, the else branch of handle_execute.
        submit_as(
            &mut host,
            &m2,
            7,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "up-rej".into(),
            }),
        )
        .await
        .expect("execute settles the rejection");

        assert_eq!(
            proposal_status(&host, "up-rej").await,
            Some(ProposalStatus::Rejected)
        );
        assert!(
            seen.borrow().is_empty(),
            "a REJECTED upgrade proposal emits NO follow-up (emit is on the passing branch only)"
        );
    });
}

#[test]
fn snapshot_install_round_trips_a_cancel_upgrade_proposal() {
    block_on(async {
        let (mut host, _seen) = gov_host_with_upgrade(false);
        let m1 = member_key(1);
        // an OPEN proposal carrying CancelUpgrade (tag 4) — the decode arm the
        // ScheduleUpgrade round-trip does not exercise.
        submit_as(
            &mut host,
            &m1,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "up-cancel".into(),
                action: GovAction::CancelUpgrade {
                    name: "forge-multi-repo".into(),
                },
                voting_period: 100,
            }),
        )
        .await
        .expect("propose");

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

        // tag 4 (CancelUpgrade) round-trips through encode/decode_state.
        let mut rebuilt = Governance::new("governance", "valset", "upgrade");
        rebuilt.install(&bytes, root).expect("install");
        assert_eq!(
            rebuilt.root(),
            root,
            "installed root equals the source root"
        );
        assert_ne!(root, StateRoot::ZERO, "the proposal is actually present");

        let reply = rebuilt
            .query(&gov_query(&GovQuery::Proposal {
                proposal_id: "up-cancel".into(),
            }))
            .await
            .expect("query");
        let GovReply::Proposal(Some(view)) = gov_decode(&reply).expect("decode") else {
            panic!("proposal must be present after install");
        };
        assert_eq!(
            view.action,
            GovAction::CancelUpgrade {
                name: "forge-multi-repo".into(),
            }
        );
    });
}
