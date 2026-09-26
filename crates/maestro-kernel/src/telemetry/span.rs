//! The spans the kernel opens, and the names they carry, pinned in one place.
//!
//! A rename is a change to this file, which a review sees. The names follow
//! the span taxonomy of docs/architecture/05 §4 and, where it names them, the
//! semantic conventions for generative AI.

use tracing::Span;

/// The span of one call of a registered tool.
pub const GEN_AI_EXECUTE_TOOL: &str = "gen_ai.execute_tool";

/// The attribute naming the operation a span stands for.
pub const GEN_AI_OPERATION_NAME: &str = "gen_ai.operation.name";

/// The attribute naming the tool a call runs.
pub const GEN_AI_TOOL_NAME: &str = "gen_ai.tool.name";

/// The operation of a tool call: the value of [`GEN_AI_OPERATION_NAME`] on
/// a [`GEN_AI_EXECUTE_TOOL`] span.
pub const EXECUTE_TOOL: &str = "execute_tool";

/// The span of one call of `tool`: [`GEN_AI_EXECUTE_TOOL`], carrying the
/// operation and the tool's name. Enter it for the length of the call.
#[must_use]
pub fn tool_call(tool: &str) -> Span {
    tracing::info_span!(
        GEN_AI_EXECUTE_TOOL,
        { GEN_AI_OPERATION_NAME } = EXECUTE_TOOL,
        { GEN_AI_TOOL_NAME } = tool,
    )
}
