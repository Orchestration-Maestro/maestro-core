//! MCP input and schema for `knowledge_ask`.

use crate::{
    knowledge::{RequestError, with_defaults},
    settings::KnowledgeSettings,
};
use maestro_knowledge::answer::{Answer, AskRequest};
use rmcp::{
    ErrorData as McpError,
    handler::server::common::{schema_for_input, schema_for_output},
    model::{Tool, ToolAnnotations},
};
use serde_json::{Map, Value};

/// Creates the strict request and response schemas for the answer tool.
pub(super) fn definition() -> Result<Tool, McpError> {
    let input = schema_for_input::<AskRequest>()
        .map_err(|_| McpError::internal_error("tool schema unavailable", None))?;
    Ok(Tool::new(
        "knowledge_ask",
        "Answer from visible collection evidence, or refuse when the passages do not suffice.",
        input,
    )
    .with_raw_output_schema(schema_for_output::<Answer>())
    .with_annotations(
        ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .open_world(false)
            .idempotent(true),
    ))
}

/// Parses one bounded, strict ask request without opening the kernel; the
/// session's `settings` fill the model and the budget's bounds a call leaves
/// out.
pub(super) fn parse(
    value: Value,
    settings: &KnowledgeSettings,
) -> Result<AskRequest, RequestError> {
    let mut value = with_defaults(value, &[("model", settings.model.as_str().into())]);
    if let Value::Object(object) = &mut value {
        let budget = settings.ask_budget;
        let given = object
            .entry("budget")
            .or_insert_with(|| Value::Object(Map::new()));
        *given = with_defaults(
            given.take(),
            &[
                ("k", budget.k.into()),
                ("max_tokens", budget.max_tokens.into()),
                ("search_deadline_ms", budget.search_deadline_ms.into()),
                ("output_tokens", budget.output_tokens.into()),
            ],
        );
    }
    serde_json::from_value(value).map_err(|_| RequestError::InvalidArguments)
}

#[cfg(test)]
mod tests {
    use super::parse as parse_with;
    use crate::{knowledge::RequestError, settings::KnowledgeSettings};
    use maestro_knowledge::answer::{AskBudget, AskRequest};
    use serde_json::{Value, json};

    /// `value` parsed under today's settings.
    fn parse(value: Value) -> Result<AskRequest, RequestError> {
        parse_with(value, &KnowledgeSettings::default())
    }

    #[test]
    fn the_session_settings_fill_what_a_call_leaves_out_and_a_given_value_wins() {
        let settings = KnowledgeSettings {
            model: "qwen3-8b".to_owned(),
            ask_budget: AskBudget {
                k: 7,
                max_tokens: 3000,
                search_deadline_ms: 8000,
                output_tokens: Some(512),
            },
            ..KnowledgeSettings::default()
        };
        let filled = parse_with(json!({"collection": "docs", "question": "How?"}), &settings)
            .expect("valid bounded ask");
        assert_eq!(filled.model, "qwen3-8b");
        assert_eq!(filled.budget, settings.ask_budget);
        let given = parse_with(
            json!({
                "collection": "docs",
                "question": "How?",
                "model": "qwen3-4b",
                "budget": {"k": 2, "output_tokens": 64}
            }),
            &settings,
        )
        .expect("valid bounded ask");
        assert_eq!(given.model, "qwen3-4b");
        assert_eq!(
            given.budget,
            AskBudget {
                k: 2,
                output_tokens: Some(64),
                ..settings.ask_budget
            }
        );
        assert!(
            parse_with(
                json!({"collection": "docs", "question": "How?", "budget": 5}),
                &settings
            )
            .is_err()
        );
    }

    #[test]
    fn asks_reject_unknown_fields_and_use_the_shared_model_default() {
        let request = parse(json!({
            "collection": "docs",
            "question": "How is the service configured?"
        }))
        .expect("valid bounded ask");
        assert_eq!(request.model, "qwen3-4b");
        assert!(
            parse(json!({
                "collection": "docs",
                "question": "How is the service configured?",
                "principal": "untrusted"
            }))
            .is_err()
        );
    }

    #[test]
    fn an_ask_leaves_its_output_tokens_to_the_answerer_card_unless_it_sets_them() {
        let ask = |budget| {
            parse(json!({
                "collection": "docs",
                "question": "How is the service configured?",
                "budget": budget
            }))
            .expect("valid bounded ask")
            .budget
            .output_tokens
        };
        assert_eq!(ask(json!({})), None);
        assert_eq!(ask(json!({"output_tokens": null})), None);
        assert_eq!(ask(json!({"output_tokens": 2048})), Some(2048));
    }

    #[test]
    fn i3_ask_tool_does_not_accept_a_language_parameter() {
        assert!(
            parse(json!({
                "collection": "docs",
                "question": "How is the service configured?",
                "language": "french"
            }))
            .is_err()
        );
    }
}
