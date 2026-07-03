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
use commonware_cryptography::{ed25519::PrivateKey, Signer as _};
use futures::executor::block_on;
use governance::Governance;
use governance_interface::{
    encode_msg as gov_encode, encode_query as gov_query, decode_reply as gov_decode,
    GovAction, GovMsg, GovQuery, GovReply, ProposalStatus,
};
use host::{BlockContext, Host, SubmitError};
use sdk::{Error, Module as _, Msg, Origin};
use valset::Valset;
use valset_interface::{
    decode_reply as valset_decode, encode_msg as valset_encode, encode_query as valset_query,
    ValsetMsg, ValsetQuery, ValsetReply,
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
fn gov_host() -> Host {
    let mut valset = Valset::new("valset");
    valset.insert(member_key(1));
    valset.insert(member_key(2));
    Host::genesis(vec![
        Box::new(valset),
        Box::new(Governance::new("governance", "valset")),
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
        Msg { target: target.into(), payload },
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
    }
}

async fn proposal_status(host: &Host, id: &str) -> Option<ProposalStatus> {
    let reply = host
        .query("governance", &gov_query(&GovQuery::Proposal { proposal_id: id.into() }))
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
        let mut host = gov_host();
        let (m1, m2, newcomer) = (member_key(1), member_key(2), member_key(9));

        // DIRECT external valset writes are refused — the old one-message
        // liveness kill is closed.
        let err = submit_as(&mut host, &m1, 1, "valset", valset_encode(&ValsetMsg::Leave { key: m2.clone() }))
            .await
            .expect_err("external valset write must be refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("governance")),
            "got {err:?}"
        );
        assert_eq!(validators(&host).await.len(), 2, "membership untouched");

        // propose + both members vote yes -> early-decidable majority.
        submit_as(&mut host, &m1, 2, "governance", gov_encode(&GovMsg::Propose {
            proposal_id: "add-9".into(),
            action: GovAction::AddValidator { key: newcomer.clone() },
            voting_period: 100,
        }))
        .await
        .expect("propose");
        submit_as(&mut host, &m1, 3, "governance", gov_encode(&GovMsg::Vote {
            proposal_id: "add-9".into(),
            approve: true,
        }))
        .await
        .expect("vote m1");
        submit_as(&mut host, &m2, 4, "governance", gov_encode(&GovMsg::Vote {
            proposal_id: "add-9".into(),
            approve: true,
        }))
        .await
        .expect("vote m2");

        // early execution: yes-ballots already form a strict majority. the
        // valset op rides the SAME block as a governance-origin follow-up.
        submit_as(&mut host, &m2, 5, "governance", gov_encode(&GovMsg::Execute {
            proposal_id: "add-9".into(),
        }))
        .await
        .expect("execute");

        assert_eq!(proposal_status(&host, "add-9").await, Some(ProposalStatus::Passed));
        let members = validators(&host).await;
        assert_eq!(members.len(), 3, "the admitted validator is a member");
        assert!(members.contains(&newcomer));
    });
}

/// the solo-network tally: with one member, majority = 1/2 + 1 = 1, so the
/// founder's own ballot decides immediately — `invite-accept` on a network
/// of one admits the friend in a single propose/vote/execute round, no
/// second party required.
#[test]
fn a_single_member_ballot_is_a_deciding_majority() {
    block_on(async {
        let founder = member_key(1);
        let friend = member_key(9);
        let mut valset = Valset::new("valset");
        valset.insert(founder.clone());
        let mut host = Host::genesis(vec![
            Box::new(valset),
            Box::new(Governance::new("governance", "valset")),
        ])
        .expect("genesis");

        submit_as(&mut host, &founder, 1, "governance", gov_encode(&GovMsg::Propose {
            proposal_id: "add-friend".into(),
            action: GovAction::AddValidator { key: friend.clone() },
            voting_period: 100,
        }))
        .await
        .expect("propose");

        // proposing is NOT voting: executing with zero ballots is not
        // decidable before the deadline, even at n=1.
        let err = submit_as(&mut host, &founder, 2, "governance", gov_encode(&GovMsg::Execute {
            proposal_id: "add-friend".into(),
        }))
        .await
        .expect_err("no ballots -> not decidable early");
        assert!(matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("not decidable")));

        submit_as(&mut host, &founder, 3, "governance", gov_encode(&GovMsg::Vote {
            proposal_id: "add-friend".into(),
            approve: true,
        }))
        .await
        .expect("vote");
        submit_as(&mut host, &founder, 4, "governance", gov_encode(&GovMsg::Execute {
            proposal_id: "add-friend".into(),
        }))
        .await
        .expect("the single ballot decides");

        assert_eq!(proposal_status(&host, "add-friend").await, Some(ProposalStatus::Passed));
        let members = validators(&host).await;
        assert_eq!(members.len(), 2, "the friend is admitted");
        assert!(members.contains(&friend));
    });
}

