//! Non-transferable account shares through a real Host. The legacy validator
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
}

impl IdentityStub {
    fn new(entries: Vec<(Vec<u8>, Vec<Vec<u8>>)>) -> Self {
        let mut accounts = BTreeMap::new();
        let mut by_node = BTreeMap::new();
        for (account_id, nodes) in entries {
            for node in &nodes {
                by_node.insert(node.clone(), account_id.clone());
            }
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
                    nodes,
                    updated_at: 0,
                },
            );
        }
        Self { accounts, by_node }
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
            IdentityQuery::OfMember { member_key } => self.accounts.get(&member_key).cloned(),
            IdentityQuery::All { .. } => {
                return Ok(identity_encode_reply(&IdentityReply::Accounts(
                    self.accounts.values().cloned().collect(),
                )));
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
        Box::new(Governance::new(
            "governance",
            "valset",
            "upgrade",
            "identity",
        )),
        Box::new(identity),
    ])
    .expect("genesis");
    (host, nodes, accounts)
}

async fn submit(host: &mut Host, node: &[u8], at: u64, msg: GovMsg) -> Result<(), SubmitError> {
    host.submit_at(
        BlockContext {
            protocol_version: 0,
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
            .expect("legacy validator vote");
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

        // The extension is app-hashed and survives state sync byte-for-byte.
        let root = host.module_root("governance").expect("root");
        let sdk::StateSyncHandle::SnapshotBytes(bytes) = host
            .capture_finalized_snapshot(host::FinalizedBlock {
                height: 26,
                app_hash: host.app_hash(),
            })
            .expect("capture")
            .module("governance")
            .expect("governance")
            .state_sync
            .clone()
        else {
            panic!("snapshot bytes")
        };
        let mut rebuilt = Governance::new("governance", "valset", "upgrade", "identity");
        rebuilt
            .install(&bytes, root)
            .expect("install shares snapshot");
        assert_eq!(rebuilt.root(), root);
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

        // The explicit inactive-mode override is app-hashed and state-syncable.
        let inactive_root = host.module_root("governance").expect("inactive root");
        let sdk::StateSyncHandle::SnapshotBytes(inactive_bytes) = host
            .capture_finalized_snapshot(host::FinalizedBlock {
                height: 30,
                app_hash: host.app_hash(),
            })
            .expect("capture inactive mode")
            .module("governance")
            .expect("governance")
            .state_sync
            .clone()
        else {
            panic!("snapshot bytes")
        };
        let mut inactive_rebuilt = Governance::new("governance", "valset", "upgrade", "identity");
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
