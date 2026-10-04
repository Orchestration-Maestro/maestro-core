//! Borrow existing synthetic ports for private source contracts, without live IO.
use super::{
    command::{freeze, new_receipt},
    controls::{Controls, Runtime, SourceWork},
    flow_fixture::Fixture,
    flow_tests::{Dns, Grants, Host, Site},
    output::Report,
    sync_budget::RunBudget,
};
use maestro_acquisition::{
    Principal,
    lifecycle::{full::Mode, resources::Resources},
    transport::pacing::OriginLedger,
};
use maestro_kernel::{
    acquisition::{Handle, Receipts},
    store::Database,
};
use std::{
    env,
    sync::Arc,
    time::{Instant, SystemTime},
};

/// Use the same mandatory source composition, with deterministic run time.
pub(super) fn with_work<R>(
    fixture: &Fixture,
    now: SystemTime,
    call: impl FnOnce(&mut SourceWork<'_, Database, Site>, &Resources, &mut RunBudget) -> R,
) -> R {
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &fixture.scopes,
    };
    let authority = Grants::default();
    let controls = Controls::new(
        &fixture.policy,
        &authority,
        "reader",
        fixture.scope.as_str(),
    );
    let dns = Dns::default();
    let pacing = OriginLedger::new(0);
    let resources = Resources::new(Arc::new(Host(
        fixture.policy.policy().aggregate_limits.clone(),
    )));
    let runtime = Runtime {
        authority: &authority,
        resolver: &dns,
        transport: &fixture.site,
        pacing: &pacing,
        controls: &controls,
        frontier_page_size: 2,
        kernel_principal: "reader",
        collection: fixture.collection.clone(),
        epoch: Instant::now(),
        run_now: now,
        clock: &|| now,
        mode: Mode::Incremental,
    };
    let inputs = freeze(
        &fixture.db,
        &fixture.scope,
        &runtime,
        &fixture.policy,
        &principal,
    )
    .unwrap();
    let mut report = Report::new();
    report.run = Some(Handle::new());
    report.receipt = Some(Handle::new());
    let receipt = new_receipt(&report, inputs).unwrap();
    fixture.db.begin(&fixture.scope, &receipt).unwrap();
    let mut writers = vec![];
    let run_id = receipt.run.to_string();
    let mut work = SourceWork {
        store: &fixture.db,
        policy: &fixture.policy,
        principal: &principal,
        runtime: &runtime,
        resources: &resources,
        scope: &fixture.scope,
        source: &fixture.policy.policy().sources[0],
        receipt: &receipt,
        run_id: &run_id,
        writers: &mut writers,
    };
    let mut budget = RunBudget::new(&fixture.policy.policy().aggregate_limits).unwrap();
    call(&mut work, &resources, &mut budget)
}
