//! Untrusted snapshots preserve model ownership, live sessions and queued work.
mod support;
use futures::executor::block_on;
use sdk::{Module, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use support::*;

fn module() -> runs::RunsModule {
    runs::RunsModule::new("runs", "chat", "saga", "attribution", "dispatch", "agent", Some("tasks".into()), Some("tasks".into()))
}

async fn source() -> (Vec<u8>, StateRoot, Network) {
    let mut network = Network::new().await;
    let run = network.provision().await;
    network.submit(session(), msg("runs", &runs::RunsMsg::AgentAction {
        run_id: run, action: runs::AgentAction::CreateTask { task_id: "pending".into(), title: "persisted request".into() },
    })).await;
    let (snapshot, _) = network.host.capture_current_snapshot(network.height, host::CapturePayloads::All, || std::time::Duration::ZERO);
    let runs = snapshot.module("runs").unwrap();
    let StateSyncHandle::SnapshotBytes(bytes) = &runs.state_sync else { panic!("runs snapshot bytes"); };
    (bytes.clone(), runs.root, network)
}

#[test]
fn installing_real_pending_state_preserves_queries_and_queued_items() {
    block_on(async {
        let (bytes, root, network) = source().await;
        let mut restored = module();
        restored.install(&bytes, root).unwrap();
        assert_eq!(restored.snapshot(), bytes);
        assert_eq!(restored.root(), root);
        for query in [runs::RunsQuery::PendingRuns, runs::RunsQuery::AgentSessions, runs::RunsQuery::Model { query: runs::ModelQuery::Agents }] {
            let query = runs::encode_query(&query);
            assert_eq!(restored.query(&query).await.unwrap(), network.host.query("runs", &query).await.unwrap());
        }
        let items = restored.pending_items().await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].target, "runs");
        assert!(matches!(items[0].cause, sdk::Cause::Chain { hop: sdk::Hop::Delivery(_), .. }));
    });
}

#[test]
fn malformed_snapshots_never_replace_existing_state() {
    block_on(async {
        let (bytes, root, _) = source().await;
        let mut restored = module();
        restored.install(&bytes, root).unwrap();
        let mut padded = bytes.clone();
        padded.push(0);
        let mut bad_length = bytes.clone();
        bad_length[..8].copy_from_slice(&u64::MAX.to_le_bytes());
        let mut bad_json = bytes.clone();
        bad_json[8] = b'!';
        for bad in [bytes[..bytes.len()-1].to_vec(), padded, bad_length, bad_json] {
            let authenticated_bad = StateRoot(Sha256::digest(&bad).into());
            assert!(restored.install(&bad, authenticated_bad).is_err());
            assert_eq!(restored.snapshot(), bytes);
        }
        assert!(restored.install(&bytes, StateRoot([0; 32])).is_err());
        assert_eq!(restored.root(), root);
    });
}
