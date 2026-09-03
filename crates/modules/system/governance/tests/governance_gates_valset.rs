//! the governance loop end-to-end through a REAL host: a member proposes
//! admitting a validator, members vote, execution tallies deterministically
//! and performs the membership change by emitting the valset op as a
//! host-drained follow-up — while direct external valset writes are refused.
//!
//! ops are driven through `Host::submit_at` with `Origin::External(member
//! key)`, exactly the shape the ordered lane hands the host after VERIFYING a
//! frame signature — so what these tests pin is the authorization model the
//! live network runs.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use futures::executor::block_on;
use governance::Governance;
use governance::{
    GovAction, GovMsg, GovQuery, GovReply, ProposalStatus, decode_reply as gov_decode,
    encode_msg as gov_encode, encode_query as gov_query,
};
use host::{BlockContext, Host, SubmitError};
use sdk::{Error, Msg, Origin};
use sdk_testkit::MemStore;
use valset::Valset;
use valset::{
    ValsetMsg, ValsetQuery, ValsetReply, decode_reply as valset_decode,
    encode_msg as valset_encode, encode_query as valset_query,
};

fn member_key(seed: u8) -> Vec<u8> {
    let seed = [seed; 32];
    PrivateKey::decode(&seed[..])
        .expect("any 32 bytes is a valid seed")
        .public_key()
        .as_ref()
        .to_vec()
}

/// a host with governance gating a valset seeded with members 1 and 2.
async fn gov_host() -> Host {
    let mut valset = Valset::new("valset", Box::new(MemStore::new()));
    valset.seed(member_key(1)).await.expect("seed valset");
    valset.seed(member_key(2)).await.expect("seed valset");
    valset.finish_seed().await.expect("seed valset");
    Host::genesis(vec![
        Box::new(valset),
        Box::new(Governance::new(
            "governance",
            Box::new(MemStore::new()),
            "valset",
            "identity",
        )),
    ])
    .expect("genesis")
}

/// submit one op as an EXTERNAL (verified-origin) submitter at a consensus time.
async fn submit_as(
    host: &mut Host,
    who: &[u8],
    at: u64,
    target: &str,
    payload: Vec<u8>,
) -> Result<(), SubmitError> {
    host.submit_at(
        BlockContext {
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

async fn validators(host: &Host) -> Vec<Vec<u8>> {
    let reply = host
        .query("valset", &valset_query(&ValsetQuery::Validators))
        .await
        .expect("valset query");
    match valset_decode(&reply).expect("decode") {
        ValsetReply::Validators(v) => v,
        other => panic!("expected Validators, got {other:?}"),
    }
}

async fn residents(host: &Host) -> Vec<Vec<u8>> {
    let reply = host
        .query("valset", &valset_query(&ValsetQuery::Residents))
        .await
        .expect("valset query");
    match valset_decode(&reply).expect("decode") {
        ValsetReply::Residents(v) => v,
        other => panic!("expected Residents, got {other:?}"),
    }
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

#[test]
fn a_passing_proposal_admits_the_validator_and_direct_writes_are_refused() {
    block_on(async {
        let mut host = gov_host().await;
        let (m1, m2, newcomer) = (member_key(1), member_key(2), member_key(9));

        // DIRECT external valset writes are refused — the old one-message
        // liveness kill is closed.
        let err = submit_as(
            &mut host,
            &m1,
            1,
            "valset",
            valset_encode(&ValsetMsg::Leave { key: m2.clone() }),
        )
        .await
        .expect_err("external valset write must be refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("governance")),
            "got {err:?}"
        );
        assert_eq!(validators(&host).await.len(), 2, "membership untouched");

        // propose + both members vote yes -> early-decidable majority.
        submit_as(
            &mut host,
            &m1,
            2,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "add-9".into(),
                action: GovAction::AddValidator {
                    key: newcomer.clone(),
                },
                voting_period: 100,
            }),
        )
        .await
        .expect("propose");
        submit_as(
            &mut host,
            &m1,
            3,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "add-9".into(),
                approve: true,
            }),
        )
        .await
        .expect("vote m1");
        submit_as(
            &mut host,
            &m2,
            4,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "add-9".into(),
                approve: true,
            }),
        )
        .await
        .expect("vote m2");

        // early execution: yes-ballots already form a strict majority. the
        // valset op rides the SAME block as a governance-origin follow-up.
        submit_as(
            &mut host,
            &m2,
            5,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "add-9".into(),
            }),
        )
        .await
        .expect("execute");

        assert_eq!(
            proposal_status(&host, "add-9").await,
            Some(ProposalStatus::Passed)
        );
        let members = validators(&host).await;
        assert_eq!(members.len(), 3, "the admitted validator is a member");
        assert!(members.contains(&newcomer));
    });
}

