//! SHA-256 as lower-case hexadecimal, the form every identity and content hash takes.
use sha2::{Digest, Sha256};

/// Lower-case hexadecimal SHA-256 of the bytes.
pub(crate) fn digest(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

/// Two lower-case hexadecimal digits per byte; sha2's hashes no longer format as hex themselves.
pub(crate) fn lower_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            hex.extend(char::from_digit(u32::from(nibble), 16));
        }
    }
    hex
}
