//! network-shape live admission: a fresh identity produced by `join` can start
//! immediately, park as a read-only observer, and promote through the
//! TWO-PHASE membership protocol once a running member admits it through
//! governance — registration lands it STANDBY (cutover #1, quorum unchanged),
//! the parked node proves a full state sync and announces ONLINE with its own
//! signed proof, a member relays that into the ordered lane, and the
//! ACTIVATION cutover (#2) widens the quorum, at which point the joiner
//! promotes.

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

    // direct admission: ONE cutover seats the friend; it syncs the frozen
    // boundary and promotes there. (the staged observer flow has its own
    // leg below.)
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
///   2. the observer SERVES: local reads (rpc + http) answer from its own
///      pre-synced boundary, a write is refused as reads-only, and a value the
///      founder finalizes becomes readable through the observer's surface (the
///      continuous follow);
///   3. the chain keeps finalizing with the observer KILLED (under the old
///      one-step flow the friend would already hold a quorum seat here, and a
///      2-member quorum with one member down is a stall);
///   4. a restarted observer parks straight back into observer mode (the
///      pre-sync writes no checkpoint manifest — a reboot is clean) and serves
///      again;
///   5. `promote` then seats a WARM validator through the normal path.
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

    // (2) the SERVING observer: the same local read surfaces a validator
    //     binds, answered from the observer's own pre-synced boundary.
    //     rpc status names the served boundary…
    poll("the observer to serve rpc status", Box::new(|| {
        let st = cluster.rpc(1, serde_json::json!({ "cmd": "status" }));
        st["ok"] == serde_json::json!(true)
            && st["status"]["height"].as_u64().is_some_and(|h| h > 0)
    }));
    //     …module reads answer from the OBSERVER's surface (the tier split is
    //     visible through the observer itself, not just the founder)…
    poll("the observer to serve valset reads", Box::new(|| {
        cluster
            .query(1, "valset", &valset_interface::encode_query(&ValsetQuery::Observers))
            .and_then(|raw| valset_interface::decode_reply(&raw).ok())
            .is_some_and(|r| matches!(
                r,
                ValsetReply::Observers(v) if v == vec![common::unhex(&friend_key)]
            ))
    }));
    //     …the http app surface answers its status route from the same host…
    {
        use std::io::{Read as _, Write as _};
        let mut conn =
            std::net::TcpStream::connect(("127.0.0.1", cluster.http_ports[1]))
                .expect("connect the observer's app surface");
        conn.set_read_timeout(Some(Duration::from_secs(15))).expect("http timeout");
        conn.write_all(b"GET /v1/status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .expect("http write");
        let mut raw = String::new();
        conn.read_to_string(&mut raw).expect("http read");
        assert!(raw.starts_with("HTTP/1.1 200"), "observer /v1/status must answer 200:\n{raw}");
        assert!(raw.contains("\"height\""), "observer /v1/status carries a height:\n{raw}");
    }
    //     …a write is refused as reads-only…
    let refused = cluster.rpc(
        1,
        serde_json::json!({
            "cmd": "submit",
            "target": "directory",
            "payload_hex": common::hex(&directory_interface::encode_msg(&DirMsg::Set {
                key: "observer-no-writes".into(),
                value: "refused".into(),
            })),
        }),
    );
    assert_eq!(refused["ok"], serde_json::json!(false), "observer must refuse writes: {refused}");
    assert!(
        refused["error"].as_str().unwrap_or_default().contains("reads only"),
        "the refusal names the reads-only contract: {refused}"
    );
    //     …and the follow is CONTINUOUS: a value the founder finalizes now
    //     becomes readable through the observer within a few boundaries.
    cluster.submit(
        0,
        "directory",
        &directory_interface::encode_msg(&DirMsg::Set {
            key: "observer-follow".into(),
            value: "fresh".into(),
        }),
    );
    poll("the observer to serve the followed write", Box::new(|| {
        cluster
            .query(
                1,
                "directory",
                &directory_interface::encode_query(&DirQuery::Get {
                    key: "observer-follow".into(),
                }),
            )
            .and_then(|raw| directory_interface::decode_reply(&raw).ok())
            .is_some_and(|r| matches!(r, DirReply::Value(Some(v)) if v == "fresh"))
    }));
    //     …and the DERIVED tier follows the boundary too: the explorer
    //     records the followed boundary (an honest boundary row — verified
    //     height + app-hash, frame-derived fields empty)…
    poll("the observer explorer to record a followed boundary", Box::new(|| {
        let (status, body) = common::http_request(cluster.http_ports[1], "GET", "/v1/blocks", None);
        status == 200
            && body["blocks"].as_array().is_some_and(|rows| {
                rows.iter().any(|b| {
                    b["hash"] == serde_json::json!("")
                        && b["height"].as_u64().is_some_and(|h| h > 0)
                        && !b["commitHash"].as_str().unwrap_or_default().is_empty()
                })
            })
    }));
    //     …and /v1/index/* answers from boundary-healed read models: every
    //     watermark sits at a followed boundary, visibly boundary-stamped
    //     (the backfill floor), with the store healthy. polled: a heal drops
    //     the watermark FIRST (crash-safety by re-trigger), so a read racing
    //     an in-flight heal legitimately sees 0 for a moment.
    poll("the observer index to report boundary-stamped watermarks", Box::new(|| {
        let (status, index_status) =
            common::http_request(cluster.http_ports[1], "GET", "/v1/index/status", None);
        let watermark = index_status["modules"]["directory"].as_u64().unwrap_or(0);
        status == 200
            && index_status["poisoned"] == serde_json::json!(false)
            && watermark > 0
            && index_status["backfilled"]["directory"].as_u64() == Some(watermark)
    }));

    // (3) quorum untouched: kill the observer; the founder keeps finalizing.
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

    // (4) a restarted observer parks straight back into observer mode — the
    //     pre-sync left NO checkpoint manifest behind. (the config-time
    //     joiner banner may not reprint: the first run's recovery-journal
    //     files flip the cheap boot probe, and the runtime then re-decides
    //     from the real store — the observer marker alone is the proof.)
    //     it then SERVES again from a fresh pre-sync.
    cluster.spawn(1);
    cluster.wait_marker(1, "observer: pre-synced boundary", CONVERGE);
    poll("the restarted observer to serve reads again", Box::new(|| {
        cluster.rpc(1, serde_json::json!({ "cmd": "status" }))["ok"]
            == serde_json::json!(true)
    }));

    // (5) promote: the warm observer becomes a validator through the normal
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
