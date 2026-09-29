//! The migrations: the SQL files of `migrations/`, embedded in the binary,
//! applied in number order and recorded by name.

use super::error::Error;
use crate::vocabulary::{EntityKind, Predicate};
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior};
use serde_json::json;

/// The migration that closes the graph vocabulary; it refuses a database
/// holding a claim it cannot carry forward ([`refused`]).
const CLAIM_VOCABULARY: &str = "0013_graph_claim_vocabulary";

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
        "0003_scopes",
        include_str!("../../migrations/0003_scopes.sql"),
    ),
    (
        "0004_documents",
        include_str!("../../migrations/0004_documents.sql"),
    ),
    ("0005_jobs", include_str!("../../migrations/0005_jobs.sql")),
    (
        "0006_eval_reports",
        include_str!("../../migrations/0006_eval_reports.sql"),
    ),
    (
        "0007_chunk_sets",
        include_str!("../../migrations/0007_chunk_sets.sql"),
    ),
    (
        "0008_document_guards",
        include_str!("../../migrations/0008_document_guards.sql"),
    ),
    (
        "0009_model_cards",
        include_str!("../../migrations/0009_model_cards.sql"),
    ),
    (
        "0010_search",
        include_str!("../../migrations/0010_search.sql"),
    ),
    (
        "0011_exact_identifiers",
        include_str!("../../migrations/0011_exact_identifiers.sql"),
    ),
    (
        "0012_graph_claims",
        include_str!("../../migrations/0012_graph_claims.sql"),
    ),
    (
        CLAIM_VOCABULARY,
        include_str!("../../migrations/0013_graph_claim_vocabulary.sql"),
    ),
    (
        "0014_graph_builds",
        include_str!("../../migrations/0014_graph_builds.sql"),
    ),
    (
        "0015_graph_resolution",
        include_str!("../../migrations/0015_graph_resolution.sql"),
    ),
    (
        "0016_extractor_role",
        include_str!("../../migrations/0016_extractor_role.sql"),
    ),
    (
        "0017_unit_graphs",
        include_str!("../../migrations/0017_unit_graphs.sql"),
    ),
    (
        "0018_retrieval_representations",
        include_str!("../../migrations/0018_retrieval_representations.sql"),
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
    for (name, sql) in pending(connection, migrations)? {
        apply(connection, name, sql)?;
    }
    Ok(())
}

/// The migrations of `migrations` the database of `connection` does not
/// record, in name order, the order they apply in; all of them when it
/// records none.
///
/// # Errors
///
/// [`Error::UnknownMigration`] when the database records a name `migrations`
/// lacks, and [`Error::Sqlite`] when it cannot be read.
pub(super) fn pending<'m>(
    connection: &Connection,
    migrations: &'m [(&'m str, &'m str)],
) -> Result<Vec<(&'m str, &'m str)>, Error> {
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
    Ok(missing)
}

/// Refuses incompatible legacy claims before opening a writer or changing journal mode.
/// Databases predating claims are checked when migration 0013 runs instead.
pub(super) fn preflight(connection: &Connection, migrations: &[(&str, &str)]) -> Result<(), Error> {
    let recorded = recorded(connection)?;
    if recorded.iter().any(|name| name == "0012_graph_claims")
        && !recorded.iter().any(|name| name == CLAIM_VOCABULARY)
        && migrations.iter().any(|(name, _)| *name == CLAIM_VOCABULARY)
    {
        check_refused(connection, CLAIM_VOCABULARY)?;
    }
    Ok(())
}

/// Checks the same refusal policy in the read-only preflight and the migration transaction.
fn check_refused(connection: &Connection, name: &str) -> Result<(), Error> {
    let ids = refused(connection, name)?;
    if ids.is_empty() {
        Ok(())
    } else {
        Err(Error::RefusedMigration {
            name: name.to_owned(),
            ids,
        })
    }
}

/// Applies the migration `name` of statements `sql` in a transaction of its
/// own that records it, unless the database records it already: another
/// process opening the database may have applied it since the list was read.
///
/// # Errors
///
/// [`Error::RefusedMigration`] when the database holds records the
/// migration cannot carry forward, and [`Error::Sqlite`] when the migration
/// fails; either leaves it neither applied nor recorded.
pub(super) fn apply(connection: &mut Connection, name: &str, sql: &str) -> Result<(), Error> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let applied = transaction
        .query_row("SELECT 1 FROM migrations WHERE name = ?1", [name], |_| {
            Ok(())
        })
        .optional()?;
    if applied.is_none() {
        check_refused(&transaction, name)?;
        transaction.execute_batch(&vocabulary_sql(name, sql))?;
        transaction.execute(
            "INSERT INTO migrations (name, applied_at)
             VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            [name],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

/// The ids of the records the migration `name` cannot carry forward, in
/// order: for [`CLAIM_VOCABULARY`], the claims whose subject kind is outside
/// the closed list, the one field of a claim 0012 admitted that can be (its
/// predicate was `DEFAULTS_TO` and its object a typed literal); none for
/// every other migration. Nothing is ever mapped to a listed kind.
fn refused(connection: &Connection, name: &str) -> Result<Vec<String>, Error> {
    if name != CLAIM_VOCABULARY {
        return Ok(Vec::new());
    }
    let kinds = json!(EntityKind::ALL.map(EntityKind::as_str)).to_string();
    let mut statement = connection.prepare(
        "SELECT id FROM claims
         WHERE subject_kind NOT IN (SELECT value FROM json_each(?1)) ORDER BY id",
    )?;
    let ids = statement
        .query_map([kinds], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// The names of the migrations `connection` records: none when its database
/// has no `migrations` table, as one no binary migrated yet.
fn recorded(connection: &Connection) -> Result<Vec<String>, Error> {
    let tables: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = 'migrations'",
        [],
        |row| row.get(0),
    )?;
    if tables == 0 {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare("SELECT name FROM migrations")?;
    let names = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(names)
}

/// Expands only the vocabulary migration from the authoritative Rust types.
fn vocabulary_sql(name: &str, sql: &str) -> String {
    if name != CLAIM_VOCABULARY {
        return sql.to_owned();
    }
    let kinds = EntityKind::ALL
        .map(|kind| format!("'{}'", kind.as_str()))
        .join(", ");
    let predicates = Predicate::ALL
        .into_iter()
        .filter(|predicate| predicate.is_claimable())
        .map(|predicate| format!("'{}'", predicate.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    sql.replace("{entity_kinds}", &kinds)
        .replace("{claim_predicates}", &predicates)
}
