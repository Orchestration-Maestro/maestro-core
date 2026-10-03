//! Lease-bound native producer session behind the public facade.

use super::{
    backend::{Backend, Publication},
    config::native,
};
use crate::graph::projection::{
    access::{Access, open_root},
    build::{ProjectionBuild, PublishedProjection},
    content,
    port::{EntityFact, ProjectionEdge, ProjectionError},
    settings::{EngineSettings, ProjectionConfiguration},
    writer::{
        BuildVerification, CatalogRelationVocabulary, ProjectionBackend, ProjectionWriter,
        check_read_scope, receipt_from_verification,
    },
};
use lbug::RootDirectory;
use maestro_canonicalization::OwnedRoot;
use maestro_kernel::{facts::ProjectionReceipt, job::JobState, scope::ScopeSet, store::Database};
use serde_json::json;
use std::{
    fmt, process,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

/// Distinguishes reservations by one process; create-new refuses any crash leftovers.
static NEXT_STAGING: AtomicU64 = AtomicU64::new(0);

/// One opaque lifecycle session. Native handles drop before the permanent guards.
pub(in crate::graph::projection) struct Session<'a> {
    /// Writable native database and its terminal install callback.
    backend: Backend<Install<'a>>,
    /// Frozen settings carried unchanged through the session and publication result.
    settings: EngineSettings,
    /// Exact authoritative identities and lease.
    build: ProjectionBuild,
    /// Current caller authority used for validation and kernel writes.
    scopes: ScopeSet,
    /// Caller-owned kernel, never opened by the native backend.
    kernel: &'a Database,
    /// Caller-owned live clock, not a new time budget/default.
    clock: &'a dyn Fn() -> SystemTime,
    /// Last field: no native handle can outlive shared access.
    _access: Access,
}

