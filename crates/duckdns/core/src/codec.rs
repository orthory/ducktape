//! Shared strict codec for the DuckDNS registry root preimage and snapshot.
//! Every read is bounds-checked before allocation; booleans are exactly 0/1;
//! callers finish the whole input so trailing bytes reject.

pub(crate) fn push_u64(out: &mut Vec<u8>, value: usize) {
    out.extend_from_slice(&(value as u64).to_le_bytes());
}

pub(crate) fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    push_u64(out, bytes.len());
    out.extend_from_slice(bytes);
}

pub(crate) fn push_string(out: &mut Vec<u8>, value: &str) {
    push_bytes(out, value.as_bytes());
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let end = self
            .offset
            .checked_add(N)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| "duckdns: registry snapshot truncated".to_string())?;
        let mut value = [0u8; N];
        value.copy_from_slice(&self.bytes[self.offset..end]);
        self.offset = end;
        Ok(value)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, String> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.array::<8>()?))
    }

    pub(crate) fn boolean(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("duckdns: registry snapshot boolean is not 0/1".into()),
        }
    }

    pub(crate) fn count(&mut self, what: &str, minimum_bytes: usize) -> Result<usize, String> {
        let count = self.u64()?;
        let maximum = self.remaining() / minimum_bytes.max(1);
        if count > maximum as u64 {
            return Err(format!(
                "duckdns: registry snapshot {what} count exceeds remaining bytes"
            ));
        }
        usize::try_from(count)
            .map_err(|_| format!("duckdns: registry snapshot {what} count overflows usize"))
    }

    pub(crate) fn bytes(&mut self, maximum: usize, what: &str) -> Result<Vec<u8>, String> {
        let length = self.u64()?;
        if length > maximum as u64 {
            return Err(format!(
                "duckdns: registry snapshot {what} exceeds {maximum} bytes"
            ));
        }
        let length = usize::try_from(length)
            .map_err(|_| format!("duckdns: registry snapshot {what} length overflows usize"))?;
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| "duckdns: registry snapshot truncated".to_string())?;
        let value = self.bytes[self.offset..end].to_vec();
        self.offset = end;
        Ok(value)
    }

    pub(crate) fn string(&mut self, maximum: usize, what: &str) -> Result<String, String> {
        String::from_utf8(self.bytes(maximum, what)?)
            .map_err(|_| format!("duckdns: registry snapshot {what} is not UTF-8"))
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    pub(crate) fn finish(self) -> Result<(), String> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err("duckdns: registry snapshot has trailing bytes".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_cursor_rejects_noncanonical_frames() {
        let mut truncated = Reader::new(&[0; 7]);
        assert!(truncated.u64().unwrap_err().contains("truncated"));

        let mut boolean = Reader::new(&[2]);
        assert!(boolean.boolean().unwrap_err().contains("not 0/1"));

        let mut trailing = Reader::new(&[0, 1]);
        trailing.u8().unwrap();
        assert!(trailing.finish().unwrap_err().contains("trailing"));
    }
}
