//! Read-only MCP tools backed by fresh, local-principal kernel operations.

use crate::{
    failure::Failure,
    kernel::Kernel,
    knowledge::{
        CollectionsData, GetData, GetRequest, KnowledgeError, RESPONSE_LIMIT_BYTES, RequestError,
        collections_with, ensure_current_scopes, get_with,
    },
};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::common::{schema_for_input, schema_for_output},
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorCode,
        Implementation, ListToolsResult, MetaObject, PaginatedRequestParams, ProtocolVersion,
        RequestId, ServerCapabilities, ServerConfig, ServerResult, Tool, ToolAnnotations,
    },
    service::{RequestContext, RoleServer, TxJsonRpcMessage},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    task::spawn_blocking,
    time::sleep,
};

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

/// Schema identifier embedded in JSON text for MCP tool errors.
const TOOL_ERROR_SCHEMA: &str = "maestro-mcp-error/1";
/// Maximum number of simultaneous local knowledge operations.
const WORKER_LIMIT: usize = 4;
/// Per-call deadline; blocking workers retain their permit until the operation ends.
const CALL_DEADLINE: Duration = Duration::from_secs(15);
/// The local kernel opener kept behind a private test seam.
type KernelOpener = Arc<dyn Fn() -> Result<Kernel, Failure> + Send + Sync>;

/// MCP response fields needed to measure the exact wire representation.
struct ResponseContext<'a> {
    /// Request ID serialized by the protocol transport.
    request_id: &'a RequestId,
    /// Peer protocol version, or `None` before version negotiation.
    protocol_version: Option<&'a ProtocolVersion>,
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

/// The only server state is a hard cap on blocking local-kernel workers.
pub(super) struct KnowledgeServer {
    /// Hard limit on operations running against the local kernel.
    pub(super) workers: Arc<Semaphore>,
    /// One fresh local kernel per call, injectable only inside this module's tests.
    open_kernel: KernelOpener,
    /// Bounds how long a caller waits for one local operation.
    call_deadline: Duration,
}

impl KnowledgeServer {
    /// Creates the read-only local knowledge server.
    pub(super) fn new() -> Self {
        Self {
            workers: Arc::new(Semaphore::new(WORKER_LIMIT)),
            open_kernel: Arc::new(Kernel::open),
            call_deadline: CALL_DEADLINE,
        }
    }

    /// Creates a bounded test server with an isolated kernel opener and deadline.
    #[cfg(test)]
    pub(super) fn with_kernel_opener(
        open_kernel: impl Fn() -> Result<Kernel, Failure> + Send + Sync + 'static,
        call_deadline: Duration,
    ) -> Self {
        Self {
            workers: Arc::new(Semaphore::new(WORKER_LIMIT)),
            open_kernel: Arc::new(open_kernel),
            call_deadline,
        }
    }
}

impl ServerHandler for KnowledgeServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("maestro", env!("CARGO_PKG_VERSION")))
            .with_instructions(concat!(
                "Read collection metadata or exact source-backed chunks granted ",
                "to the local principal.",
            ))
    }

    /// Lists tools locally; the trait requires an async method even without I/O.
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "ServerHandler requires an async method, but listing tools is local"
    )]
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(tool_definitions()?))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tool_definitions()
            .ok()?
            .into_iter()
            .find(|tool| tool.name.as_ref() == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let operation = match parse_operation(request.name.as_ref(), arguments) {
            Ok(operation) => operation,
            Err(InputFailure::Protocol(error)) => return Err(error),
            Err(InputFailure::Tool { code, message }) => {
                return Ok(tool_error(code, message, false, Vec::new()).into());
            }
        };
        let Ok(permit) = self.workers.clone().try_acquire_owned() else {
            return Ok(tool_error(
                "busy",
                "the local knowledge server is busy",
                false,
                Vec::new(),
            )
            .into());
        };

        let cancellation = context.ct.clone();
        let worker_cancellation = cancellation.clone();
        let request_id = context.id.clone();
        let protocol_version = context.protocol_version();
        let open_kernel = self.open_kernel.clone();
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
            () = sleep(self.call_deadline) => Ok(tool_error(
                "deadline_exceeded",
                "the local knowledge operation exceeded its deadline",
                false,
                Vec::new(),
            ).into()),
        }
    }
}

