//! Lease-bound native producer session behind the public facade.

use super::input_pins;
use super::{
    backend::{Backend, Publication},
    config::native,
};
use crate::graph::projection::binding;
use crate::graph::projection::checkpoint::{self, Journal};
use crate::graph::projection::{
    access::{Access, open_root},
    build::{ProjectionBuild, PublishedProjection},
    configuration::ProjectionConfiguration,
    content,
    port::{EntityFact, ProjectionEdge, ProjectionError},
    settings::EngineSettings,
    writer::{
        BuildVerification, CatalogRelationVocabulary, ProjectionBackend, ProjectionWriter,
        check_read_scope, receipt_from_verification,
    },
};
use lbug::RootDirectory;
use maestro_filesystem::OwnedRoot;
use maestro_kernel::facts::Error as FactError;
use maestro_kernel::{facts::ProjectionReceipt, job::JobState, scope::ScopeSet, store::Database};
use serde_json::json;
use std::{fmt, io::ErrorKind, path::PathBuf, time::SystemTime};

/// One opaque lifecycle session. Native handles drop before the permanent guards.
pub(in crate::graph::projection) struct Session<'a> {
    /// Writable native database and its terminal install callback.
    backend: Backend<Install<'a>>,
    /// Held immutable loader records, absent until the first explicit load.
    pub(super) journal: Option<Journal>,
    /// Recovery cannot write or publish before load validates all durable checkpoints.
    pub(super) loader_validated: bool,
    /// Frozen settings carried unchanged through the session and publication result.
    settings: EngineSettings,
    /// Exact authoritative identities and lease.
    pub(super) build: ProjectionBuild,
    /// Immutable persisted predecessor; live authority is never inferred from this pin.
    pub(super) predecessor: Option<i64>,
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
        Self::start(factory, kernel, scopes, (build, false), clock)
    }

    /// Explicit recovery is fenced against the refreshed kernel lease before any native open.
    pub(in crate::graph::projection) fn resume(
        factory: &ProjectionConfiguration<'_>,
        kernel: &'a Database,
        scopes: &ScopeSet,
        mut build: ProjectionBuild,
        clock: &'a dyn Fn() -> SystemTime,
    ) -> Result<Self, ProjectionError> {
        let current = kernel
            .job(scopes, build.lease.job)
            .map_err(checkpoint::refusal)?
            .and_then(|job| job.lease)
            .ok_or_else(|| checkpoint::refusal("no current lease"))?;
        if current.holder != build.lease.holder || current.number != build.lease.number {
            return Err(checkpoint::refusal("stale or foreign lease"));
        }
        build.lease = current;
        Self::start(factory, kernel, scopes, (build, true), clock).map_err(|error| match error {
            ProjectionError::InputMismatch(_) => error,
            error => checkpoint::refusal(error),
        })
    }

    /// Shared admission and guards; recovery changes only reservation and native initialization.
    fn start(
        factory: &ProjectionConfiguration<'_>,
        kernel: &'a Database,
        scopes: &ScopeSet,
        request: (ProjectionBuild, bool),
        clock: &'a dyn Fn() -> SystemTime,
    ) -> Result<Self, ProjectionError> {
        let (build, resume) = request;
        let (root, access, predecessor) = Self::admit(factory, kernel, scopes, &build, clock)?;
        let pins = input_pins::build_pins(&build);
        let reservation = format!(".build-{}", build.lease.job);
        let staging = if resume {
            OwnedRoot::open(
                &root
                    .resolved_path()
                    .map_err(checkpoint::refusal)?
                    .join(&reservation),
                false,
            )
        } else {
            root.reserve_child(&reservation)
        }
        .map_err(checkpoint::refusal)?;
        let journal = if resume {
            Some(Journal::open(
                &staging.resolved_path().map_err(checkpoint::refusal)?,
                &build,
                predecessor,
                factory.settings.max_db_size,
            )?)
        } else {
            None
        };
        let file_name = content::build_basename(&build.scope, &build.claim_set_id, build.build_id)
            .map_err(ProjectionError::Backend)?;
        if resume {
            staging
                .check_regular(&file_name)
                .map_err(checkpoint::refusal)?;
            match root.check_regular(&file_name) {
                Ok(()) => return Err(checkpoint::refusal("published or receiptless final exists")),
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(checkpoint::refusal(error)),
            }
        }
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
            (native(&factory.settings), pins, build.build_id),
            install,
        );
        if resume {
            backend.resume(&build.scope).map_err(checkpoint::refusal)?;
        } else {
            drop(ProjectionWriter::create(&mut backend, build.scope.clone())?);
        }
        Ok(Self {
            backend,
            journal,
            loader_validated: !resume,
            settings: factory.settings.clone(),
            build,
            predecessor,
            scopes: scopes.clone(),
            kernel,
            clock,
            _access: access,
        })
    }

    /// Perform all scope/input/job admission before reservation or opening native content.
    fn admit(
        factory: &ProjectionConfiguration<'_>,
        kernel: &Database,
        scopes: &ScopeSet,
        build: &ProjectionBuild,
        clock: &dyn Fn() -> SystemTime,
    ) -> Result<(OwnedRoot, Access, Option<i64>), ProjectionError> {
        check_read_scope(scopes, &build.scope, &build.scope)?;
        let pins = input_pins::build_pins(build);
        binding::admitted(&pins, &factory.settings)?;
        kernel
            .validate_projection_inputs(
                scopes,
                &build.claim_set_id,
                &build.resolution_id,
                &build.resolver_version,
            )
            .map_err(|error| match error {
                FactError::ProjectionInputMismatch(kind) => ProjectionError::InputMismatch(kind),
                error => ProjectionError::Backend(error.to_string()),
            })?;
        kernel
            .validate_projection_lease(
                scopes,
                (&build.scope.collection_id, build.build_id),
                &build.lease,
                clock(),
            )
            .map_err(|error| ProjectionError::Backend(error.to_string()))?;
        let reserved = kernel
            .projection_build(scopes, build.build_id)
            .map_err(|error| ProjectionError::Backend(error.to_string()))?
            .ok_or(ProjectionError::NotReady)?;
        input_pins::reserved(build, &reserved)?;
        let root = open_root(&factory.path)?;
        let access = Access::acquire(&root, factory.locks)?;
        access.serialize_writer(factory.locks)?;
        // Windows rooted writes refuse before reservation; no new orphan is created.
        super::schema::writable(cfg!(windows)).map_err(ProjectionError::Backend)?;
        Ok((root, access, reserved.request.expected_active_build_id))
    }

    /// Validated held staging location for the journal capability.
    pub(super) fn staging_path(&mut self) -> Result<PathBuf, ProjectionError> {
        self.backend
            .publication_mut()
            .staging
            .resolved_path()
            .map_err(checkpoint::refusal)
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
        if !self.loader_validated {
            return Err(checkpoint::refusal(
                "call load to validate resumed checkpoints before writing",
            ));
        }
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
        self,
        expected: &BuildVerification,
    ) -> Result<PublishedProjection, ProjectionError> {
        self.publish_with(expected, || {})
    }

    /// Mandatory private boundary after readiness and before any cleanup.
    pub(super) fn publish_with(
        mut self,
        expected: &BuildVerification,
        after_ready: impl FnOnce(),
    ) -> Result<PublishedProjection, ProjectionError> {
        self.validate_lease()?;
        if !self.loader_validated {
            return Err(checkpoint::refusal(
                "call load before publishing a resumed build",
            ));
        }
        if let Some(journal) = self.journal.as_ref() {
            journal.manifest.verify_final(expected)?;
            journal.revalidate()?;
        }
        let file_name = content::build_basename(
            &self.build.scope,
            &self.build.claim_set_id,
            self.build.build_id,
        )
        .map_err(ProjectionError::Backend)?;
        let receipt = receipt_from_verification(
            (&self.build.scope, self.build.build_id),
            self.build.claim_set_id.clone(),
            file_name,
            expected,
            &input_pins::build_pins(&self.build),
        )?;
        self.backend.publication_mut().receipt = Some(receipt.clone());
        ProjectionWriter::resume(&mut self.backend, self.build.scope.clone())
            .verify_and_publish(expected, (&self.build.claim_set_id, self.build.build_id))?;
        after_ready();
        let staging = self.staging_path()?;
        if let Some(journal) = self.journal.as_mut() {
            journal.cleanup(&staging).map_err(|error| {
                ProjectionError::Backend(format!(
                    "projection is published; loader cleanup failed: {error:?}"
                ))
            })?;
        }
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
    pub(super) fn validate_lease(&self) -> Result<(), ProjectionError> {
        self.kernel
            .validate_projection_lease(
                &self.scopes,
                (&self.build.scope.collection_id, self.build.build_id),
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
        if staging != published || published != receipt.identity.file_name {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::projection::engine::public_fixture::{Fixture, now};
    #[cfg(not(windows))]
    use maestro_filesystem::SystemFileLock;

    #[test]
    fn install_refuses_each_independently_mismatched_basename() {
        let fixture = Fixture::new();
        let root = open_root(&fixture.native.path).unwrap();
        let staging = root.reserve_child("private-staging").unwrap();
        let name = content::build_basename(
            &fixture.build.scope,
            &fixture.build.claim_set_id,
            fixture.build.build_id,
        )
        .unwrap();
        let receipt = receipt_from_verification(
            (&fixture.build.scope, fixture.build.build_id),
            fixture.build.claim_set_id.clone(),
            name.clone(),
            &BuildVerification::expected(&fixture.edges, &[]).unwrap(),
            &input_pins::build_pins(&fixture.build),
        )
        .unwrap();
        let clock = || now(0);
        let mut install = Install {
            root,
            staging,
            kernel: &fixture.authority.database,
            scopes: fixture.authority.scopes.clone(),
            build: fixture.build.clone(),
            clock: &clock,
            receipt: Some(receipt),
        };
        for (staged, published) in [("other", name.as_str()), ("other", "other")] {
            assert_eq!(
                install.install(staged, published),
                Err("publication names differ from the reserved receipt basename".into())
            );
        }
        assert!(!fixture.native.path.join(name).exists());
    }

    #[cfg(not(windows))]
    #[test]
    fn session_debug_redacts_native_roots_kernel_clock_and_guards() {
        let fixture = Fixture::new();
        let clock = || now(0);
        let factory = ProjectionConfiguration {
            path: fixture.native.path.clone(),
            settings: super::super::public_fixture::settings(),
            locks: &SystemFileLock,
        };
        let session = Session::create(
            &factory,
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
        let debug = format!("{session:?}");
        assert!(debug.starts_with("ProjectionSession"));
        assert!(!debug.contains(&fixture.native.path.display().to_string()));
        for field in ["backend:", "kernel:", "clock:", "_access:"] {
            assert!(!debug.contains(field), "opaque field leaked: {field}");
        }
    }
}

#[cfg(all(test, not(windows)))]
mod lease_tests {
    use super::*;
    use crate::graph::projection::engine::{
        open::tests::OPEN_CALLS,
        public_fixture::{Fixture, now, settings},
    };
    use maestro_filesystem::SystemFileLock;
    use std::cell::Cell;

    #[test]
    fn guard_expired_session_verify_and_publish_preflight_make_zero_native_reopens() {
        for publish in [false, true] {
            let fixture = Fixture::new();
            let time = Cell::new(now(0));
            let clock = || time.get();
            let configuration = ProjectionConfiguration {
                path: fixture.native.path.clone(),
                settings: settings(),
                locks: &SystemFileLock,
            };
            let mut session = Session::create(
                &configuration,
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock,
            )
            .unwrap();
            session.write_batch(&fixture.edges, &[], None).unwrap();
            let expected = BuildVerification::expected(&fixture.edges, &[]).unwrap();
            time.set(now(60));
            OPEN_CALLS.set(0);
            let refused = if publish {
                session.publish(&expected).map(|_| ())
            } else {
                session.verify().map(|_| ())
            };
            assert!(
                refused.is_err(),
                "expired admission must refuse before native verification"
            );
            assert_eq!(
                OPEN_CALLS.get(),
                0,
                "expired admission must make zero native reopens"
            );
        }
    }
}
