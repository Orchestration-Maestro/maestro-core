//! Aggregate-only output for model-backed graph extraction.

use maestro_kernel::{
    document::Revision,
    facts::{ClaimSetRecord, Provenance},
};
use maestro_knowledge::graph::rules::Rejection;
use serde::Serialize;
use ulid::Ulid;

/// Version of the aggregate-only model build output.
const SCHEMA: &str = "maestro-cli/knowledge-graph-extract-summary/1";

/// Safe aggregate output; this type cannot serialize claim names or values.
#[derive(Debug, Serialize)]
pub(super) struct Document<'a> {
    /// The summary contract.
    schema: &'static str,
    /// Durable job identity.
    job: String,
    /// Collection application ID.
    collection: &'a str,
    /// Extractor card application ID.
    extractor: &'a str,
    /// Frozen extraction profile digest.
    profile: &'a str,
    /// Source revision application IDs.
    revisions: Vec<&'a str>,
    /// Completed claim-set digest, if any.
    claim_set: Option<&'a str>,
    /// Number of accepted claims.
    accepted: usize,
    /// Total candidates rejected.
    rejected: usize,
    /// Number of retained rejection receipts.
    retained_rejections: usize,
    /// Retained rejection IDs and fixed reason code only.
    rejections: Vec<RejectionDocument<'a>>,
}

/// One sanitized model-extraction rejection receipt.
#[derive(Debug, Serialize)]
struct RejectionDocument<'a> {
    /// Source revision application ID.
    revision: &'a str,
    /// Canonical source block ID when known.
    block: Option<&'a str>,
    /// Fixed code; never a model-written value.
    code: &'static str,
}

/// Inputs needed to render the aggregate-only build summary.
#[derive(Clone, Copy)]
pub(super) struct Input<'a> {
    /// Collection application ID.
    pub(super) collection: &'a str,
    /// Frozen extractor identity.
    pub(super) provenance: &'a Provenance,
    /// Eligible source revisions.
    pub(super) revisions: &'a [Revision],
    /// Durable build job ID.
    pub(super) job: Ulid,
    /// Completed authority-backed claim set, if any.
    pub(super) record: Option<&'a ClaimSetRecord>,
    /// Total candidate rejection count.
    pub(super) rejected: usize,
    /// Retained refusal receipts.
    pub(super) rejections: &'a [Rejection],
}

/// Builds an output that contains only aggregates, IDs and digests.
pub(super) fn document(input: Input<'_>) -> Document<'_> {
    let Input {
        collection,
        provenance,
        revisions,
        job,
        record,
        rejected,
        rejections,
    } = input;
    Document {
        schema: SCHEMA,
        job: job.to_string(),
        collection,
        extractor: &provenance.extractor,
        profile: provenance.profile.as_str(),
        revisions: revisions
            .iter()
            .map(|revision| revision.id.as_str())
            .collect(),
        claim_set: record.map(|record| record.id.as_str()),
        accepted: record.map_or(0, |record| record.claims.len()),
        rejected,
        retained_rejections: rejections.len(),
        rejections: rejections
            .iter()
            .map(|rejection| RejectionDocument {
                revision: &rejection.revision_id,
                block: rejection.block_id.as_deref(),
                code: rejection_code(&rejection.reason),
            })
            .collect(),
    }
}

/// Maps fixed extraction refusal reasons to sanitized output codes.
fn rejection_code(reason: &str) -> &'static str {
    match reason {
        "source verification failed" => "source_verification_failed",
        "extractor call refused" => "extractor_call_refused",
        "block belongs to another revision" => "foreign_source_block",
        "ambiguous source block span" => "ambiguous_source_span",
        "invalid source block span" => "invalid_source_span",
        "window count exceeds policy" => "window_limit_exceeded",
        "window policy splits a UTF-8 character" => "window_splits_character",
        "window overlap prevents progress" => "window_overlap_prevents_progress",
        "predicate is not claimable" => "predicate_not_claimable",
        "empty quote" => "empty_quote",
        "quote is not in its source window" => "quote_not_in_window",
        "ambiguous quote" => "ambiguous_quote",
        "quote source mismatch" => "quote_source_mismatch",
        "quote splits a UTF-8 character" => "quote_splits_character",
        _ => "candidate_rejected",
    }
}

/// User-visible summary with no model-written text.
pub(super) fn summary(document: &Document<'_>) -> String {
    format!(
        "graph extraction: {} accepted, {} rejected ({} retained); job {}",
        document.accepted, document.rejected, document.retained_rejections, document.job
    )
}

#[cfg(test)]
mod tests {
    use super::{Input, document, rejection_code};
    use maestro_kernel::{artifact::Digest, facts::Provenance};
    use maestro_knowledge::graph::rules::Rejection;
    use ulid::Ulid;

    #[test]
    fn fixed_rejection_reasons_map_to_closed_output_codes() {
        for (reason, code) in [
            ("source verification failed", "source_verification_failed"),
            ("extractor call refused", "extractor_call_refused"),
            ("block belongs to another revision", "foreign_source_block"),
            ("ambiguous source block span", "ambiguous_source_span"),
            ("invalid source block span", "invalid_source_span"),
            ("window count exceeds policy", "window_limit_exceeded"),
            (
                "window policy splits a UTF-8 character",
                "window_splits_character",
            ),
            (
                "window overlap prevents progress",
                "window_overlap_prevents_progress",
            ),
            ("predicate is not claimable", "predicate_not_claimable"),
            ("empty quote", "empty_quote"),
            ("quote is not in its source window", "quote_not_in_window"),
            ("ambiguous quote", "ambiguous_quote"),
            ("quote source mismatch", "quote_source_mismatch"),
            ("quote splits a UTF-8 character", "quote_splits_character"),
        ] {
            assert_eq!(rejection_code(reason), code, "{reason}");
        }
        assert_eq!(rejection_code("PRIVATE_MODEL_QUOTE"), "candidate_rejected");
    }

    #[test]
    fn model_written_rejection_text_is_never_serialized() {
        let rejection = Rejection {
            revision_id: "rev-id".to_owned(),
            block_id: Some("block-id".to_owned()),
            reason: "PRIVATE_MODEL_QUOTE".to_owned(),
        };
        let provenance = Provenance {
            extractor: "model/card-digest".to_owned(),
            profile: Digest::of(b"profile"),
        };
        let rejections = [rejection];
        let report = document(Input {
            collection: "collection-id",
            provenance: &provenance,
            revisions: &[],
            job: Ulid::nil(),
            record: None,
            rejected: 1,
            rejections: &rejections,
        });
        assert_eq!(
            super::summary(&report),
            format!(
                "graph extraction: 0 accepted, 1 rejected (1 retained); job {}",
                Ulid::nil()
            )
        );
        let text = serde_json::to_string(&report).expect("summary serializes");
        assert!(!text.contains("PRIVATE_MODEL_QUOTE"));
        assert!(text.contains("candidate_rejected"));
    }
}
