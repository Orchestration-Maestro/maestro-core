//! Prepared atomic native batches; the one-shot fault hook is unit-test-only.

use super::{rows, schema};
use crate::graph::projection::{EdgeFamily, EntityFact, ProjectionEdge, ProjectionScope};
use lbug::{Connection, LogicalType, Value};
use maestro_kernel::artifact::Digest;
#[cfg(test)]
use std::mem;

/// State retained for one unpublished session, including rollback poisoning.
#[derive(Default)]
pub(super) struct Transactions {
    /// A failed rollback prevents any further write or publication.
    poisoned: bool,
    /// Arm exactly the next nonempty admitted batch in test builds only.
    #[cfg(test)]
    fail_next_batch: bool,
    /// Number of completed row writes before the last injected error.
    #[cfg(test)]
    mutations_before_failure: usize,
}
impl Transactions {
    /// Refuse a poisoned session; the backend must call this before verification/publication.
    pub(super) fn ensure_usable(&self) -> Result<(), String> {
        if self.poisoned {
            return Err("native projection session is poisoned after rollback failure".into());
        }
        Ok(())
    }

    /// Commit bound values only, or roll back all entity/edge/fact changes.
    pub(super) fn write_batch(
        &mut self,
        connection: &Connection<'_>,
        scope: &ProjectionScope,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) -> Result<(), String> {
        schema::writable()?;
        self.ensure_usable()?;
        connection
            .query("BEGIN TRANSACTION")
            .map_err(|error| error.to_string())?;
        let result = self.apply(connection, scope, edges, facts);
        self.finish(connection, result)
    }

