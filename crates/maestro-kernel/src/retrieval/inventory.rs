//! Exact, bounded inventories over a pinned generation's own member revisions.

use super::{
    error::Error,
    read::{classify, controlled_reader, ready_projection},
    types::{ChunkHit, InventoryRequest, InventorySelection, SearchRead},
};
use crate::{
    evidence::{Inventory, InventoryCount},
    scope::ScopeSet,
    store::{self, Database},
};
use rusqlite::{Transaction, TransactionBehavior, params};

/// The largest complete inventory response.
const MAX_GROUPS: usize = 1000;
/// The maximum number of supporting chunks.
const MAX_SUPPORTS: usize = 20;

impl Database {
    /// Counts the pinned generation's manifest members by exact metadata and
    /// returns up to twenty chunks owned by those members.
    ///
    /// All population filters execute in SQLite before grouping and support
    /// limits; a timeout or oversized result returns no partial inventory.
    ///
    /// # Errors
    ///
    /// [`Error::ProjectionMissing`] or [`Error::ProfileMismatch`] when the
    /// search projection is unavailable; [`Error::TooLarge`] when the
    /// complete response exceeds its fixed bounds; [`Error::Cancelled`] or
    /// [`Error::TimedOut`] when the controlled read stops; or [`Error::Store`].
    pub fn inventory(
        &self,
        read: &SearchRead<'_>,
        request: &InventoryRequest,
    ) -> Result<InventorySelection, Error> {
        read.control.check()?;
        let (set_filter, group_key) = match request {
            InventoryRequest::DocumentsBySet { set } => (set.as_deref(), "set"),
            InventoryRequest::Versions { set } => (set.as_deref(), "version"),
        };
        if set_filter.is_some_and(str::is_empty) {
            return Err(Error::InvalidInput(
                "inventory set filter is empty".to_owned(),
            ));
        }
        let selected = selected_sql(group_key);
        let mut connection = controlled_reader(self, read.control)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|error| classify(error, read.control))?;
        ready_projection(&transaction, read)?;
        let total_documents = total_documents(&transaction, read, &selected, set_filter)?;
        let groups = groups(&transaction, read, &selected, set_filter)?;
        let inventory = match request {
            InventoryRequest::DocumentsBySet { .. } => Inventory::DocumentsBySet {
                set_filter: set_filter.map(str::to_owned),
                total_documents,
                sets: groups,
            },
            InventoryRequest::Versions { .. } => Inventory::Versions {
                set_filter: set_filter.map(str::to_owned),
                total_documents,
                versions: groups,
            },
        };
        if let Err(reason) = inventory.validate() {
            return Err(if reason.starts_with("inventory exceeds ") {
                Error::TooLarge
            } else {
                Error::InvalidInput(reason)
            });
        }
        let supports = supports(&transaction, read, &selected, set_filter)?;
        read.control.check()?;
        transaction
            .commit()
            .map_err(|error| classify(error, read.control))?;
        read.control.check()?;
        Ok(InventorySelection {
            inventory,
            supports,
        })
    }
}

/// The exact eligible member population and its requested grouping key.
fn selected_sql(group_key: &str) -> String {
    let scope = ScopeSet::source_condition("documents.collection_id", "documents.source_id", 3);
    format!(
        "WITH selected AS (
           SELECT DISTINCT documents.id AS document_id,
             CASE json_type(revisions.metadata_json, '$.{group_key}')
               WHEN 'text' THEN json_extract(revisions.metadata_json, '$.{group_key}') END
               AS group_value,
             chunk_set_members.revision_id AS revision_id
           FROM chunk_set_members
           JOIN revisions ON revisions.id = chunk_set_members.revision_id
           JOIN documents ON documents.id = revisions.document_id
           JOIN quality_dispositions
             ON quality_dispositions.revision_id = revisions.id
           WHERE chunk_set_members.chunk_set_id = ?1
             AND documents.collection_id = ?2 AND {scope}
             AND (?4 IS NULL OR
               CASE json_type(revisions.metadata_json, '$.version')
                 WHEN 'text' THEN json_extract(revisions.metadata_json, '$.version') END = ?4)
             AND (?5 IS NULL OR
               CASE json_type(revisions.metadata_json, '$.set')
                 WHEN 'text' THEN json_extract(revisions.metadata_json, '$.set') END = ?5)
             AND revisions.status <> 'failed'
             AND quality_dispositions.disposition IN ('accepted', 'accepted_with_warnings')
         )"
    )
}

