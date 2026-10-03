//! Leased build writes: claims, receipts, renewal and progress commit together.

use super::{
    build_read::{integer, load_build},
    build_types::{Batch, BatchReceipt, BuildPlan, BuildRecord},
    error::Error,
    read::{batch_claims, load_set},
    types::{ClaimSet, ClaimSetRecord, Object},
    write::{Prepared, authorize, freeze_set},
};
use crate::{
    artifact::Digest,
    job::{self, Lease, LeaseTiming},
    scope::{ScopeSet, collection_path},
    store::Database,
};
use rusqlite::{Transaction, params};
use serde_json::json;
use std::collections::BTreeSet;

impl Database {
    /// Begin or resume a build with frozen inputs under its collection job's lease.
    ///
    /// # Errors
    /// Refuses invalid plans, missing revisions, changed plans, unauthorized
    /// collections, lost leases and database errors. Nothing is written on error.
    pub fn begin_graph_build(
        &self,
        scopes: &ScopeSet,
        lease: &Lease,
        plan: &BuildPlan,
    ) -> Result<BuildRecord, Error> {
        authorize(scopes, &plan.collection_id)?;
        validate_plan(plan)?;
        let sources = json!(plan.sources).to_string();
        self.write(|transaction| {
            let job = job::validate_lease(transaction, lease)?;
            if job.scope.as_str() != collection_path(&plan.collection_id) {
                return Err(Error::Unauthorized);
            }
            if let Some(existing) = load_build(transaction, scopes, lease.job)? {
                return (existing.plan == *plan)
                    .then_some(existing)
                    .ok_or_else(|| Error::Conflict("build plan changed".into()));
            }
            check_sources(transaction, plan)?;
            transaction.execute(
                "INSERT INTO graph_builds (job_id, collection_id, extractor, profile_digest,
                   sources_json, batch_count, max_claims, max_rejections)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    lease.job.to_string(),
                    plan.collection_id,
                    plan.provenance.extractor,
                    plan.provenance.profile.as_str(),
                    sources,
                    integer(plan.sources.len())?,
                    integer(plan.budget.max_claims)?,
                    integer(plan.budget.max_rejections)?
                ],
            )?;
            Ok(BuildRecord {
                plan: plan.clone(),
                batches: Vec::new(),
                rejections: Vec::new(),
                claim_set_id: None,
            })
        })
    }

    /// Record a source batch, its lease renewal and its journal checkpoint atomically.
    /// Replaying identical content returns its original receipt without another step.
    /// Verification I/O happens before taking the write lock; eligibility and the
    /// lease fence are checked inside it. No partial claim set is created.
    ///
    /// # Errors
    /// Refuses invalid or out-of-order batches, changed replays, exceeded budgets,
    /// invalid supports, lost leases, invalid times and database errors, without writes.
    pub fn record_graph_batch(
        &self,
        scopes: &ScopeSet,
        lease: &mut Lease,
        timing: LeaseTiming,
        batch: &Batch,
    ) -> Result<BatchReceipt, Error> {
        let build = self
            .graph_build(scopes, lease.job)?
            .ok_or(Error::UnknownBuild(lease.job))?;
        validate_batch(&build.plan, batch)?;
        // A replay needs no original artifact I/O: its acceptance is already durable.
        let set = ClaimSet {
            collection_id: build.plan.collection_id.clone(),
            claims: batch.claims.clone(),
        };
        let prepared = if batch.claims.is_empty() || batch.ordinal < build.batches.len() {
            None
        } else {
            Some(self.prepare_claims(scopes, &set)?)
        };
        let (receipt, renewed) = self.write(|transaction| {
            job::validate_lease(transaction, lease)?;
            let current = load_build(transaction, scopes, lease.job)?
                .ok_or(Error::UnknownBuild(lease.job))?;
            if let Some(receipt) = current.batches.get(batch.ordinal) {
                let stored: String = transaction.query_row(
                    "SELECT content_digest FROM graph_build_batches
                     WHERE job_id = ?1 AND ordinal = ?2",
                    params![lease.job.to_string(), integer(batch.ordinal)?],
                    |row| row.get(0),
                )?;
                return (stored == batch_digest(batch).as_str())
                    .then(|| (receipt.clone(), lease.clone()))
                    .ok_or_else(|| Error::Conflict("batch replay differs".into()));
            }
            check_next(&current, batch)?;
            let receipt = insert_batch(transaction, lease, batch, &current, prepared.as_ref())?;
            let (renewed, _) = job::checkpoint(
                transaction,
                lease,
                timing.now,
                timing.term,
                &json!({ "graph_batch": batch.ordinal, "claims": receipt.claims.len(),
                    "rejected": receipt.rejected }),
            )?;
            Ok((receipt, renewed))
        })?;
        *lease = renewed;
        Ok(receipt)
    }

    /// Freeze the completed build's admitted receipts into one immutable set.
    /// The freeze, renewal and completion checkpoint share the lease transaction.
    /// Replaying completion returns the existing set without another checkpoint.
    ///
    /// # Errors
    /// Refuses missing or incomplete builds, empty accepted sets, lost leases,
    /// invalid times and database errors. Nothing is written on error.
    pub fn finish_graph_build(
        &self,
        scopes: &ScopeSet,
        lease: &mut Lease,
        timing: LeaseTiming,
    ) -> Result<ClaimSetRecord, Error> {
        let (set, renewed) = self.write(|transaction| {
            let build = load_build(transaction, scopes, lease.job)?
                .ok_or(Error::UnknownBuild(lease.job))?;
            job::validate_lease(transaction, lease)?;
            if let Some(id) = build.claim_set_id {
                let set = load_set(transaction, scopes, &id)?
                    .ok_or_else(|| Error::Conflict("finished set missing".into()))?;
                return Ok((set, lease.clone()));
            }
            if build.batches.len() != build.plan.sources.len() {
                return Err(Error::Unfinished {
                    recorded: build.batches.len(),
                    expected: build.plan.sources.len(),
                });
            }
            let ids: Vec<_> = build
                .batches
                .iter()
                .flat_map(|batch| &batch.claims)
                .map(|claim| claim.id.as_str())
                .collect();
            if ids.is_empty() {
                return Err(Error::Invalid("build accepted no claims".into()));
            }
            let set = freeze_set(transaction, scopes, &build.plan.collection_id, &ids)?;
            transaction.execute(
                "UPDATE graph_builds SET claim_set_id = ?2 WHERE job_id = ?1",
                params![lease.job.to_string(), set.id.as_str()],
            )?;
            let (renewed, _) = job::checkpoint(
                transaction,
                lease,
                timing.now,
                timing.term,
                &json!({ "graph_claim_set": set.id.as_str() }),
            )?;
            Ok((set, renewed))
        })?;
        *lease = renewed;
        Ok(set)
    }
}

