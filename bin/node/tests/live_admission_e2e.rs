//! network-shape live admission: a fresh identity produced by `join` can start
//! immediately, park as a read-only resident, and promote through the
//! TWO-PHASE membership protocol once a running member admits it through
//! governance — registration lands it STANDBY (cutover #1, quorum unchanged),
//! the parked node proves a full state sync and announces ONLINE with its own
//! signed proof, a member relays that into the ordered lane, and the
//! ACTIVATION cutover (#2) widens the quorum, at which point the joiner
//! promotes.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use common::{NetworkShapeCluster, poll_until, serial};
use files::{
    Change, Content, EntryInfo, FilesMsg, FilesQuery, FilesReply, Kind, RefsInfo,
    decode_reply as files_decode_reply, encode_msg as files_encode_msg, encode_putblob,
    encode_query as files_encode_query, objects::object_id, to_hex,
};

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

    // In the manual flow, the pubkey travels out-of-band
    // and no lobby announce happens — the tokened flavor has its own e2e
    // (join_request_e2e).
    let invite = cluster.invite();
    let friend_key = cluster.join_friend_manual(&invite);
    assert_eq!(
        friend_key.len(),
        64,
        "join should print the friend's public key hex"
    );

    // opt the friend into the shipped-index warm start (indexable spec §7
    // lane 2) the way an operator would: EDIT the generated line's value
    // (the file is complete — every key already present, appends would be
    // duplicate-key parse errors). the whole lane then rides this admission
    // for real — the founder cuts and serves its index checkpoints over the
    // mesh, the friend fetches and stages them, and the promoted reboot
    // adopts the set.
    let cfg = cluster.config_file(1);
    let toml = std::fs::read_to_string(&cfg).expect("read friend node.toml");
    assert!(
        toml.contains("sync_index = false"),
        "generated file carries the key"
    );
    std::fs::write(
        &cfg,
        toml.replace("sync_index = false", "sync_index = true"),
    )
        .expect("write friend node.toml");

    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode:", Duration::from_secs(60));
    cluster.wait_marker(1, "joining:", Duration::from_secs(60));

    let (ok, out) = cluster.run_promote(&friend_key);
    assert!(ok, "promote failed:\n{out}");
    assert!(out.contains("admitted"), "unexpected verb output:\n{out}");

    // direct admission: ONE cutover seats the friend; it syncs the frozen
    // boundary and promotes there. (the staged resident flow has its own
    // leg below.)
    cluster.wait_marker(0, "cutover complete: epoch 1", CONVERGE);
    cluster.wait_marker(1, "admitted at epoch 1", CONVERGE);
    cluster.wait_marker(1, "synced app_hash=", CONVERGE);
    cluster.wait_marker(1, "shipped index staged", CONVERGE);
    cluster.wait_marker(1, "promoted: validator at epoch 1", CONVERGE);
}

