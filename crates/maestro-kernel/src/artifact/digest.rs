//! A SHA-256 digest: the name every artifact is stored under.

use sha2::{Digest as _, Sha256};
use std::{error, fmt};

/// A SHA-256 digest: 64 lowercase hexadecimal characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(String);

impl Digest {
    /// The digest of `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(
            Sha256::digest(bytes)
                .iter()
                .flat_map(|byte| [byte >> 4, byte & 0x0f])
                .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
                .collect(),
        )
    }

    /// A digest from its text. Anything but 64 lowercase hexadecimal
    /// characters is refused, so no path is ever built from other text.
    ///
    /// # Errors
    ///
    /// [`InvalidDigest`] with the refused text.
    pub fn parse(text: &str) -> Result<Self, InvalidDigest> {
        let hex = text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        if text.len() == 64 && hex {
            Ok(Self(text.to_owned()))
        } else {
            Err(InvalidDigest(text.to_owned()))
        }
    }

    /// The 64 hexadecimal characters.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Text that is not 64 lowercase hexadecimal characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidDigest(String);

impl InvalidDigest {
    /// The refused text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InvalidDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "not a SHA-256 digest of 64 lowercase hexadecimal characters: {:?}",
            self.0
        )
    }
}

impl error::Error for InvalidDigest {}
