//! Platform-independent byte limit for reads through an already-held file handle.
use std::io::{self, Read};

/// Read at most `max_bytes + 1` bytes, retaining the extra byte only to detect oversize input.
pub(crate) fn read_limited(reader: impl Read, max_bytes: u64) -> io::Result<Vec<u8>> {
    let bytes = read_prefix(reader, max_bytes)?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > max_bytes) {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            format!("file is larger than {max_bytes} bytes"),
        ));
    }
    Ok(bytes)
}

/// Retain at most the byte ceiling and one oversize sentinel through a held reader.
pub(crate) fn read_prefix(reader: impl Read, max_bytes: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}