/// Reject malformed or unrepresentable frozen inputs before inserting a build.
fn validate_plan(plan: &BuildPlan) -> Result<(), Error> {
    if plan.sources.is_empty()
        || plan.budget.max_claims == 0
        || plan.provenance.extractor.is_empty()
        || plan.sources.iter().any(String::is_empty)
        || plan.sources.iter().collect::<BTreeSet<_>>().len() != plan.sources.len()
    {
        return Err(Error::Invalid("invalid graph build plan".into()));
    }
    integer(plan.sources.len())?;
    integer(plan.budget.max_claims)?;
    integer(plan.budget.max_rejections)?;
    Ok(())
}

/// Check every candidate belongs to the frozen source and extraction profile.
fn validate_batch(plan: &BuildPlan, batch: &Batch) -> Result<(), Error> {
    let expected = plan
        .sources
        .get(batch.ordinal)
        .ok_or_else(|| Error::Invalid("batch ordinal outside plan".into()))?;
    if batch.claims.iter().any(|claim| {
        claim.provenance != plan.provenance
            || claim
                .supports
                .iter()
                .any(|support| support.revision_id != *expected)
    }) || batch
        .rejections
        .iter()
        .any(|rejection| rejection.revision_id != *expected || rejection.reason.is_empty())
    {
        return Err(Error::Invalid(
            "batch does not match frozen source and profile".into(),
        ));
    }
    integer(batch.claims.len())?;
    integer(batch.rejections.len())?;
    Ok(())
}

/// Enforce ordered progress and cumulative budgets from the transaction's snapshot.
fn check_next(build: &BuildRecord, batch: &Batch) -> Result<(), Error> {
    if build.claim_set_id.is_some() || batch.ordinal != build.batches.len() {
        return Err(Error::Invalid(
            "batch is not next in the unfinished build".into(),
        ));
    }
    let recorded: usize = build
        .batches
        .iter()
        .map(|receipt| receipt.claims.len())
        .sum();
    let needed = recorded.saturating_add(batch.claims.len());
    if needed > build.plan.budget.max_claims {
        return Err(Error::OverBudget {
            limit: build.plan.budget.max_claims,
            needed,
        });
    }
    Ok(())
}

