//! Durable request identities and item rows; contexts are never normalized together.
use super::error::Error;
use crate::{artifact::Digest, job::unsigned};
use rusqlite::{Row, types::Type};
use ulid::Ulid;

/// A fetch identity with separate authorization and representation contexts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItem {
    /// Stable, sanitized identity supplied by policy, never a transient signed URL.
    pub fetch_identity: String,
    /// Digest of the exact authorization context.
    pub authorization_context: Digest,
    /// Digest of the exact representation profile.
    pub representation_profile: Digest,
}
/// One durable frontier item, pending or leased until its capture is acknowledged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Opaque stable item identity and paging cursor.
    pub id: Ulid,
    /// Source whose writer owns the item.
    pub source: String,
    /// Logical source job retained through lease takeovers.
    pub job: Ulid,
    /// Request and its two independent contexts.
    pub request: NewItem,
    /// Durable number of dispatch attempts, separate from distinct items.
    pub attempts: u64,
    /// Item fencing epoch, incremented for every dispatch attempt.
    pub epoch: u64,
    /// Verified, pinned artifact after acknowledgement; absent for pending work.
    pub capture: Option<Digest>,
}
/// Columns in the order the row decoder expects.
pub(super) const COLUMNS: &str = "id, source, job, fetch_identity, authorization_context, \
    representation_profile, attempts, lease_epoch, capture";

/// Validates the source ID's bounded ASCII grammar.
pub(super) fn source_id(text: &str) -> Result<(), Error> {
    let first = text
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric);
    let valid = text
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte));
    if first && valid && text.len() <= 128 {
        Ok(())
    } else {
        Err(Error::Invalid)
    }
}
/// Refuses an empty, oversized or NUL-bearing stable identity.
pub(super) fn validate(request: &NewItem) -> Result<(), Error> {
    if request.fetch_identity.is_empty()
        || request.fetch_identity.len() > 8192
        || request.fetch_identity.contains('\0')
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
/// Decodes one durable item, never guessing at malformed IDs or digests.
pub(super) fn item_row(row: &Row<'_>) -> rusqlite::Result<Item> {
    Ok(Item {
        id: id(row, 0)?,
        source: row.get(1)?,
        job: id(row, 2)?,
        request: NewItem {
            fetch_identity: row.get(3)?,
            authorization_context: digest(row, 4)?,
            representation_profile: digest(row, 5)?,
        },
        attempts: unsigned(row, 6)?,
        epoch: unsigned(row, 7)?,
        capture: row
            .get::<_, Option<String>>(8)?
            .map(|text| {
                Digest::parse(&text).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(8, Type::Text, Box::new(error))
                })
            })
            .transpose()?,
    })
}
/// Reads an opaque ULID token from a row.
fn id(row: &Row<'_>, index: usize) -> rusqlite::Result<Ulid> {
    Ulid::from_string(&row.get::<_, String>(index)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}
/// Reads a typed immutable context digest from a row.
fn digest(row: &Row<'_>, index: usize) -> rusqlite::Result<Digest> {
    Digest::parse(&row.get::<_, String>(index)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}