/// the promotion REBOOT leg the markers above stop short of: after
/// `promoted: validator at epoch` the node exec-reboots and must complete a
/// post-reboot catch-up dialogue against the founder BEFORE it can serve or
/// vote — and the founder must keep answering the statesync channel through
/// that whole window, cutover included. every other promote leg in this
/// suite latches the `promoted:` marker and stops, so a founder that goes
/// silent right after the cutover (the field failure: ten
/// `catch-up manifest unavailable ... timed out` retries, then FATAL, in a
/// supervisor crash-loop) was invisible to CI. the exec keeps the same log
/// fd, so markers span the reboot; a FATAL exit panics `wait_marker` with
/// BOTH log tails — the founder's tail is the diagnosis.
#[test]
fn promoted_resident_boots_through_post_reboot_catchup() {
    use directory::{DirMsg, DirQuery, DirReply};

    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("promote-reboot");
    assert!(
        !chain_id.is_empty(),
        "init should print the founded chain id"
    );
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // resident standing first — the field node parked as a resident (staged
    // admission) before its promote, so the reboot starts from a warm,
    // boundary-following node exactly as it did in the field.
    let invite = cluster.invite();
    let friend_key = cluster.join_friend_manual(&invite);
    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode:", Duration::from_secs(60));
    let (ok, out) = cluster.run_membership_verb("resident accept", &friend_key);
    assert!(ok, "resident accept failed:\n{out}");
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    let (ok, out) = cluster.run_promote(&friend_key);
    assert!(ok, "promote failed:\n{out}");
    cluster.wait_marker(1, "promoted: validator at epoch", CONVERGE);

    // THE property: the catch-up completes — the success line's " frames)"
    // suffix is printed by no failure path ("unavailable" retries included).
    cluster.wait_marker(1, " frames)", CONVERGE);

    // and the network the reboot lands in is LIVE end to end: a write
    // finalized through the founder becomes readable from the promoted
    // friend's own surface…
    cluster.submit(
        0,
        "directory",
        &directory::encode_msg(&DirMsg::Set {
            key: "post-promote-founder".into(),
            value: "landed".into(),
        }),
    );
    poll_until(
        "the promoted friend to serve the founder's write",
        CONVERGE,
        || {
        cluster
            .query(
                1,
                "directory",
                &directory::encode_query(&DirQuery::Get {
                    key: "post-promote-founder".into(),
                }),
            )
            .and_then(|raw| directory::decode_reply(&raw).ok())
            .and_then(|r| match r {
                DirReply::Value(Some(v)) if v == "landed" => Some(()),
                _ => None,
            })
        },
    );
    // …and the promoted friend's own ordered lane finalizes into the widened
    // quorum (a halted founder can never land this).
    cluster.submit(
        1,
        "directory",
        &directory::encode_msg(&DirMsg::Set {
            key: "post-promote-friend".into(),
            value: "landed".into(),
        }),
    );
    poll_until("the founder to serve the friend's write", CONVERGE, || {
        cluster
            .query(
                0,
                "directory",
                &directory::encode_query(&DirQuery::Get {
                    key: "post-promote-friend".into(),
                }),
            )
            .and_then(|raw| directory::decode_reply(&raw).ok())
            .and_then(|r| match r {
                DirReply::Value(Some(v)) if v == "landed" => Some(()),
                _ => None,
            })
    });
}

// ---- duckfs joiner proof: full object possession over the REAL wire ---------
//
// the in-process `duckfs_resolver` test (statesync/tests) proves the resolver
// moves bytes at the module level. THIS proves it end to end over the real mesh
// transport: a founder holds non-trivial duckfs state (inline files, a putblob-
// staged chunk-object file, and a pin), a fresh node joins through the real
// `join`/`promote` ceremony, and its `sync_all_modules` pass loops GetObjects to
// FULL object possession over the p2p statesync lane before the joiner reads a
// file back BYTE-IDENTICAL. (`synced app_hash=` latches only after the resolver
// reports `possession_complete`, so the promoted read is proof bytes crossed.)

fn df_put_inline(path: &str, bytes: &[u8]) -> Change {
    Change::Put {
        path: path.into(),
        exec: false,
        meta: BTreeMap::new(),
        content: Content::Inline {
            b64: STANDARD.encode(bytes),
        },
    }
}

fn df_chunk_hex(bytes: &[u8]) -> String {
    to_hex(&object_id(Kind::Chunk, bytes))
}

/// a distinctive, non-uniform byte pattern (251 is prime — a truncated or
/// corrupt sync is caught, not masked by a run of a repeated byte).
fn df_pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn df_stat(cluster: &NetworkShapeCluster, idx: usize, path: &str) -> Option<EntryInfo> {
    let reply = cluster.query(
        idx,
        "files",
        &files_encode_query(&FilesQuery::Stat {
            path: path.into(),
            snapshot: None,
        }),
    )?;
    match files_decode_reply(&reply).ok()? {
        FilesReply::Stat(info) => info,
        _ => None,
    }
}

