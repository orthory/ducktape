//! Epoch-engine construction and recovery resume.
//!
//! `EpochSpawner` owns the pre-registered channel bank so boot and live
//! cutover use one engine-construction path.

use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_consensus::types::Epoch;
use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::authenticated::discovery;
use commonware_runtime::Supervisor;
use commonware_utils::ordered::Set;

use consensus::{ConsensusScheme, ContentStore, SimplexOrderer};
use directory::{DirMsg, encode_msg};
use host::Host;
use node::OrderedNode;
use recovery::Recovery;
use sdk::Msg;

use crate::constants::{CONSENSUS_SCHEME, CUTOVER_DELAY, EPOCH_CHANNEL_BANK};
use crate::host_reads::resume_resident_keys;
use crate::util::{diag_log, epoch_floor, hex};

pub(super) struct EpochSpawner<'a> {
    context: &'a commonware_runtime::tokio::Context,
    oracle: discovery::Oracle<ed25519::PublicKey>,
    signer: ed25519::PrivateKey,
    namespace: Vec<u8>,
    label: String,
    bank_base: u64,
    channel_bank: super::ChannelBank,
}

impl<'a> EpochSpawner<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        context: &'a commonware_runtime::tokio::Context,
        oracle: discovery::Oracle<ed25519::PublicKey>,
        signer: ed25519::PrivateKey,
        namespace: Vec<u8>,
        label: String,
        bank_base: u64,
        channel_bank: super::ChannelBank,
    ) -> Self {
        Self {
            context,
            oracle,
            signer,
            namespace,
            label,
            bank_base,
            channel_bank,
        }
    }

    pub(super) fn spawn(
        &mut self,
        epoch: u64,
        participants: Set<ed25519::PublicKey>,
        store: ContentStore,
        floor_bytes: Option<Vec<u8>>,
    ) -> SimplexOrderer {
        let slot = self.channel_bank
        .get_mut(epoch.checked_sub(self.bank_base).expect("epochs never rebase down") as usize)
        .and_then(|s| s.take())
        .unwrap_or_else(|| {
            eprintln!(
                "[node {}] FATAL: epoch {epoch} exhausts the pre-registered                          channel bank ({EPOCH_CHANNEL_BANK}) — rebuild with a wider bank", self.label
            );
            std::process::exit(1);
        });
        let (vote, certificate, resolver, payload, fetch) = slot;
        let scheme = match CONSENSUS_SCHEME {
            ConsensusScheme::V1Ed25519 => {
                simplex_ed25519::Scheme::signer(&self.namespace, participants, self.signer.clone())
                    .expect("our key is in the validator participant set")
            }
            // the engine and tests are V2-capable (see consensus::BlsScheme);
            // wiring V2 into the epoch respawn machinery needs the bls
            // participant BiMap derived per epoch (valset-registered bls
            // keys + proof-of-possession) — fail-stop until that lands.
            ConsensusScheme::V2Bls => {
                unimplemented!(
                    "V2Bls node wiring lands with valset bls key registration; \
                 the consensus engine itself is V2-capable"
                )
            }
        };
        // a SAME-EPOCH respawn passes the persisted finalization floor so
        // the reopened journal's replay does not re-report history the
        // recovered state already contains. a damaged floor FAILS — a
        // silent genesis-floor fallback would resurrect the wedge.
        let floor =
            floor_bytes.map(
                |bytes| match consensus::decode_finalization(&scheme, &bytes) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("[node {}] FATAL: {e}", self.label);
                        std::process::exit(1);
                    }
                },
            );
        let label: &'static str = Box::leak(format!("consensus_e{epoch}").into_boxed_str());
        // spawn WITH the lazy payload-fetch backstop: quorum is a SUBSET
        // (n - floor((n-1)/3)), so a validator can finalize a view it never
        // voted in — and if it also missed the one-shot relay gossip (mesh
        // still forming, transient disconnect), relay-only wiring would
        // silently drop that op's slot and wedge/fork the node. the
        // resolver fetches missing bytes by digest from the tracked mesh
        // (the oracle is provider AND blocker) and fills the ordered slot.
        SimplexOrderer::spawn_with_resolver(
            self.context.child(label),
            scheme,
            self.oracle.clone(),
            self.oracle.clone(),
            self.signer.public_key(),
            format!("{}-e{epoch}", self.signer.public_key()),
            Epoch::new(epoch),
            epoch_floor(&self.namespace, epoch),
            floor,
            // per-process, PER-EPOCH content store: pins/pending of a torn
            // down epoch die with it (in-flight ops are resubmitted). a
            // RESTART's store arrives pre-seeded from the recovery journal.
            store,
            vote,
            certificate,
            resolver,
            payload,
            fetch,
            false,
        )
    }
}

