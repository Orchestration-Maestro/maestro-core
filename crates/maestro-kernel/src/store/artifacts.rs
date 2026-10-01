//! The `artifacts` table: what the artifact store holds, the pins that keep
//! it, and the garbage collection that removes what nothing pins.
//!
//! A table that refers to an artifact pins it in the transaction that records
//! the reference, through [`pin`] and [`unpin`], so that a reference and its
//! pin are committed or rolled back together.

use super::{database::Database, error::Error};
use crate::artifact::{self, Digest};
use rusqlite::{Connection, OptionalExtension as _, Row, Transaction, params, types::Type};

/// An artifact as the database records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// The SHA-256 digest of its bytes, which names it.
    pub digest: Digest,
    /// How many bytes it holds.
    pub bytes: u64,
    /// Its media type, as its first put gave it.
    pub media: String,
    /// How many records refer to it; garbage collection removes it only at
    /// zero.
    pub pins: u64,
}

/// What a check of the artifact tree found: how many artifacts the database
/// records, and which of them the tree does not hold intact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactCheck {
    /// How many artifacts the database records.
    pub recorded: u64,
    /// Those whose file is not in the tree, in digest order.
    pub missing: Vec<Digest>,
    /// Those whose file no longer matches its digest, is no regular file or
    /// cannot be read, in digest order.
    pub damaged: Vec<Digest>,
}

