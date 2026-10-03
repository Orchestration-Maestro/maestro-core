//! Bounded MCP response shapes, errors and wire-size checks.

use super::types::TOOL_ERROR_SCHEMA;
pub(super) use crate::knowledge::output::response_fits;
#[cfg(test)]
pub(super) use crate::knowledge::output::response_size;
use crate::knowledge::output::structured;
use crate::knowledge::{
    RESPONSE_LIMIT_BYTES,
    operations::{CollectionsData, GetData, KnowledgeError},
};
use maestro_kernel::json::canonical;
use rmcp::{
    ErrorData as McpError,
    model::{CallToolResult, ContentBlock, MetaObject, ProtocolVersion, RequestId},
};
use serde::Serialize;
use serde_json::json;

/// MCP response fields needed to measure the exact wire representation.
pub(super) struct ResponseContext<'a> {
    /// Request ID serialized by the protocol transport.
    pub(super) request_id: &'a RequestId,
    /// Peer protocol version, or `None` before version negotiation.
    pub(super) protocol_version: Option<&'a ProtocolVersion>,
}

/// Internal collection response state; truncation details travel in `_meta`.
#[derive(Debug)]
pub(super) struct CollectionsOutput {
    /// Shared versioned collection data.
    pub(super) data: CollectionsData,
    /// Whether trailing collection entries were omitted.
    pub(super) truncated: bool,
    /// Names of omitted result fields, empty when the complete result fits.
    pub(super) omitted: Vec<&'static str>,
}

/// A bounded, typed tool error safe for clients and logs.
#[derive(Debug, Serialize)]
struct ToolErrorOutput {
    /// Error contract.
    schema: &'static str,
    /// Public error code and message.
    error: ToolErrorBody,
    /// Whether the response was reduced to fit the wire limit.
    truncated: bool,
    /// Maximum serialized response size.
    limit_bytes: usize,
    /// Names of omitted result fields.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    omitted: Vec<&'static str>,
}

/// Public fields for a bounded MCP tool failure.
#[derive(Debug, Serialize)]
struct ToolErrorBody {
    /// Stable public error code.
    code: &'static str,
    /// Privacy-safe diagnostic.
    message: &'static str,
}

/// Serializes an exact get result; any oversized excerpt is refused whole.
pub(super) fn get_result(data: GetData) -> Result<CallToolResult, McpError> {
    if data.excerpt.text.len() > RESPONSE_LIMIT_BYTES {
        return Ok(tool_error(
            "response_too_large",
            "the exact excerpt exceeds the response limit",
            true,
            vec!["excerpt"],
        ));
    }
    let value = serde_json::to_value(data)
        .map_err(|_| McpError::internal_error("response serialization failed", None))?;
    Ok(structured(value))
}

/// Drops trailing collection records until a response fits, never slicing a record.
pub(super) fn bounded_collections_result(
    mut output: CollectionsOutput,
    request_id: &RequestId,
    protocol_version: Option<&ProtocolVersion>,
) -> Result<CallToolResult, McpError> {
    loop {
        let value = serde_json::to_value(&output.data)
            .map_err(|_| McpError::internal_error("response serialization failed", None))?;
        let mut result = structured(value);
        if output.truncated {
            let mut metadata = MetaObject::new();
            metadata.0.insert(
                "maestro/truncation".to_owned(),
                canonical(json!({
                    "truncated": true,
                    "limit_bytes": RESPONSE_LIMIT_BYTES,
                    "omitted": &output.omitted,
                    "warning": "Some collection entries were omitted to fit the response limit.",
                })),
            );
            result = result.with_meta(Some(metadata));
        }
        if response_fits(&result, request_id, protocol_version)? {
            return Ok(result);
        }
        if output.data.collections.pop().is_none() {
            return Ok(tool_error(
                "response_too_large",
                "the collection response exceeds the response limit",
                true,
                vec!["collections"],
            ));
        }
        output.truncated = true;
        output.omitted = vec!["collections"];
    }
}

/// Builds a bounded typed operation failure.
pub(super) fn operation_error(error: KnowledgeError) -> CallToolResult {
    match error {
        KnowledgeError::Refused { code, message } | KnowledgeError::Failed { code, message } => {
            let truncated = code == "response_too_large";
            let omitted = if truncated {
                vec!["excerpt"]
            } else {
                Vec::new()
            };
            tool_error(code, message, truncated, omitted)
        }
    }
}

/// Creates a bounded JSON-text, privacy-safe tool error.
pub(super) fn tool_error(
    code: &'static str,
    message: &'static str,
    truncated: bool,
    omitted: Vec<&'static str>,
) -> CallToolResult {
    let metadata_omitted = omitted.clone();
    let error = ToolErrorOutput {
        schema: TOOL_ERROR_SCHEMA,
        error: ToolErrorBody { code, message },
        truncated,
        limit_bytes: RESPONSE_LIMIT_BYTES,
        omitted,
    };
    match serde_json::to_string(&error) {
        Ok(text) => {
            let mut result = CallToolResult::error(vec![ContentBlock::text(text)]);
            if truncated {
                let mut metadata = MetaObject::new();
                metadata.0.insert(
                    "maestro/truncation".to_owned(),
                    canonical(json!({
                        "truncated": true,
                        "limit_bytes": RESPONSE_LIMIT_BYTES,
                        "omitted": metadata_omitted,
                    })),
                );
                result = result.with_meta(Some(metadata));
            }
            result
        }
        Err(_) => CallToolResult::error(vec![ContentBlock::text("the request failed")]),
    }
}
