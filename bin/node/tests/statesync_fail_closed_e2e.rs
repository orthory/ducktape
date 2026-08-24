//! statesync fail-closed (ADR §5.1, the server half of R4): a validator refuses
//! statesync/manifest service to any key WITHOUT committed standing (validators ∪
//! residents), so a valid targeted invite alone leaks ZERO chain state.
//!
//! The decisive fact this pins is that TRANSPORT reachability is NOT enough:
//! mesh admission and committed standing are separate facts — a peer the mesh
//! authorizes may hold no standing at all — so a transport gate cannot tell
//! them apart. Enforcement is a REQUEST-LEVEL real-key proof
//! checked against committed standing. This test drives a peer that is EVEN MORE
//! privileged than an invite holder — its key is in the founder's descriptor
//! mesh (`peer_seeds`), so the mesh authorizes it and it CONNECTS — yet it holds
//! no committed standing (not a validator, not a resident). It attempts a real
//! `--sync-only` manifest fetch and is REFUSED every time: it connects, retries,
//! and never obtains a boundary. An invite holder (not even in the descriptor
//! mesh) is refused a fortiori.
//!
//! The two must-not-break directions — the restore path (a validator restart:
//! `restart_e2e`) and an admitted resident's boundary pre-sync
//! (`live_admission_e2e`) — dial under REAL keys that ARE in committed standing,
//! and their own suites prove they still sync. Here we assert only the refusal
//! the fail-closed gate newly imposes.

mod common;

use std::time::Duration;

use common::{Cluster, poll_until};
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};

#[test]
fn a_non_standing_peer_is_refused_statesync() {
    let _guard = common::serial();
    // node 0 is the sole validator; node 1 is a mesh PEER (in `peer_seeds`, so
    // the founder's descriptor mesh authorizes its transport) that is NOT in the
    // validator set and holds no resident grant — i.e. no committed standing.
    let mut cluster = Cluster::new(&[0, 1], &[0]);
    cluster.spawn(0);
    cluster.wait_marker(0, "genesis root_hash=", Duration::from_secs(60));

    // give the founder REAL, servable state and a finalized boundary, so the
    // ONLY reason node 1 cannot obtain a manifest is the fail-closed refusal —
    // not a server that simply has nothing to serve yet.
    cluster.submit(0, "directory", &encode_msg(&DirMsg::Set {
        key: "secret".into(),
        value: "chain-state".into(),
    }));
    poll_until("the founder's write to finalize", Duration::from_secs(30), || {
        cluster
            .query(0, "directory", &encode_query(&DirQuery::Get { key: "secret".into() }))
            .and_then(|raw| decode_reply(&raw).ok())
            .and_then(|r| match r {
                DirReply::Value(Some(v)) if v == "chain-state" => Some(()),
                _ => None,
            })
    });

    // node 1 (mesh-reachable, NON-STANDING) runs `--sync-only`: it connects to
    // the founder's statesync channel and loops `fetch_manifest`, but every
    // request is dropped by the fail-closed gate. so the run TIMES OUT with no
    // served boundary.
    let (ok, log) = cluster.run_sync_only(1, Duration::from_secs(35));
    assert!(
        !ok,
        "a NON-STANDING peer must be REFUSED statesync (fail-closed, ADR §5.1); it \
         instead synced:\n{log}"
    );
    assert!(
        !log.contains("synced root_hash="),
        "no boundary may be served to a standing-less key:\n{log}"
    );
    // it DID reach the wire and get refused (not merely fail to connect): the
    // sync-only loop prints this line on each dropped `fetch_manifest`.
    assert!(
        log.contains("manifest not ready"),
        "the non-standing peer should connect and be refused on every manifest \
         fetch (proving the SERVE gate, not a transport failure):\n{log}"
    );
}
