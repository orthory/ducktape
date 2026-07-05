//! network-shape live admission: a fresh identity produced by `join` can start
//! immediately, park as a read-only observer, and promote once a running member
//! admits it through governance.

mod common;

use std::time::Duration;

use common::{NetworkShapeCluster, serial};

const CONVERGE: Duration = Duration::from_secs(180);

#[test]
fn network_shape_joiner_parks_until_promote() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("live-admission");
    assert!(
        !chain_id.is_empty(),
        "init should print the founded chain id"
    );
    cluster.spawn(0);
    // network-shape nodes never print the dev-demo `converged app_hash=`; the
    // founder is up and finalizing once its rpc surface is listening (genesis
    // is already crossed by then), which is all `invite`/`promote` need.
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // the MANUAL flavor (token-less v2 blob): the pubkey travels out-of-band
    // and no lobby announce happens — the tokened flavor has its own e2e
    // (join_request_e2e).
    let invite = cluster.invite_manual();
    let friend_key = cluster.join_friend(&invite);
    assert_eq!(
        friend_key.len(),
        64,
        "join should print the friend's public key hex"
    );

    // opt the friend into the shipped-index warm start (indexable spec §7
    // lane 2) the way an operator would: a hand-edited node-local policy
    // line. the whole lane then rides this admission for real — the founder
    // cuts and serves its index checkpoints over the mesh, the friend
    // fetches and stages them, and the promoted reboot adopts the set.
    let cfg = cluster.config_file(1);
    let toml = std::fs::read_to_string(&cfg).expect("read friend node.toml");
    std::fs::write(&cfg, format!("{toml}sync_index = true\n")).expect("write friend node.toml");

    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode: parking", Duration::from_secs(60));
    cluster.wait_marker(1, "parked:", Duration::from_secs(60));

    let (ok, out) = cluster.run_promote(&friend_key);
    assert!(ok, "promote failed:\n{out}");
    assert!(out.contains("admitted"), "unexpected verb output:\n{out}");

    cluster.wait_marker(0, "cutover complete: epoch 1", CONVERGE);
    cluster.wait_marker(1, "admitted at epoch 1", CONVERGE);
    cluster.wait_marker(1, "synced app_hash=", CONVERGE);
    cluster.wait_marker(1, "shipped index staged", CONVERGE);
    cluster.wait_marker(1, "promoted: validator at epoch 1", CONVERGE);
}

