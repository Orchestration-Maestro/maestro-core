//! Async MCP dispatch and complete-response bounds for search.

use super::{
    knowledge_server::KnowledgeServer,
    response::{operation_error, response_fits, tool_error},
};
use crate::{
    failure::Failure,
    knowledge::{
        SearchRequest,
        operations::{KnowledgeError, ensure_current_scopes, search_with},
        output::{SearchOutputError, search_tool_result, truncate_search_bundle},
    },
};
use maestro_kernel::{evidence::Bundle, gateway::ModelPort};
use rmcp::{
    ErrorData as McpError,
    model::{CallToolResponse, CallToolResult, ProtocolVersion, RequestId},
    service::{RequestContext, RoleServer},
};
use tokio::{
    sync::OwnedSemaphorePermit,
    task::spawn_blocking,
    time::{Instant, sleep},
};

/// Opens one kernel off the executor, runs the shared search path, and bounds the full MCP frame.
pub(super) async fn call_search<P: ModelPort + Send + Sync + 'static>(
    server: &KnowledgeServer<P>,
    request: SearchRequest,
    permit: OwnedSemaphorePermit,
    context: RequestContext<RoleServer>,
) -> Result<CallToolResponse, McpError> {
    let call_deadline = server.call_deadline;
    let opener = server.open_kernel.clone();
    let model_port = server.model_port.clone();
    let qdrant = server.qdrant.clone();
    let cancellation = context.ct.clone();
    let worker_cancellation = cancellation.clone();
    let request_id = context.id.clone();
    let protocol_version = context.protocol_version();
    let work = tokio::spawn(async move {
        let _permit = permit;

        let kernel = match spawn_blocking(move || opener()).await {
            Ok(Ok(kernel)) => kernel,
            Ok(Err(failure)) => {
                return Ok(operation_error(kernel_open_failure(&failure)).into());
            }
            Err(_) => return Ok(operation_error(kernel_unavailable()).into()),
        };
        let scoped = match search_with(kernel, &request, model_port.as_ref(), qdrant.as_ref()).await
        {
            Ok(scoped) => scoped,
            Err(error) => return Ok(operation_error(error).into()),
        };
        let deadline = scoped.data.deadline;
        let result = bounded_result(scoped.data.bundle, &request_id, protocol_version.as_ref())?;
        if Instant::now() >= deadline {
            return Ok(operation_error(deadline_exceeded()).into());
        }
        let kernel = scoped.kernel;
        let scopes = scoped.scopes;
        match spawn_blocking(move || {
            let mut kernel = kernel;
            ensure_current_scopes(&mut kernel, &scopes).map(|()| kernel)
        })
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => return Ok(operation_error(error).into()),
            Err(_) => return Ok(operation_error(kernel_unavailable()).into()),
        }
        if Instant::now() >= deadline {
            return Ok(operation_error(deadline_exceeded()).into());
        }
        if worker_cancellation.is_cancelled() {
            return Ok(tool_error(
                "cancelled",
                "the request was cancelled before a result was delivered",
                false,
                Vec::new(),
            )
            .into());
        }
        Ok(result.into())
    });
    tokio::select! {
        result = work => match result {
            Ok(result) => result,
            Err(_) => Err(McpError::internal_error("knowledge search failed", None)),
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

/// Maps failed kernel workers to a safe public failure.
fn kernel_unavailable() -> KnowledgeError {
    KnowledgeError::Failed {
        code: "kernel_unavailable",
        message: "the local knowledge store is unavailable",
    }
}

/// Maps an expired evidence deadline to a safe public failure.
fn deadline_exceeded() -> KnowledgeError {
    KnowledgeError::Failed {
        code: "deadline_exceeded",
        message: "search exceeded its accepted deadline",
    }
}

/// Reduces one bundle through the shared CLI/MCP truncation policy and checks its frame.
fn bounded_result(
    bundle: Bundle,
    request_id: &RequestId,
    protocol_version: Option<&ProtocolVersion>,
) -> Result<CallToolResult, McpError> {
    let bounded = match truncate_search_bundle(bundle) {
        Ok(bounded) => bounded,
        Err(SearchOutputError::Format) => {
            return Err(McpError::internal_error(
                "response serialization failed",
                None,
            ));
        }
        Err(SearchOutputError::TooLarge) => {
            return Ok(tool_error(
                "response_too_large",
                "the search response exceeds the response limit",
                true,
                vec!["bundle"],
            ));
        }
    };
    let result = search_tool_result(&bounded.bundle, bounded.truncation)
        .map_err(|_| McpError::internal_error("response serialization failed", None))?;
    if response_fits(&result, request_id, protocol_version)? {
        Ok(result)
    } else {
        Ok(tool_error(
            "response_too_large",
            "the search response exceeds the response limit",
            true,
            vec!["bundle"],
        ))
    }
}

/// Maps local kernel setup failures without disclosing the configuration or backend.
fn kernel_open_failure(failure: &Failure) -> KnowledgeError {
    match failure {
        Failure::Refused(_) => KnowledgeError::Refused {
            code: "invalid_configuration",
            message: "local access configuration is invalid",
        },
        Failure::Failed(_) => KnowledgeError::Failed {
            code: "kernel_unavailable",
            message: "the local knowledge store is unavailable",
        },
    }
}

#[cfg(test)]
mod tests;
