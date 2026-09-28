//! Runs the stdio MCP server without sending diagnostics to stdout.

use super::{server::KnowledgeServer, transport::BoundedStdio};
use crate::failure::Failure;
use maestro_kernel::gateway::RouterClient;
use maestro_knowledge::index::Qdrant;
use rmcp::{ServiceExt, service::QuitReason};
use tokio::{io, runtime::Builder};

/// Serves one stdio MCP process until EOF or transport cancellation.
pub(crate) fn run(model_port: RouterClient, qdrant: Qdrant) -> Result<(), Failure> {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Failure::Failed("could not start the MCP runtime".to_owned()))?;
    runtime.block_on(async {
        let transport = BoundedStdio::new(io::stdin(), io::stdout());
        let server = KnowledgeServer::new(model_port, qdrant);
        let warmup = server.warmup();
        let service = Box::pin(server.serve(transport))
            .await
            .map_err(|_| Failure::Failed("could not start the MCP service".to_owned()))?;
        tokio::spawn(warmup);
        match service.waiting().await {
            Ok(QuitReason::Closed | QuitReason::Cancelled) => Ok(()),
            Ok(_) | Err(_) => Err(Failure::Failed(
                "the MCP service stopped unexpectedly".to_owned(),
            )),
        }
    })
}
