//! State-sync request handling over loop-owned consensus state.

use super::ValidatorRuntime;
use crate::explorer::ship_index_blobs;
use crate::host_reads::{read_valset_members, read_valset_residents};
use crate::sync::serve::{SyncBoundary, SyncStateRequest};
use crate::util::{participant_bytes, resident_bytes};

impl ValidatorRuntime<'_> {
    pub(super) async fn on_sync(&mut self, req: SyncStateRequest) {
        let Self {
            node,
            orchestrator,
            latest_floor,
            label,
            index,
            ..
        } = self;

        // the statesync serve task's state touches (the
        // [`SyncStateRequest`] seam): each is one bounded read
        // against loop-owned state — the heavy serving (decode,
        // captures, slicing, replies) lives on the serve task.
        match req {
            SyncStateRequest::Boundary { known, reply } => {
                // the boundary's consensus coordinates ride the manifest.
                // the floor certificate is served only when it certifies
                // exactly the current boundary — a cert behind the
                // boundary would make a joiner skip history it needs.
                let coords = statesync::BoundaryCoords {
                    epoch: orchestrator.epoch(),
                    view_base: orchestrator.epoch_base(),
                    participants: participant_bytes(orchestrator),
                    residents: resident_bytes(orchestrator),
                    floor_cert: latest_floor
                        .as_ref()
                        .filter(|fc| fc.epoch == orchestrator.epoch())
                        .filter(|fc| node.finalized().is_some_and(|f| f.height == fc.height))
                        .map(|fc| fc.cert.clone()),
                };
                let finalized_for_sync = node
                    .finalized()
                    .filter(|f| f.height <= coords.view_base || coords.floor_cert.is_some());
                let answer = match finalized_for_sync {
                    // two refusals, named apart: no boundary at
                    // all (pre-first-block), vs the per-block
                    // window where the tip advanced but its
                    // finalization certificate has not persisted
                    // yet — a retry lands once they align.
                    None => Err(match node.finalized() {
                        Some(f) => format!(
                            "boundary {} awaiting its finalization certificate — \
                                     retry",
                            f.height
                        ),
                        None => "no finalized boundary to serve yet".to_string(),
                    }),
                    Some(finalized) => {
                        let id = statesync::BoundaryId {
                            height: finalized.height,
                            app_hash: finalized.app_hash,
                        };
                        if known.contains(&id) {
                            // the serve task holds this boundary's
                            // payload — coordinates only.
                            Ok(SyncBoundary {
                                id,
                                coords,
                                data: None,
                            })
                        } else {
                            statesync::capture_boundary(node.host(), finalized, &coords)
                                .await
                                .map(|(id, data)| SyncBoundary {
                                    id,
                                    coords,
                                    data: Some(data),
                                })
                        }
                    }
                };
                let _ = reply.send(answer);
            }
            SyncStateRequest::ModuleServe {
                module_id,
                body,
                reply,
            } => {
                let served = node
                    .host()
                    .serve_sync(&module_id, &body)
                    .await
                    .map_err(|e| format!("module {module_id} serve_sync: {e}"));
                let _ = reply.send(served);
            }
            SyncStateRequest::Frames {
                after_height,
                up_to_height,
                reply,
            } => {
                let read = node
                    .sink_mut()
                    .read_finalized_frames(after_height, up_to_height)
                    .await;
                let _ = reply.send(read);
            }
            SyncStateRequest::IndexCut { reply } => {
                let _ = reply.send(ship_index_blobs(index, label));
            }
            SyncStateRequest::TipCoords { reply } => {
                // the detection lane: everything here is already
                // loop-owned state — no capture, and deliberately
                // no floor-cert alignment gate. that gate protects
                // a JOINER from syncing a boundary whose history
                // it would skip; a detection reply carries a
                // presence bit, never certificate bytes, and every
                // action taken on it (ascension, promotion)
                // re-fetches a full manifest through the gated
                // Boundary path.
                let answer = match node.finalized() {
                    None => Err("no finalized boundary to serve yet".to_string()),
                    Some(f) => Ok(statesync::TipCoords {
                        height: f.height,
                        app_hash: f.app_hash,
                        epoch: orchestrator.epoch(),
                        view_base: orchestrator.epoch_base(),
                        participants: participant_bytes(orchestrator),
                        residents: resident_bytes(orchestrator),
                        has_floor: latest_floor
                            .as_ref()
                            .filter(|fc| fc.epoch == orchestrator.epoch())
                            .is_some_and(|fc| fc.height == f.height),
                    }),
                };
                let _ = reply.send(answer);
            }
            SyncStateRequest::Standing { requester, reply } => {
                // the fail-closed standing gate (ADR §5.1). read the COMMITTED
                // valset projection (updates at the Redeem block, unlike the
                // orchestrator's transport set which lags to the cutover), so a
                // freshly-admitted resident is servable the instant its grant
                // commits. these are local in-memory host queries on the loop's
                // own state — the same read point `read_valset_residents` uses
                // elsewhere, between drains, no deadlock.
                let host = node.host();
                let members = read_valset_members(host).await;
                let residents = read_valset_residents(host).await;
                let standing = members
                    .iter()
                    .chain(residents.iter())
                    .any(|k| k.as_slice() == requester);
                let _ = reply.send(standing);
            }
        }
    }
}
