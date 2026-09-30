//! MCP protocol handler and advertised knowledge tool schemas.

use super::{
    super::{
        ask_tool as ask,
        outcome::{call_outcome, tool_name},
    },
    knowledge_server::KnowledgeServer,
    operations::{
        BlockingOperation, InputFailure, Operation, call_blocking_operation, parse_operation,
    },
    response::tool_error,
    search,
    types::ASK_CALL_DEADLINE,
};
use crate::knowledge::{
    GetRequest, SearchRequest,
    operations::{CollectionsData, GetData},
};
use maestro_kernel::{evidence::Bundle, gateway::ModelPort, telemetry::span};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::common::{schema_for_input, schema_for_output},
    model::{
        CallToolRequestParams, CallToolResponse, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
};
use serde_json::{Value, json};
use std::sync::Arc;

impl<P: ModelPort + Send + Sync + 'static> ServerHandler for KnowledgeServer<P> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("maestro", env!("CARGO_PKG_VERSION")))
            .with_instructions(format!(
                concat!(
                    "Search visible published collections, read their exact source-backed chunks ",
                    "and sections, or answer from passages granted to the local principal. {}",
                ),
                self.preference_context
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
        let stage = span::tool_call(tool_name(request.name.as_ref()));
        let response = stage.instrument(self.call(request, context)).await;
        stage.finish(call_outcome(&response));
        response
    }
}

impl<P: ModelPort + Send + Sync + 'static> KnowledgeServer<P> {
    /// Runs one tool call on a bounded worker, within its deadline.
    async fn call(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let operation = match parse_operation(request.name.as_ref(), arguments, &self.settings) {
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

        match operation {
            Operation::Search(request) => search::call_search(self, request, permit, context).await,
            Operation::Ask(request) => {
                call_blocking_operation(
                    BlockingOperation::Ask(request, self.settings.clone()),
                    permit,
                    self.open_kernel.clone(),
                    ASK_CALL_DEADLINE,
                    context,
                )
                .await
            }
            Operation::Collections => {
                call_blocking_operation(
                    BlockingOperation::Collections,
                    permit,
                    self.open_kernel.clone(),
                    self.call_deadline,
                    context,
                )
                .await
            }
            Operation::Get(request) => {
                call_blocking_operation(
                    BlockingOperation::Get(request),
                    permit,
                    self.open_kernel.clone(),
                    self.call_deadline,
                    context,
                )
                .await
            }
        }
    }
}

/// Creates the same strict schemas for tools/list and the SDK's tool validation hook.
fn tool_definitions() -> Result<Vec<Tool>, McpError> {
    let mut collections_input = schema_for_input::<super::operations::CollectionsRequest>()
        .map_err(|_| McpError::internal_error("tool schema unavailable", None))?;
    let mut get_input = schema_for_input::<GetRequest>()
        .map_err(|_| McpError::internal_error("tool schema unavailable", None))?;
    let search_input = schema_for_input::<SearchRequest>()
        .map_err(|_| McpError::internal_error("tool schema unavailable", None))?;
    Arc::make_mut(&mut collections_input).insert("properties".to_owned(), json!({}));
    Arc::make_mut(&mut get_input).insert(
        "oneOf".to_owned(),
        json!([
            { "required": ["chunk_id"], "not": { "required": ["section_id"] } },
            { "required": ["section_id"], "not": { "required": ["chunk_id"] } },
        ]),
    );
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
    let search = Tool::new(
        "knowledge_search",
        "Search a visible published generation and return bounded source-backed evidence.",
        search_input,
    )
    .with_raw_output_schema(schema_for_output::<Bundle>())
    .with_annotations(
        ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .open_world(true)
            .idempotent(true),
    );
    let get = Tool::new(
        "knowledge_get",
        concat!(
            "Read an exact source-backed chunk or section from a visible ",
            "published or retained generation.",
        ),
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
    let ask = ask::definition()?;
    Ok(vec![collections, get, search, ask])
}