/// The exact distinct document count selected by the query's filters.
fn total_documents(
    transaction: &Transaction<'_>,
    read: &SearchRead<'_>,
    selected: &str,
    set_filter: Option<&str>,
) -> Result<u64, Error> {
    read.control.check()?;
    let sql = format!("{selected} SELECT count(DISTINCT document_id) FROM selected");
    let count = transaction
        .query_row(
            &sql,
            params![
                read.generation.chunk_set_id,
                read.generation.collection_id,
                read.scopes.parameter(),
                read.version,
                set_filter,
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| classify(error, read.control))?;
    read.control.check()?;
    u64::try_from(count).map_err(|_| {
        Error::Store(store::Error::Sqlite(
            rusqlite::Error::IntegralValueOutOfRange(0, count),
        ))
    })
}

/// Every exact group, null last, refusing a 1001st row before returning any.
fn groups(
    transaction: &Transaction<'_>,
    read: &SearchRead<'_>,
    selected: &str,
    set_filter: Option<&str>,
) -> Result<Vec<InventoryCount>, Error> {
    read.control.check()?;
    let sql = format!(
        "{selected}
         SELECT group_value, count(DISTINCT document_id) FROM selected
         GROUP BY group_value ORDER BY group_value IS NULL, group_value COLLATE BINARY
         LIMIT 1001"
    );
    let mut statement = transaction
        .prepare(&sql)
        .map_err(|error| classify(error, read.control))?;
    let mut rows = statement
        .query(params![
            read.generation.chunk_set_id,
            read.generation.collection_id,
            read.scopes.parameter(),
            read.version,
            set_filter,
        ])
        .map_err(|error| classify(error, read.control))?;
    let mut groups = Vec::new();
    while let Some(row) = rows.next().map_err(|error| classify(error, read.control))? {
        read.control.check()?;
        let value = row
            .get::<_, Option<String>>(0)
            .map_err(|error| classify(error, read.control))?;
        let documents = row
            .get::<_, i64>(1)
            .map_err(|error| classify(error, read.control))?;
        let documents = u64::try_from(documents).map_err(|_| {
            Error::Store(store::Error::Sqlite(
                rusqlite::Error::IntegralValueOutOfRange(1, documents),
            ))
        })?;
        groups.push(InventoryCount { value, documents });
        if groups.len() > MAX_GROUPS {
            return Err(Error::TooLarge);
        }
    }
    read.control.check()?;
    Ok(groups)
}

/// Up to twenty smallest chunk IDs owned by selected member revisions.
fn supports(
    transaction: &Transaction<'_>,
    read: &SearchRead<'_>,
    selected: &str,
    set_filter: Option<&str>,
) -> Result<Vec<ChunkHit>, Error> {
    read.control.check()?;
    let sql = format!(
        "{selected}
         SELECT selected.revision_id, min(chunks.id) FROM selected
         JOIN chunks ON chunks.chunk_set_id = ?1
           AND chunks.revision_id = selected.revision_id
         GROUP BY selected.document_id, selected.revision_id
         ORDER BY selected.document_id, min(chunks.id) LIMIT {MAX_SUPPORTS}"
    );
    let mut statement = transaction
        .prepare(&sql)
        .map_err(|error| classify(error, read.control))?;
    let mut rows = statement
        .query(params![
            read.generation.chunk_set_id,
            read.generation.collection_id,
            read.scopes.parameter(),
            read.version,
            set_filter,
        ])
        .map_err(|error| classify(error, read.control))?;
    let mut supports = Vec::new();
    while let Some(row) = rows.next().map_err(|error| classify(error, read.control))? {
        read.control.check()?;
        supports.push(ChunkHit {
            revision_id: row.get(0).map_err(|error| classify(error, read.control))?,
            chunk_id: row.get(1).map_err(|error| classify(error, read.control))?,
        });
    }
    read.control.check()?;
    Ok(supports)
}

#[cfg(test)]
mod bounds_tests {
    use super::{groups, selected_sql, total_documents};
    use crate::{
        evidence::InventoryCount,
        generation::{Generation, GenerationState},
        retrieval::{Error, ReadControl, SearchRead, SystemClock},
        scope::{Scope, ScopeSet},
    };
    use rusqlite::Connection;
    use std::collections::BTreeSet;
    use std::{
        sync::{Arc, atomic::AtomicBool},
        time::{Duration, Instant},
    };

    #[test]
    fn inventory_group_bound_is_exact() {
        let mut connection = Connection::open_in_memory().unwrap();
        let transaction = connection.transaction().unwrap();
        let generation = Generation {
            id: 1,
            collection_id: "ctm".to_owned(),
            chunk_set_id: "set-a".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
            state: GenerationState::Building,
            point_count: None,
            published_at: None,
        };
        let scopes = ScopeSet::default_workspace();
        let control = ReadControl {
            deadline: Instant::now() + Duration::from_secs(5),
            clock: Arc::new(SystemClock),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let read = SearchRead {
            generation: &generation,
            scopes: &scopes,
            version: None,
            control: &control,
        };

        let exact = groups(&transaction, &read, &selected(1000), None).unwrap();
        assert_eq!(exact.len(), 1000);
        assert_eq!(
            exact.first().and_then(|group| group.value.as_deref()),
            Some("set-0001")
        );
        assert_eq!(
            exact.last().and_then(|group| group.value.as_deref()),
            Some("set-1000")
        );
        assert!(matches!(
            groups(&transaction, &read, &selected(1001), None),
            Err(Error::TooLarge)
        ));
    }

    #[test]
    fn inventory_filters_scope_before_group_limit_and_count() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE chunk_set_members (chunk_set_id TEXT, revision_id TEXT);
                 CREATE TABLE revisions (
                   id TEXT, document_id TEXT, status TEXT, metadata_json TEXT
                 );
                 CREATE TABLE documents (id TEXT, collection_id TEXT, source_id TEXT);
                 CREATE TABLE quality_dispositions (revision_id TEXT, disposition TEXT);",
            )
            .unwrap();
        connection
            .execute_batch(
                "INSERT INTO documents VALUES ('visible-doc', 'ctm', 'docs');
                 INSERT INTO revisions VALUES (
                   'visible-revision', 'visible-doc', 'valid', '{\"set\":\"visible\"}'
                 );
                 INSERT INTO quality_dispositions VALUES ('visible-revision', 'accepted');
                 INSERT INTO chunk_set_members VALUES ('set-a', 'visible-revision');",
            )
            .unwrap();
        for index in 0..1001 {
            let document = format!("hidden-doc-{index}");
            let revision = format!("hidden-revision-{index}");
            let set = format!("hidden-{index:04}");
            connection
                .execute(
                    "INSERT INTO documents VALUES (?1, 'ctm', 'hidden')",
                    [&document],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO revisions VALUES (?1, ?2, 'valid', ?3)",
                    rusqlite::params![revision, document, format!("{{\"set\":\"{set}\"}}")],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO quality_dispositions VALUES (?1, 'accepted')",
                    [&revision],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO chunk_set_members VALUES ('set-a', ?1)",
                    [&revision],
                )
                .unwrap();
        }
        let scope: Scope = "workspace/default/collection/ctm/source/docs"
            .parse()
            .unwrap();
        let scopes = ScopeSet::new(BTreeSet::from([scope]));
        let generation = Generation {
            id: 1,
            collection_id: "ctm".to_owned(),
            chunk_set_id: "set-a".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
            state: GenerationState::Building,
            point_count: None,
            published_at: None,
        };
        let control = ReadControl {
            deadline: Instant::now() + Duration::from_secs(5),
            clock: Arc::new(SystemClock),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let read = SearchRead {
            generation: &generation,
            scopes: &scopes,
            version: None,
            control: &control,
        };
        let transaction = connection.transaction().unwrap();
        let selected = selected_sql("set");
        assert_eq!(
            total_documents(&transaction, &read, &selected, None).unwrap(),
            1
        );
        assert_eq!(
            groups(&transaction, &read, &selected, None).unwrap(),
            [InventoryCount {
                value: Some("visible".to_owned()),
                documents: 1,
            }]
        );
    }

    fn selected(count: usize) -> String {
        format!(
            "WITH RECURSIVE selected(document_id, group_value, revision_id) AS (
               SELECT 1, 'set-0001', 'revision-1'
               WHERE ?1 IS NOT NULL AND ?2 IS NOT NULL AND ?3 IS NOT NULL
                 AND ?4 IS NULL AND ?5 IS NULL
               UNION ALL
               SELECT document_id + 1, printf('set-%04d', document_id + 1),
                 'revision-' || (document_id + 1)
               FROM selected WHERE document_id < {count}
             )"
        )
    }
}
