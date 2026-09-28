//! Frozen build identity and source-bounded extraction outside write transactions.

use super::{
    rules::Extractor,
    verify::{Source, SourceError},
};
use maestro_kernel::store::Database;
use maestro_kernel::{
    document::Revision,
    facts::{Batch, BuildPlan, Rejection},
    store,
};
use serde_json::{Value, json};

/// Canonical submission inputs: every field of the frozen plan participates.
#[must_use]
pub fn inputs(plan: &BuildPlan) -> Value {
    json!({"collection": plan.collection_id, "extractor": plan.provenance.extractor,
        "profile": plan.provenance.profile.as_str(), "sources": plan.sources,
        "max_claims": plan.budget.max_claims, "max_rejections": plan.budget.max_rejections})
}

/// Extract one source through the replaceable extractor; no authority writes occur.
///
/// # Errors
/// Artifact store failures are retryable failures, not candidate rejections.
pub fn extract(
    database: &Database,
    extractor: &dyn Extractor,
    revision: &Revision,
    ordinal: usize,
) -> Result<Batch, store::Error> {
    let mut batch = Batch {
        ordinal,
        claims: Vec::new(),
        rejections: Vec::new(),
    };
    match Source::read(database, revision) {
        Ok(source) => {
            let found = extractor.extract(&source);
            batch.claims = found.claims;
            batch.rejections = found
                .rejections
                .into_iter()
                .map(|rejected| Rejection {
                    revision_id: rejected.revision_id,
                    block_id: rejected.block_id,
                    reason: rejected.reason,
                })
                .collect();
        }
        Err(SourceError::Store(error)) => return Err(error),
        Err(error) => batch.rejections.push(Rejection {
            revision_id: revision.id.clone(),
            block_id: None,
            reason: error.to_string(),
        }),
    }
    Ok(batch)
}