#[test]
fn an_add_resident_proposal_grants_resident_standing() {
    // end to end: a block proposes AddResident, a majority votes, and
    // execution emits the valset Grant follow-up — resident standing lands.
    block_on(async {
        let mut host = gov_host().await;
        let (m1, m2, friend) = (member_key(1), member_key(2), member_key(9));

        submit_as(
            &mut host,
            &m1,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "observe-9".into(),
                action: GovAction::AddResident {
                    key: friend.clone(),
                },
                voting_period: 100,
            }),
        )
        .await
        .expect("propose AddResident at v0");
        for (who, at) in [(&m1, 2u64), (&m2, 3u64)] {
            submit_as(
                &mut host,
                who,
                at,
                "governance",
                gov_encode(&GovMsg::Vote {
                    proposal_id: "observe-9".into(),
                    approve: true,
                }),
            )
            .await
            .expect("vote");
        }
        submit_as(
            &mut host,
            &m2,
            4,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "observe-9".into(),
            }),
        )
        .await
        .expect("execute");

        assert_eq!(
            proposal_status(&host, "observe-9").await,
            Some(ProposalStatus::Passed)
        );
        assert_eq!(
            residents(&host).await,
            vec![friend],
            "resident standing granted from genesis"
        );
        assert_eq!(
            validators(&host).await.len(),
            2,
            "the validator set is untouched by a resident grant"
        );
    });
}

#[test]
fn a_passing_proposal_removes_the_validator_and_emits_leave() {
    block_on(async {
        let mut host = gov_host().await;
        let (m1, m2) = (member_key(1), member_key(2));
        assert_eq!(validators(&host).await.len(), 2, "seeded with two members");

        // the `member-remove` verb's on-chain shape: a member opens a
        // RemoveValidator proposal, members vote, and execution PERFORMS the
        // membership change by emitting `ValsetMsg::Leave` as a
        // governance-origin follow-up — the same path `node member promote` drives
        // for AddValidator/Join, inverted.
        submit_as(
            &mut host,
            &m1,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "remove-2".into(),
                action: GovAction::RemoveValidator { key: m2.clone() },
                voting_period: 100,
            }),
        )
        .await
        .expect("propose removal");
        // both current members vote yes -> early-decidable strict majority
        // (2 of 2). a member may cast the deciding ballot to remove another.
        submit_as(
            &mut host,
            &m1,
            2,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "remove-2".into(),
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
                proposal_id: "remove-2".into(),
                approve: true,
            }),
        )
        .await
        .expect("vote m2");

        // execution tallies and rides the valset Leave on the SAME block.
        submit_as(
            &mut host,
            &m2,
            4,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "remove-2".into(),
            }),
        )
        .await
        .expect("execute");

        assert_eq!(
            proposal_status(&host, "remove-2").await,
            Some(ProposalStatus::Passed)
        );
        let members = validators(&host).await;
        assert_eq!(members.len(), 1, "the removed validator is gone");
        assert!(members.contains(&m1), "the remaining member stays");
        assert!(
            !members.contains(&m2),
            "the removed key is dropped from the set"
        );
    });
}

