//! Physical journal chunks. Logical records can contain large query replies
//! and dispatch effects; each physical entry still obeys the journal codec cap.

use crate::{Error, MAX_RECORD_FIELD_LEN, Record};

const HEADER_BYTES: usize = 16;
const PAYLOAD_BYTES: usize = MAX_RECORD_FIELD_LEN - HEADER_BYTES;

pub(super) fn pieces(record: &[u8]) -> impl Iterator<Item = Vec<u8>> + '_ {
    record
        .chunks(PAYLOAD_BYTES)
        .enumerate()
        .map(|(index, chunk)| {
            let offset = index * PAYLOAD_BYTES;
            let mut bytes = Vec::with_capacity(HEADER_BYTES + chunk.len());
            bytes.extend_from_slice(&(record.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&(offset as u64).to_le_bytes());
            bytes.extend_from_slice(chunk);
            bytes
        })
}

struct Pending {
    position: u64,
    total: usize,
    bytes: Vec<u8>,
}

pub(super) struct Records {
    pending: Option<Pending>,
    start: Start,
}

enum Start {
    Pruned,
    Aligned,
}

impl Records {
    pub(super) fn new(first_position: u64) -> Self {
        Self {
            pending: None,
            start: match first_position {
                0 => Start::Aligned,
                _ => Start::Pruned,
            },
        }
    }

    pub(super) fn incomplete_position(&self) -> Option<u64> {
        self.pending.as_ref().map(|pending| pending.position)
    }

    pub(super) fn push(
        &mut self,
        position: u64,
        bytes: &[u8],
    ) -> Result<Option<(u64, Record)>, Error> {
        let Some((header, payload)) = bytes.split_at_checked(HEADER_BYTES) else {
            return Err(Error::Corrupt("truncated journal chunk header".into()));
        };
        let total = usize::try_from(u64::from_le_bytes(header[..8].try_into().unwrap()))
            .map_err(|_| Error::Corrupt("journal record length does not fit usize".into()))?;
        let offset = usize::try_from(u64::from_le_bytes(header[8..].try_into().unwrap()))
            .map_err(|_| Error::Corrupt("journal chunk offset does not fit usize".into()))?;
        let valid_payload = !payload.is_empty() && payload.len() <= PAYLOAD_BYTES;
        let end = offset.checked_add(payload.len());
        let valid_extent = end.is_some_and(|end| end <= total);
        let valid_chunk = valid_payload && valid_extent;
        if !valid_chunk {
            return Err(Error::Corrupt("invalid journal chunk extent".into()));
        }
        // A physical section can begin in a record whose prefix was pruned.
        // Only this leading fragment may be skipped; all later offsets match.
        let leading_pruned_fragment = matches!(self.start, Start::Pruned) && offset != 0;
        if leading_pruned_fragment {
            return Ok(None);
        }
        self.start = Start::Aligned;
        if self.pending.is_none() {
            if offset != 0 {
                return Err(Error::Corrupt(
                    "journal record starts at nonzero offset".into(),
                ));
            }
            self.pending = Some(Pending {
                position,
                total,
                bytes: Vec::new(),
            });
        }
        let pending = self.pending.as_mut().unwrap();
        let matches_record = pending.total == total && pending.bytes.len() == offset;
        if !matches_record {
            return Err(Error::Corrupt(
                "journal chunks are missing or out of order".into(),
            ));
        }
        pending.bytes.extend_from_slice(payload);
        if pending.bytes.len() != total {
            return Ok(None);
        }
        let pending = self.pending.take().unwrap();
        Ok(Some((pending.position, Record::decode(&pending.bytes)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_are_bounded_and_reassemble_large_effects() {
        let record = Record::Trace {
            height: 2,
            unit: 0,
            trace: host::Trace::Read(host::Read::Query {
                module: "source".into(),
                request: Vec::new(),
                answer: Ok(vec![7; 3 * 1024 * 1024]),
            }),
        };
        let encoded = record.encode();
        let chunks: Vec<_> = pieces(&encoded).collect();
        assert!(chunks.len() > 1);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.len() <= MAX_RECORD_FIELD_LEN)
        );
        let mut records = Records::new(0);
        assert!(records.push(0, &chunks[0]).unwrap().is_none());
        assert_eq!(records.incomplete_position(), Some(0));
        assert_eq!(records.push(1, &chunks[1]).unwrap(), Some((0, record)));
        assert_eq!(records.incomplete_position(), None);
    }

    #[test]
    fn missing_prefix_and_reordered_chunks_are_errors() {
        let encoded = vec![0; PAYLOAD_BYTES + 1];
        let chunks: Vec<_> = pieces(&encoded).collect();
        assert!(Records::new(0).push(0, &chunks[1]).is_err());
        let mut records = Records::new(0);
        records.push(0, &chunks[0]).unwrap();
        assert!(records.push(1, &chunks[0]).is_err());
    }
}