fn df_read(
    cluster: &NetworkShapeCluster,
    idx: usize,
    path: &str,
    offset: u64,
    len: u64,
) -> Option<Vec<u8>> {
    let reply = cluster.query(
        idx,
        "files",
        &files_encode_query(&FilesQuery::Read {
            path: path.into(),
            snapshot: None,
            offset,
            len,
        }),
    )?;
    match files_decode_reply(&reply).ok()? {
        FilesReply::Read { b64, .. } => STANDARD.decode(b64.as_bytes()).ok(),
        _ => None,
    }
}

fn df_refs(cluster: &NetworkShapeCluster, idx: usize) -> Option<RefsInfo> {
    let reply = cluster.query(idx, "files", &files_encode_query(&FilesQuery::Refs {}))?;
    match files_decode_reply(&reply).ok()? {
        FilesReply::Refs(info) => Some(info),
        _ => None,
    }
}

fn df_head(cluster: &NetworkShapeCluster, idx: usize) -> Option<String> {
    df_refs(cluster, idx)?.head
}

#[test]
fn network_shape_joiner_rebuilds_duckfs_over_the_wire() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("duckfs-joiner");
    assert!(
        !chain_id.is_empty(),
        "init should print the founded chain id"
    );
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // ---- seed non-trivial duckfs state on the founder BEFORE the join, so the
    //      boundary the friend syncs carries it -------------------------------
    // (1) two inline files in nested dirs, one commit off the empty tree.
    cluster.submit(
        0,
        "files",
        &files_encode_msg(&FilesMsg::Commit {
            base_snapshot: None,
            message: "seed inline".into(),
            changes: vec![
                df_put_inline("/shared/a", b"alpha"),
                df_put_inline("/shared/dir/b", b"beta"),
            ],
        }),
    );
    poll_until("founder inline files to finalize", CONVERGE, || {
        df_stat(&cluster, 0, "/shared/a").map(|_| ())
    });
    let s1 = poll_until("founder head after inline commit", CONVERGE, || {
        df_head(&cluster, 0)
    });

    // (2) a file whose bytes are STAGED as a chunk object via putblob and
    // referenced by digest in a Chunks commit — the odb-object path. 128 KiB is
    // a real, multi-page odb object that keeps THIS test about admission, not
    // payload size; the full-CHUNK_SIZE multi-chunk graph over the op path is
    // large_file_e2e's job (#215: the binary frame codec carries a 1 MiB chunk
    // whole). same-origin submits finalize in seq order.
    let chunk = df_pattern(128 * 1024);
    let chunk_size = chunk.len() as u64;
    cluster.submit(0, "files", &encode_putblob(&chunk));
    cluster.submit(
        0,
        "files",
        &files_encode_msg(&FilesMsg::Commit {
            base_snapshot: Some(s1.clone()),
            message: "seed chunked".into(),
            changes: vec![Change::Put {
                path: "/shared/big".into(),
                exec: false,
                meta: BTreeMap::new(),
                content: Content::Chunks {
                    size: chunk_size,
                    chunks: vec![df_chunk_hex(&chunk)],
                },
            }],
        }),
    );
    poll_until("founder chunked file to finalize", CONVERGE, || {
        df_stat(&cluster, 0, "/shared/big")
            .filter(|e| e.size == chunk_size)
            .map(|_| ())
    });
    let s2 = poll_until("founder head after chunked commit", CONVERGE, || {
        df_head(&cluster, 0)
    });

    // (3) pin the head — a gc root the joiner must reconstruct too.
    cluster.submit(
        0,
        "files",
        &files_encode_msg(&FilesMsg::Pin {
            snapshot: s2.clone(),
            name: "release".into(),
        }),
    );
    poll_until("founder pin to finalize", CONVERGE, || {
        df_refs(&cluster, 0)
            .filter(|r| r.pins.contains_key("release"))
            .map(|_| ())
    });
    // the source bytes the joiner must reconstruct byte-for-byte.
    let src_chunk =
        df_read(&cluster, 0, "/shared/big", 0, chunk_size).expect("founder read chunked");
    assert_eq!(src_chunk, chunk, "founder holds the seeded bytes");

    // ---- invite -> park -> promote the friend (real join verb, real mesh) ----
    let invite = cluster.invite();
    let friend_key = cluster.join_friend_manual(&invite);
    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode:", Duration::from_secs(60));
    cluster.wait_marker(1, "joining:", Duration::from_secs(60));

    let (ok, out) = cluster.run_promote(&friend_key);
    assert!(ok, "promote failed:\n{out}");
    assert!(
        out.contains("admitted"),
        "unexpected promote output:\n{out}"
    );

    cluster.wait_marker(0, "cutover complete: epoch 1", CONVERGE);
    cluster.wait_marker(1, "admitted at epoch 1", CONVERGE);
    // `synced app_hash=` latches only AFTER `sync_all_modules` -> the duckfs
    // resolver reaches FULL object possession over the real p2p statesync lane
    // (it loops GetObjects until `possession_complete`). this marker alone is the
    // production proof that every duckfs object crossed the wire.
    cluster.wait_marker(1, "synced app_hash=", CONVERGE);
    cluster.wait_marker(1, "promoted: validator at epoch 1", CONVERGE);

    // ---- THE property: the promoted joiner reads the founder's files back
    // BYTE-IDENTICAL. it holds them only because the possession loop moved every
    // chunk / file / tree / snapshot object over the wire and its post-promotion
    // reboot recovered them from disk; an empty odb errors on Read.
    let joined_chunk = poll_until("joiner to read the chunked file", CONVERGE, || {
        df_read(&cluster, 1, "/shared/big", 0, chunk_size)
    });
    assert_eq!(
        joined_chunk, chunk,
        "joiner rebuilt the chunked file byte-identical over the wire"
    );
    assert_eq!(
        df_read(&cluster, 1, "/shared/a", 0, 64).as_deref(),
        Some(b"alpha".as_ref()),
        "joiner rebuilt inline /shared/a"
    );
    assert_eq!(
        df_read(&cluster, 1, "/shared/dir/b", 0, 64).as_deref(),
        Some(b"beta".as_ref()),
        "joiner rebuilt nested inline /shared/dir/b"
    );
    // head and pin (the refs image) match the source's exactly.
    let refs = df_refs(&cluster, 1).expect("joiner refs");
    assert_eq!(
        refs.head.as_deref(),
        Some(s2.as_str()),
        "joiner head matches the source"
    );
    assert_eq!(
        refs.pins.get("release").map(String::as_str),
        Some(s2.as_str()),
        "joiner pin matches the source"
    );
}

