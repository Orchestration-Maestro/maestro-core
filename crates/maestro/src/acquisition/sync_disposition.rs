//! Definitive historical exclusions precede readiness, allocation and dispatch holds.
use super::{
    controls::{SourceWork, decision, request, storage},
    output::{Entry, Report},
};
use crate::failure::Failure;
use maestro_acquisition::{
    Refusal,
    policy::{format_time, identity::FetchIdentity},
};
use maestro_kernel::acquisition::{Item, ItemDisposition, Receipts, StageItem};
use std::time::SystemTime;

/// Current historical references share the same definitive policy exclusions as seeds.
pub(crate) fn exclusion<S: Receipts, T>(
    work: &SourceWork<'_, S, T>,
    item: &Item,
    report: &mut Report,
) -> Result<Option<StageItem>, Failure> {
    let url = &item.request.fetch_identity;
    let time = format_time(SystemTime::now()).map_err(|_| storage())?;
    let reason = decision(
        work.policy,
        &request(&work.source.id, url, &time),
        work.runtime.controls,
    );
    let parsed = FetchIdentity::parse(work.source, url);
    let reason = if reason == "policy_denial" || parsed == Err(Refusal::Access) {
        "policy_denial"
    } else if parsed.is_err() {
        "unresolved_identity"
    } else {
        return Ok(None);
    };
    let entry = Entry::new(url, reason);
    let denied = reason == "policy_denial";
    let evidence = if denied {
        let bytes = serde_json::to_vec(&entry).map_err(|_| storage())?;
        report.discarded.push(entry);
        Some(
            work.store
                .retain(work.scope, &bytes, &[])
                .map_err(|_| storage())?,
        )
    } else {
        report.pending.push(entry);
        None
    };
    Ok(Some(StageItem {
        item: item.id.to_string().parse().map_err(|_| storage())?,
        disposition: if denied {
            ItemDisposition::Denied
        } else {
            ItemDisposition::Pending
        },
        evidence,
    }))
}