#[test]
fn non_members_cannot_propose_or_vote_and_minority_rejects() {
    block_on(async {
        let mut host = gov_host();
        let (m1, m2, outsider) = (member_key(1), member_key(2), member_key(7));

        // an outsider cannot propose...
        let err = submit_as(&mut host, &outsider, 1, "governance", gov_encode(&GovMsg::Propose {
            proposal_id: "p".into(),
            action: GovAction::Signal { text: "hi".into() },
            voting_period: 10,
        }))
        .await
        .expect_err("outsider propose must be refused");
        assert!(matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("member")));

        // ...a member proposes, an outsider cannot vote...
        submit_as(&mut host, &m1, 2, "governance", gov_encode(&GovMsg::Propose {
            proposal_id: "p".into(),
            action: GovAction::Signal { text: "hi".into() },
            voting_period: 10,
        }))
        .await
        .expect("member propose");
        let err = submit_as(&mut host, &outsider, 3, "governance", gov_encode(&GovMsg::Vote {
            proposal_id: "p".into(),
            approve: true,
        }))
        .await
        .expect_err("outsider vote must be refused");
        assert!(matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("member")));

        // ...one yes of two members is NOT a strict majority: not decidable
        // early, and after the deadline it rejects.
        submit_as(&mut host, &m1, 4, "governance", gov_encode(&GovMsg::Vote {
            proposal_id: "p".into(),
            approve: true,
        }))
        .await
        .expect("m1 votes yes");
        let err = submit_as(&mut host, &m2, 5, "governance", gov_encode(&GovMsg::Execute {
            proposal_id: "p".into(),
        }))
        .await
        .expect_err("not decidable before the deadline without a majority");
        assert!(matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("not decidable")));

        // past the deadline (proposed at 2, period 10 -> deadline 12).
        submit_as(&mut host, &m2, 13, "governance", gov_encode(&GovMsg::Execute {
            proposal_id: "p".into(),
        }))
        .await
        .expect("execute after deadline");
        assert_eq!(proposal_status(&host, "p").await, Some(ProposalStatus::Rejected));
    });
}

#[test]
fn votes_close_at_the_deadline_and_ballots_are_per_member() {
    block_on(async {
        let mut host = gov_host();
        let (m1, m2) = (member_key(1), member_key(2));

        submit_as(&mut host, &m1, 1, "governance", gov_encode(&GovMsg::Propose {
            proposal_id: "p".into(),
            action: GovAction::Signal { text: "x".into() },
            voting_period: 5,
        }))
        .await
        .expect("propose");

        // a re-vote overwrites (yes -> no), it does not double-count.
        submit_as(&mut host, &m1, 2, "governance", gov_encode(&GovMsg::Vote { proposal_id: "p".into(), approve: true }))
            .await
            .expect("m1 yes");
        submit_as(&mut host, &m1, 3, "governance", gov_encode(&GovMsg::Vote { proposal_id: "p".into(), approve: false }))
            .await
            .expect("m1 flips to no");

        // voting closes AT the deadline (proposed at 1, period 5 -> deadline 6).
        let err = submit_as(&mut host, &m2, 6, "governance", gov_encode(&GovMsg::Vote { proposal_id: "p".into(), approve: true }))
            .await
            .expect_err("vote at the deadline is closed");
        assert!(matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("closed")));

        // tally: zero CURRENT yes-ballots -> rejected.
        submit_as(&mut host, &m2, 7, "governance", gov_encode(&GovMsg::Execute { proposal_id: "p".into() }))
            .await
            .expect("execute");
        assert_eq!(proposal_status(&host, "p").await, Some(ProposalStatus::Rejected));
    });
}

#[test]
fn snapshot_install_round_trips_and_rejects_tampering() {
    block_on(async {
        let mut host = gov_host();
        let m1 = member_key(1);
        submit_as(&mut host, &m1, 1, "governance", gov_encode(&GovMsg::Propose {
            proposal_id: "p".into(),
            action: GovAction::Signal { text: "snapshot me".into() },
            voting_period: 50,
        }))
        .await
        .expect("propose");
        submit_as(&mut host, &m1, 2, "governance", gov_encode(&GovMsg::Vote { proposal_id: "p".into(), approve: true }))
            .await
            .expect("vote");

        // rebuild a fresh instance from the snapshot, gated on the root.
        let root = host.module_root("governance").expect("gov root");
        let sdk::StateSyncHandle::SnapshotBytes(bytes) = ({
            // reach the handle through the host's finalized-snapshot surface.
            let finalized = host::FinalizedBlock { height: 2, app_hash: host.app_hash() };
            host.capture_finalized_snapshot(finalized)
                .expect("capture")
                .module("governance")
                .expect("gov entry")
                .state_sync
                .clone()
        }) else {
            panic!("governance must advertise snapshot bytes");
        };

        let mut rebuilt = Governance::new("governance", "valset");
        rebuilt.install(&bytes, root).expect("install");
        assert_eq!(rebuilt.root(), root, "installed root equals the source root");

        // a flipped bit must be refused without touching state.
        let mut tampered = bytes.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        let mut fresh = Governance::new("governance", "valset");
        assert!(fresh.install(&tampered, root).is_err(), "tampered snapshot refused");
        assert_eq!(fresh.root(), sdk::StateRoot::ZERO, "refused install leaves no trace");
    });
}