/// the STAGED admission flow end-to-end: invite → resident (mesh + pre-sync,
/// NO quorum seat) → promote → validator. the payoff assertions are the
/// quorum ones the one-step flow could never make:
///
///   1. while the friend holds resident standing, the valset's VALIDATOR set
///      still names only the founder — committed state proves the tier split;
///   2. the resident SERVES: local reads (rpc + http) answer from its own
///      pre-synced boundary, a write lands through the submit relay (and the
///      resident reads its own write back), and a value the founder finalizes
///      becomes readable through the resident's surface (the continuous
///      follow);
///   3. the chain keeps finalizing with the resident KILLED (under the old
///      one-step flow the friend would already hold a quorum seat here, and a
///      2-member quorum with one member down is a stall);
///   4. a restarted resident parks straight back into resident mode (the
///      pre-sync writes no checkpoint manifest — a reboot is clean) and serves
///      again;
///   5. `resident remove` REVOKES standing through the same ceremony: committed
///      residents empty, the node falls back to a parked joiner, and a second
///      run is an honest no-op;
///   6. `resident accept` re-grants, and the resident resumes the follow (a
///      post-re-grant write becomes readable through its surface);
///   7. `promote` then seats a WARM validator through the normal path.
///
#[test]
fn staged_admission_resident_presyncs_then_promotes_warm() {
    use directory::{DirMsg, DirQuery, DirReply};
    use valset::{ValsetQuery, ValsetReply};

    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("staged-admission");
    assert!(
        !chain_id.is_empty(),
        "init should print the founded chain id"
    );
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

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

    // ---- invite → park → resident grant ------------------------------------
    let invite = cluster.invite();
    let friend_key = cluster.join_friend_manual(&invite);
    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode:", Duration::from_secs(60));

    let (ok, text) = cluster.run_membership_verb("resident accept", &friend_key);
    assert!(ok, "resident accept failed:\n{text}");
    assert!(
        text.contains("granted resident standing"),
        "unexpected resident accept output:\n{text}"
    );

    // the grant's boundary admits the resident to the mesh; the parked node
    // then pre-syncs.
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    // (1) the tier split in COMMITTED state: validators = founder only,
    //     residents = the friend.
    let validators = cluster
        .query(0, "valset", &valset::encode_query(&ValsetQuery::Validators))
        .and_then(|raw| valset::decode_reply(&raw).ok())
        .map(|r| match r {
            ValsetReply::Validators(v) => v,
            other => panic!("expected Validators, got {other:?}"),
        })
        .expect("valset validators readable");
    assert_eq!(
        validators.len(),
        1,
        "the quorum still seats ONLY the founder"
    );
    let residents = cluster
        .query(0, "valset", &valset::encode_query(&ValsetQuery::Residents))
        .and_then(|raw| valset::decode_reply(&raw).ok())
        .map(|r| match r {
            ValsetReply::Residents(v) => v,
            other => panic!("expected Residents, got {other:?}"),
        })
        .expect("valset residents readable");
    assert_eq!(
        residents,
        vec![common::unhex(&friend_key)],
        "the friend holds resident standing"
    );

    // (2) the SERVING resident: the same local read surfaces a validator
    //     binds, answered from the resident's own pre-synced boundary.
    //     rpc status names the served boundary…
    poll(
        "the resident to serve rpc status",
        Box::new(|| {
        let st = cluster.rpc(1, serde_json::json!({ "cmd": "status" }));
        st["ok"] == serde_json::json!(true)
            && st["status"]["height"].as_u64().is_some_and(|h| h > 0)
        }),
    );
    //     …module reads answer from the RESIDENT's surface (the tier split is
    //     visible through the resident itself, not just the founder)…
    poll(
        "the resident to serve valset reads",
        Box::new(|| {
        cluster
            .query(1, "valset", &valset::encode_query(&ValsetQuery::Residents))
            .and_then(|raw| valset::decode_reply(&raw).ok())
                .is_some_and(|r| {
                    matches!(
                r,
                ValsetReply::Residents(v) if v == vec![common::unhex(&friend_key)]
                    )
                })
        }),
    );
    //     …the http app surface answers its status route from the same host…
    {
        let (status, body) = nettest::http_text(cluster.http_ports[1], "GET", "/v1/status");
        assert_eq!(status, 200, "resident /v1/status must answer 200:\n{body}");
        assert!(
            body.contains("\"height\""),
            "resident /v1/status carries a height:\n{body}"
        );
    }
    //     …a write LANDS through the submit relay: the resident signs with its
    //     own key, ships the frame to the validator, and the rpc reply holds
    //     until the frame finalizes (ok == Applied)…
    let landed = cluster.rpc(
        1,
        serde_json::json!({
            "cmd": "submit",
            "target": "directory",
            "payload_hex": common::hex(&directory::encode_msg(&DirMsg::Set {
                key: "resident-writes".into(),
                value: "landed".into(),
            })),
        }),
    );
    assert_eq!(
        landed["ok"],
        serde_json::json!(true),
        "the resident submit should relay + finalize (ok == Applied): {landed}"
    );
    //     …and the resident READS ITS OWN WRITE once its follow arm crosses
    //     the boundary that carries it…
    poll(
        "the resident to serve its own relayed write",
        Box::new(|| {
        cluster
            .query(
                1,
                "directory",
                &directory::encode_query(&DirQuery::Get {
                    key: "resident-writes".into(),
                }),
            )
            .and_then(|raw| directory::decode_reply(&raw).ok())
            .is_some_and(|r| matches!(r, DirReply::Value(Some(v)) if v == "landed"))
        }),
    );
    //     …and the follow is CONTINUOUS: a value the founder finalizes now
    //     becomes readable through the resident within a few boundaries.
    cluster.submit(
        0,
        "directory",
        &directory::encode_msg(&DirMsg::Set {
            key: "resident-follow".into(),
            value: "fresh".into(),
        }),
    );
    poll(
        "the resident to serve the followed write",
        Box::new(|| {
        cluster
            .query(
                1,
                "directory",
                &directory::encode_query(&DirQuery::Get {
                    key: "resident-follow".into(),
                }),
            )
            .and_then(|raw| directory::decode_reply(&raw).ok())
            .is_some_and(|r| matches!(r, DirReply::Value(Some(v)) if v == "fresh"))
        }),
    );
    //     …and the DERIVED tier follows the boundary too: the explorer
    //     records the followed boundary (an honest boundary row — verified
    //     height + app-hash, frame-derived fields empty)…
    poll(
        "the resident explorer to record a followed boundary",
        Box::new(|| {
            let (status, body) =
                common::http_request(cluster.http_ports[1], "GET", "/v1/blocks", None);
        status == 200
            && body["blocks"].as_array().is_some_and(|rows| {
                rows.iter().any(|b| {
                    b["hash"] == serde_json::json!("")
                        && b["height"].as_u64().is_some_and(|h| h > 0)
                        && !b["commit_hash"].as_str().unwrap_or_default().is_empty()
                })
            })
        }),
    );
    //     …and /v1/index/* answers from healthy read models. under the
    //     replica pipeline the resident FOLDS blocks, so watermarks advance
    //     per block PAST the ascension heal's backfill floor (the old
    //     boundary-healed model pinned them equal — that trailing-watermark
    //     era is exactly what the fold retired). polled: a heal drops the
    //     watermark FIRST (crash-safety by re-trigger), so a read racing an
    //     in-flight heal legitimately sees 0 for a moment.
    poll(
        "the resident index to report folding watermarks",
        Box::new(|| {
        let (status, index_status) =
            common::http_request(cluster.http_ports[1], "GET", "/v1/index/status", None);
        let watermark = index_status["modules"]["directory"].as_u64().unwrap_or(0);
        status == 200
            && index_status["poisoned"] == serde_json::json!(false)
            && watermark > 0
            && index_status["backfilled"]["directory"]
                .as_u64()
                .is_some_and(|floor| floor <= watermark)
        }),
    );

    // (3) quorum untouched: kill the resident; the founder keeps finalizing.
    cluster.kill(1);
    cluster.submit(
        0,
        "directory",
        &directory::encode_msg(&DirMsg::Set {
            key: "resident-down-liveness".into(),
            value: "alive".into(),
        }),
    );
    poll(
        "a finalized op with the resident down",
        Box::new(|| {
        cluster
            .query(
                0,
                "directory",
                &directory::encode_query(&DirQuery::Get {
                    key: "resident-down-liveness".into(),
                }),
            )
            .and_then(|raw| directory::decode_reply(&raw).ok())
            .is_some_and(|r| matches!(r, DirReply::Value(Some(_))))
        }),
    );

    // (4) a restarted resident parks straight back into resident mode — the
    //     pre-sync left NO checkpoint manifest behind. (the config-time
    //     joiner banner may not reprint: the first run's recovery-journal
    //     files flip the cheap boot probe, and the runtime then re-decides
    //     from the real store — the resident marker alone is the proof.)
    //     it then SERVES again from a fresh pre-sync.
    cluster.spawn(1);
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);
    poll(
        "the restarted resident to serve reads again",
        Box::new(|| {
            cluster.rpc(1, serde_json::json!({ "cmd": "status" }))["ok"] == serde_json::json!(true)
        }),
    );

    // (5) resident remove: the ceremony verb revokes standing. committed
    //     state clears, and the resident — whose respawned log is fresh, so
    //     the parked marker is unambiguously post-revoke — falls back to a
    //     parked joiner at the boundary whose manifest drops it.
    let (ok, out) = cluster.run_membership_verb("resident remove", &friend_key);
    assert!(ok, "resident remove failed:\n{out}");
    assert!(
        out.contains("revoked resident standing"),
        "unexpected resident remove output:\n{out}"
    );
    poll(
        "the revoke to clear resident standing",
        Box::new(|| {
        cluster
            .query(0, "valset", &valset::encode_query(&ValsetQuery::Residents))
            .and_then(|raw| valset::decode_reply(&raw).ok())
            .is_some_and(|r| matches!(r, ValsetReply::Residents(v) if v.is_empty()))
        }),
    );
    cluster.wait_marker(1, "joining: awaiting redemption", CONVERGE);
    //     a second run is an honest no-op — the inverted guard, end to end.
    let (ok, out) = cluster.run_membership_verb("resident remove", &friend_key);
    assert!(ok, "resident remove (no standing) failed:\n{out}");
    assert!(
        out.contains("holds no resident standing"),
        "unexpected no-op resident remove output:\n{out}"
    );

    // (6) re-grant: resident accept restores standing and the resident resumes
    //     the follow — a write finalized AFTER the re-grant becomes readable
    //     through the resident's own surface (stale serves can't fake this:
    //     the revoked node never synced a boundary carrying this key).
    let (ok, out) = cluster.run_membership_verb("resident accept", &friend_key);
    assert!(ok, "re-grant resident accept failed:\n{out}");
    assert!(
        out.contains("granted resident standing"),
        "unexpected re-grant output:\n{out}"
    );
    poll(
        "the re-grant to restore resident standing",
        Box::new(|| {
        cluster
            .query(0, "valset", &valset::encode_query(&ValsetQuery::Residents))
            .and_then(|raw| valset::decode_reply(&raw).ok())
                .is_some_and(|r| {
                    matches!(
                r,
                ValsetReply::Residents(v) if v == vec![common::unhex(&friend_key)]
                    )
                })
        }),
    );
    cluster.submit(
        0,
        "directory",
        &directory::encode_msg(&DirMsg::Set {
            key: "post-revoke-follow".into(),
            value: "back".into(),
        }),
    );
    poll(
        "the re-granted resident to resume the follow",
        Box::new(|| {
        cluster
            .query(
                1,
                "directory",
                &directory::encode_query(&DirQuery::Get {
                    key: "post-revoke-follow".into(),
                }),
            )
            .and_then(|raw| directory::decode_reply(&raw).ok())
            .is_some_and(|r| matches!(r, DirReply::Value(Some(v)) if v == "back"))
        }),
    );

    // (7) promote: the warm resident becomes a validator through the replica
    //     promotion collapse — it checkpoints its OWN folded state and
    //     reboots (no re-sync: at a quorum-widening cutover the founder
    //     halts awaiting this very node, so there is nothing to sync FROM);
    //     valset Join clears its resident standing.
    let (ok, out) = cluster.run_promote(&friend_key);
    assert!(ok, "promote failed:\n{out}");
    assert!(
        out.contains("admitted"),
        "unexpected promote output:\n{out}"
    );
    cluster.wait_marker(1, "admitted at epoch", CONVERGE);
    cluster.wait_marker(1, "promoted: validator at epoch", CONVERGE);
    let residents = cluster
        .query(0, "valset", &valset::encode_query(&ValsetQuery::Residents))
        .and_then(|raw| valset::decode_reply(&raw).ok())
        .map(|r| match r {
            ValsetReply::Residents(v) => v,
            other => panic!("expected Residents, got {other:?}"),
        })
        .expect("valset residents readable");
    assert!(
        residents.is_empty(),
        "promotion must clear resident standing (got {residents:?})"
    );
}
