//! account-signed governance FRAMES over the sim's real `/v1/submit/frame` wire
//! (ADR A1, the W2 governance migration): admit/promote/demote/leave now leave
//! the bespoke node-local re-signing lane and arrive as frames the app signs
//! with the USER's ACCOUNT key. The governance module authorizes the verified
//! frame origin by resolving it — via the committed `BindNode` — to the
//! account's bound nodes and checking THEIR valset standing.
//!
//! This is the network-level standing gate the module unit tests
//! (`governance_shares.rs`) prove in-process: here the ops travel the exact
//! signed-frame transport (`node::encode_frame` → POST /v1/submit/frame) a real
//! validator verifies, authored by an account member key that is NOT any node
//! key. A signer with no bound member node is refused at the module door.

mod harness;

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use governance::{GovAction, GovMsg, encode_msg as gov_encode};
use harness::{Sim, ed_bind_auth};
use identity::bind_preimage;
use sdk::Msg;
use serde_json::{Value, json};
use std::path::Path;

type Ed = PrivateKey;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// the `hex:` origin escape resolving to a node's raw pubkey (setup only).
fn origin(k: &Ed) -> String {
    format!("hex:{}", hex(k.public_key().as_ref()))
}

fn governed(storage: &Path) -> (Sim, Vec<Ed>) {
    let validators: Vec<Ed> = (1..=3u64).map(Ed::from_seed).collect();
    let hexes = validators
        .iter()
        .map(|k| hex(k.public_key().as_ref()))
        .collect::<Vec<_>>()
        .join(",");
    let sim = Sim::spawn(storage, &["--auto", "--with-valset", &hexes]);
    (sim, validators)
}

fn has_key(list: &Value, key: &[u8]) -> bool {
    let want = json!(key.to_vec());
    list.as_array().is_some_and(|a| a.contains(&want))
}

/// submit a governance op as a SIGNED FRAME authored by `signer` (its verified
/// key becomes `Origin::External`), asserting it commits.
fn gov_frame_ok(sim: &Sim, signer: &Ed, seq: u64, msg: &GovMsg) {
    let frame = node::encode_frame(
        signer,
        seq,
        &Msg {
            target: "governance".into(),
            payload: gov_encode(msg),
        },
    );
    let (code, reply) = sim.submit_frame(&frame);
    assert_eq!(code, 200, "governance frame must commit: {reply}");
}

/// bind `node` (a genesis validator) to a fresh account founded by member key
/// `account`, over the frameless setup lane — chain_id is "" in the sim.
fn bind_node_to_account(sim: &Sim, node: &Ed, account: &Ed) {
    let preimage = bind_preimage("", node.public_key().as_ref(), 0);
    sim.submit_ok(
        "identity",
        json!({ "bind_node": { "authorizer": ed_bind_auth(account, &preimage) } }),
        Some(origin(node).as_str()),
    );
}

/// The crown scenario: an account member key — bound to a validator node —
/// drives a full admit ceremony (propose → vote → execute of an AddResident)
/// as SIGNED FRAMES, and the resident grant lands. The account's ballot is its
/// bound node's; the other two validators complete the majority.
#[test]
fn an_account_signed_frame_admits_a_resident_through_its_bound_node_standing() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, validators) = governed(storage.path());

    // account key K (never a node key) owns validator node[0].
    let account = Ed::from_seed(100);
    bind_node_to_account(&sim, &validators[0], &account);

    let newcomer = Ed::from_seed(50);
    let admit = GovAction::AddResident {
        key: newcomer.public_key().as_ref().to_vec(),
    };

    // propose + vote, authored by the ACCOUNT key over the frame lane.
    gov_frame_ok(
        &sim,
        &account,
        1,
        &GovMsg::Propose {
            proposal_id: "admit".into(),
            action: admit.clone(),
            voting_period: 1_000_000,
        },
    );
    gov_frame_ok(
        &sim,
        &account,
        2,
        &GovMsg::Vote {
            proposal_id: "admit".into(),
            approve: true,
        },
    );
    // the other two validators complete the majority (node-key frames — a
    // validator remains a first-class governance actor).
    gov_frame_ok(
        &sim,
        &validators[1],
        1,
        &GovMsg::Vote {
            proposal_id: "admit".into(),
            approve: true,
        },
    );
    gov_frame_ok(
        &sim,
        &validators[2],
        1,
        &GovMsg::Vote {
            proposal_id: "admit".into(),
            approve: true,
        },
    );
    gov_frame_ok(
        &sim,
        &account,
        3,
        &GovMsg::Execute {
            proposal_id: "admit".into(),
        },
    );

    // the proposer was recorded as the ACCOUNT, and the grant landed.
    let proposal = sim.query("governance", json!({ "proposal": { "proposal_id": "admit" } }));
    assert_eq!(proposal["proposal"]["status"], "passed", "{proposal}");
    assert_eq!(
        proposal["proposal"]["proposer"],
        json!(account.public_key().as_ref().to_vec()),
        "the proposal is authored by the account, not a node key: {proposal}"
    );
    let residents = sim.query("valset", json!("residents"));
    assert!(
        has_key(&residents["residents"], newcomer.public_key().as_ref()),
        "the account-signed admit granted resident standing: {residents}"
    );
}

/// A frame signer with NO validator-set standing — no bound member node, and
/// not a validator itself — is refused at the governance door.
#[test]
fn a_frame_from_a_key_without_standing_is_refused() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, _validators) = governed(storage.path());

    let stranger = Ed::from_seed(200);
    let frame = node::encode_frame(
        &stranger,
        1,
        &Msg {
            target: "governance".into(),
            payload: gov_encode(&GovMsg::Propose {
                proposal_id: "nope".into(),
                action: GovAction::Signal { text: "hi".into() },
                voting_period: 20,
            }),
        },
    );
    let (code, reply) = sim.submit_frame(&frame);
    assert_eq!(code, 400, "a signer without standing must be refused: {reply}");
    let err = reply["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("standing"),
        "the refusal names the missing standing: {reply}"
    );
}

/// A bound account can also LEAVE (self-remove one of its validator nodes) via a
/// signed frame; with the remaining two validators approving, the set shrinks.
#[test]
fn an_account_signed_frame_demotes_a_validator() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, validators) = governed(storage.path());

    let account = Ed::from_seed(101);
    bind_node_to_account(&sim, &validators[0], &account);

    // demote validator[2] (a RemoveValidator proposal). account (node[0]) +
    // node[1] approve → 2 of 3.
    let demote = GovAction::RemoveValidator {
        key: validators[2].public_key().as_ref().to_vec(),
    };
    gov_frame_ok(
        &sim,
        &account,
        1,
        &GovMsg::Propose {
            proposal_id: "demote".into(),
            action: demote,
            voting_period: 1_000_000,
        },
    );
    gov_frame_ok(
        &sim,
        &account,
        2,
        &GovMsg::Vote {
            proposal_id: "demote".into(),
            approve: true,
        },
    );
    gov_frame_ok(
        &sim,
        &validators[1],
        1,
        &GovMsg::Vote {
            proposal_id: "demote".into(),
            approve: true,
        },
    );
    gov_frame_ok(
        &sim,
        &account,
        3,
        &GovMsg::Execute {
            proposal_id: "demote".into(),
        },
    );

    let proposal = sim.query("governance", json!({ "proposal": { "proposal_id": "demote" } }));
    assert_eq!(proposal["proposal"]["status"], "passed", "{proposal}");
    let members = sim.query("valset", json!("validators"));
    assert!(
        !has_key(&members["validators"], validators[2].public_key().as_ref()),
        "the demoted validator is gone: {members}"
    );
}
