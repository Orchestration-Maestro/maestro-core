//! Strict MCP request parsing and bounded kernel workers.

use super::{
    super::ask_tool as ask,
    response::{
        CollectionsOutput, ResponseContext, bounded_collections_result, get_result,
        operation_error, response_fits, tool_error,
    },
    types::KernelOpener,
};
use crate::{
    knowledge::operations::{
        ask::run::ask_with, collections_with, ensure_current_scopes, get_with,
    },
    knowledge::{GetRequest, RequestError, SearchRequest},
};
use maestro_knowledge::answer::AskRequest;
use rmcp::{
    ErrorData as McpError,
    model::{CallToolResponse, CallToolResult, ErrorCode},
    service::{RequestContext, RoleServer},
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::{sync::OwnedSemaphorePermit, task::spawn_blocking, time::sleep};

/// The empty strict argument object for `knowledge_collections`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CollectionsRequest {}

impl CollectionsRequest {
    /// Parses the required empty object.
    pub(super) fn parse(value: Value) -> Result<Self, RequestError> {
        serde_json::from_value(value).map_err(|_| RequestError::InvalidArguments)
    }
}

/// A parsed operation before it acquires a blocking worker.
pub(super) enum Operation {
    /// List local collection metadata.
    Collections,
    /// Retrieve one exact source-backed chunk or section.
    Get(GetRequest),
    /// Search one visible published generation.
    Search(SearchRequest),
    /// Answer from one visible published generation.
    Ask(AskRequest),
}

/// The synchronous operations executed by the bounded blocking-worker path.
pub(super) enum BlockingOperation {
    /// List local collection metadata.
    Collections,
    /// Retrieve one exact source-backed chunk or section.
    Get(GetRequest),
    /// Answer from evidence, or return a safe refusal.
    Ask(AskRequest),
}

/// Input errors distinguish malformed protocol requests from expected tool refusals.
pub(super) enum InputFailure {
    /// Invalid JSON-RPC arguments or an unknown tool.
    Protocol(McpError),
    /// A valid tool request that is refused by its public contract.
    Tool {
        /// Stable public error code.
        code: &'static str,
        /// Privacy-safe explanation.
        message: &'static str,
    },
}

/// Parses strict tool arguments without consulting the kernel.
pub(super) fn parse_operation(name: &str, arguments: Value) -> Result<Operation, InputFailure> {
    match name {
        "knowledge_collections" => CollectionsRequest::parse(arguments)
            .map(|_| Operation::Collections)
            .map_err(|error| InputFailure::Tool {
                code: error.code(),
                message: error.message(),
            }),
        "knowledge_get" => GetRequest::parse(arguments)
            .map(Operation::Get)
            .map_err(|error| InputFailure::Tool {
                code: error.code(),
                message: error.message(),
            }),
        "knowledge_search" => SearchRequest::parse(arguments)
            .map(Operation::Search)
            .map_err(|error| InputFailure::Tool {
                code: error.code(),
                message: error.message(),
            }),
        "knowledge_ask" => {
            ask::parse(arguments)
                .map(Operation::Ask)
                .map_err(|error| InputFailure::Tool {
                    code: error.code(),
                    message: error.message(),
                })
        }
        _ => Err(InputFailure::Protocol(McpError::new(
            ErrorCode::INVALID_PARAMS,
            "unknown tool",
            None,
        ))),
    }
}

/// Runs a synchronous operation on one bounded blocking worker.
pub(super) async fn call_blocking_operation(
    operation: BlockingOperation,
    permit: OwnedSemaphorePermit,
    open_kernel: KernelOpener,
    call_deadline: Duration,
    context: RequestContext<RoleServer>,
) -> Result<CallToolResponse, McpError> {
    let cancellation = context.ct.clone();
    let worker_cancellation = cancellation.clone();
    let request_id = context.id.clone();
    let protocol_version = context.protocol_version();
    let work = spawn_blocking(move || {
        let response = ResponseContext {
            request_id: &request_id,
            protocol_version: protocol_version.as_ref(),
        };
        run_operation(
            operation,
            permit,
            &open_kernel,
            move || worker_cancellation.is_cancelled(),
            &response,
        )
    });
    tokio::select! {
        result = work => match result {
            Ok(result) => result.map(Into::into),
            Err(_) => Err(McpError::internal_error("knowledge operation failed", None)),
        },
        () = cancellation.cancelled() => Ok(tool_error(
            "cancelled",
            "the request was cancelled before a result was delivered",
            false,
            Vec::new(),
        ).into()),
        () = sleep(call_deadline) => Ok(tool_error(
            "deadline_exceeded",
            "the local knowledge operation exceeded its deadline",
            false,
            Vec::new(),
        ).into()),
    }
}

/// Runs one operation on a bounded blocking worker and bounds the exact response message.
fn run_operation(
    operation: BlockingOperation,
    _permit: OwnedSemaphorePermit,
    open_kernel: &KernelOpener,
    cancelled: impl Fn() -> bool,
    response: &ResponseContext<'_>,
) -> Result<CallToolResult, McpError> {
    if cancelled() {
        return Ok(tool_error(
            "cancelled",
            "the request was cancelled before a result was delivered",
            false,
            Vec::new(),
        ));
    }
    let (size_message, omitted) = match &operation {
        BlockingOperation::Collections => (
            "the collection response exceeds the response limit",
            "collections",
        ),
        BlockingOperation::Get(_) => ("the exact excerpt exceeds the response limit", "excerpt"),
        BlockingOperation::Ask(_) => ("the answer exceeds the response limit", "answer"),
    };
    let (result, admitted) = match operation {
        BlockingOperation::Collections => match collections_with(|| open_kernel()) {
            Ok(scoped) => {
                let kernel = scoped.kernel;
                let scopes = scoped.scopes;
                let result = bounded_collections_result(
                    CollectionsOutput {
                        data: scoped.data,
                        truncated: false,
                        omitted: Vec::new(),
                    },
                    response.request_id,
                    response.protocol_version,
                );
                (result, Some((kernel, scopes)))
            }
            Err(error) => (Ok(operation_error(error)), None),
        },
        BlockingOperation::Get(request) => match get_with(|| open_kernel(), &request) {
            Ok(scoped) => (
                get_result(scoped.data),
                Some((scoped.kernel, scoped.scopes)),
            ),
            Err(error) => (Ok(operation_error(error)), None),
        },
        BlockingOperation::Ask(request) => match ask_with(|| open_kernel(), &request) {
            Ok(scoped) => {
                let value = serde_json::to_value(scoped.data)
                    .map_err(|_| McpError::internal_error("response serialization failed", None));
                (
                    value.map(CallToolResult::structured),
                    Some((scoped.kernel, scoped.scopes)),
                )
            }
            Err(error) => (Ok(operation_error(error)), None),
        },
    };
    let result = result?;
    let response_fits = response_fits(&result, response.request_id, response.protocol_version)?;
    if let Some((mut kernel, scopes)) = admitted
        && let Err(error) = ensure_current_scopes(&mut kernel, &scopes)
    {
        return Ok(operation_error(error));
    }
    if cancelled() {
        return Ok(tool_error(
            "cancelled",
            "the request was cancelled before a result was delivered",
            false,
            Vec::new(),
        ));
    }
    if response_fits {
        Ok(result)
    } else {
        Ok(tool_error(
            "response_too_large",
            size_message,
            true,
            vec![omitted],
        ))
    }
}