/// the STAGED admission flow end-to-end: invite → observer (mesh + pre-sync,
/// NO quorum seat) → promote → validator. the payoff assertions are the
/// quorum ones the one-step flow could never make:
///
///   1. while the friend holds observer standing, the valset's VALIDATOR set
///      still names only the founder — committed state proves the tier split;
///   2. the chain keeps finalizing with the observer KILLED (under the old
///      one-step flow the friend would already hold a quorum seat here, and a
///      2-member quorum with one member down is a stall);
///   3. a restarted observer parks straight back into observer mode (the
///      pre-sync writes no checkpoint manifest — a reboot is clean);
///   4. `promote` then seats a WARM validator through the normal path.
///
/// observer ops are protocol-v3-gated, so the leg first drives the upgrade
/// ceremony to v3 on the solo founder (schedule → auto-signal → activate).
#[test]
fn staged_admission_observer_presyncs_then_promotes_warm() {
    use directory_interface::{DirMsg, DirQuery, DirReply};
    use governance_interface::{GovAction, GovMsg, GovQuery, GovReply, ProposalStatus};
    use valset_interface::{ValsetQuery, ValsetReply};

    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("staged-admission");
    assert!(!chain_id.is_empty(), "init should print the founded chain id");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // ---- protocol v3: schedule → auto-signal (solo R==n) → activate --------
    let proposal_status = |cluster: &NetworkShapeCluster, id: &str| -> Option<ProposalStatus> {
        let reply = cluster.query(
            0,
            "governance",
            &governance_interface::encode_query(&GovQuery::Proposal {
                proposal_id: id.into(),
            }),
        )?;
        match governance_interface::decode_reply(&reply).ok()? {
            GovReply::Proposal(Some(view)) => Some(view.status),
            _ => None,
        }
    };
    let poll = |what: &str, mut pred: Box<dyn FnMut() -> bool + '_>| {
        let deadline = std::time::Instant::now() + CONVERGE;
        while !pred() {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for {what}"
            );
            std::thread::sleep(Duration::from_millis(300));
        }
    };
    // self-correcting lead against the (slow, nop-gated) block rate: retry
    // with a doubled lead if the min-lead gate aborted the execute.
    let mut lead = 30u64;
    let mut activation = 0u64;
    for attempt in 0..4u32 {
        let base = cluster.status(0)["height"].as_u64().unwrap_or(0);
        activation = base + lead;
        let pid = format!("observer-tier-a{attempt}");
        cluster.submit(
            0,
            "governance",
            &governance_interface::encode_msg(&GovMsg::Propose {
                proposal_id: pid.clone(),
                action: GovAction::ScheduleUpgrade {
                    name: "observer-tier".into(),
                    activation_height: activation,
                    to_version: 3,
                },
                voting_period: 600_000,
            }),
        );
        poll("the v3 proposal to open", Box::new(|| {
            proposal_status(&cluster, &pid).is_some()
        }));
        cluster.submit(
            0,
            "governance",
            &governance_interface::encode_msg(&GovMsg::Vote {
                proposal_id: pid.clone(),
                approve: true,
            }),
        );
        // a passing tally never auto-executes — the deciding voter drives the
        // explicit Execute (same-origin submits finalize in seq order, so the
        // ballot lands first).
        cluster.submit(
            0,
            "governance",
            &governance_interface::encode_msg(&GovMsg::Execute {
                proposal_id: pid.clone(),
            }),
        );
        poll("the solo ballot to pass", Box::new(|| {
            proposal_status(&cluster, &pid)
                .is_some_and(|s| s != ProposalStatus::Open)
        }));
        // did the schedule take? (a min-lead abort leaves no pending slot.)
        let took = {
            let deadline = std::time::Instant::now() + Duration::from_secs(20);
            loop {
                let pending = cluster
                    .query(
                        0,
                        "upgrade",
                        &upgrade_interface::encode_query(&upgrade_interface::UpgradeQuery::Status),
                    )
                    .and_then(|raw| upgrade_interface::decode_reply(&raw).ok())
                    .map(|upgrade_interface::UpgradeReply::Status(st)| st.pending.is_some())
                    .unwrap_or(false);
                if pending {
                    break true;
                }
                if std::time::Instant::now() >= deadline {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
        };
        if took {
            break;
        }
        lead *= 2;
        assert!(attempt < 3, "could not schedule the v3 upgrade (lead {lead})");
    }
    cluster.wait_marker(0, "signaled ready name=observer-tier", CONVERGE);
    cluster.wait_marker(0, "upgrade armed name=observer-tier to_version=3", CONVERGE);
    cluster.wait_marker(0, "upgrade activated name=observer-tier version=3", CONVERGE);
    let _ = activation;

    // ---- invite → park → observer grant ------------------------------------
    let invite = cluster.invite_manual();
    let friend_key = cluster.join_friend(&invite);
    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode: parking", Duration::from_secs(60));

    let cfg = cluster.config_file(0);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["invite-accept", &friend_key, "--config"])
        .arg(&cfg)
        .output()
        .expect("run invite-accept");
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "invite-accept failed:\n{text}");
    assert!(
        text.contains("granted observer standing"),
        "unexpected invite-accept output:\n{text}"
    );

    // the grant's boundary admits the observer to the mesh; the parked node
    // then pre-syncs.
    cluster.wait_marker(1, "observer: pre-synced boundary", CONVERGE);

    // (1) the tier split in COMMITTED state: validators = founder only,
    //     observers = the friend.
    let validators = cluster
        .query(0, "valset", &valset_interface::encode_query(&ValsetQuery::Validators))
        .and_then(|raw| valset_interface::decode_reply(&raw).ok())
        .map(|r| match r {
            ValsetReply::Validators(v) => v,
            other => panic!("expected Validators, got {other:?}"),
        })
        .expect("valset validators readable");
    assert_eq!(validators.len(), 1, "the quorum still seats ONLY the founder");
    let observers = cluster
        .query(0, "valset", &valset_interface::encode_query(&ValsetQuery::Observers))
        .and_then(|raw| valset_interface::decode_reply(&raw).ok())
        .map(|r| match r {
            ValsetReply::Observers(v) => v,
            other => panic!("expected Observers, got {other:?}"),
        })
        .expect("valset observers readable");
    assert_eq!(
        observers,
        vec![common::unhex(&friend_key)],
        "the friend holds observer standing"
    );

    // (2) quorum untouched: kill the observer; the founder keeps finalizing.
    cluster.kill(1);
    cluster.submit(
        0,
        "directory",
        &directory_interface::encode_msg(&DirMsg::Set {
            key: "observer-down-liveness".into(),
            value: "alive".into(),
        }),
    );
    poll("a finalized op with the observer down", Box::new(|| {
        cluster
            .query(
                0,
                "directory",
                &directory_interface::encode_query(&DirQuery::Get {
                    key: "observer-down-liveness".into(),
                }),
            )
            .and_then(|raw| directory_interface::decode_reply(&raw).ok())
            .is_some_and(|r| matches!(r, DirReply::Value(Some(_))))
    }));

    // (3) a restarted observer parks straight back into observer mode — the
    //     pre-sync left NO checkpoint manifest behind. (the config-time
    //     joiner banner may not reprint: the first run's recovery-journal
    //     files flip the cheap boot probe, and the runtime then re-decides
    //     from the real store — the observer marker alone is the proof.)
    cluster.spawn(1);
    cluster.wait_marker(1, "observer:", CONVERGE);

    // (4) promote: the warm observer becomes a validator through the normal
    //     promotion path; valset Join clears its observer standing.
    let (ok, out) = cluster.run_promote(&friend_key);
    assert!(ok, "promote failed:\n{out}");
    assert!(out.contains("admitted"), "unexpected promote output:\n{out}");
    cluster.wait_marker(1, "admitted at epoch", CONVERGE);
    cluster.wait_marker(1, "synced app_hash=", CONVERGE);
    cluster.wait_marker(1, "promoted: validator at epoch", CONVERGE);
    let observers = cluster
        .query(0, "valset", &valset_interface::encode_query(&ValsetQuery::Observers))
        .and_then(|raw| valset_interface::decode_reply(&raw).ok())
        .map(|r| match r {
            ValsetReply::Observers(v) => v,
            other => panic!("expected Observers, got {other:?}"),
        })
        .expect("valset observers readable");
    assert!(
        observers.is_empty(),
        "promotion must clear observer standing (got {observers:?})"
    );
}
