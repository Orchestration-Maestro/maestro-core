//! One native immutable handle per physical path across all public factories.

use super::{config::native, reader::Reader};
use crate::graph::projection::{
    access::{Access, open_root},
    cancellation::ProjectionCancellation,
    configuration::ProjectionConfiguration,
    handle::ProjectionHandle,
    port::{
        EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope,
        TypedEdgeProjection,
    },
    settings::EngineSettings,
    writer::{BuildVerification, ProjectionBackendReader, ProjectionReader, check_read_scope},
};
use lbug::RootDirectory;
use maestro_filesystem::OwnedRoot;
use maestro_kernel::{
    artifact::Digest, facts::ProjectionReceipt, scope::ScopeSet, store::Database,
};
#[cfg(test)]
use std::path::Path;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

/// ponytail: serialize opens process-wide; shard by root only if open contention is measured.
static READERS: Mutex<BTreeMap<PathBuf, Entry>> = Mutex::new(BTreeMap::new());

/// The registry never keeps a native handle alive after its last supported reader drops.
struct Entry {
    /// Strong registry reference, removed when the last consumer drops under the mutex.
    reader: Arc<Reader>,
    /// Revalidate the held root before reusing a cached handle.
    root: OwnedRoot,
    /// Sharing requires the same frozen settings/lock, not just the same basename.
    settings: EngineSettings,
    /// Sharing cannot substitute a previously admitted but different receipt.
    receipt: ProjectionReceipt,
}

/// Open only the receipt-named physical file under shared access, before receipt lookup.
pub(in crate::graph::projection) fn open(
    factory: &ProjectionConfiguration<'_>,
    kernel: &Database,
    scopes: &ScopeSet,
    scope: ProjectionScope,
    cancellation: Option<ProjectionCancellation>,
) -> Result<ProjectionHandle, ProjectionError> {
    check_read_scope(scopes, &scope, &scope)?;
    if cancellation
        .as_ref()
        .is_some_and(ProjectionCancellation::is_cancelled)
    {
        return Err(ProjectionError::Backend("projection read cancelled".into()));
    }
    let root = open_root(&factory.path)?;
    let access = Access::acquire(&root, factory.locks)?;
    let receipt = kernel
        .projection_ready(scopes, scope.generation_id)
        .map_err(|error| ProjectionError::Backend(error.to_string()))?
        .ok_or(ProjectionError::NotReady)?;
    let key = key(&root, &receipt.file_name)?;
    let mut registry = READERS
        .lock()
        .map_err(|_| ProjectionError::Backend("native handle registry poisoned".into()))?;
    let shared = if let Some(entry) = registry.get(&key) {
        entry
            .root
            .resolved_path()
            .map_err(|error| ProjectionError::Backend(error.to_string()))?;
        if entry.settings != factory.settings || entry.receipt != receipt {
            return Err(ProjectionError::NotReady);
        }
        Arc::clone(&entry.reader)
    } else {
        let native_root = RootDirectory::open(
            root.resolved_path()
                .map_err(|error| ProjectionError::Backend(error.to_string()))?,
        )
        .map_err(|error| ProjectionError::Backend(error.to_string()))?;
        let reader = Arc::new(
            Reader::published(&native_root, native(&factory.settings), &scope, &receipt)
                .map_err(ProjectionError::Backend)?,
        );
        registry.insert(
            key.clone(),
            Entry {
                reader: Arc::clone(&reader),
                root,
                settings: factory.settings.clone(),
                receipt,
            },
        );
        reader
    };
    Ok(ProjectionHandle {
        reader: Box::new(Guarded {
            reader: ProjectionReader::from_verified(
                scope,
                Box::new(Shared {
                    reader: Some(shared),
                    key,
                    cancellation,
                }),
            ),
            _access: access,
        }),
        settings: factory.settings.clone(),
    })
}

/// Each consumer owns its own cancellation token while sharing only the native database.
struct Shared {
    /// The one immutable physical handle retained for this consumer.
    reader: Option<Arc<Reader>>,
    /// Physical key used for serialized removal of the final native handle.
    key: PathBuf,
    /// Optional cancellation; ordinary reads allocate no interrupt thread.
    cancellation: Option<ProjectionCancellation>,
}

