//! Builds, authenticates and queries the private fingerprint bank.

use super::{
    lookup::LookupStatements,
    mac::HmacSha256,
    normalize::{hex, key_id},
};
use rusqlite::{Connection, OpenFlags};
use std::{
    error::Error,
    fs::{self, File},
    io::{self, Read},
    path::Path,
    result::Result as StdResult,
    str,
};

/// On-disk bank schema version.
pub(super) const FORMAT_VERSION: &str = "2";
/// Text normalization version stored in the bank.
pub(super) const NORMALIZATION_VERSION: &str = "1";

/// Internal error type for bank operations.
pub(super) type Result<T> = StdResult<T, Box<dyn Error>>;

/// Authenticated read-only view of the private fingerprint bank.
pub(super) struct Bank {
    /// SQLite connection opened read-only after the keyed file digest passes.
    connection: Connection,
    /// Secret used for fingerprint lookup.
    key: [u8; 32],
}

impl Bank {
    /// Opens a mode-600 bank after checking its streamed keyed file digest.
    pub(super) fn open(
        path: &Path,
        key_path: &Path,
        digest_path: &Path,
        expected_inventory: &str,
    ) -> Result<Self> {
        check_private_file(path)?;
        check_private_file(digest_path)?;
        let key = load_key(key_path)?;
        let digest_file = File::open(digest_path)?;
        let mut stored_digest = Vec::with_capacity(65);
        digest_file.take(65).read_to_end(&mut stored_digest)?;
        if stored_digest.len() != 64 {
            return Err(invalid().into());
        }
        let stored_digest = str::from_utf8(&stored_digest)?;
        if !valid_hex(stored_digest) || Self::file_hmac(path, &key)? != stored_digest {
            return Err(invalid().into());
        }
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let format = metadata(&connection, "format_version")?;
        let normalization = metadata(&connection, "normalization_version")?;
        let stored_key_id = metadata(&connection, "key_id")?;
        let inventory_id = metadata(&connection, "inventory_id")?;
        let stored_integrity = metadata(&connection, "integrity")?;
        if format != FORMAT_VERSION
            || normalization != NORMALIZATION_VERSION
            || stored_key_id != key_id(&key)
            || inventory_id != expected_inventory
            || !valid_hex(&inventory_id)
            || !valid_hex(&stored_integrity)
        {
            return Err(invalid().into());
        }
        Ok(Self { connection, key })
    }

    /// Fully verifies SQLite structure and the keyed logical-row integrity.
    pub(super) fn verify(
        path: &Path,
        key_path: &Path,
        digest_path: &Path,
        expected_inventory: &str,
    ) -> Result<()> {
        let bank = Self::open(path, key_path, digest_path, expected_inventory)?;
        let check: String = bank
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if check != "ok"
            || metadata(&bank.connection, "integrity")? != integrity(&bank.connection, &bank.key)?
        {
            return Err(invalid().into());
        }
        Ok(())
    }

    /// Computes a streamed keyed digest of the complete SQLite file.
    pub(super) fn file_hmac(path: &Path, key: &[u8; 32]) -> Result<String> {
        let mut file = File::open(path)?;
        let mut mac = HmacSha256::new(key);
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            mac.update(buffer.get(..read).ok_or_else(invalid)?);
        }
        Ok(hex(&mac.finalize()))
    }

    /// Prepares the two fingerprint lookups once for a complete scan.
    pub(super) fn lookup_statements(&self) -> Result<LookupStatements<'_>> {
        LookupStatements::new(&self.connection, self.key)
    }
}

/// Loads a 32-byte key from a mode-600 regular file.
pub(super) fn load_key(path: &Path) -> Result<[u8; 32]> {
    check_private_file(path)?;
    let bytes = fs::read(path)?;
    bytes.try_into().map_err(|_| invalid().into())
}

/// Rejects symlinks, non-regular files and permissive Unix modes.
pub(super) fn check_private_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(invalid().into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(invalid().into());
        }
    }
    Ok(())
}

/// Reads one required bank metadata value.
fn metadata(connection: &Connection, key: &str) -> Result<String> {
    Ok(
        connection.query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get(0)
        })?,
    )
}

/// Computes keyed integrity over metadata and all stored fingerprints.
pub(super) fn integrity(connection: &Connection, key: &[u8; 32]) -> Result<String> {
    let mut digest = HmacSha256::new(key);
    digest.update(b"maestro-privacy-bank-integrity-v1\0");
    for name in [
        "format_version",
        "normalization_version",
        "key_id",
        "inventory_id",
    ] {
        frame_mac(&mut digest, name.as_bytes());
        frame_mac(&mut digest, metadata(connection, name)?.as_bytes());
    }
    frame_mac(&mut digest, b"fingerprints");
    {
        let mut statement = connection
            .prepare("SELECT kind, length, tag FROM fingerprints ORDER BY kind, length, tag")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        for row in rows {
            let (kind, length, tag) = row?;
            frame_mac(&mut digest, &kind.to_be_bytes());
            frame_mac(&mut digest, &length.to_be_bytes());
            frame_mac(&mut digest, &tag);
        }
    }
    frame_mac(&mut digest, b"allowed_units");
    let mut statement =
        connection.prepare("SELECT length, tag FROM allowed_units ORDER BY length, tag")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    for row in rows {
        let (length, tag) = row?;
        frame_mac(&mut digest, &length.to_be_bytes());
        frame_mac(&mut digest, &tag);
    }
    Ok(hex(&digest.finalize()))
}

/// Adds a length-prefixed field to the keyed integrity message.
fn frame_mac(mac: &mut HmacSha256, bytes: &[u8]) {
    mac.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    mac.update(bytes);
}

/// Validates a lowercase-or-uppercase 64-character SHA-256 identifier.
fn valid_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Constructs the generic refusal error used for invalid private data.
pub(super) fn invalid() -> io::Error {
    io::Error::other("invalid privacy data")
}

/// Bank metadata unit tests.
#[cfg(test)]
mod tests {
    use super::valid_hex;

    #[test]
    /// Ensures inventory identifiers use a 64-character hexadecimal form.
    fn inventory_identifiers_require_sha256_hex() {
        assert!(valid_hex(&"a1".repeat(32)));
        assert!(!valid_hex("not-an-inventory"));
    }
}