/// Digest the complete candidate receipt, including rejections not retained.
pub(super) fn batch_digest(batch: &Batch) -> Digest {
    // Claim's Debug form is not a persistence format. Explicit JSON arrays keep
    // this versioned replay identity independent of Rust formatting and field order.
    let claims: Vec<_> = batch
        .claims
        .iter()
        .map(|claim| {
            let (kind, text) = match &claim.object {
                Object::Literal(literal) => (json!(literal.kind.as_str()), &literal.lexeme),
                Object::Entity(entity) => (json!(["entity", entity.kind.as_str()]), &entity.name),
            };
            json!([
                claim.subject.kind.as_str(),
                claim.subject.name,
                claim.predicate.as_str(),
                kind,
                text,
                claim.conditions,
                validity(&claim.version),
                validity(&claim.world),
                claim.provenance.extractor,
                claim.provenance.profile.as_str(),
                claim
                    .supports
                    .iter()
                    .map(|support| json!([
                        support.revision_id,
                        support.block_id,
                        support.span.start,
                        support.span.end,
                        support.quote_digest.as_str()
                    ]))
                    .collect::<Vec<_>>()
            ])
        })
        .collect();
    let rejections: Vec<_> = batch
        .rejections
        .iter()
        .map(|rejection| json!([rejection.revision_id, rejection.block_id, rejection.reason]))
        .collect();
    Digest::of(
        json!(["maestro-graph-batch/1", batch.ordinal, claims, rejections])
            .to_string()
            .as_bytes(),
    )
}

/// Encode validity without depending on Rust's diagnostic representation.
fn validity(value: &super::types::Validity) -> serde_json::Value {
    match value {
        super::types::Validity::Unknown => serde_json::Value::Null,
        super::types::Validity::Bounded { start, end } => json!([start, end]),
    }
}

/// Insert all receipt rows and hydrate claims before the transaction commits.
fn insert_batch(
    transaction: &Transaction<'_>,
    lease: &Lease,
    batch: &Batch,
    build: &BuildRecord,
    prepared: Option<&Prepared<'_>>,
) -> Result<BatchReceipt, Error> {
    if let Some(prepared) = prepared {
        prepared.record_claims(transaction)?;
    }
    let kept =
        (build.plan.budget.max_rejections - build.rejections.len()).min(batch.rejections.len());
    let revision = build
        .plan
        .sources
        .get(batch.ordinal)
        .ok_or_else(|| Error::Invalid("batch ordinal outside plan".into()))?;
    let job = lease.job.to_string();
    let ordinal = integer(batch.ordinal)?;
    transaction.execute(
        "INSERT INTO graph_build_batches (job_id, ordinal, revision_id, lease_number,
           content_digest, claim_count, rejected, kept) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            job,
            ordinal,
            revision,
            i64::try_from(lease.number)
                .map_err(|_| Error::Invalid("lease exceeds SQLite range".into()))?,
            batch_digest(batch).as_str(),
            integer(batch.claims.len())?,
            integer(batch.rejections.len())?,
            integer(kept)?
        ],
    )?;
    if let Some(prepared) = prepared {
        for (position, id) in prepared.ids().iter().enumerate() {
            transaction.execute(
                "INSERT INTO graph_build_claims (job_id, ordinal, position, claim_id)
                 VALUES (?1, ?2, ?3, ?4)",
                params![job, ordinal, integer(position)?, id],
            )?;
        }
    }
    for (position, rejection) in batch.rejections.iter().take(kept).enumerate() {
        transaction.execute(
            "INSERT INTO graph_build_rejections
               (job_id, position, ordinal, revision_id, block_id, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                job,
                integer(build.rejections.len() + position)?,
                ordinal,
                rejection.revision_id,
                rejection.block_id,
                rejection.reason
            ],
        )?;
    }
    Ok(BatchReceipt {
        ordinal: batch.ordinal,
        revision_id: revision.clone(),
        lease_number: lease.number,
        claims: batch_claims(transaction, &job, ordinal)?,
        rejected: batch.rejections.len(),
        kept,
    })
}

/// Every planned revision must belong to this build's collection.
fn check_sources(transaction: &Transaction<'_>, plan: &BuildPlan) -> Result<(), Error> {
    for source in &plan.sources {
        let found: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM revisions
             JOIN documents ON documents.id = revisions.document_id
             WHERE revisions.id = ?1 AND documents.collection_id = ?2)",
            params![source, plan.collection_id],
            |row| row.get(0),
        )?;
        if !found {
            return Err(Error::UnknownRevision {
                revision_id: source.clone(),
            });
        }
    }
    Ok(())
}