impl Database {
    /// Checks each artifact the database records against the tree: a
    /// regular file, read whole and hashed to its digest. Nothing changes. It
    /// reads the whole tree, so it takes as long as the tree is large;
    /// `maestro doctor` runs it.
    ///
    /// # Errors
    ///
    /// [`Error::Sqlite`] when the database cannot be read.
    pub fn check_artifacts(&self) -> Result<ArtifactCheck, Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare("SELECT digest FROM artifacts ORDER BY digest")?;
        let digests: Vec<Digest> = statement
            .query_map([], |row| digest_column(row, 0))?
            .collect::<Result<_, _>>()?;
        let mut check = ArtifactCheck {
            recorded: u64::try_from(digests.len()).unwrap_or(u64::MAX),
            missing: Vec::new(),
            damaged: Vec::new(),
        };
        for digest in digests {
            match self.artifacts.get(&digest) {
                Ok(_) => {}
                Err(artifact::Error::Missing(_)) => check.missing.push(digest),
                Err(_) => check.damaged.push(digest),
            }
        }
        Ok(check)
    }

    /// Stores `bytes` as an artifact of type `media` and records it, without
    /// a pin, and returns its digest. Bytes already recorded keep their
    /// record, pins included.
    ///
    /// The bytes are stored before their row is recorded, so a crash never
    /// leaves a row without its artifact; they are stored again after it,
    /// since a garbage collection may have removed them in between, and the
    /// store writes only a missing or damaged copy.
    ///
    /// # Errors
    ///
    /// [`Error::MediaConflict`] when the bytes are recorded under another
    /// media type, [`Error::Artifact`] when they cannot be stored, and
    /// [`Error::Sqlite`] when they cannot be recorded.
    pub fn put(&self, bytes: &[u8], media: &str) -> Result<Digest, Error> {
        let digest = self.artifacts.put(bytes)?;
        self.record_then_store(bytes, &digest, media)?;
        Ok(digest)
    }

    /// The rest of a put once the bytes are stored under `digest`: records
    /// their row, then stores them again, which writes them only if a
    /// collection removed them before the row was recorded.
    pub(super) fn record_then_store(
        &self,
        bytes: &[u8],
        digest: &Digest,
        media: &str,
    ) -> Result<(), Error> {
        // A slice holds at most `isize::MAX` bytes, which `i64` holds.
        let size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
        self.write(|transaction| record(transaction, digest, size, media))?;
        self.artifacts.put(bytes)?;
        Ok(())
    }

    /// The bytes of the artifact `digest`, checked against it.
    ///
    /// # Errors
    ///
    /// [`Error::Artifact`] with what the artifact store reports: missing,
    /// corrupt or unreadable.
    pub fn get(&self, digest: &Digest) -> Result<Vec<u8>, Error> {
        Ok(self.artifacts.get(digest)?)
    }

    /// The record of the artifact `digest`, if the database has one.
    ///
    /// # Errors
    ///
    /// [`Error::Sqlite`] when the database cannot be read.
    pub fn artifact(&self, digest: &Digest) -> Result<Option<Artifact>, Error> {
        let found = self
            .reader()?
            .query_row(
                "SELECT digest, bytes, media, pins FROM artifacts WHERE digest = ?1",
                [digest.as_str()],
                artifact_row,
            )
            .optional()?;
        Ok(found)
    }

    /// Adds a pin to the artifact `digest`, in a write of its own: one more
    /// record refers to it.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownArtifact`] when no artifact is recorded under
    /// `digest`, and [`Error::Sqlite`] when the pin cannot be written.
    pub fn pin(&self, digest: &Digest) -> Result<(), Error> {
        self.write(|transaction| pin(transaction, digest))
    }

    /// Removes a pin from the artifact `digest`, in a write of its own: one
    /// record fewer refers to it.
    ///
    /// # Errors
    ///
    /// [`Error::NotPinned`] when it has no pin, [`Error::UnknownArtifact`]
    /// when no artifact is recorded under `digest`, and [`Error::Sqlite`]
    /// when the change cannot be written.
    pub fn unpin(&self, digest: &Digest) -> Result<(), Error> {
        self.write(|transaction| unpin(transaction, digest))
    }

    /// What a garbage collection would remove now: every artifact with no
    /// pin, in digest order. Nothing changes; this is the dry run.
    ///
    /// # Errors
    ///
    /// [`Error::Sqlite`] when the database cannot be read.
    pub fn garbage(&self) -> Result<Vec<Artifact>, Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare(
            "SELECT digest, bytes, media, pins FROM artifacts WHERE pins = 0 ORDER BY digest",
        )?;
        let garbage = statement
            .query_map([], artifact_row)?
            .collect::<Result<_, _>>()?;
        Ok(garbage)
    }

    /// Removes every artifact with no pin and returns what it removed, in
    /// digest order: it lists them, deletes the rows of those still unpinned
    /// in one transaction, then removes each file unless a put recorded the
    /// artifact again meanwhile. Temporary files are left alone.
    ///
    /// # Errors
    ///
    /// [`Error::Artifact`], the first failure, when a file cannot be removed:
    /// the collection still tries every other file first. The rows are gone
    /// already, so what stays are files no row names, which a put of the same
    /// bytes records again. [`Error::Sqlite`] when the database cannot be
    /// read or written.
    pub fn collect_garbage(&self) -> Result<Vec<Artifact>, Error> {
        let removed = self.delete_rows(&self.garbage()?)?;
        let mut failed = None;
        for artifact in &removed {
            // Only a row visits a file, so a file skipped now is never tried again.
            let removal = self.remove_unrecorded(&artifact.digest);
            failed = failed.or(removal.err());
        }
        failed.map_or(Ok(removed), Err)
    }

    /// Deletes, in one short transaction, the rows of `garbage` that still
    /// have no pin, and returns them: a pin taken since the listing keeps its
    /// artifact.
    pub(super) fn delete_rows(&self, garbage: &[Artifact]) -> Result<Vec<Artifact>, Error> {
        self.write(|transaction| {
            let mut deleted = Vec::new();
            for artifact in garbage {
                let changed = transaction.execute(
                    "DELETE FROM artifacts WHERE digest = ?1 AND pins = 0",
                    [artifact.digest.as_str()],
                )?;
                deleted.extend((changed > 0).then(|| artifact.clone()));
            }
            Ok(deleted)
        })
    }

    /// Removes the file of `digest` unless a row records it again, holding
    /// the write lock from the check to the removal, so that a put in any
    /// process either records its row first, and the file stays, or after,
    /// and stores the file again. The removal is the one file-system
    /// operation the kernel performs inside a transaction: a single unlink,
    /// which keeps the transaction short.
    pub(super) fn remove_unrecorded(&self, digest: &Digest) -> Result<(), Error> {
        self.write(|transaction| {
            if !recorded(transaction, digest)? {
                self.artifacts.remove(digest)?;
            }
            Ok(())
        })
    }
}

