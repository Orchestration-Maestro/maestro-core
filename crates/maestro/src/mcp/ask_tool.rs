//! MCP input and schema for `knowledge_ask`.

use crate::knowledge::RequestError;
use maestro_knowledge::answer::{Answer, AskRequest};
use rmcp::{
    ErrorData as McpError,
    handler::server::common::{schema_for_input, schema_for_output},
    model::{Tool, ToolAnnotations},
};
use serde_json::Value;

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

/// Parses one bounded, strict ask request without opening the kernel.
pub(super) fn parse(value: Value) -> Result<AskRequest, RequestError> {
    serde_json::from_value(value).map_err(|_| RequestError::InvalidArguments)
}

#[cfg(test)]
mod tests {
    use super::parse;
    use serde_json::json;

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