/// Creates the same strict schemas for tools/list and the SDK's tool validation hook.
fn tool_definitions() -> Result<Vec<Tool>, McpError> {
    let mut collections_input = schema_for_input::<CollectionsRequest>()
        .map_err(|_| McpError::internal_error("tool schema unavailable", None))?;
    let get_input = schema_for_input::<GetRequest>()
        .map_err(|_| McpError::internal_error("tool schema unavailable", None))?;
    Arc::make_mut(&mut collections_input).insert("properties".to_owned(), json!({}));
    let collections = Tool::new(
        "knowledge_collections",
        "List collection titles and published generations visible to the local principal.",
        collections_input,
    )
    .with_raw_output_schema(schema_for_output::<CollectionsData>())
    .with_annotations(
        ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .open_world(false)
            .idempotent(true),
    );
    let get = Tool::new(
        "knowledge_get",
        "Read an exact source-backed chunk from a visible published or retained generation.",
        get_input,
    )
    .with_raw_output_schema(schema_for_output::<GetData>())
    .with_annotations(
        ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .open_world(false)
            .idempotent(true),
    );
    Ok(vec![collections, get])
}

/// A parsed operation before it acquires a blocking worker.
enum Operation {
    /// List local collection metadata.
    Collections,
    /// Retrieve one exact source-backed chunk.
    Get(GetRequest),
}

/// Input errors distinguish malformed protocol requests from expected tool refusals.
enum InputFailure {
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
fn parse_operation(name: &str, arguments: Value) -> Result<Operation, InputFailure> {
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
        _ => Err(InputFailure::Protocol(McpError::new(
            ErrorCode::INVALID_PARAMS,
            "unknown tool",
            None,
        ))),
    }
}

/// Runs one operation on a bounded blocking worker and bounds the exact response message.
fn run_operation(
    operation: Operation,
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
        Operation::Collections => (
            "the collection response exceeds the response limit",
            "collections",
        ),
        Operation::Get(_) => ("the exact excerpt exceeds the response limit", "excerpt"),
    };
    let (result, admitted) = match operation {
        Operation::Collections => match collections_with(|| open_kernel()) {
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
        Operation::Get(request) => match get_with(
            || open_kernel(),
            &request.chunk_id,
            request.collection.as_deref(),
            request.generation,
        ) {
            Ok(scoped) => (
                get_result(scoped.data),
                Some((scoped.kernel, scoped.scopes)),
            ),
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
    Ok(CallToolResult::structured(value))
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
        let mut result = CallToolResult::structured(value);
        if output.truncated {
            let mut metadata = MetaObject::new();
            metadata.0.insert(
                "maestro/truncation".to_owned(),
                json!({
                    "truncated": true,
                    "limit_bytes": RESPONSE_LIMIT_BYTES,
                    "omitted": &output.omitted,
                    "warning": "Some collection entries were omitted to fit the response limit.",
                }),
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
fn operation_error(error: KnowledgeError) -> CallToolResult {
    match error {
        KnowledgeError::Refused { code, message } | KnowledgeError::Failed { code, message } => {
            tool_error(code, message, false, Vec::new())
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
                    json!({
                        "truncated": true,
                        "limit_bytes": RESPONSE_LIMIT_BYTES,
                        "omitted": metadata_omitted,
                    }),
                );
                result = result.with_meta(Some(metadata));
            }
            result
        }
        Err(_) => CallToolResult::error(vec![ContentBlock::text("the request failed")]),
    }
}

/// Measures the actual framed JSON-RPC response for the caller's ID and protocol version.
pub(super) fn response_fits(
    result: &CallToolResult,
    request_id: &RequestId,
    protocol_version: Option<&ProtocolVersion>,
) -> Result<bool, McpError> {
    Ok(response_size(result, request_id, protocol_version)? <= RESPONSE_LIMIT_BYTES)
}

/// Measures the complete response line, including its framing newline.
pub(super) fn response_size(
    result: &CallToolResult,
    request_id: &RequestId,
    protocol_version: Option<&ProtocolVersion>,
) -> Result<usize, McpError> {
    let mut result: ServerResult = CallToolResponse::Complete(result.clone()).into();
    if protocol_version
        .is_none_or(|version| version.as_str() < ProtocolVersion::V_2026_07_28.as_str())
    {
        result.strip_result_type_for_legacy_peer();
    }
    let response = TxJsonRpcMessage::<RoleServer>::response(result, request_id.clone());
    let bytes = serde_json::to_vec(&response)
        .map_err(|_| McpError::internal_error("response serialization failed", None))?;
    Ok(bytes.len().saturating_add(1))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod wire_tests;
