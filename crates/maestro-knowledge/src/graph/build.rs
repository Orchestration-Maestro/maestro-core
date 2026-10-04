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
use serde_json::{Map, Value, json};

/// Canonical submission inputs: every field of the frozen plan participates.
#[must_use]
pub fn inputs(plan: &BuildPlan, extractor_inputs: Option<Value>) -> Value {
    let mut inputs = Map::from_iter([
        ("collection".to_owned(), json!(plan.collection_id)),
        ("extractor".to_owned(), json!(plan.provenance.extractor)),
        (
            "profile".to_owned(),
            json!(plan.provenance.profile.as_str()),
        ),
        ("sources".to_owned(), json!(plan.sources)),
        ("max_claims".to_owned(), json!(plan.budget.max_claims)),
        (
            "max_rejections".to_owned(),
            json!(plan.budget.max_rejections),
        ),
    ]);
    if let Some(extra) = extractor_inputs {
        inputs.insert("extractor_inputs".to_owned(), extra);
    }
    Value::Object(inputs)
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
