//! One native immutable handle per physical path across all public factories.

use super::{config::native, reader::Reader};
use crate::graph::projection::{
    access::{Access, open_root},
    cancellation::ProjectionCancellation,
    handle::ProjectionHandle,
    port::{
        EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope,
        TypedEdgeProjection,
    },
    settings::{EngineSettings, ProjectionConfiguration},
    writer::{BuildVerification, ProjectionBackendReader, ProjectionReader, check_read_scope},
};
use lbug::RootDirectory;
use maestro_canonicalization::OwnedRoot;
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
mod tests {
    use super::*;
    use crate::graph::projection::engine::{
        public_fixture::{Fixture, settings},
        public_tests::publish,
    };
    use crate::graph::projection::{ProjectionEngine, ProjectionFactory, content};
    use maestro_canonicalization::SystemFileLock;

    /// Open a supported factory spelling of the same fixture's owned root.
    fn reader(fixture: &Fixture, path: &Path) -> ProjectionHandle {
        ProjectionFactory::new(path, ProjectionEngine::Ladybug, settings(), &SystemFileLock)
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
            .unwrap()
    }

    #[test]
    fn lifecycle_registry_shares_root_spellings_and_separates_other_roots() {
        let fixture = Fixture::new();
        publish(&fixture);
        let name = content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap();
        let path = &fixture.native.path;
        let alias = path
            .parent()
            .unwrap()
            .join(".")
            .join(path.file_name().unwrap());
        assert_ne!(path.as_os_str(), alias.as_os_str());
        let first = reader(&fixture, path);
        let second = reader(&fixture, &alias);
        assert_eq!(owner_count(path, &name), 3);
        assert_eq!(owner_count(&alias, &name), 3);
        #[cfg(unix)]
        {
            use crate::graph::projection::engine::open::tests::Scratch;
            use std::os::unix::fs::symlink;

            let linked_parent = Scratch::new();
            symlink(path.parent().unwrap(), linked_parent.0.join("parent")).unwrap();
            let linked = linked_parent
                .0
                .join("parent")
                .join(path.file_name().unwrap());
            let third = reader(&fixture, &linked);
            assert_eq!(owner_count(&linked, &name), 4);
            assert_eq!(owner_count(path, &name), 4);
            drop(third);
        }
        assert_eq!(owner_count(path, &name), 3);

        let other = Fixture::new();
        publish(&other);
        let other_name = content::basename(&other.build.scope, &other.build.claim_set_id).unwrap();
        let separate = reader(&other, &other.native.path);
        assert_eq!(owner_count(&other.native.path, &other_name), 2);
        assert_eq!(owner_count(path, &name), 3);
        drop(separate);
        assert_eq!(owner_count(&other.native.path, &other_name), 0);
        drop(first);
        assert_eq!(owner_count(&alias, &name), 2);
        drop(second);
        assert_eq!(owner_count(path, &name), 0);
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn lifecycle_registry_shares_case_swapped_owned_leaf() {
        let fixture = Fixture::new();
        publish(&fixture);
        let name = content::basename(&fixture.build.scope, &fixture.build.claim_set_id).unwrap();
        let path = &fixture.native.path;
        let leaf = path.file_name().unwrap().to_str().unwrap();
        let alias = path.with_file_name(leaf.to_uppercase());
        assert_ne!(path.as_os_str(), alias.as_os_str());
        let first = reader(&fixture, path);
        let second = reader(&fixture, &alias);
        assert_eq!(owner_count(path, &name), 3);
        assert_eq!(owner_count(&alias, &name), 3);
        drop(first);
        drop(second);
        assert_eq!(owner_count(&alias, &name), 0);
    }
}
