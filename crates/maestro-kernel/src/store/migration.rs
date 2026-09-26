//! The migrations: the SQL files of `migrations/`, embedded in the binary,
//! applied in number order and recorded by name.

use super::error::Error;
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior};

/// Every migration this binary carries, as `(name, SQL)`. A name starts with
/// its four-digit number, so name order is number order; each task appends the
/// migration its number reserves (tasks.md), and parallel tasks merge in any
/// order. A migration holds statements only, never `BEGIN` or `COMMIT`.
pub(super) const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_artifacts",
        include_str!("../../migrations/0001_artifacts.sql"),
    ),
    (
        "0002_journal",
        include_str!("../../migrations/0002_journal.sql"),
    ),
    (
        "0004_documents",
        include_str!("../../migrations/0004_documents.sql"),
    ),
];

/// Applies to `connection` each of `migrations` it does not record yet, in
/// name order, each in a transaction of its own that records its name in
/// `migrations`. A migration numbered below one already applied, merged
/// later, still applies.
///
/// # Errors
///
/// [`Error::UnknownMigration`] when the database records a name `migrations`
/// lacks, before anything changes; [`Error::Sqlite`] when a migration fails,
/// which leaves it neither applied nor recorded.
pub(super) fn migrate(
    connection: &mut Connection,
    migrations: &[(&str, &str)],
) -> Result<(), Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS migrations (
            name TEXT PRIMARY KEY NOT NULL,
            applied_at TEXT NOT NULL
        ) STRICT;",
    )?;
    let recorded = recorded(connection)?;
    let unknown = recorded
        .iter()
        .find(|name| !migrations.iter().any(|(known, _)| known == name));
    if let Some(name) = unknown {
        return Err(Error::UnknownMigration(name.clone()));
    }
    let mut missing: Vec<(&str, &str)> = migrations
        .iter()
        .filter(|(name, _)| !recorded.iter().any(|applied| applied == name))
        .copied()
        .collect();
    missing.sort_unstable_by_key(|(name, _)| *name);
    for (name, sql) in missing {
        apply(connection, name, sql)?;
    }
    Ok(())
}

/// Applies the migration `name` of statements `sql` in a transaction of its
/// own that records it, unless the database records it already: another
/// process opening the database may have applied it since the list was read.
///
/// # Errors
///
/// [`Error::Sqlite`] when the migration fails, which leaves it neither
/// applied nor recorded.
pub(super) fn apply(connection: &mut Connection, name: &str, sql: &str) -> Result<(), Error> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let applied = transaction
        .query_row("SELECT 1 FROM migrations WHERE name = ?1", [name], |_| {
            Ok(())
        })
        .optional()?;
    if applied.is_none() {
        transaction.execute_batch(sql)?;
        transaction.execute(
            "INSERT INTO migrations (name, applied_at)
             VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            [name],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

/// The names of the migrations `connection` records.
fn recorded(connection: &Connection) -> Result<Vec<String>, Error> {
    let mut statement = connection.prepare("SELECT name FROM migrations")?;
    let names = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(names)
}