/// the `member-leave` verb's on-chain shape: a member drives its OWN removal by
/// opening a RemoveValidator proposal for its own key and casting its yes-ballot
/// — the SAME governance path as `member-remove`, targeting self. this pins the
/// honesty of a unilateral leave: at n=2 the leaver's single ballot is NOT a
/// strict majority (majority = 2), so its own execute is not decidable early —
/// the removal stays PENDING until the remaining member also approves, and only
/// then does the leaver drop from the set.
#[test]
fn a_member_leaves_by_removing_itself_pending_the_remaining_majority() {
    block_on(async {
        let mut host = gov_host().await;
        let (m1, m2) = (member_key(1), member_key(2));
        assert_eq!(validators(&host).await.len(), 2, "seeded with two members");

        // m2 (the leaver) opens a self-removal and casts its own yes-ballot.
        submit_as(
            &mut host,
            &m2,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "leave-2".into(),
                action: GovAction::RemoveValidator { key: m2.clone() },
                voting_period: 100,
            }),
        )
        .await
        .expect("leaver proposes its own removal");
        submit_as(
            &mut host,
            &m2,
            2,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "leave-2".into(),
                approve: true,
            }),
        )
        .await
        .expect("leaver votes to leave");

        // the leaver's lone ballot is 1 of 2 — NOT a majority. its own execute
        // is refused as not-yet-decidable: leaving is not unilateral at n>=2.
        let err = submit_as(
            &mut host,
            &m2,
            3,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "leave-2".into(),
            }),
        )
        .await
        .expect_err("a lone leave ballot is not a deciding majority");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("not decidable")),
            "got {err:?}"
        );
        assert_eq!(
            validators(&host).await.len(),
            2,
            "the leaver is still in the set while pending"
        );

        // the remaining member approves -> strict majority (2 of 2) -> execute
        // removes the leaver at the valset Leave that rides the same block.
        submit_as(
            &mut host,
            &m1,
            4,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "leave-2".into(),
                approve: true,
            }),
        )
        .await
        .expect("remaining member approves the departure");
        submit_as(
            &mut host,
            &m1,
            5,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "leave-2".into(),
            }),
        )
        .await
        .expect("execute once the majority approves");

        assert_eq!(
            proposal_status(&host, "leave-2").await,
            Some(ProposalStatus::Passed)
        );
        let members = validators(&host).await;
        assert_eq!(members.len(), 1, "the leaver is gone");
        assert!(members.contains(&m1), "the remaining member stays");
        assert!(
            !members.contains(&m2),
            "the leaver's key is dropped from the set"
        );
    });
}

/// the solo-network tally: with one member, majority = 1/2 + 1 = 1, so the
/// founder's own ballot decides immediately — `node member promote` on a network
/// of one admits the friend in a single propose/vote/execute round, no
/// second party required.
#[test]
fn a_single_member_ballot_is_a_deciding_majority() {
    block_on(async {
        let founder = member_key(1);
        let friend = member_key(9);
        let mut valset = Valset::new("valset", Box::new(MemStore::new()));
        valset.seed(founder.clone()).await.expect("seed valset");
        valset.finish_seed().await.expect("seed valset");
        let mut host = Host::genesis(vec![
            Box::new(valset),
            Box::new(Governance::new(
                "governance",
                Box::new(MemStore::new()),
                "valset",
                "identity",
            )),
        ])
        .expect("genesis");

        submit_as(
            &mut host,
            &founder,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "add-friend".into(),
                action: GovAction::AddValidator {
                    key: friend.clone(),
                },
                voting_period: 100,
            }),
        )
        .await
        .expect("propose");

        // proposing is NOT voting: executing with zero ballots is not
        // decidable before the deadline, even at n=1.
        let err = submit_as(
            &mut host,
            &founder,
            2,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "add-friend".into(),
            }),
        )
        .await
        .expect_err("no ballots -> not decidable early");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("not decidable"))
        );

        submit_as(
            &mut host,
            &founder,
            3,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "add-friend".into(),
                approve: true,
            }),
        )
        .await
        .expect("vote");
        submit_as(
            &mut host,
            &founder,
            4,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "add-friend".into(),
            }),
        )
        .await
        .expect("the single ballot decides");

        assert_eq!(
            proposal_status(&host, "add-friend").await,
            Some(ProposalStatus::Passed)
        );
        let members = validators(&host).await;
        assert_eq!(members.len(), 2, "the friend is admitted");
        assert!(members.contains(&friend));
    });
}

