//! Synthetic write-port conformance; no installed catalog admission is claimed.
#![expect(clippy::indexing_slicing, reason = "authored fixture resource IDs")]
use super::support;
use maestro_acquisition::{
    DirectFiles, LocalResource, Principal, ResourceSource,
    adaptation::{
        ActivationAuthority, AtomicCommit, Change, Commit, Notify, Proposal, WriteError,
        WriterContext,
    },
    policy::{
        manifest::{AcquisitionManifest, BaselineKind, ManifestSchema},
        resource::Resource,
    },
};
use maestro_kernel::{
    acquisition::{Handle, Progress, Receipts},
    artifact::Digest,
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use maestro_knowledge::collection::Declaration;
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Debug)]
pub(super) struct Grant;
impl ActivationAuthority for Grant {
    fn check(
        &self,
        _proposal: &Proposal,
        _gate: Handle,
        _principal: &Principal<'_>,
    ) -> Result<(), WriteError> {
        Ok(())
    }
}
#[derive(Debug, Default)]
pub(super) struct Events(pub(super) Mutex<Vec<Progress>>);
impl Notify for Events {
    fn notify(&self, event: Progress) -> Result<(), WriteError> {
        self.0.lock().unwrap().push(event);
        Ok(())
    }
}
#[derive(Debug)]
pub(super) struct Crash(pub(super) bool);
impl Commit for Crash {
    fn replace(&self, root: &Path, bytes: &[u8]) -> Result<(), WriteError> {
        if self.0 {
            AtomicCommit.replace(root, bytes)?;
        }
        Err(WriteError::Storage)
    }
}
/// One actual scoped artifact database and separately bound overlay.
pub(super) struct Fixture {
    pub(super) root: PathBuf,
    pub(super) db: Database,
    pub(super) scopes: ScopeSet,
    pub(super) collection: Declaration,
    pub(super) catalog: support::Catalog,
    pub(super) manifest: AcquisitionManifest,
    pub(super) proposal: Proposal,
    pub(super) gate: Handle,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let root = scratch_directory().unwrap();
        let db = Database::open_in(&root.join("kernel")).unwrap();
        let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
        db.grant("synthetic-reader", &scope, Right::Read, "owner")
            .unwrap();
        let scopes = db.visible("synthetic-reader").unwrap();
        let (collection, catalog) = support::fixture();
        let collection: Declaration = serde_json::from_value(collection).unwrap();
        let policy =
            maestro_acquisition::validate(&catalog, &collection, &support::principal(&scopes))
                .unwrap();
        let metadata = &policy.policy().resource;
        let resource = Resource {
            schema: ManifestSchema::V1,
            id: "manifest".into(),
            version: 1.try_into().unwrap(),
            collection_id: metadata.collection_id.clone(),
            visibility: metadata.visibility,
            scope_tags: metadata.scope_tags.clone(),
            owner_ref: metadata.owner_ref.clone(),
        };
        let baseline = catalog.0["policy"].reference.clone();
        let manifest = AcquisitionManifest {
            resource,
            baseline: baseline.clone(),
            baseline_kind: BaselineKind::Catalog,
            proposals: vec![],
            activations: vec![],
            active: baseline.clone(),
            effective_digest: Digest::of(b"unused"),
        };
        let evidence = db
            .retain(&scope, b"synthetic private evidence", &[])
            .unwrap();
        let report = db
            .retain(
                &scope,
                b"synthetic rule diff, complete gate outcomes and rollback",
                &[evidence],
            )
            .unwrap();
        let gate = db.retain(&scope, b"synthetic gate", &[report]).unwrap();
        let proposal = Proposal {
            expected_baseline: baseline.clone(),
            expected_active: baseline.clone(),
            changes: vec![Change::SetCleanup {
                rules: catalog.0["evidence"].reference.clone(),
            }],
            candidate: Digest::of(b"synthetic candidate"),
            evidence: vec![evidence],
            report,
            rollback: baseline,
        };
        fs::create_dir(root.join("overlay")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(root.join("overlay"), fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            root,
            db,
            scopes,
            collection,
            catalog,
            manifest,
            proposal,
            gate,
        }
    }
    pub(super) fn context<'a>(
        &'a self,
        principal: &'a Principal<'a>,
        events: &'a Events,
        authority: &'a dyn ActivationAuthority,
        commit: &'a dyn Commit,
    ) -> WriterContext<'a> {
        WriterContext {
            source: &self.catalog,
            collection: &self.collection,
            principal: principal.id,
            platform: principal.platform,
            grants: &self.db,
            receipts: &self.db,
            authority,
            notify: events,
            commit,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

impl Fixture {
    /// Separately bound direct-file adapter over the same synthetic closure.
    pub(super) fn direct(&self) -> DirectFiles {
        use std::collections::BTreeMap;
        let root = self.root.join("direct");
        fs::create_dir(&root).unwrap();
        let mut files = BTreeMap::new();
        for (id, resource) in &self.catalog.0 {
            let path = root.join(format!("{id}.json"));
            fs::write(&path, &resource.bytes).unwrap();
            files.insert(
                id.clone(),
                LocalResource {
                    path,
                    admission: resource.admission.clone(),
                },
            );
        }
        DirectFiles::new(files)
    }
}

impl Fixture {
    /// Choose a read-only source without changing the caller's write contract.
    pub(super) fn context_source<'a>(
        &'a self,
        principal: &'a Principal<'a>,
        events: &'a Events,
        adapters: (&'a dyn ActivationAuthority, &'a dyn Commit),
        source: &'a dyn ResourceSource,
    ) -> WriterContext<'a> {
        let mut context = self.context(principal, events, adapters.0, adapters.1);
        context.source = source;
        context
    }
}
