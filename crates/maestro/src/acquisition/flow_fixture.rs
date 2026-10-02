//! Shared real-kernel admitted source fixture with explicit lifecycle windows.
use super::{
    command::{resolve, sync},
    controls::{Controls, Runtime},
    flow_tests::{Dns, Grants, Host, Site, fixture_with},
    output::Report,
};
use crate::failure::Failure;
use maestro_acquisition::{
    CheckedPolicy, Principal,
    lifecycle::{full::Mode, resources::Resources},
    policy::authority::Authority,
    transport::pacing::OriginLedger,
};
use maestro_kernel::{
    artifact::Digest,
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{
    env, fs,
    path::PathBuf,
    sync::Arc,
    time::{Instant, SystemTime},
};
use tokio::runtime::Builder;

/// A real kernel and replaceable synthetic site; no source host or catalog exists.
pub(super) struct Fixture {
    /// Real scoped storage.
    pub(super) db: Database,
    /// Owned test directory, removed after closing the database.
    pub(super) root: PathBuf,
    /// Current authorized collection scope.
    pub(super) scope: Scope,
    /// Current grants from the real kernel.
    pub(super) scopes: ScopeSet,
    /// Validated immutable executable policy.
    pub(super) policy: CheckedPolicy,
    /// The composition root freezes one OS-to-kernel mapping, without setters.
    os_principal: String,
    /// The kernel authorizer of this OS caller.
    kernel_principal: String,
    /// Independently bound collection bytes.
    pub(super) collection: Digest,
    /// Explicit synthetic public wire adapter.
    pub(super) site: Site,
}
impl Fixture {
    /// Each case changes source data only, never production callers.
    pub(super) fn new(edit: impl FnMut(&mut Value)) -> Self {
        Self::mapped(edit, "reader", "reader")
    }
    /// Inject the same explicit mapping as the local composition root.
    pub(super) fn mapped(
        edit: impl FnMut(&mut Value),
        os_principal: &str,
        kernel_principal: &str,
    ) -> Self {
        let root = scratch_directory().unwrap();
        let db = Database::open_in(&root).unwrap();
        let scope = "workspace/default/collection/garden".parse().unwrap();
        db.grant(kernel_principal, &scope, Right::Read, "owner")
            .unwrap();
        let scopes = db.visible(kernel_principal).unwrap();
        let principal = Principal {
            id: os_principal,
            platform: env::consts::OS,
            scopes: &scopes,
        };
        let (collection, files) = fixture_with(edit);
        let policy = resolve(&files, &collection, &principal).unwrap();
        Self {
            root,
            db,
            scope,
            scopes,
            policy,
            os_principal: os_principal.into(),
            kernel_principal: kernel_principal.into(),
            collection: Digest::of(&serde_json::to_vec(&collection).unwrap()),
            site: Site {
                clean: true,
                ..Site::default()
            },
        }
    }
    /// Invoke unchanged production composition with synthetic ports.
    pub(super) fn sync(&self) -> Report {
        self.try_sync().unwrap()
    }
    /// The same production API retains typed refusal evidence.
    pub(super) fn try_sync(&self) -> Result<Report, Failure> {
        self.try_sync_page_size(2)
    }
    /// Exercise production cursor size without changing the ordinary regression fixtures.
    pub(super) fn try_sync_page_size(&self, page_size: u16) -> Result<Report, Failure> {
        self.run_window(
            page_size,
            Mode::Incremental,
            SystemTime::now(),
            &Grants::default(),
        )
    }
    /// Freeze a deterministic lifecycle window without changing authority or HTTP clocks.
    pub(super) fn sync_window(&self, mode: Mode, now: SystemTime) -> Report {
        self.run_window(2, mode, now, &Grants::default()).unwrap()
    }
    /// Same production operation with explicit clock/mode and bounded cursor pages.
    pub(super) fn run_window(
        &self,
        page_size: u16,
        mode: Mode,
        now: SystemTime,
        authority: &dyn Authority,
    ) -> Result<Report, Failure> {
        let scopes = self.db.visible(&self.kernel_principal).unwrap();
        let principal = Principal {
            id: &self.os_principal,
            platform: env::consts::OS,
            scopes: &scopes,
        };
        let dns = Dns::default();
        let controls = Controls::new(
            &self.policy,
            authority,
            &self.os_principal,
            self.scope.as_str(),
        );
        let resources = Resources::new(Arc::new(Host(
            self.policy.policy().aggregate_limits.clone(),
        )));
        let pacing = OriginLedger::new(0);
        let runtime = Runtime {
            authority,
            resolver: &dns,
            transport: &self.site,
            pacing: &pacing,
            controls: &controls,
            epoch: Instant::now(),
            run_now: now,
            clock: &|| now,
            mode,
            collection: self.collection.clone(),
            kernel_principal: &self.kernel_principal,
            frontier_page_size: page_size,
        };
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(sync(
                &self.db,
                &self.policy,
                &principal,
                &runtime,
                &resources,
            ))
    }
    /// Close SQLite before removing files on Windows too.
    pub(super) fn finish(self) {
        drop(self.db);
        fs::remove_dir_all(self.root).unwrap();
    }
}
/// Clean capture scope, distinct child bytes and bounded empty leaf inventories.
pub(super) fn clean(value: &mut Value) {
    value["sources"][0]["seeds"] = json!(["https://garden.example/docs/start"]);
}
