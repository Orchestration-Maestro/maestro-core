//! Runs the stdio MCP server without sending diagnostics to stdout.

use super::{server::KnowledgeServer, transport::BoundedStdio};
use crate::{
    failure::Failure,
    settings::{Compute, KnowledgeSettings},
};
use maestro_kernel::gateway::RouterClient;
use maestro_knowledge::index::Qdrant;
use rmcp::{ServiceExt, service::QuitReason};
use tokio::{io, runtime::Builder};

/// Serves one stdio MCP process under the session's `settings` until EOF or
/// transport cancellation; with models off, nothing warms a model.
pub(crate) fn run(
    model_port: RouterClient,
    qdrant: Qdrant,
    settings: KnowledgeSettings,
) -> Result<(), Failure> {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Failure::Failed("could not start the MCP runtime".to_owned()))?;
    runtime.block_on(async {
        let transport = BoundedStdio::new(io::stdin(), io::stdout());
        let models = warms_models(settings.compute);
        let server = KnowledgeServer::new(model_port, qdrant, settings);
        let warmup = server.warmup();
        let service = Box::pin(server.serve(transport))
            .await
            .map_err(|_| Failure::Failed("could not start the MCP service".to_owned()))?;
        if models {
            tokio::spawn(warmup);
        }
        match service.waiting().await {
            Ok(QuitReason::Closed | QuitReason::Cancelled) => Ok(()),
            Ok(_) | Err(_) => Err(Failure::Failed(
                "the MCP service stopped unexpectedly".to_owned(),
            )),
        }
    })
}

/// Whether MCP startup should warm model-backed routes.
fn warms_models(compute: Compute) -> bool {
    compute == Compute::Gpu
}

#[cfg(test)]
mod tests {
    use super::{Compute, warms_models};

    #[test]
    fn model_warmup_tracks_the_compute_setting() {
        assert!(warms_models(Compute::Gpu));
        assert!(!warms_models(Compute::Off));
    }
}
