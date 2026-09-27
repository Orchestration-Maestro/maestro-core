//! Computes versioned HMAC-SHA-256 fingerprints over framed content.

use sha2::{Digest, Sha256};

/// SHA-256 compression block size in bytes.
pub(super) const BLOCK_SIZE: usize = 64;

/// Incremental RFC 2104 HMAC-SHA-256 state.
pub(super) struct HmacSha256 {
    /// Inner SHA-256 state after the ipad block.
    inner: Sha256,
    /// Outer key block, retained until finalization.
    outer_pad: [u8; BLOCK_SIZE],
}

impl HmacSha256 {
    /// Normalizes a key and initializes the inner and outer HMAC pads.
    pub(super) fn new(key: &[u8]) -> Self {
        let mut key_block = [0_u8; BLOCK_SIZE];
        if key.len() > BLOCK_SIZE {
            let hashed_key = Sha256::digest(key);
            for (slot, byte) in key_block.iter_mut().zip(hashed_key.iter()) {
                *slot = *byte;
            }
        } else {
            for (slot, byte) in key_block.iter_mut().zip(key) {
                *slot = *byte;
            }
        }
        let mut inner_pad = [0_u8; BLOCK_SIZE];
        let mut outer_pad = [0_u8; BLOCK_SIZE];
        for ((inner, outer), byte) in inner_pad
            .iter_mut()
            .zip(outer_pad.iter_mut())
            .zip(key_block)
        {
            *inner = byte ^ 0x36;
            *outer = byte ^ 0x5c;
        }
        let mut inner = Sha256::new();
        inner.update(inner_pad);
        Self { inner, outer_pad }
    }

    /// Adds message bytes to the inner digest.
    pub(super) fn update(&mut self, bytes: &[u8]) {
        self.inner.update(bytes);
    }

    /// Finalizes both digest layers and returns the 32-byte HMAC.
    pub(super) fn finalize(self) -> [u8; 32] {
        let inner_digest = self.inner.finalize();
        let mut outer = Sha256::new();
        outer.update(self.outer_pad);
        outer.update(inner_digest);
        outer.finalize().into()
    }
}

/// Computes HMAC-SHA-256 for one message.
pub(super) fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new(key);
    mac.update(message);
    mac.finalize()
}

#[cfg(test)]
/// RFC 4231 test vectors for HMAC-SHA-256.
mod tests {
    use super::hmac_sha256;

    /// Encodes one digest as lowercase hexadecimal.
    fn hex(bytes: &[u8]) -> String {
        let mut output = String::new();
        for byte in bytes {
            output.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
            output.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
        }
        output
    }

    /// Compares one HMAC result with the RFC's expected digest.
    fn assert_vector(key: &[u8], message: &[u8], expected: &str) {
        assert_eq!(hex(&hmac_sha256(key, message)), expected);
    }

    #[test]
    /// Verifies all seven RFC 4231 SHA-256 cases, including long keys.
    fn matches_rfc_4231_cases_one_through_seven() {
        assert_vector(
            &[0x0b; 20],
            b"Hi There",
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
        );
        assert_vector(
            b"Jefe",
            b"what do ya want for nothing?",
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
        );
        assert_vector(
            &[0xaa; 20],
            &[0xdd; 50],
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe",
        );
        let case_four_key = (1_u8..=25).collect::<Vec<_>>();
        assert_vector(
            &case_four_key,
            &[0xcd; 50],
            "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b",
        );
        assert_vector(
            &[0x0c; 20],
            b"Test With Truncation",
            "a3b6167473100ee06e0c796c2955552bfa6f7c0a6a8aef8b93f860aab0cd20c5",
        );
        assert_vector(
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
        );
        let case_seven_message = concat!(
            "This is a test using a larger than block-size key and a larger than block-size data. ",
            "The key needs to be hashed before being used by the HMAC algorithm."
        );
        assert_vector(
            &[0xaa; 131],
            case_seven_message.as_bytes(),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2",
        );
    }
}
