//! The shared wire limit for complete serialized knowledge responses.

/// Maximum serialized UTF-8 bytes in a complete response line.
pub(crate) const RESPONSE_LIMIT_BYTES: usize = 65_536;