/// the last-validator guard at the governance layer: a solo member's ballot IS
/// a deciding majority (1 of 1), but enacting its own removal would empty the
/// validator set — a zero-validator orderer hits commonware `quorum(0)`, which
/// panics. governance refuses: it does NOT emit the set-emptying valset Leave,
/// marks the proposal Rejected instead, and the sole validator stays. this is
/// the "solo can't leave on-chain" case — a solo node forgets its workspace
/// locally rather than removing the last validator.
#[test]
fn removing_the_last_validator_is_refused_and_the_set_stays_non_empty() {
    block_on(async {
        let founder = member_key(1);
        let mut valset = Valset::new("valset", Box::new(MemStore::new()));
        valset.seed(founder.clone()).await.expect("seed valset");
        valset.finish_seed().await.expect("seed valset");
        let mut host = Host::genesis(vec![
            Box::new(valset),
            Box::new(Governance::new(
                "governance",
                Box::new(MemStore::new()),
                "valset",
                "identity",
            )),
        ])
        .expect("genesis");
        assert_eq!(validators(&host).await.len(), 1, "a solo network of one");

        // the sole member opens a self-removal and casts its own yes-ballot —
        // majority = 1/2 + 1 = 1, so this is early-decidable.
        submit_as(
            &mut host,
            &founder,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "leave-solo".into(),
                action: GovAction::RemoveValidator {
                    key: founder.clone(),
                },
                voting_period: 100,
            }),
        )
        .await
        .expect("solo proposes its own removal");
        submit_as(
            &mut host,
            &founder,
            2,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "leave-solo".into(),
                approve: true,
            }),
        )
        .await
        .expect("solo votes to leave");

        // executing is a CLEAN op (the block commits): governance pre-checks that
        // the removal would empty the set and REJECTS the proposal rather than
        // emitting the set-emptying valset Leave. no block-abort, no panic.
        submit_as(
            &mut host,
            &founder,
            3,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "leave-solo".into(),
            }),
        )
        .await
        .expect("execute settles the proposal cleanly");

        assert_eq!(
            proposal_status(&host, "leave-solo").await,
            Some(ProposalStatus::Rejected),
            "a last-validator removal is refused, not passed"
        );
        let members = validators(&host).await;
        assert_eq!(members.len(), 1, "the set never went empty");
        assert!(members.contains(&founder), "the sole validator stays");
    });
}

/// belt-and-suspenders: even if a set-emptying valset `Leave` reached the valset
/// module directly (a module-origin write, bypassing governance's pre-check),
/// the valset handler itself refuses it. this pins the AUTHORITATIVE guard —
/// the invariant lives in the module that owns the set, not only in its caller.
#[test]
fn a_direct_module_origin_leave_of_the_last_validator_is_refused() {
    block_on(async {
        let founder = member_key(1);
        let mut valset = Valset::new("valset", Box::new(MemStore::new()));
        valset.seed(founder.clone()).await.expect("seed valset");
        valset.finish_seed().await.expect("seed valset");
        let mut host = Host::genesis(vec![Box::new(valset)]).expect("genesis");

        // a System-origin op (genesis orchestration shape) that would empty the
        // set is refused deterministically — the block is rejected, set intact.
        let err = host
            .submit_at(
                BlockContext {
                    height: 1,
                    consensus_time: 1,
                    origin: Origin::System,
                },
                Msg {
                    target: "valset".into(),
                    payload: valset_encode(&ValsetMsg::Leave {
                        key: founder.clone(),
                    }),
                },
            )
            .await
            .expect_err("emptying the set must be refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("last validator")),
            "got {err:?}"
        );
        assert_eq!(
            validators(&host).await,
            vec![founder],
            "the set is untouched"
        );
    });
}