impl ProjectionBackendReader for Shared {
    fn verification(&self) -> Result<BuildVerification, String> {
        self.native()?.verification()
    }
    fn edges_adjacent(
        &self,
        family: EdgeFamily,
        entity: &Digest,
    ) -> Result<Vec<ProjectionEdge>, String> {
        if let Some(token) = &self.cancellation {
            return Ok(self
                .native()?
                .cancellable_rows(token)?
                .edges
                .into_iter()
                .filter(|edge| {
                    edge.family == family && (edge.source == *entity || edge.target == *entity)
                })
                .collect());
        }
        self.native()?.edges_adjacent(family, entity)
    }
    fn facts_for(&self, subject: &Digest) -> Result<Vec<EntityFact>, String> {
        if let Some(token) = &self.cancellation {
            return Ok(self
                .native()?
                .cancellable_rows(token)?
                .facts
                .into_iter()
                .filter(|fact| fact.subject == *subject)
                .collect());
        }
        self.native()?.facts_for(subject)
    }
}

impl Shared {
    /// The consumer owns a reader until its serialized Drop begins.
    fn native(&self) -> Result<&Reader, String> {
        self.reader
            .as_deref()
            .ok_or_else(|| "native reader was closed".into())
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let mut registry = READERS.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(reader) = self.reader.take() {
            if Arc::strong_count(&reader) == 2
                && registry
                    .get(&self.key)
                    .is_some_and(|entry| Arc::ptr_eq(&entry.reader, &reader))
            {
                drop(registry.remove(&self.key));
            }
            // Serialize the actual last native destructor with the next open.
            drop(reader);
        }
    }
}

/// Normalize the held root's final leaf only after its no-follow identity validation.
fn key(root: &OwnedRoot, basename: &str) -> Result<PathBuf, ProjectionError> {
    let validated = root
        .resolved_path()
        .map_err(|error| ProjectionError::Backend(error.to_string()))?;
    let canonical =
        fs::canonicalize(validated).map_err(|error| ProjectionError::Backend(error.to_string()))?;
    Ok(canonical.join(basename))
}

/// Test evidence: registry ownership plus live consumers of this one physical native handle.
#[cfg(test)]
pub(super) fn owner_count(path: &Path, basename: &str) -> usize {
    let key = key(&open_root(path).unwrap(), basename).unwrap();
    READERS
        .lock()
        .unwrap()
        .get(&key)
        .map_or(0, |entry| Arc::strong_count(&entry.reader))
}

/// A backend-neutral pin plus its guard; native ownership is dropped first.
struct Guarded {
    /// Owns the shared native handle selected by the registry.
    reader: ProjectionReader,
    /// Last field: retains shared access until all native handles have dropped.
    _access: Access,
}