/// Adds a pin to the artifact `digest` inside `transaction`, a write that
/// records a reference to it: the pin commits or rolls back with the
/// reference.
///
/// # Errors
///
/// [`Error::UnknownArtifact`] when no artifact is recorded under `digest`,
/// and [`Error::Sqlite`] when the pin cannot be written.
pub(crate) fn pin(transaction: &Transaction<'_>, digest: &Digest) -> Result<(), Error> {
    repin(transaction, digest, 1)
}

/// Removes a pin from the artifact `digest` inside `transaction`, a write that
/// removes a reference to it.
///
/// # Errors
///
/// [`Error::NotPinned`] when it has no pin, [`Error::UnknownArtifact`] when no
/// artifact is recorded under `digest`, and [`Error::Sqlite`] when the change
/// cannot be written.
pub(crate) fn unpin(transaction: &Transaction<'_>, digest: &Digest) -> Result<(), Error> {
    repin(transaction, digest, -1)
}

/// Adds `change`, one pin or minus one, to the pins of `digest`, never below
/// zero.
fn repin(transaction: &Transaction<'_>, digest: &Digest, change: i64) -> Result<(), Error> {
    let changed = transaction.execute(
        "UPDATE artifacts SET pins = pins + ?2 WHERE digest = ?1 AND pins + ?2 >= 0",
        params![digest.as_str(), change],
    )?;
    if changed > 0 {
        Ok(())
    } else if recorded(transaction, digest)? {
        Err(Error::NotPinned(digest.clone()))
    } else {
        Err(Error::UnknownArtifact(digest.clone()))
    }
}

/// Records the artifact `digest` of `bytes` bytes and type `media`, unless it
/// is recorded already under the same type.
fn record(
    transaction: &Transaction<'_>,
    digest: &Digest,
    bytes: i64,
    media: &str,
) -> Result<(), Error> {
    transaction.execute(
        "INSERT INTO artifacts (digest, bytes, media) VALUES (?1, ?2, ?3)
         ON CONFLICT (digest) DO NOTHING",
        params![digest.as_str(), bytes, media],
    )?;
    let recorded: String = transaction.query_row(
        "SELECT media FROM artifacts WHERE digest = ?1",
        [digest.as_str()],
        |row| row.get(0),
    )?;
    if recorded == media {
        Ok(())
    } else {
        Err(Error::MediaConflict {
            digest: digest.clone(),
            recorded,
            given: media.to_owned(),
        })
    }
}

/// Whether `connection` records the artifact `digest`.
fn recorded(connection: &Connection, digest: &Digest) -> Result<bool, Error> {
    let found = connection
        .query_row(
            "SELECT 1 FROM artifacts WHERE digest = ?1",
            [digest.as_str()],
            |_| Ok(()),
        )
        .optional()?;
    Ok(found.is_some())
}

/// The artifact of a row of `digest, bytes, media, pins`.
fn artifact_row(row: &Row<'_>) -> rusqlite::Result<Artifact> {
    Ok(Artifact {
        digest: digest_column(row, 0)?,
        bytes: unsigned(row, 1)?,
        media: row.get(2)?,
        pins: unsigned(row, 3)?,
    })
}

/// The digest column `index` of `row` holds.
fn digest_column(row: &Row<'_>, index: usize) -> rusqlite::Result<Digest> {
    let hex: String = row.get(index)?;
    Digest::parse(&hex).map_err(|invalid| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(invalid))
    })
}

/// The integer of column `index`, which the table keeps at zero or above.
fn unsigned(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
