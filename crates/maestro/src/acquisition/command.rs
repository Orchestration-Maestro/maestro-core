//! Public manual acquisition composition over the existing admission and frontier ports.
use super::{
    controls::{Controls, Runtime, SourceWork, decision, request, storage},
    output::{Entry, Report},
    sync_budget::RunBudget,
    sync_source::{SOURCE_OWNED, execute},
};
use crate::failure::Failure;
use maestro_acquisition::capture::prepare_receipt;
use maestro_acquisition::{
    CheckedPolicy, PolicySource, Principal,
    lifecycle::resources::Resources,
    policy::{
        acquisition::Transport, format_time, identity::FetchIdentity, resource::Visibility,
        source::Discovery,
    },
    transport::connect::PinnedTransport,
};
use maestro_kernel::{
    acquisition::{
        BudgetUsage, Captures, Frontier, Handle, InventoryPage, Item, Partitions, Reason, Receipt,
        ReceiptSchema, Receipts, SourceLease, Status,
    },
    artifact::Digest,
    scope::Scope,
};
use maestro_knowledge::collection::Declaration;
use std::{
    io::{self, Write as _},
    time::{Instant, SystemTime},
};
use tokio::io::{AsyncRead, AsyncWrite};

/// Resolve the existing policy port before leasing, resource starts or fetching.
pub(crate) fn resolve(
    source: &dyn PolicySource,
    collection: &Declaration,
    principal: &Principal<'_>,
) -> Result<CheckedPolicy, Failure> {
    let policy = source
        .resolve(collection, principal)
        .map_err(|_| Failure::refused("acquisition policy refused"))?;
    if policy.policy().resource.visibility != Visibility::Public {
        return Err(Failure::refused(
            "public acquisition requires a public policy",
        ));
    }
    let seeds = policy
        .policy()
        .sources
        .iter()
        .try_fold(0_usize, |sum, source| sum.checked_add(source.seeds.len()));
    if seeds.is_none_or(|count| count > 1000) {
        return Err(Failure::refused(
            "manual seed inventory exceeds the bounded MVP",
        ));
    }
    for source in &policy.policy().sources {
        let profile = policy
            .acquisition_profiles()
            .get(&source.acquisition_profile.id)
            .ok_or_else(|| Failure::refused("acquisition profile missing"))?;
        if profile.transport != Transport::Http
            || source.auth_role.is_some()
            || source
                .discovery
                .iter()
                .any(|selection| !matches!(selection, Discovery::Links { .. }))
        {
            return Err(Failure::refused(
                "public manual acquisition capability unsupported",
            ));
        }
    }
    Ok(policy)
}
/// Offline readiness projection: robots are unknown until an admitted sync fetch.
pub(crate) fn preview(
    policy: &CheckedPolicy,
    controls: &Controls<'_>,
    now: SystemTime,
) -> Result<Report, Failure> {
    let now = format_time(now).map_err(|_| Failure::refused("acquisition clock invalid"))?;
    let mut report = Report::new();
    for source in &policy.policy().sources {
        for url in &source.seeds {
            let candidate = request(&source.id, url, &now);
            let reason = decision(policy, &candidate, controls);
            let entry = Entry {
                reference: Digest::of(url.as_bytes()),
                reason: reason.into(),
                url: FetchIdentity::parse(source, url)
                    .ok()
                    .map(|identity| identity.as_str().to_owned()),
                observed_ms: None,
            };
            report.decisions.push(entry.clone());
            if reason == "policy_denial" {
                report.discarded.push(entry);
            } else if reason != "allowed" {
                report.pending.push(entry);
            }
        }
    }
    report.deduplicate();
    Ok(report)
}
/// A unique receipt begins before any network effects. All work uses the frontier.
pub(crate) async fn sync<T: PinnedTransport>(
    store: &(impl Frontier + Captures + Partitions + Receipts),
    policy: &CheckedPolicy,
    principal: &Principal<'_>,
    runtime: &Runtime<'_, T>,
    resources: &Resources,
) -> Result<Report, Failure>
where
    T::Connection: AsyncRead + AsyncWrite + Unpin + Send,
{
    let scope: Scope = format!(
        "workspace/default/collection/{}",
        policy.policy().resource.collection_id
    )
    .parse()
    .map_err(|_| Failure::refused("acquisition scope invalid"))?;
    if !principal.scopes.covers(&scope) {
        return Err(Failure::refused("acquisition scope refused"));
    }
    let inputs = freeze(store, &scope, runtime, policy, principal)?;
    let mut report = Report::new();
    report.run = Some(Handle::new());
    report.receipt = Some(Handle::new());
    let mut receipt = new_receipt(&report, inputs)?;
    store.begin(&scope, &receipt).map_err(|_| storage())?;
    let run_id = receipt.run.to_string();
    let mut writers = Writers {
        store,
        leases: vec![],
    };
    let mut pages = Vec::new();
    let mut budget = RunBudget::new(&policy.policy().aggregate_limits)?;
    let performed = async {
        let mut frontier = Vec::new();
        for source in &policy.policy().sources {
            let mut work = SourceWork {
                store,
                policy,
                principal,
                runtime,
                resources,
                scope: &scope,
                source,
                receipt: &receipt,
                run_id: &run_id,
                writers: &mut writers.leases,
            };
            let result = execute(&mut work, &mut report, &mut budget).await?;
            receipt.budget.response_bytes = receipt
                .budget
                .response_bytes
                .checked_add(result.response_bytes)
                .ok_or_else(storage)?;
            receipt.budget.staging_bytes = receipt
                .budget
                .staging_bytes
                .checked_add(result.staging_bytes)
                .ok_or_else(storage)?;
            pages.extend(result.pages);
            frontier.extend(result.items);
        }
        report.deduplicate();
        report.status = if report.pending.is_empty() && report.overflow == 0 {
            Status::Complete
        } else {
            Status::Partial
        };
        receipt.status = report.status;
        receipt.reason = if report.pending.is_empty() && report.overflow == 0 {
            Reason::None
        } else {
            Reason::Unsupported
        };
        receipt.budget.elapsed_ms =
            u64::try_from(runtime.epoch.elapsed().as_millis()).map_err(|_| storage())?;
        finish(
            store,
            &scope,
            &mut receipt,
            (&pages, &frontier),
            &mut report,
        )?;
        Ok::<(), Failure>(())
    }
    .await;
    if let Err(error) = performed {
        report.status = if matches!(error, Failure::Refused(_)) {
            Status::Blocked
        } else {
            Status::Failed
        };
        let reason = match error {
            Failure::Refused(message) if message == SOURCE_OWNED => "source_lease_unavailable",
            _ => "attempt_not_finalized",
        };
        report
            .pending
            .push(Entry::new(&receipt.attempt.to_string(), reason));
    }
    Ok(report)
}
/// Freeze original manifest and resource digests with the effective caller/scope.
fn freeze<T>(
    store: &dyn Receipts,
    scope: &Scope,
    runtime: &Runtime<'_, T>,
    policy: &CheckedPolicy,
    principal: &Principal<'_>,
) -> Result<Handle, Failure> {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "collection":runtime.collection,
        "resources":policy.references(),
        "os_principal":principal.id,
        "kernel_principal":runtime.kernel_principal,
        "scope":scope.as_str(),
        "mode":runtime.mode,
        "run_now":runtime.run_now
    }))
    .map_err(|_| storage())?;
    store.retain(scope, &bytes, &[]).map_err(|_| storage())
}
/// Frozen inputs and unique identities, retained before the first dispatch.
pub(super) fn new_receipt(report: &Report, inputs: Handle) -> Result<Receipt, Failure> {
    Ok(Receipt {
        schema: ReceiptSchema::V1,
        run: report.run.ok_or_else(storage)?,
        attempt: report.receipt.ok_or_else(storage)?,
        inputs,
        tightening: vec![],
        inventories: vec![],
        attempts: 0,
        budget: BudgetUsage {
            elapsed_ms: 0,
            response_bytes: 0,
            staging_bytes: 0,
        },
        downstream: vec![],
        status: Status::Pending,
        reason: Reason::None,
    })
}
/// Retain the authorized summary and the same reconciled inventories in N06/N12.
pub(super) fn finish(
    store: &(impl Receipts + Captures),
    scope: &Scope,
    receipt: &mut Receipt,
    inventory: (&[InventoryPage], &[Item]),
    report: &mut Report,
) -> Result<(), Failure> {
    let (pages, frontier) = inventory;
    let (mut terminal, outcome) =
        prepare_receipt(store, scope, receipt, pages, frontier).map_err(|_| storage())?;
    if report.status == Status::Complete && outcome.status != Status::Complete {
        report.status = outcome.status;
        receipt.status = outcome.status;
        receipt.reason = Reason::Unsupported;
        report.pending.push(Entry::new(
            &receipt.attempt.to_string(),
            "capture_reconciliation_pending",
        ));
    }
    terminal.status = report.status;
    terminal.reason = receipt.reason;
    let summary = store
        .retain(
            scope,
            &serde_json::to_vec(report).map_err(|_| storage())?,
            &terminal.inventories,
        )
        .map_err(|_| storage())?;
    terminal.downstream.push(summary);
    store.finish(&terminal).map_err(|_| storage())?;
    Ok(())
}
/// Release after all owned futures and persisted dispositions, including errors.
struct Writers<'a, S: Frontier> {
    /// Kernel frontier port, never another job store.
    store: &'a S,
    /// Acquired source leases, retained until the receipt is finalized.
    leases: Vec<SourceLease>,
}
impl<S: Frontier> Drop for Writers<'_, S> {
    fn drop(&mut self) {
        for writer in &self.leases {
            if self
                .store
                .release_source(writer, SystemTime::now())
                .is_err()
            {
                let remaining = writer.deadline.saturating_duration_since(Instant::now());
                let expiry = SystemTime::now()
                    .checked_add(remaining)
                    .and_then(|time| format_time(time).ok())
                    .unwrap_or_else(|| "source lease expiry".into());
                drop(writeln!(
                    io::stderr().lock(),
                    "the next sync can start after {expiry}"
                ));
            }
        }
    }
}
