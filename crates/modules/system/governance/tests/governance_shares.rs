//! Non-transferable account shares through a real Host. The validator-mode
//! electorate adopts one explicit allocation; later proposals freeze account
//! power, so two nodes owned by one account share one ballot and later share
//! changes cannot rewrite an open proposal's decision boundary.

use std::collections::BTreeMap;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use futures::executor::block_on;
use governance::{
    GovAction, GovMsg, GovQuery, GovReply, Governance, ProposalStatus, ShareAllocation, VoterKind,
    VotingRule, decode_reply, encode_msg, encode_query,
};
use host::{BlockContext, Host, SubmitError};
use identity::{
    AccountView, IdentityQuery, IdentityReply, KeyKind, MemberKeyView,
    decode_query as identity_decode_query, encode_reply as identity_encode_reply,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use valset::Valset;

fn key(seed: u8) -> Vec<u8> {
    let seed = [seed; 32];
    PrivateKey::decode(&seed[..])
        .expect("valid seed")
        .public_key()
        .as_ref()
        .to_vec()
}

struct IdentityStub {
    accounts: BTreeMap<Vec<u8>, AccountView>,
    by_node: BTreeMap<Vec<u8>, Vec<u8>>,
    /// member key -> account id. seeded with each account's founding key
    /// (member key == account id); [`Self::add_member`] extends an account
    /// with a second key, mirroring the real module's member index.
    by_member: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl IdentityStub {
    /// register an EXTRA member key on an existing account — the "one human,
    /// two devices" shape the overwrite-dedup tests exercise.
    fn add_member(&mut self, account_id: &[u8], key: Vec<u8>) {
        let account = self.accounts.get_mut(account_id).expect("account exists");
        account.member_keys.push(MemberKeyView {
            pubkey: key.clone(),
            kind: KeyKind::Ed25519,
            label: None,
            added_at: 0,
        });
        self.by_member.insert(key, account_id.to_vec());
    }

    fn new(entries: Vec<(Vec<u8>, Vec<Vec<u8>>)>) -> Self {
        let mut accounts = BTreeMap::new();
        let mut by_node = BTreeMap::new();
        let mut by_member = BTreeMap::new();
        for (account_id, nodes) in entries {
            for node in &nodes {
                by_node.insert(node.clone(), account_id.clone());
            }
            by_member.insert(account_id.clone(), account_id.clone());
            accounts.insert(
                account_id.clone(),
                AccountView {
                    account_id: account_id.clone(),
                    display_name: None,
                    avatar: None,
                    bio: None,
                    nonce: 0,
                    member_keys: vec![MemberKeyView {
                        pubkey: account_id,
                        kind: KeyKind::Ed25519,
                        label: None,
                        added_at: 0,
                    }],
                    nodes: nodes
                        .into_iter()
                        .map(|node_key| identity::NodeView {
                            node_key,
                            label: None,
                        })
                        .collect(),
                    updated_at: 0,
                },
            );
        }
        Self {
            accounts,
            by_node,
            by_member,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for IdentityStub {
    fn id(&self) -> ModuleId {
        "identity".into()
    }

    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::Stateless)
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        Err(Error::Module("identity test stub is read-only".into()))
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let account = match identity_decode_query(req).map_err(Error::Module)? {
            IdentityQuery::Get { account_id } => self.accounts.get(&account_id).cloned(),
            IdentityQuery::OfNode { node_key } => self
                .by_node
                .get(&node_key)
                .and_then(|account_id| self.accounts.get(account_id))
                .cloned(),
            IdentityQuery::OfMember { member_key } => self
                .by_member
                .get(&member_key)
                .and_then(|account_id| self.accounts.get(account_id))
                .cloned(),
            IdentityQuery::All { .. } => {
                return Ok(identity_encode_reply(&IdentityReply::Accounts(
                    self.accounts.values().cloned().collect(),
                )));
            }
            IdentityQuery::Clients => {
                return Ok(identity_encode_reply(&IdentityReply::Clients(Vec::new())));
            }
        };
        Ok(identity_encode_reply(&IdentityReply::Account(account)))
    }
}

fn share_host() -> (Host, [Vec<u8>; 4], [Vec<u8>; 3]) {
    let nodes = [key(1), key(2), key(3), key(4)];
    let accounts = [key(11), key(12), key(13)];
    let mut valset = Valset::new("valset");
    for node in &nodes[..3] {
        valset.insert(node.clone());
    }
    let identity = IdentityStub::new(vec![
        (
            accounts[0].clone(),
            vec![nodes[0].clone(), nodes[1].clone()],
        ),
        (accounts[1].clone(), vec![nodes[2].clone()]),
        (accounts[2].clone(), vec![nodes[3].clone()]),
    ]);
    let host = Host::genesis(vec![
        Box::new(valset),
        Box::new(Governance::new("governance", "valset", "identity")),
        Box::new(identity),
    ])
    .expect("genesis");
    (host, nodes, accounts)
}

async fn submit(host: &mut Host, node: &[u8], at: u64, msg: GovMsg) -> Result<(), SubmitError> {
    host.submit_at(
        BlockContext {
            height: at,
            consensus_time: at,
            origin: Origin::External(node.to_vec()),
        },
        Msg {
            target: "governance".into(),
            payload: encode_msg(&msg),
        },
    )
    .await
    .map(|_| ())
}

async fn proposal(host: &Host, id: &str) -> governance::ProposalView {
    let reply = host
        .query(
            "governance",
            &encode_query(&GovQuery::Proposal {
                proposal_id: id.into(),
            }),
        )
        .await
        .expect("proposal query");
    let GovReply::Proposal(Some(proposal)) = decode_reply(&reply).expect("decode") else {
        panic!("proposal exists")
    };
    proposal
}

async fn shares(host: &Host) -> governance::SharesView {
    let reply = host
        .query("governance", &encode_query(&GovQuery::Shares))
        .await
        .expect("shares query");
    let GovReply::Shares(shares) = decode_reply(&reply).expect("decode") else {
        panic!("shares reply")
    };
    shares
}

#[test]
fn shares_are_account_scoped_weighted_and_frozen_per_proposal() {
    block_on(async {
        let (mut host, nodes, accounts) = share_host();
        let defaults = shares(&host).await;
        assert!(!defaults.active, "validator ballots are the default mode");
        assert!(defaults.allocations.is_empty());
        submit(
            &mut host,
            &nodes[0],
            0,
            GovMsg::Propose {
                proposal_id: "premature-share-mode".into(),
                action: GovAction::SetShareMode { enabled: true },
                voting_period: 20,
            },
        )
        .await
        .expect_err("share mode requires a configured registry");
        let initial = vec![
            ShareAllocation {
                account_id: accounts[2].clone(),
                shares: 10,
            },
            ShareAllocation {
                account_id: accounts[0].clone(),
                shares: 60,
            },
            ShareAllocation {
                account_id: accounts[1].clone(),
                shares: 30,
            },
        ];

        submit(
            &mut host,
            &nodes[0],
            1,
            GovMsg::Propose {
                proposal_id: "adopt".into(),
                action: GovAction::AdoptShares {
                    allocations: initial,
                },
                voting_period: 20,
            },
        )
        .await
        .expect("propose adoption");
        for (node, at) in [(&nodes[0], 2), (&nodes[1], 3)] {
            submit(
                &mut host,
                node,
                at,
                GovMsg::Vote {
                    proposal_id: "adopt".into(),
                    approve: true,
                },
            )
            .await
            .expect("validator-mode ballot on the adopt proposal");
        }
        let adoption = proposal(&host, "adopt").await;
        assert_eq!(adoption.voter_kind, VoterKind::ValidatorNode);
        assert_eq!(
            adoption.votes.len(),
            2,
            "nodes sharing an account still have separate ballots in validator mode"
        );
        submit(
            &mut host,
            &nodes[2],
            4,
            GovMsg::Execute {
                proposal_id: "adopt".into(),
            },
        )
        .await
        .expect("adopt shares");

        let adopted = shares(&host).await;
        assert!(adopted.active);
        assert_eq!(adopted.total, 100);
        let mut sorted_accounts = accounts.to_vec();
        sorted_accounts.sort();
        assert_eq!(
            adopted
                .allocations
                .iter()
                .map(|allocation| allocation.account_id.clone())
                .collect::<Vec<_>>(),
            sorted_accounts,
            "the adoption action is normalized by account id"
        );

        // Open a signal before changing C's shares. Its 60/30/10 electorate
        // must remain unchanged after the structural update lands.
        submit(
            &mut host,
            &nodes[0],
            5,
            GovMsg::Propose {
                proposal_id: "signal".into(),
                action: GovAction::Signal {
                    text: "ship".into(),
                },
                voting_period: 20,
            },
        )
        .await
        .expect("shareholder signal");
        submit(
            &mut host,
            &nodes[0],
            6,
            GovMsg::Propose {
                proposal_id: "raise-c".into(),
                action: GovAction::SetShares {
                    account_id: accounts[2].clone(),
                    shares: 20,
                },
                voting_period: 20,
            },
        )
        .await
        .expect("structural proposal");

        // A owns two validator nodes but only one 60-share ballot. Its second
        // node overwrites the same principal and cannot reach the 67-share
        // structural threshold by itself.
        for (node, at) in [(&nodes[0], 7), (&nodes[1], 8)] {
            submit(
                &mut host,
                node,
                at,
                GovMsg::Vote {
                    proposal_id: "raise-c".into(),
                    approve: true,
                },
            )
            .await
            .expect("account A vote");
        }
        let structural = proposal(&host, "raise-c").await;
        assert_eq!(structural.voter_kind, VoterKind::Account);
        assert_eq!(
            structural.votes.len(),
            1,
            "two nodes share one account ballot"
        );
        assert_eq!(
            structural.voting_rule,
            VotingRule::Threshold { required_yes: 67 }
        );
        submit(
            &mut host,
            &nodes[0],
            9,
            GovMsg::Execute {
                proposal_id: "raise-c".into(),
            },
        )
        .await
        .expect_err("60 shares cannot decide a two-thirds action");

        submit(
            &mut host,
            &nodes[2],
            10,
            GovMsg::Vote {
                proposal_id: "raise-c".into(),
                approve: true,
            },
        )
        .await
        .expect("account B vote");
        submit(
            &mut host,
            &nodes[2],
            11,
            GovMsg::Execute {
                proposal_id: "raise-c".into(),
            },
        )
        .await
        .expect("90 shares pass the structural action");
        assert_eq!(shares(&host).await.total, 110);

        let signal = proposal(&host, "signal").await;
        assert_eq!(signal.voter_kind, VoterKind::Account);
        assert_eq!(
            signal.voting_rule,
            VotingRule::ParticipatingMajority { quorum: 50 }
        );
        assert_eq!(
            signal
                .electorate
                .iter()
                .find(|(account, _)| account == &accounts[2])
                .map(|(_, power)| *power),
            Some(10),
            "an open proposal keeps its proposal-time share snapshot"
        );

        // A's second node flips A's ballot rather than adding another one.
        submit(
            &mut host,
            &nodes[0],
            12,
            GovMsg::Vote {
                proposal_id: "signal".into(),
                approve: true,
            },
        )
        .await
        .expect("A yes");
        submit(
            &mut host,
            &nodes[1],
            13,
            GovMsg::Vote {
                proposal_id: "signal".into(),
                approve: false,
            },
        )
        .await
        .expect("A flips through another node");
        submit(
            &mut host,
            &nodes[2],
            14,
            GovMsg::Vote {
                proposal_id: "signal".into(),
                approve: true,
            },
        )
        .await
        .expect("B yes");
        submit(
            &mut host,
            &nodes[3],
            15,
            GovMsg::Vote {
                proposal_id: "signal".into(),
                approve: true,
            },
        )
        .await
        .expect("C yes");
        assert_eq!(proposal(&host, "signal").await.votes.len(), 3);
        submit(
            &mut host,
            &nodes[0],
            26,
            GovMsg::Execute {
                proposal_id: "signal".into(),
            },
        )
        .await
        .expect("settle after deadline");
        assert_eq!(
            proposal(&host, "signal").await.status,
            ProposalStatus::Rejected
        );

        // The extension is root-hashed and survives state sync byte-for-byte.
        let root = host.module_root("governance").expect("root");
        let sdk::StateSyncHandle::SnapshotBytes(bytes) = host
            .capture_finalized_snapshot(host::FinalizedBlock {
                height: 26,
                root_hash: host.root_hash(),
            })
            .expect("capture")
            .module("governance")
            .expect("governance")
            .state_sync
            .clone()
        else {
            panic!("snapshot bytes")
        };
        let mut rebuilt = Governance::new("governance", "valset", "identity");
        rebuilt
            .install(&bytes, root)
            .expect("install shares snapshot");
        assert_eq!(rebuilt.root(), root);
        let mut missing_mode = bytes.clone();
        missing_mode.pop();
        assert!(
            Governance::new("governance", "valset", "identity")
                .install(&missing_mode, root)
                .is_err(),
            "a share snapshot must state its mode"
        );
        let reply = rebuilt
            .query(&encode_query(&GovQuery::Shares))
            .await
            .expect("query rebuilt shares");
        let GovReply::Shares(rebuilt_shares) = decode_reply(&reply).expect("decode") else {
            panic!("shares")
        };
        assert_eq!(rebuilt_shares.total, 110);

        // The current share electorate can restore the default validator mode.
        submit(
            &mut host,
            &nodes[0],
            27,
            GovMsg::Propose {
                proposal_id: "validator-mode".into(),
                action: GovAction::SetShareMode { enabled: false },
                voting_period: 20,
            },
        )
        .await
        .expect("propose validator mode");
        for (node, at) in [(&nodes[0], 28), (&nodes[2], 29)] {
            submit(
                &mut host,
                node,
                at,
                GovMsg::Vote {
                    proposal_id: "validator-mode".into(),
                    approve: true,
                },
            )
            .await
            .expect("shareholder approves validator mode");
        }
        submit(
            &mut host,
            &nodes[2],
            30,
            GovMsg::Execute {
                proposal_id: "validator-mode".into(),
            },
        )
        .await
        .expect("switch to validator mode");
        let inactive = shares(&host).await;
        assert!(!inactive.active);
        assert_eq!(inactive.total, 110, "switching modes retains allocations");

        // The explicit inactive-mode override is root-hashed and state-syncable.
        let inactive_root = host.module_root("governance").expect("inactive root");
        let sdk::StateSyncHandle::SnapshotBytes(inactive_bytes) = host
            .capture_finalized_snapshot(host::FinalizedBlock {
                height: 30,
                root_hash: host.root_hash(),
            })
            .expect("capture inactive mode")
            .module("governance")
            .expect("governance")
            .state_sync
            .clone()
        else {
            panic!("snapshot bytes")
        };
        let mut inactive_rebuilt = Governance::new("governance", "valset", "identity");
        inactive_rebuilt
            .install(&inactive_bytes, inactive_root)
            .expect("install inactive shares snapshot");
        let reply = inactive_rebuilt
            .query(&encode_query(&GovQuery::Shares))
            .await
            .expect("query inactive shares");
        let GovReply::Shares(inactive_rebuilt_shares) = decode_reply(&reply).expect("decode")
        else {
            panic!("shares")
        };
        assert!(!inactive_rebuilt_shares.active);
        assert_eq!(inactive_rebuilt_shares.total, 110);

        // In validator mode, the two nodes owned by A again count separately.
        submit(
            &mut host,
            &nodes[0],
            31,
            GovMsg::Propose {
                proposal_id: "validator-signal".into(),
                action: GovAction::Signal {
                    text: "one validator, one ballot".into(),
                },
                voting_period: 20,
            },
        )
        .await
        .expect("validator-mode signal");
        let validator_signal = proposal(&host, "validator-signal").await;
        assert_eq!(validator_signal.voter_kind, VoterKind::ValidatorNode);
        assert_eq!(validator_signal.electorate.len(), 3);
        assert!(
            validator_signal
                .electorate
                .iter()
                .all(|(_, power)| *power == 1)
        );
        assert_eq!(
            validator_signal.voting_rule,
            VotingRule::Threshold { required_yes: 2 }
        );
        for (node, at) in [(&nodes[0], 32), (&nodes[1], 33)] {
            submit(
                &mut host,
                node,
                at,
                GovMsg::Vote {
                    proposal_id: "validator-signal".into(),
                    approve: true,
                },
            )
            .await
            .expect("validator ballot");
        }
        assert_eq!(proposal(&host, "validator-signal").await.votes.len(), 2);
        submit(
            &mut host,
            &nodes[2],
            34,
            GovMsg::Execute {
                proposal_id: "validator-signal".into(),
            },
        )
        .await
        .expect("validator majority passes");

        // The validator electorate can enable the retained account registry.
        submit(
            &mut host,
            &nodes[0],
            35,
            GovMsg::Propose {
                proposal_id: "share-mode".into(),
                action: GovAction::SetShareMode { enabled: true },
                voting_period: 20,
            },
        )
        .await
        .expect("propose share mode");
        for (node, at) in [(&nodes[0], 36), (&nodes[1], 37)] {
            submit(
                &mut host,
                node,
                at,
                GovMsg::Vote {
                    proposal_id: "share-mode".into(),
                    approve: true,
                },
            )
            .await
            .expect("validator approves share mode");
        }
        submit(
            &mut host,
            &nodes[2],
            38,
            GovMsg::Execute {
                proposal_id: "share-mode".into(),
            },
        )
        .await
        .expect("switch back to account shares");
        assert!(shares(&host).await.active);
    });
}

// ── account-signed governance frames (ADR A1) ──
//
// admit/promote/demote/leave now arrive as account-signed frames on the public
// surface: the verified origin is an account MEMBER key, and the module
// authorizes it by resolving that key to the account's committed bound nodes
// and checking their valset standing. In `share_host`, the IdentityStub uses
// account_id == member key, and each account owns the nodes listed at genesis.

#[test]
fn an_account_member_key_proposes_and_its_vote_casts_all_its_bound_node_ballots() {
    block_on(async {
        // validators = nodes[0..3]; account[0] owns nodes[0]+nodes[1],
        // account[1] owns nodes[2]. Default (validator) mode: N validators = N
        // votes, ballots node-keyed.
        let (mut host, nodes, accounts) = share_host();

        // account[0] (a member KEY, never a node key) opens a proposal.
        submit(
            &mut host,
            &accounts[0],
            1,
            GovMsg::Propose {
                proposal_id: "acct-signal".into(),
                action: GovAction::Signal { text: "hi".into() },
                voting_period: 50,
            },
        )
        .await
        .expect("an account member with bound-node standing may propose");

        let opened = proposal(&host, "acct-signal").await;
        assert_eq!(opened.voter_kind, VoterKind::ValidatorNode);
        assert_eq!(
            opened.proposer, accounts[0],
            "the proposer is recorded as the ACCOUNT, not a node key"
        );
        // the frozen electorate is still the three validator NODES.
        let mut electorate: Vec<Vec<u8>> =
            opened.electorate.iter().map(|(k, _)| k.clone()).collect();
        electorate.sort();
        let mut want = vec![nodes[0].clone(), nodes[1].clone(), nodes[2].clone()];
        want.sort();
        assert_eq!(electorate, want);

        // ONE account-signed Vote casts BOTH of account[0]'s bound member
        // nodes' ballots — the exact power it held when each node voted itself.
        submit(
            &mut host,
            &accounts[0],
            2,
            GovMsg::Vote {
                proposal_id: "acct-signal".into(),
                approve: true,
            },
        )
        .await
        .expect("account vote");
        let voted = proposal(&host, "acct-signal").await;
        let mut yes_nodes: Vec<Vec<u8>> = voted
            .votes
            .iter()
            .filter(|(_, y)| *y)
            .map(|(k, _)| k.clone())
            .collect();
        yes_nodes.sort();
        let mut expect_nodes = vec![nodes[0].clone(), nodes[1].clone()];
        expect_nodes.sort();
        assert_eq!(
            yes_nodes, expect_nodes,
            "one account op casts every bound electorate node's ballot"
        );

        // account[1] adds its node's yes → 3 of 3, a Signal needs a majority.
        submit(
            &mut host,
            &accounts[1],
            3,
            GovMsg::Vote {
                proposal_id: "acct-signal".into(),
                approve: true,
            },
        )
        .await
        .expect("second account vote");
        submit(
            &mut host,
            &nodes[0],
            4,
            GovMsg::Execute {
                proposal_id: "acct-signal".into(),
            },
        )
        .await
        .expect("execute");
        assert_eq!(
            proposal(&host, "acct-signal").await.status,
            ProposalStatus::Passed
        );
    });
}

#[test]
fn a_key_with_no_bound_member_node_is_refused() {
    block_on(async {
        // account[2] owns nodes[3], which is NOT in the validator set — so it
        // holds no governance standing and cannot open a proposal.
        let (mut host, _nodes, accounts) = share_host();
        let err = submit(
            &mut host,
            &accounts[2],
            1,
            GovMsg::Propose {
                proposal_id: "no-standing".into(),
                action: GovAction::Signal {
                    text: "nope".into(),
                },
                voting_period: 20,
            },
        )
        .await
        .expect_err("a key whose only bound node is not a validator has no standing");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("standing")),
            "rejection names the missing standing, got {err:?}"
        );

        // and a total stranger (no account at all → its own node key, not a
        // member) is likewise refused.
        let stranger = key(200);
        let err = submit(
            &mut host,
            &stranger,
            2,
            GovMsg::Propose {
                proposal_id: "stranger".into(),
                action: GovAction::Signal {
                    text: "nope".into(),
                },
                voting_period: 20,
            },
        )
        .await
        .expect_err("a non-member stranger cannot propose");
        assert!(matches!(err, SubmitError::Rejected(Error::Module(_))));
    });
}

// ── ballot overwrite-dedup (review M2): one node, one ballot, whoever casts it ──
//
// Node-keyed (validator-mode) ballots must stay exactly-once per node no matter
// which principal form casts or re-casts them: the account, the node itself, or
// a second member key of the same account. Re-voting OVERWRITES by node key.

/// (a) an account re-votes: still a single ballot per bound node, carrying the
/// LATEST direction.
#[test]
fn an_account_revote_overwrites_its_node_ballots_not_doubles_them() {
    block_on(async {
        // account[0] owns validator nodes[0] + nodes[1].
        let (mut host, _nodes, accounts) = share_host();
        submit(
            &mut host,
            &accounts[0],
            1,
            GovMsg::Propose {
                proposal_id: "p".into(),
                action: GovAction::Signal { text: "x".into() },
                voting_period: 100,
            },
        )
        .await
        .expect("propose");

        submit(
            &mut host,
            &accounts[0],
            2,
            GovMsg::Vote {
                proposal_id: "p".into(),
                approve: true,
            },
        )
            .await
            .expect("first vote");
        submit(
            &mut host,
            &accounts[0],
            3,
            GovMsg::Vote {
                proposal_id: "p".into(),
                approve: false,
            },
        )
            .await
            .expect("re-vote");

        let view = proposal(&host, "p").await;
        assert_eq!(
            view.votes.len(),
            2,
            "one ballot per bound node, never doubled"
        );
        assert!(
            view.votes.iter().all(|(_, approve)| !approve),
            "the re-vote overwrote both node ballots: {:?}",
            view.votes
        );
    });
}

/// (b) a node votes directly, then its owner account votes: the node's ballot is
/// OVERWRITTEN, not doubled.
#[test]
fn an_account_vote_overwrites_its_nodes_direct_ballot() {
    block_on(async {
        let (mut host, nodes, accounts) = share_host();
        submit(
            &mut host,
            &nodes[0],
            1,
            GovMsg::Propose {
                proposal_id: "p".into(),
                action: GovAction::Signal { text: "x".into() },
                voting_period: 100,
            },
        )
        .await
        .expect("propose");

        // nodes[0] casts its own ballot (a node stays a first-class actor)…
        submit(
            &mut host,
            &nodes[0],
            2,
            GovMsg::Vote {
                proposal_id: "p".into(),
                approve: true,
            },
        )
            .await
            .expect("node vote");
        // …then the owning account votes the other way: nodes[0]'s ballot flips
        // (overwritten by node key) and nodes[1] gains its ballot.
        submit(
            &mut host,
            &accounts[0],
            3,
            GovMsg::Vote {
                proposal_id: "p".into(),
                approve: false,
            },
        )
            .await
            .expect("account vote");

        let view = proposal(&host, "p").await;
        assert_eq!(
            view.votes.len(),
            2,
            "nodes[0] was overwritten, not doubled: {:?}",
            view.votes
        );
        for (node, approve) in &view.votes {
            assert!(
                !approve,
                "ballot for {node:?} carries the account's later direction"
            );
        }
    });
}

/// (c) two member keys of ONE account vote: both resolve to the same node
/// ballots — no double count, latest direction wins.
#[test]
fn two_member_keys_of_one_account_share_the_same_node_ballots() {
    block_on(async {
        // rebuild share_host's shape, with a SECOND member key on account[0].
        let nodes = [key(1), key(2), key(3), key(4)];
        let accounts = [key(11), key(12), key(13)];
        let second_key = key(21);
        let mut identity = IdentityStub::new(vec![
            (
                accounts[0].clone(),
                vec![nodes[0].clone(), nodes[1].clone()],
            ),
            (accounts[1].clone(), vec![nodes[2].clone()]),
            (accounts[2].clone(), vec![nodes[3].clone()]),
        ]);
        identity.add_member(&accounts[0], second_key.clone());
        let mut valset = Valset::new("valset");
        for node in &nodes[..3] {
            valset.insert(node.clone());
        }
        let mut host = Host::genesis(vec![
            Box::new(valset),
            Box::new(Governance::new("governance", "valset", "identity")),
            Box::new(identity),
        ])
        .expect("genesis");

        submit(
            &mut host,
            &accounts[0],
            1,
            GovMsg::Propose {
                proposal_id: "p".into(),
                action: GovAction::Signal { text: "x".into() },
                voting_period: 100,
            },
        )
        .await
        .expect("propose");

        // founding key votes yes, the second device's key votes no: SAME two
        // node ballots, overwritten — never four.
        submit(
            &mut host,
            &accounts[0],
            2,
            GovMsg::Vote {
                proposal_id: "p".into(),
                approve: true,
            },
        )
            .await
            .expect("founding-key vote");
        submit(
            &mut host,
            &second_key,
            3,
            GovMsg::Vote {
                proposal_id: "p".into(),
                approve: false,
            },
        )
            .await
            .expect("second-member-key vote");

        let view = proposal(&host, "p").await;
        assert_eq!(
            view.votes.len(),
            2,
            "both keys cast the SAME node ballots: {:?}",
            view.votes
        );
        let mut balloted: Vec<Vec<u8>> = view.votes.iter().map(|(k, _)| k.clone()).collect();
        balloted.sort();
        let mut expect = vec![nodes[0].clone(), nodes[1].clone()];
        expect.sort();
        assert_eq!(
            balloted, expect,
            "ballots are keyed by the account's bound nodes"
        );
        assert!(
            view.votes.iter().all(|(_, approve)| !approve),
            "the later member key's direction won: {:?}",
            view.votes
        );
    });
}