impl fmt::Debug for Session<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionSession")
            .field("build", &self.build)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl<'a> Session<'a> {
    /// Start a reserved build under the matching project lease.
    pub(in crate::graph::projection) fn create(
        factory: &ProjectionConfiguration<'_>,
        kernel: &'a Database,
        scopes: &ScopeSet,
        build: ProjectionBuild,
        clock: &'a dyn Fn() -> SystemTime,
    ) -> Result<Self, ProjectionError> {
        check_read_scope(scopes, &build.scope, &build.scope)?;
        let root = open_root(&factory.path)?;
        let access = Access::acquire(&root, factory.locks)?;
        access.serialize_writer(factory.locks)?;
        kernel
            .validate_projection_lease(
                scopes,
                (&build.scope.collection_id, build.scope.generation_id),
                &build.lease,
                clock(),
            )
            .map_err(|error| ProjectionError::Backend(error.to_string()))?;
        if kernel
            .projection_ready(scopes, build.scope.generation_id)
            .map_err(|error| ProjectionError::Backend(error.to_string()))?
            .is_some()
        {
            return Err(ProjectionError::NotReady);
        }
        // Windows rooted writes refuse before reservation; no new orphan is created.
        super::schema::writable().map_err(ProjectionError::Backend)?;
        let reservation = format!(
            ".build-{}-{}-{}-{}",
            build.lease.job,
            build.lease.number,
            process::id(),
            NEXT_STAGING.fetch_add(1, Ordering::Relaxed)
        );
        let staging = root.reserve_child(&reservation).map_err(|error| {
            ProjectionError::Backend(format!(
                "staging reservation refused: {error}; preserve existing orphans \
                 and report for explicit recovery"
            ))
        })?;
        let file_name = content::basename(&build.scope, &build.claim_set_id)
            .map_err(ProjectionError::Backend)?;
        let staged_root = RootDirectory::open(
            staging
                .resolved_path()
                .map_err(|error| ProjectionError::Backend(error.to_string()))?,
        )
        .map_err(|error| ProjectionError::Backend(error.to_string()))?;
        let final_root = RootDirectory::open(
            root.resolved_path()
                .map_err(|error| ProjectionError::Backend(error.to_string()))?,
        )
        .map_err(|error| ProjectionError::Backend(error.to_string()))?;
        let install = Install {
            root,
            staging,
            kernel,
            scopes: scopes.clone(),
            build: build.clone(),
            clock,
            receipt: None,
        };
        let mut backend = Backend::new(
            staged_root,
            final_root,
            file_name,
            native(&factory.settings),
            install,
        );
        drop(ProjectionWriter::create(&mut backend, build.scope.clone())?);
        Ok(Self {
            backend,
            settings: factory.settings.clone(),
            build,
            scopes: scopes.clone(),
            kernel,
            clock,
            _access: access,
        })
    }

    /// Exact settings carried by this unpublished session.
    pub(in crate::graph::projection) fn settings(&self) -> &EngineSettings {
        &self.settings
    }

    /// Route every batch through the existing validated backend-neutral writer.
    pub(in crate::graph::projection) fn write_batch(
        &mut self,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
        vocabulary: Option<&dyn CatalogRelationVocabulary>,
    ) -> Result<(), ProjectionError> {
        self.validate_lease()?;
        let mut writer = ProjectionWriter::resume(&mut self.backend, self.build.scope.clone());
        if let Some(vocabulary) = vocabulary {
            writer.write_batch_with_catalog_vocabulary(&self.scopes, edges, facts, vocabulary)
        } else {
            writer.write_batch(&self.scopes, edges, facts)
        }
    }

    /// Checkpoint, close and verify actual content, leaving this session writable.
    pub(in crate::graph::projection) fn verify(
        &mut self,
    ) -> Result<BuildVerification, ProjectionError> {
        self.validate_lease()?;
        self.backend
            .verify_unpublished(&self.build.scope)
            .map_err(ProjectionError::Backend)
    }

    /// Consume this writer even on failure; never blindly reopen a closed/partial install.
    pub(in crate::graph::projection) fn publish(
        mut self,
        expected: &BuildVerification,
    ) -> Result<PublishedProjection, ProjectionError> {
        self.validate_lease()?;
        let file_name = content::basename(&self.build.scope, &self.build.claim_set_id)
            .map_err(ProjectionError::Backend)?;
        let receipt = receipt_from_verification(
            &self.build.scope,
            self.build.claim_set_id.clone(),
            file_name,
            expected,
        )?;
        self.backend.publication_mut().receipt = Some(receipt.clone());
        ProjectionWriter::resume(&mut self.backend, self.build.scope.clone())
            .verify_and_publish(expected, &self.build.claim_set_id)?;
        Ok(PublishedProjection {
            receipt,
            settings: self.settings.clone(),
        })
    }

    /// Close native staging, release both guards, preserve all orphan entries, then cancel.
    pub(in crate::graph::projection) fn cancel(self) -> Result<(), ProjectionError> {
        let kernel = self.kernel;
        let lease = self.build.lease.clone();
        drop(self);
        kernel
            .complete_job(
                &lease,
                JobState::Cancelled,
                &json!({"reason": "projection_cancelled", "orphans": "preserved"}),
            )
            .map_err(|error| ProjectionError::Backend(error.to_string()))?;
        Ok(())
    }

    /// Fence writes/verification using the same kernel lease check as publication.
    fn validate_lease(&self) -> Result<(), ProjectionError> {
        self.kernel
            .validate_projection_lease(
                &self.scopes,
                (
                    &self.build.scope.collection_id,
                    self.build.scope.generation_id,
                ),
                &self.build.lease,
                (self.clock)(),
            )
            .map_err(|error| ProjectionError::Backend(error.to_string()))
    }
}

/// One terminal lease-bound anchored install. Both roots outlive the native writer.
struct Install<'a> {
    /// Held final graph root.
    root: OwnedRoot,
    /// Exclusively reserved unpublished child.
    staging: OwnedRoot,
    /// Receipt authority, never touched inside native I/O.
    kernel: &'a Database,
    /// Scoped caller authority.
    scopes: ScopeSet,
    /// Exact frozen generation and project lease.
    build: ProjectionBuild,
    /// Consulted again after installation to detect expiry during native I/O.
    clock: &'a dyn Fn() -> SystemTime,
    /// Prepared only by the existing verified writer path.
    receipt: Option<ProjectionReceipt>,
}

impl Publication for Install<'_> {
    fn install(&mut self, staging: &str, published: &str) -> Result<(), String> {
        let receipt = self
            .receipt
            .as_ref()
            .ok_or("publication has no verified receipt")?;
        if staging != published || published != receipt.file_name {
            return Err("publication names differ from the reserved receipt basename".into());
        }
        let install = || -> Result<(), String> {
            self.kernel
                .validate_projection_publication(
                    &self.scopes,
                    receipt,
                    &self.build.lease,
                    (self.clock)(),
                )
                .map_err(|error| error.to_string())?;
            self.root
                .install_from(&self.staging, published)
                .map_err(|error| error.to_string())?;
            self.kernel
                .record_projection_ready(&self.scopes, receipt, &self.build.lease, (self.clock)())
                .map_err(|error| error.to_string())
        };
        install().map_err(|error| format!(
            "publication refused: {error}; writer closed; preserve staging and receiptless finals \
             and report for explicit recovery; do not retry this writer"
        ))
    }
}