    /// Read and validate in the same transaction as all native writes.
    #[cfg_attr(
        not(test),
        expect(
            clippy::unused_self,
            reason = "post-write fault state exists in tests only"
        )
    )]
    fn apply(
        &mut self,
        connection: &Connection<'_>,
        scope: &ProjectionScope,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) -> Result<(), String> {
        let stored = rows::read(connection, scope)?;
        // ponytail: scan rows per batch; add a row-ID index only if loader measurements need it.
        let mut all_edges = stored.edges;
        all_edges.extend_from_slice(edges);
        let mut all_facts = stored.facts;
        all_facts.extend_from_slice(facts);
        rows::validate(scope, &all_edges, &all_facts)?;
        if edges.is_empty() && facts.is_empty() {
            return Ok(());
        }
        #[cfg(test)]
        let fail = mem::take(&mut self.fail_next_batch);
        for edge in edges {
            write_edge(connection, edge)?;
            #[cfg(test)]
            if fail {
                self.mutations_before_failure = 1;
                return Err("injected failure after native row mutation".into());
            }
        }
        for fact in facts {
            write_fact(connection, fact)?;
            #[cfg(test)]
            if fail {
                self.mutations_before_failure = 1;
                return Err("injected failure after native row mutation".into());
            }
        }
        Ok(())
    }

    /// Even a commit error requires rollback; failed rollback poisons the session.
    fn finish(
        &mut self,
        connection: &Connection<'_>,
        result: Result<(), String>,
    ) -> Result<(), String> {
        let result = result.and_then(|()| {
            connection
                .query("COMMIT")
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
        if let Err(error) = result {
            if let Err(rollback) = connection.query("ROLLBACK") {
                self.poisoned = true;
                return Err(format!(
                    "native rollback failed; session poisoned: {rollback}"
                ));
            }
            return Err(error);
        }
        Ok(())
    }

    /// Arm the next nonempty valid batch, never a pre-transaction early return.
    #[cfg(test)]
    pub(super) fn inject_batch_failure(&mut self) -> Result<(), String> {
        self.ensure_usable()?;
        self.fail_next_batch = true;
        Ok(())
    }
    /// Test evidence that the fault happened after a completed real row mutation.
    #[cfg(test)]
    pub(super) fn mutations_before_failure(&self) -> usize {
        self.mutations_before_failure
    }
}

/// Bind an application entity ID and an explicitly typed empty property list.
fn entity(connection: &Connection<'_>, id: &Digest) -> Result<(), String> {
    let mut statement = connection
        .prepare("MERGE (e:Entity {id: $id}) ON CREATE SET e.facts = $facts")
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            &mut statement,
            vec![
                ("id", Value::String(id.as_str().into())),
                ("facts", Value::List(LogicalType::Blob, vec![])),
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Create one directed typed relationship, with no literal/claim nodes.
fn write_edge(connection: &Connection<'_>, edge: &ProjectionEdge) -> Result<(), String> {
    entity(connection, &edge.source)?;
    entity(connection, &edge.target)?;
    let mut statement = connection
        .prepare(
            "MATCH (s:Entity), (t:Entity) WHERE s.id = $source AND t.id = $target
            CREATE (s)-[:Edge {id: $id, family: $family, relation: $relation,
                collection: $collection, generation: $generation}]->(t)",
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            &mut statement,
            vec![
                ("source", Value::String(edge.source.as_str().into())),
                ("target", Value::String(edge.target.as_str().into())),
                ("id", Value::String(edge.id.as_str().into())),
                (
                    "family",
                    Value::String(
                        match edge.family {
                            EdgeFamily::KnowledgeClaim => "knowledge_claim",
                            EdgeFamily::CatalogDependency => "catalog_dependency",
                        }
                        .into(),
                    ),
                ),
                ("relation", Value::String(edge.relation.clone())),
                (
                    "collection",
                    Value::String(edge.scope.collection_id.clone()),
                ),
                ("generation", Value::Int64(edge.scope.generation_id)),
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Append a complete claim property to its subject, preserving existing facts.
fn write_fact(connection: &Connection<'_>, fact: &EntityFact) -> Result<(), String> {
    entity(connection, &fact.subject)?;
    let mut statement = connection
        .prepare(
            "MATCH (e:Entity) WHERE e.id = $subject SET e.facts = list_concat(e.facts, $facts)",
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            &mut statement,
            vec![
                ("subject", Value::String(fact.subject.as_str().into())),
                (
                    "facts",
                    Value::List(
                        LogicalType::Blob,
                        vec![Value::Blob(rows::encode_fact(fact)?)],
                    ),
                ),
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
#[cfg(not(windows))]
pub(super) mod tests {
    use super::*;
    use crate::graph::projection::engine::tests::{Fixture, edge, full_fact, scope};

    /// Produce real rollback poisoning for the backend's verify/publication guard tests.
    pub(in crate::graph::projection::engine) fn poison(
        transactions: &mut Transactions,
        connection: &Connection<'_>,
    ) {
        assert!(
            transactions
                .finish(connection, Err("operation failed".into()))
                .is_err()
        );
    }

    #[test]
    fn failed_native_rollback_poisons_all_later_session_operations() {
        let fixture = Fixture::new();
        let database = fixture.writer();
        let connection = Connection::new(&database).unwrap();
        schema::create(&connection, &scope()).unwrap();
        let mut tx = Transactions::default();
        // No transaction is active, so the real native ROLLBACK must fail.
        assert!(
            tx.finish(&connection, Err("operation failed".into()))
                .is_err()
        );
        assert!(tx.ensure_usable().is_err());
        assert!(tx.inject_batch_failure().is_err());
        assert!(tx.write_batch(&connection, &scope(), &[], &[]).is_err());
        assert!(rows::read(&connection, &scope()).unwrap().edges.is_empty());
    }

    #[test]
    fn edge_and_fact_failures_after_committed_rows_are_durable_and_one_shot() {
        let fixture = Fixture::new();
        let mut tx = Transactions::default();
        {
            let database = fixture.writer();
            let connection = Connection::new(&database).unwrap();
            schema::create(&connection, &scope()).unwrap();
            tx.write_batch(&connection, &scope(), &[edge()], &[full_fact()])
                .unwrap();
            let baseline = rows::read(&connection, &scope())
                .unwrap()
                .verification()
                .unwrap();
            let mut fresh_edge = edge();
            fresh_edge.id = Digest::of(b"new-edge");
            let mut fresh_fact = full_fact();
            fresh_fact.claim.id = Digest::of(b"new-fact");
            for (edges, facts) in [
                (vec![fresh_edge.clone()], vec![fresh_fact.clone()]),
                (vec![], vec![fresh_fact.clone()]),
            ] {
                tx.inject_batch_failure().unwrap();
                tx.write_batch(&connection, &scope(), &[], &[]).unwrap();
                assert_eq!(
                    tx.write_batch(&connection, &scope(), &edges, &facts)
                        .unwrap_err(),
                    "injected failure after native row mutation"
                );
                assert_eq!(tx.mutations_before_failure(), 1);
                assert_eq!(
                    rows::read(&connection, &scope())
                        .unwrap()
                        .verification()
                        .unwrap(),
                    baseline
                );
            }
            connection.query("CHECKPOINT").unwrap();
        }
        {
            let database = fixture.reader();
            let connection = Connection::new(&database).unwrap();
            assert_eq!(
                rows::read(&connection, &scope())
                    .unwrap()
                    .verification()
                    .unwrap()
                    .content_digest
                    .as_str(),
                "c6cbe83e1fc08ba1416a1a8dea6316083e06c55886e10bed1fab59b3406d8bbd"
            );
        }
        let database = fixture.writer();
        let connection = Connection::new(&database).unwrap();
        let mut fresh_fact = full_fact();
        fresh_fact.claim.id = Digest::of(b"new-fact");
        tx.write_batch(&connection, &scope(), &[], &[fresh_fact])
            .unwrap();
        assert_eq!(rows::read(&connection, &scope()).unwrap().facts.len(), 2);
    }

    #[test]
    fn native_commit_failure_also_rolls_back_and_poisons_if_rollback_fails() {
        let fixture = Fixture::new();
        let database = fixture.writer();
        let connection = Connection::new(&database).unwrap();
        let mut tx = Transactions::default();
        // COMMIT without an active transaction fails; so does its rollback.
        assert!(tx.finish(&connection, Ok(())).is_err());
        assert!(tx.ensure_usable().is_err());
    }
}

#[cfg(test)]
#[cfg(windows)]
pub(super) mod tests {
    use super::*;
    use crate::graph::projection::engine::tests::{Fixture, scope};

    /// Populate only a legacy immutable reader fixture, never a product database.
    pub(in crate::graph::projection::engine) fn populate_reader_fixture(
        connection: &Connection<'_>,
        scope: &ProjectionScope,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) {
        connection.query("BEGIN TRANSACTION").unwrap();
        let mut tx = Transactions::default();
        tx.apply(connection, scope, edges, facts).unwrap();
        tx.finish(connection, Ok(())).unwrap();
    }

    #[test]
    fn windows_schema_and_batch_writes_refuse_before_any_native_operation() {
        let fixture = Fixture::new();
        let database = fixture.writer();
        let connection = Connection::new(&database).unwrap();
        let scope = scope();
        let mut tx = Transactions::default();
        tx.inject_batch_failure().unwrap();
        assert_eq!(tx.mutations_before_failure(), 0);
        assert_eq!(
            schema::create(&connection, &scope).unwrap_err(),
            "native graph writes are unavailable on Windows; open a published graph read-only"
        );
        assert_eq!(
            tx.write_batch(&connection, &scope, &[], &[]).unwrap_err(),
            "native graph writes are unavailable on Windows; open a published graph read-only"
        );
        assert_eq!(
            connection
                .query("CALL show_tables() RETURN name")
                .unwrap()
                .count(),
            0
        );
    }
}
