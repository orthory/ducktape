//! the network state-sync property at the crate level: a joiner rebuilds a
//! REAL qmdb-backed module purely through the wire protocol — manifest fetch,
//! live target fetch, proof-carrying op-range fetches through the remote
//! resolver — and lands on the source's exact root, with byzantine responses
//! rejected by verification rather than trust.
//!
//! the transport here is an in-process request/response channel pair: the
//! same bytes, framing, and client code a p2p channel carries, minus sockets.
//! (the real-mesh run lives in the node binary's demo script; the DETERMINISTIC
//! proof of the protocol lives here.)

use futures::channel::{mpsc, oneshot};
use futures::{SinkExt as _, StreamExt as _};
use host::{FinalizedBlock, Host};
use kv::Kv;
use kv_interface::{encode as kv_encode, KvMsg};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use statesync::qmdb::RemoteQmdbResolver;
use statesync::{
    decode_response, encode_request, fetch_manifest, fetch_snapshot, PayloadKind, SyncClient,
    SyncError, SyncRequest, SyncResponse, SyncServer, CHUNK_LEN,
};

use commonware_runtime::{deterministic, Runner as _, Supervisor as _};

// ============================================================================
// the in-process transport: Send client futures, local server loop.
// ============================================================================

type RpcPair = (Vec<u8>, oneshot::Sender<Vec<u8>>);

#[derive(Clone)]
struct ChannelClient {
    tx: mpsc::Sender<RpcPair>,
}

impl SyncClient for ChannelClient {
    fn request(
        &self,
        req: SyncRequest,
    ) -> impl std::future::Future<Output = Result<SyncResponse, SyncError>> + Send {
        let mut tx = self.tx.clone();
        async move {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send((encode_request(&req), reply_tx))
                .await
                .map_err(|e| SyncError::Transport(format!("request channel closed: {e}")))?;
            let bytes = reply_rx
                .await
                .map_err(|_| SyncError::Transport("server dropped the reply".into()))?;
            Ok(decode_response(&bytes)?)
        }
    }
}

/// run the server side over `host` until every client sender is dropped.
async fn serve_until_closed(
    server: &mut SyncServer,
    host: &Host,
    finalized: FinalizedBlock,
    mut rx: mpsc::Receiver<RpcPair>,
) {
    while let Some((frame, reply)) = rx.next().await {
        let resp = server.handle_frame(host, Some(finalized), &frame).await;
        // a dropped reply receiver just means the client gave up — server-side
        // that is not an error.
        let _ = reply.send(resp);
    }
}

// ============================================================================
// a snapshot-lane module with a payload bigger than one chunk.
// ============================================================================

struct BigSnapshot {
    bytes: Vec<u8>,
}

impl BigSnapshot {
    fn new() -> Self {
        // deterministic, incompressible-ish pattern spanning ~2.7 chunks so the
        // chunk loop takes multiple round trips and an exact tail slice.
        let len = CHUNK_LEN * 2 + CHUNK_LEN / 2 + 7;
        let bytes = (0..len).map(|i| (i * 31 + 7) as u8).collect();
        Self { bytes }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for BigSnapshot {
    fn id(&self) -> ModuleId {
        "bigsnap".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot([0xBB; 32])
    }
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.bytes.clone()))
    }
    async fn execute(&mut self, _c: &mut dyn Ctx, _m: &Msg) -> Result<(), Error> {
        Ok(())
    }
}

// ============================================================================
// the tests
// ============================================================================

#[test]
fn joiner_rebuilds_kv_over_the_wire_protocol() {
    deterministic::Runner::default().start(|context| async move {
        // ---- SOURCE: real committed kv content through the host op path ----
        let kv = Kv::init(context.child("source_kv"), "kv").await;
        let mut host = Host::genesis(vec![Box::new(kv), Box::new(BigSnapshot::new())])
            .expect("genesis");

        let set = |k: &[u8], v: &[u8]| Msg {
            target: "kv".into(),
            payload: kv_encode(&KvMsg::Set { key: k.to_vec(), value: v.to_vec() }),
        };
        host.submit(set(b"greeting", b"hello world")).await.expect("op 1");
        host.submit(set(b"motd", b"draft")).await.expect("op 2");
        // overwrite: op-log order matters — only real sync reproduces the root.
        host.submit(set(b"motd", b"final")).await.expect("op 3");

        let finalized = FinalizedBlock { height: 3, app_hash: host.app_hash() };
        let src_kv_root = host.module_root("kv").expect("kv registered");

        // ---- the wire: server loop + channel client -------------------------
        let (tx, rx) = mpsc::channel::<RpcPair>(16);
        let client = ChannelClient { tx };
        let mut server = SyncServer::new();

        let joiner_ctx = context.child("joiner_kv");
        let client_for_join = client.clone();
        let join_side = async move {
            // manifest: the captured boundary describes every module.
            let manifest = fetch_manifest(&client_for_join).await.expect("manifest");
            assert_eq!(manifest.height, 3);
            assert_eq!(manifest.app_hash, finalized.app_hash);
            let kv_entry = manifest.entry("kv").expect("kv in manifest");
            assert_eq!(kv_entry.kind, PayloadKind::Resolver);
            assert_eq!(kv_entry.root, src_kv_root);

            // live target through the module lane, gated on the manifest root.
            let resolver = RemoteQmdbResolver::new(client_for_join.clone(), "kv");
            let target = resolver.fetch_target().await.expect("target");
            assert_eq!(
                StateRoot(target.root.0),
                kv_entry.root,
                "quiescent source: live target root equals the manifest root"
            );

            // rebuild ENTIRELY through the wire: every op batch crosses the
            // channel as proof-carrying bytes and is merkle-verified.
            let rebuilt = Kv::sync_from(joiner_ctx, "kv-rebuilt", target, resolver).await;
            assert_eq!(rebuilt.root(), src_kv_root, "synced root == source root");
            assert_eq!(
                rebuilt.get(b"motd").await.as_deref(),
                Some(b"final".as_ref()),
                "overwrite history replayed in op-log order"
            );
            assert_eq!(
                rebuilt.get(b"greeting").await.as_deref(),
                Some(b"hello world".as_ref())
            );

            // multi-chunk snapshot lane: bytes reassemble exactly.
            let snap = fetch_snapshot(&client_for_join, manifest.height, "bigsnap")
                .await
                .expect("big snapshot");
            assert_eq!(snap, BigSnapshot::new().bytes, "chunked payload reassembles");
            drop(client_for_join); // close the channel so the server loop ends.
        };

        let server_side = serve_until_closed(&mut server, &host, finalized, rx);
        // the extra `client` clone must drop too or the server loop never ends.
        drop(client);
        futures::join!(join_side, server_side);
    });
}

