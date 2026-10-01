//! How a tool call ended, for its span: by the public code of the error it
//! answered with, if any, and by the name of a listed tool alone.

use maestro_kernel::telemetry::stage::Outcome;
use rmcp::{
    ErrorData as McpError,
    model::{CallToolResponse, ContentBlock, ErrorCode},
};
use serde_json::Value;

/// The name a tool call's span carries: a listed tool's, never the text a
/// client sent for another.
pub(super) fn tool_name(name: &str) -> &'static str {
    match name {
        "knowledge_collections" => "knowledge_collections",
        "knowledge_get" => "knowledge_get",
        "knowledge_search" => "knowledge_search",
        "knowledge_ask" => "knowledge_ask",
        _ => "unknown",
    }
}

/// How a tool call that answered `response` ended: refused for its request,
/// unavailable while its workers or the kernel are, out of time, or failed,
/// which includes a call its client cancelled.
pub(super) fn call_outcome(response: &Result<CallToolResponse, McpError>) -> Outcome {
    let result = match response {
        Ok(CallToolResponse::Complete(result)) => result,
        Ok(_) => return Outcome::Ok,
        Err(error) if error.code == ErrorCode::INVALID_PARAMS => return Outcome::Refused,
        Err(_) => return Outcome::Error,
    };
    if result.is_error != Some(true) {
        return Outcome::Ok;
    }
    let code = result
        .content
        .first()
        .and_then(ContentBlock::as_text)
        .and_then(|text| serde_json::from_str::<Value>(&text.text).ok())
        .and_then(|error| {
            let code = error.get("error")?.get("code")?.as_str()?;
            Some(code.to_owned())
        });
    match code.as_deref() {
        Some("busy" | "kernel_unavailable" | "answerer_unavailable") => Outcome::Unavailable,
        Some("deadline_exceeded") => Outcome::Timeout,
        Some("cancelled" | "integrity_error" | "search_failed") | None => Outcome::Error,
        Some(_) => Outcome::Refused,
    }
}
