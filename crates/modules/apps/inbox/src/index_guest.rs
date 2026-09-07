//! inbox's index-mapper shell: the engine wiring around the pure decision
//! core in [`crate::index`]. decode the feed, decide, apply — nothing else.

use index_guest::Fail;
use index_guest::guest::{self as ig, Change};

/// the attribution module's genesis-constant id: the one origin whose
/// applied ops fold as deliveries (the same id the module shell wires).
const ATTRIBUTION_ID: &str = "attribution";

fn fold(changes: Vec<Change>) -> Result<(), Fail> {
    for op in ig::ops(changes)? {
        ig::apply(crate::index::fold_op(&op, &ig::EngineRead, ATTRIBUTION_ID)?)?;
    }
    Ok(())
}

fn view(req: Vec<u8>) -> Result<Vec<u8>, Fail> {
    crate::index::serve_view(&ig::EngineRead, &req)
}

index_guest::fold!(fold);
index_guest::view!(view);