#[test]
fn stale_capture_requests_are_refused_not_mis_served() {
    deterministic::Runner::default().start(|context| async move {
        let kv = Kv::init(context.child("kv"), "kv").await;
        let mut host = Host::genesis(vec![Box::new(kv)]).expect("genesis");
        host.submit(Msg {
            target: "kv".into(),
            payload: kv_encode(&KvMsg::Set { key: b"k".to_vec(), value: b"v".to_vec() }),
        })
        .await
        .expect("op");
        let finalized = FinalizedBlock { height: 1, app_hash: host.app_hash() };

        let mut server = SyncServer::new();
        // a chunk request against a height never captured must error cleanly.
        let resp = server
            .handle(
                &host,
                Some(finalized),
                SyncRequest::Chunk { height: 999, module_id: "kv".into(), offset: 0 },
            )
            .await;
        assert!(
            matches!(resp, SyncResponse::Error(ref e) if e.contains("no capture")),
            "unknown capture height must be a clean protocol error, got {resp:?}"
        );

        // manifest against a WRONG finalized boundary (host has advanced past
        // it) is refused by the host's app-hash gate, not served stale.
        let wrong = FinalizedBlock { height: 0, app_hash: StateRoot([1u8; 32]) };
        let resp = server.handle(&host, Some(wrong), SyncRequest::Manifest).await;
        assert!(
            matches!(resp, SyncResponse::Error(ref e) if e.contains("capture failed")),
            "a mismatched boundary must refuse, got {resp:?}"
        );
    });
}

#[test]
fn byzantine_op_batches_fail_verification_not_installation() {
    // a lying server can transport-tamper exactly one thing: response bytes.
    // decode-level tampering (truncation, trailing bytes, forged counts) must
    // reject at the wire layer; this pins the decode side. (proof-level lies —
    // valid encoding, wrong ops — are rejected by the sync engine's merkle
    // verification against the target root, proven by commonware's own suite.)
    use statesync::qmdb::{decode_ops_envelope, encode_qmdb_req, QmdbSyncReq};

    deterministic::Runner::default().start(|context| async move {
        let kv = Kv::init(context.child("kv"), "kv").await;
        let mut host = Host::genesis(vec![Box::new(kv)]).expect("genesis");
        host.submit(Msg {
            target: "kv".into(),
            payload: kv_encode(&KvMsg::Set { key: b"k".to_vec(), value: b"v".to_vec() }),
        })
        .await
        .expect("op");

        // real op coordinates come from the store's own target — the log
        // carries commit-floor ops beyond the user writes, and serving below
        // the pruned floor is refused.
        let target_bytes = host
            .serve_sync("kv", &encode_qmdb_req(&QmdbSyncReq::Target))
            .await
            .expect("serve target");
        let target = statesync::qmdb::decode_target(&target_bytes).expect("decode target");

        // an honest ops envelope straight off the serve lane...
        let body = host
            .serve_sync(
                "kv",
                &encode_qmdb_req(&QmdbSyncReq::Ops {
                    op_count: target.range.end().as_u64(),
                    start_loc: target.range.start().as_u64(),
                    max_ops: 16,
                    include_pinned: true,
                }),
            )
            .await
            .expect("serve ops");
        assert!(decode_ops_envelope(&body).is_ok(), "honest envelope decodes");

        // ...rejects when truncated,
        assert!(decode_ops_envelope(&body[..body.len() - 1]).is_err());
        // ...when carrying trailing garbage,
        let mut trailing = body.clone();
        trailing.push(0);
        assert!(decode_ops_envelope(&trailing).is_err());
        // ...and when its op count is forged past the buffer.
        let mut forged = body.clone();
        let proof_len = u64::from_le_bytes(forged[0..8].try_into().unwrap()) as usize;
        let count_at = 8 + proof_len;
        forged[count_at..count_at + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode_ops_envelope(&forged).is_err());
    });
}
