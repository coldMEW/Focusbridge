use crate::Error;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub const MAX_FRAME: usize = 65535;
pub const MAX_RECORD: usize = 1024 * 1024;
pub(crate) const HEADER: usize = 16;
pub(crate) const CHUNK: usize = 60 * 1024;

#[derive(Default)]
pub(crate) struct Assembler {
    record: u64,
    total: usize,
    started: Option<Instant>,
    partial: Zeroizing<Vec<u8>>,
}
impl Assembler {
    pub(crate) fn expired(&self) -> bool {
        self.started
            .is_some_and(|start| start.elapsed() >= Duration::from_secs(30))
    }
    pub(crate) fn push(&mut self, contents: &[u8]) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        if self.expired() {
            return Err(Error::LimitReached);
        }
        if contents.len() <= HEADER || contents.len() > HEADER + CHUNK {
            return Err(Error::InvalidInput);
        }
        let record = u64::from_be_bytes(contents[..8].try_into().map_err(|_| Error::InvalidInput)?);
        let total = u32::from_be_bytes(
            contents[8..12]
                .try_into()
                .map_err(|_| Error::InvalidInput)?,
        ) as usize;
        let offset = u32::from_be_bytes(
            contents[12..16]
                .try_into()
                .map_err(|_| Error::InvalidInput)?,
        ) as usize;
        if record != self.record || total == 0 || total > MAX_RECORD || offset != self.partial.len()
        {
            return Err(Error::InvalidInput);
        }
        let data = &contents[HEADER..];
        if offset == 0 {
            self.total = total;
            self.started = Some(Instant::now());
        }
        if total != self.total || data.len() != CHUNK.min(total.saturating_sub(offset)) {
            return Err(Error::InvalidInput);
        }
        if offset == 0 {
            // Reserve before copying secrets: growing a Vec could leave a former
            // plaintext allocation behind, beyond Zeroizing's eventual cleanup.
            self.partial.reserve_exact(total);
        }
        self.partial.extend_from_slice(data);
        if self.partial.len() == total {
            self.record = self.record.checked_add(1).ok_or(Error::LimitReached)?;
            self.total = 0;
            self.started = None;
            Ok(Some(std::mem::take(&mut self.partial)))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembly_reserves_full_validated_size_before_copying_secrets() {
        let mut assembler = Assembler::default();
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&0u64.to_be_bytes());
        chunk.extend_from_slice(&((CHUNK + 1) as u32).to_be_bytes());
        chunk.extend_from_slice(&0u32.to_be_bytes());
        chunk.resize(HEADER + CHUNK, 3);
        assert_eq!(assembler.push(&chunk), Ok(None));
        assert!(assembler.partial.capacity() >= CHUNK + 1);
    }

    #[test]
    fn unfinished_record_expires_even_if_its_next_chunk_is_valid() {
        let mut assembler = Assembler::default();
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&0u64.to_be_bytes());
        chunk.extend_from_slice(&((CHUNK + 1) as u32).to_be_bytes());
        chunk.extend_from_slice(&0u32.to_be_bytes());
        chunk.resize(HEADER + CHUNK, 3);
        assert_eq!(assembler.push(&chunk), Ok(None));
        assembler.started = Some(Instant::now() - Duration::from_secs(30));
        chunk[12..16].copy_from_slice(&(CHUNK as u32).to_be_bytes());
        chunk.truncate(HEADER + 1);
        assert!(assembler.expired());
        assert_eq!(assembler.push(&chunk), Err(Error::LimitReached));
    }
}