#[test]
fn non_members_cannot_propose_or_vote_and_minority_rejects() {
    block_on(async {
        let mut host = gov_host().await;
        let (m1, m2, outsider) = (member_key(1), member_key(2), member_key(7));

        // an outsider cannot propose...
        let err = submit_as(
            &mut host,
            &outsider,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "p".into(),
                action: GovAction::Signal { text: "hi".into() },
                voting_period: 10,
            }),
        )
        .await
        .expect_err("outsider propose must be refused");
        assert!(matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("member")));

        // ...a member proposes, an outsider cannot vote...
        submit_as(
            &mut host,
            &m1,
            2,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "p".into(),
                action: GovAction::Signal { text: "hi".into() },
                voting_period: 10,
            }),
        )
        .await
        .expect("member propose");
        let err = submit_as(
            &mut host,
            &outsider,
            3,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "p".into(),
                approve: true,
            }),
        )
        .await
        .expect_err("outsider vote must be refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("frozen electorate"))
        );

        // ...one yes of two members is NOT a strict majority: not decidable
        // early, and after the deadline it rejects.
        submit_as(
            &mut host,
            &m1,
            4,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "p".into(),
                approve: true,
            }),
        )
        .await
        .expect("m1 votes yes");
        let err = submit_as(
            &mut host,
            &m2,
            5,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "p".into(),
            }),
        )
        .await
        .expect_err("not decidable before the deadline without a majority");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("not decidable"))
        );

        // past the deadline (proposed at 2, period 10 -> deadline 12).
        submit_as(
            &mut host,
            &m2,
            13,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "p".into(),
            }),
        )
        .await
        .expect("execute after deadline");
        assert_eq!(
            proposal_status(&host, "p").await,
            Some(ProposalStatus::Rejected)
        );
    });
}

#[test]
fn votes_close_at_the_deadline_and_ballots_are_per_member() {
    block_on(async {
        let mut host = gov_host().await;
        let (m1, m2) = (member_key(1), member_key(2));

        submit_as(
            &mut host,
            &m1,
            1,
            "governance",
            gov_encode(&GovMsg::Propose {
                proposal_id: "p".into(),
                action: GovAction::Signal { text: "x".into() },
                voting_period: 5,
            }),
        )
        .await
        .expect("propose");

        // a re-vote overwrites (yes -> no), it does not double-count.
        submit_as(
            &mut host,
            &m1,
            2,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "p".into(),
                approve: true,
            }),
        )
        .await
        .expect("m1 yes");
        submit_as(
            &mut host,
            &m1,
            3,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "p".into(),
                approve: false,
            }),
        )
        .await
        .expect("m1 flips to no");

        // voting closes AT the deadline (proposed at 1, period 5 -> deadline 6).
        let err = submit_as(
            &mut host,
            &m2,
            6,
            "governance",
            gov_encode(&GovMsg::Vote {
                proposal_id: "p".into(),
                approve: true,
            }),
        )
        .await
        .expect_err("vote at the deadline is closed");
        assert!(matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("closed")));

        // tally: zero CURRENT yes-ballots -> rejected.
        submit_as(
            &mut host,
            &m2,
            7,
            "governance",
            gov_encode(&GovMsg::Execute {
                proposal_id: "p".into(),
            }),
        )
        .await
        .expect("execute");
        assert_eq!(
            proposal_status(&host, "p").await,
            Some(ProposalStatus::Rejected)
        );
    });
}