pub(super) struct EngineState {
    pub(super) node: OrderedNode<SimplexOrderer, Recovery<commonware_runtime::tokio::Context>>,
    pub(super) orchestrator: consensus::ValsetOrchestrator<ed25519::PublicKey>,
    pub(super) last_cert_height: Option<u64>,
    pub(super) latest_floor: Option<recovery::FloorCert>,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn resume(
    epoch_spawner: &mut EpochSpawner<'_>,
    host: Host,
    recovery: Recovery<commonware_runtime::tokio::Context>,
    resumed: Option<&recovery::Recovered>,
    member_keys: &[ed25519::PublicKey],
    participants: &Set<ed25519::PublicKey>,
    resume_epoch: u64,
    pending_boot: Option<u64>,
    signer: &ed25519::PrivateKey,
    label: &str,
    dev_demo: bool,
) -> EngineState {
    // the boot store: seeded with every retained journaled frame so
    // finalizations the reopened engine re-reports (at most the floor
    // cert itself, plus anything finalized-but-undrained at the crash)
    // resolve locally instead of wedging the ordered gate.
    let boot_store = ContentStore::new();
    if let Some(rec) = &resumed {
        for frame in &rec.frames {
            boot_store.pin(frame.clone());
        }
    }
    // the persisted floor is only valid for the epoch it was recorded in
    // (Floor::assert pins the certificate to the engine's epoch).
    let boot_floor = match recovery.floor_cert() {
        Ok(cert) => cert.filter(|c| c.epoch == resume_epoch),
        Err(e) => {
            eprintln!("[node {label}] FATAL: persisted finalization floor is damaged: {e}");
            std::process::exit(1);
        }
    };
    let last_cert_height = boot_floor.as_ref().map(|c| c.height);
    // the newest persisted finalization floor, kept in memory so the
    // statesync service can serve it to joiners at a matching boundary.
    let latest_floor: Option<recovery::FloorCert> = boot_floor.clone();
    let recovered_height = resumed
        .as_ref()
        .and_then(|rec| rec.height)
        .map(|height| height.to_string())
        .unwrap_or_else(|| "none".to_string());
    let recovered_hash = resumed
        .as_ref()
        .map(|rec| hex(&rec.app_hash))
        .unwrap_or_else(|| "none".to_string());
    let replayed = resumed.as_ref().map(|rec| rec.applied).unwrap_or(0);
    let boot_floor_height = latest_floor
        .as_ref()
        .map(|floor| floor.height.to_string())
        .unwrap_or_else(|| "none".to_string());
    diag_log(format!(
        "DIAG promotion_recovered recovered_height={} recovered_hash={} replayed={} \
     boot_floor_height={}",
        recovered_height, recovered_hash, replayed, boot_floor_height
    ));
    let orderer = epoch_spawner.spawn(
        resume_epoch,
        participants.clone(),
        boot_store,
        boot_floor.map(|c| c.cert),
    );
    let view_base = resumed.as_ref().map(|r| r.view_base).unwrap_or(0);
    // the drain realizes code-registry swaps through the SAME source recovery
    // replay used (wired at Recovery::open) — lift it off before the move.
    let code_source = recovery.code_source();
    let mut node = match resumed {
        Some(rec) => OrderedNode::resume(
            host,
            orderer,
            recovery,
            rec.height.map(|height| host::FinalizedBlock {
                height,
                app_hash: rec.app_hash,
            }),
            rec.view_base,
        ),
        None => OrderedNode::with_sink(host, orderer, recovery),
    };
    node.set_code_source(code_source);
    // the observation barrier: every drain batch ends AT a block that
    // moves the valset root, so the orchestration step below observes a
    // membership change at exactly its block's view — the same view on
    // every validator, whatever the local batch shape. without it the
    // armed cutover view (and with it the next epoch's height base)
    // would depend on drain timing: a cross-node fork.
    node.watch_module("valset");

    // the valset ORCHESTRATOR: watches finalized valset module state and
    // schedules deterministic epoch cutovers. it resumes at the recovered
    // epoch coordinates over the epoch's ENGINE PARTICIPANT SET, and
    // re-arms a cutover the pre-crash process had scheduled.
    let resident_keys = match resume_resident_keys(resumed) {
        Ok(keys) => keys,
        Err(e) => {
            eprintln!("[node {label}] FATAL: {e}");
            std::process::exit(1);
        }
    };
    let orchestrator = consensus::ValsetOrchestrator::resume(
        CUTOVER_DELAY,
        member_keys.iter().cloned(),
        resident_keys.clone(),
        resume_epoch,
        view_base,
        pending_boot,
    );
    if let Some(ceiling) = pending_boot {
        node.set_view_ceiling(ceiling);
        println!(
            "[node {label}] re-armed pending cutover at view {ceiling} (epoch {})",
            resume_epoch + 1
        );
    }

    // the genesis app-hash BEFORE any op — the demo asserts this agrees across
    // processes (a fork here would be a genesis-determinism bug, not consensus).
    // a RESTORED boot prints its recovered line above instead.
    if resumed.is_none() {
        let genesis_hash = node.app_hash();
        println!("[node {label}] genesis app_hash={}", hex(&genesis_hash));
    }

    // introduce a DISTINCT op per process: node N writes directory key "kN" =
    // "node-N". distinct key + distinct origin -> distinct frame -> distinct
    // sha256 digest, so a peer that finalizes THIS op's digest has NO local
    // bytes for it — unless the leader's relay gossiped them on CHANNEL_PAYLOAD
    // and this process's store-only drain cached them. directory is order-
    // INDEPENDENT, so both nodes converge on {k0=node-0, k1=node-1} under any
    // interleaving, isolating the property under test (did the peer's payload
    // cross the wire?) from op ordering. ONE submit — the automaton PEEKS
    // (never pops), so the digest rides out every nullified early view until
    // the mesh forms and this node leads and proposes it.
    // dev shape only — a REAL network's genesis carries no demo scaffolding
    // (and a restored boot must not re-frame it: seq 0 was already spent).
    if dev_demo && resumed.is_none() {
        let n = label.trim_start_matches('#').to_string();
        let op = Msg {
            target: "directory".into(),
            payload: encode_msg(&DirMsg::Set {
                key: format!("k{n}"),
                value: format!("node-{n}"),
            }),
        };
        node.submit(signer, 0, op).await.expect("submit op");
    }
    EngineState {
        node,
        orchestrator,
        last_cert_height,
        latest_floor,
    }
}
