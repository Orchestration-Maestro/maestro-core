use super::search::pipeline_request;
use crate::{knowledge::SearchRequest, settings::KnowledgeSettings};
use maestro_knowledge::search::evidence::ExpansionMode;
use serde_json::json;

#[test]
fn pipeline_requests_carry_both_effective_configuration_sets() {
    let request = SearchRequest::parse(json!({
        "collection": "docs",
        "query": "question",
        "deadline_ms": 1000
    }))
    .unwrap();
    let mut settings = KnowledgeSettings::default();
    settings.search.lexical_enabled = false;
    settings.evidence.expansion = ExpansionMode::RelevantBlocks;

    let pipeline = pipeline_request(&request, &settings, request.budget());

    assert!(!pipeline.configuration.lexical_enabled);
    assert_eq!(pipeline.evidence.expansion, ExpansionMode::RelevantBlocks);
}
