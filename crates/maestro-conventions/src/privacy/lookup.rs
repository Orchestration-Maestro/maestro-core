//! Reuses prepared fingerprint lookups during one private-content scan.

use rusqlite::{Connection, Statement, params};
use std::{error::Error, result::Result as StdResult};

/// Error result for prepared fingerprint lookups.
type Result<T> = StdResult<T, Box<dyn Error>>;

/// The two bank queries and key held for one complete Git scan.
pub(super) struct LookupStatements<'connection> {
    /// Key used to fingerprint candidate text.
    key: [u8; 32],
    /// Prepared query for exact-file and text-shingle fingerprints.
    fingerprints: Statement<'connection>,
    /// Prepared query for the exact short-unit allowlist.
    allowed_units: Statement<'connection>,
}

impl<'connection> LookupStatements<'connection> {
    /// Prepares fingerprint and allowlist lookups once before scanning objects.
    pub(super) fn new(connection: &'connection Connection, key: [u8; 32]) -> Result<Self> {
        Ok(Self {
            key,
            fingerprints: connection.prepare(
                "SELECT EXISTS(SELECT 1 FROM fingerprints
                 WHERE kind = ?1 AND length = ?2 AND tag = ?3)",
            )?,
            allowed_units: connection.prepare(
                "SELECT EXISTS(SELECT 1 FROM allowed_units WHERE length = ?1 AND tag = ?2)",
            )?,
        })
    }

    /// Returns the key used to fingerprint candidates.
    pub(super) fn key(&self) -> &[u8; 32] {
        &self.key
    }

    /// Checks one fingerprint using the scan's reused SQL statement.
    pub(super) fn contains(
        &mut self,
        kind: u8,
        length: u64,
        fingerprint: &[u8; 32],
    ) -> Result<bool> {
        let length = i64::try_from(length)?;
        Ok(self.fingerprints.query_row(
            params![i64::from(kind), length, fingerprint.as_slice()],
            |row| row.get::<_, bool>(0),
        )?)
    }

    /// Checks one exact short unit against the scan's allowlist statement.
    pub(super) fn unit_allowed(&mut self, length: u64, fingerprint: &[u8; 32]) -> Result<bool> {
        let length = i64::try_from(length)?;
        Ok(self
            .allowed_units
            .query_row(params![length, fingerprint.as_slice()], |row| {
                row.get::<_, bool>(0)
            })?)
    }
}
