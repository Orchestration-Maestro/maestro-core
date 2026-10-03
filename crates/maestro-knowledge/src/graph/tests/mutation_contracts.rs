//! Observable diagnostics and storage-free extractor defaults.

use super::support::{markdown, rule_text, source};
use crate::{
    answer::LanguageCheck,
    graph::{
        descriptors::DescriptorError,
        resolve::ResolutionError,
        rules::{Extractor as _, TableRule},
    },
};
use maestro_kernel::artifact::Digest;

#[test]
fn table_rule_has_no_model_budget_estimate_or_extra_job_inputs() {
    let rule = TableRule::parse(&rule_text()).unwrap();
    assert_eq!(rule.token_budget(), None);
    assert_eq!(rule.estimated_tokens(&source(&markdown())), Ok(0));
    assert_eq!(rule.job_inputs(), None);
}

#[test]
fn graph_diagnostics_preserve_refusals_and_review_identity() {
    assert_eq!(
        DescriptorError::Refused("invalid pin".into()).to_string(),
        "invalid pin"
    );
    let entity = Digest::of(b"synthetic held entity");
    assert_eq!(
        DescriptorError::HeldForReview(entity.clone()).to_string(),
        format!("entity {} held for review", entity.as_str())
    );
    assert_eq!(
        ResolutionError.to_string(),
        "unsupported resolver version, unknown endpoint, cycle or unmatched separation"
    );
    assert_eq!(LanguageCheck::Unchecked.name(), "unchecked");
}