impl TypedEdgeProjection for Guarded {
    fn neighbors(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        family: EdgeFamily,
        entity: &Digest,
    ) -> Result<Vec<ProjectionEdge>, ProjectionError> {
        self.reader.neighbors(scopes, pin, family, entity)
    }
    fn entity_facts(
        &self,
        scopes: &ScopeSet,
        pin: &ProjectionScope,
        subject: &Digest,
    ) -> Result<Vec<EntityFact>, ProjectionError> {
        self.reader.entity_facts(scopes, pin, subject)
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;
    #[cfg(not(windows))]
    use crate::graph::projection::engine::transaction::Transactions;
    #[cfg(windows)]
    use crate::graph::projection::engine::transaction::tests::populate_reader_fixture;
    use crate::graph::projection::{
        engine::{
            schema,
            tests::{Fixture, config, scope},
        },
        tests::contract,
    };
    use lbug::Connection;
    use maestro_kernel::scope::{Right, Scope};

    #[test]
    fn cancellable_handle_filters_both_endpoints_family_and_fact_subject() {
        let fixture = Fixture::new();
        let scope = scope();
        let edge = contract::edge(scope.generation_id, EdgeFamily::KnowledgeClaim);
        let entity = edge.source.clone();
        let mut incoming = edge.clone();
        incoming.id = Digest::of(b"incoming");
        incoming.source = Digest::of(b"incoming source");
        incoming.target = entity.clone();
        let mut unrelated = edge.clone();
        unrelated.id = Digest::of(b"unrelated");
        unrelated.source = Digest::of(b"other source");
        unrelated.target = Digest::of(b"other target");
        let mut catalog = edge.clone();
        catalog.id = Digest::of(b"catalog");
        catalog.family = EdgeFamily::CatalogDependency;
        catalog.relation = "depends_on".into();
        let edges = [edge.clone(), incoming.clone(), unrelated, catalog];
        let fact = contract::fact(&scope);
        let mut other_fact = fact.clone();
        other_fact.claim.id = Digest::of(b"other fact");
        other_fact.subject = Digest::of(b"other subject");
        let shared = shared_reader(&fixture, &edges, &[fact.clone(), other_fact]);
        let kernel_path = fixture.path.join("kernel");
        fs::create_dir(&kernel_path).unwrap();
        let kernel = Database::open_in(&kernel_path).unwrap();
        kernel
            .grant(
                "reader",
                &"workspace/default".parse::<Scope>().unwrap(),
                Right::Read,
                "test",
            )
            .unwrap();
        let scopes = kernel.visible("reader").unwrap();
        let handle = ProjectionHandle {
            reader: Box::new(ProjectionReader::from_verified(
                scope.clone(),
                Box::new(shared),
            )),
            settings: super::super::public_fixture::settings(),
        };
        let debug = format!("{handle:?}");
        assert!(debug.starts_with("ProjectionHandle"));
        assert!(!debug.contains(&fixture.path.display().to_string()));
        assert!(!debug.contains("reader:"), "native handles remain opaque");
        let mut expected = vec![edge, incoming];
        expected.sort_by(|left, right| left.id.cmp(&right.id));
        assert_eq!(
            handle
                .neighbors(&scopes, &scope, EdgeFamily::KnowledgeClaim, &entity)
                .unwrap(),
            expected
        );
        assert_eq!(
            handle
                .neighbors(
                    &scopes,
                    &scope,
                    EdgeFamily::KnowledgeClaim,
                    &Digest::of(b"missing")
                )
                .unwrap(),
            vec![]
        );
        assert_eq!(
            handle.entity_facts(&scopes, &scope, &fact.subject).unwrap(),
            [fact]
        );
        assert_eq!(
            handle
                .entity_facts(&scopes, &scope, &Digest::of(b"missing"))
                .unwrap(),
            vec![]
        );
        let denied = kernel.visible("denied").unwrap();
        assert_eq!(
            handle.entity_facts(&denied, &scope, &entity),
            Err(ProjectionError::Unauthorized)
        );
    }

    fn shared_reader(fixture: &Fixture, edges: &[ProjectionEdge], facts: &[EntityFact]) -> Shared {
        let scope = scope();
        {
            let database = fixture.writer();
            let connection = Connection::new(&database).unwrap();
            #[cfg(not(windows))]
            schema::create(&connection, &scope).unwrap();
            #[cfg(windows)]
            schema::tests::install_reader_fixture(&connection, &scope);
            #[cfg(windows)]
            populate_reader_fixture(&connection, &scope, edges, facts);
            #[cfg(not(windows))]
            {
                Transactions::default()
                    .write_batch(&connection, &scope, edges, facts)
                    .unwrap();
            }
            connection.query("CHECKPOINT").unwrap();
        }
        #[cfg(windows)]
        super::super::open::tests::private_windows_fixture(&fixture.path);
        Shared {
            reader: Some(Arc::new(
                Reader::open(&fixture.root, "rows.lbdb", config(), &scope).unwrap(),
            )),
            key: fixture.path.join("rows.lbdb"),
            cancellation: Some(ProjectionCancellation::new()),
        }
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::graph::projection::engine::{public_fixture::Fixture, public_tests::publish};

    /// Remove only this test's unique physical cache key, including during assertion unwinding.
    struct CacheReset(PathBuf);
    impl Drop for CacheReset {
        fn drop(&mut self) {
            READERS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&self.0);
        }
    }

    /// Stand in for a changed immutable kernel record, isolating the other equality operand.
    fn cached_receipt(key: &Path, receipt: ProjectionReceipt) {
        READERS.lock().unwrap().get_mut(key).unwrap().receipt = receipt;
    }

    #[test]
    fn guard_cached_receipt_mismatch_content_but_matching_cache_is_served() {
        assert_cached_receipt_mismatch(false);
    }

    #[test]
    fn guard_cached_receipt_mismatch_counts_but_matching_cache_is_served() {
        assert_cached_receipt_mismatch(true);
    }

    fn assert_cached_receipt_mismatch(counts: bool) {
        let fixture = Fixture::new();
        publish(&fixture);
        let factory = fixture.factory();
        let read = || {
            factory.reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
        };
        let first = read().unwrap();
        let receipt = fixture
            .authority
            .database
            .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
            .unwrap()
            .unwrap();
        let reset = CacheReset(
            key(
                &open_root(&fixture.native.path).unwrap(),
                &receipt.file_name,
            )
            .unwrap(),
        );
        let matching = read().unwrap();
        assert_eq!(owner_count(&fixture.native.path, &receipt.file_name), 3);
        drop(matching);
        let mut different = receipt;
        if counts {
            different.entity_fact_count += 1;
        } else {
            different.content_digest = Digest::of(b"different cached content");
        }
        cached_receipt(&reset.0, different);
        assert_eq!(read().unwrap_err(), ProjectionError::NotReady);
        drop(first);
    }
}
