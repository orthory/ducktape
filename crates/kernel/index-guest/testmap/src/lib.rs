//! the reference index mapper — the smallest real fold + view, in the
//! canonical two-layer shape: pure decisions here, the engine shell behind
//! the `index-guest` feature.
//!
//! derived key space:
//! - `seen/{height:016x}/{seq:04x}` — one row per folded op, value = the op
//!   payload verbatim;
//! - `count` — total folded ops, 8-byte big-endian (a read-modify-write, so
//!   the fixture exercises transactional reads too).
//!
//! poison switches, for failure-path tests:
//! - fold: an op payload of exactly `boom` fails the invocation (code 2) —
//!   the engine must hold the queue and surface the error;
//! - view: a request of exactly `boom` fails (code 3) — the host must map it
//!   to a view error, not an engine error.
//!
//! any other view request is a raw derived key: the value echoes back, a
//! missing key fails with code 4.

use index_guest::{Fail, OpRow, StateRead, Writes};

pub fn fold_op(op: &OpRow, read: &impl StateRead) -> Result<Writes, Fail> {
    if op.payload == b"boom" {
        return Err(Fail::new(2, "poison payload"));
    }
    let count = read
        .get(b"count")
        .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
        .map(u64::from_be_bytes)
        .unwrap_or(0);
    let mut writes = Writes::new();
    index_guest::put(
        &mut writes,
        format!("seen/{:016x}/{:04x}", op.height, op.seq),
        op.payload.clone(),
    );
    index_guest::put(&mut writes, "count", (count + 1).to_be_bytes().to_vec());
    Ok(writes)
}

pub fn serve_view(read: &impl StateRead, req: &[u8]) -> Result<Vec<u8>, Fail> {
    if req == b"boom" {
        return Err(Fail::new(3, "poison view request"));
    }
    let Some(value) = read.get(req) else {
        return Err(Fail::new(4, "no such derived key"));
    };
    Ok(value)
}

#[cfg(feature = "index-guest")]
mod shell {
    use index_guest::Fail;
    use index_guest::guest::{self as ig, Change};

    fn fold(changes: Vec<Change>) -> Result<(), Fail> {
        for op in ig::ops(changes)? {
            ig::apply(crate::fold_op(&op, &ig::EngineRead)?)?;
        }
        Ok(())
    }

    fn view(req: Vec<u8>) -> Result<Vec<u8>, Fail> {
        crate::serve_view(&ig::EngineRead, &req)
    }

    index_guest::fold!(fold);
    index_guest::view!(view);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn fold_reads_its_own_earlier_writes() {
        let mut map = BTreeMap::new();
        let op = |height, seq| OpRow {
            height,
            seq,
            time: 1,
            origin: index_guest::OriginTag::external("jess"),
            payload: b"p".to_vec(),
        };
        let writes = fold_op(&op(1, 0), &map).unwrap();
        index_guest::apply_to_map(&mut map, writes);
        let writes = fold_op(&op(1, 1), &map).unwrap();
        index_guest::apply_to_map(&mut map, writes);

        assert_eq!(
            serve_view(&map, b"count").unwrap(),
            2u64.to_be_bytes().to_vec()
        );
        assert_eq!(
            serve_view(&map, b"seen/0000000000000001/0000").unwrap(),
            b"p".to_vec()
        );
        assert_eq!(serve_view(&map, b"missing").unwrap_err().code, 4);

        let boom = OpRow {
            payload: b"boom".to_vec(),
            ..op(2, 0)
        };
        assert_eq!(fold_op(&boom, &map).unwrap_err().code, 2);
    }
}
