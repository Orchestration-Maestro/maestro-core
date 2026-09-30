//! The configured local knowledge server and its startup warm-up task.

use super::{
    types::{CALL_DEADLINE, KernelOpener, WORKER_LIMIT},
    warmup,
};
#[cfg(test)]
use crate::failure::Failure;
use crate::{kernel::Kernel, settings::KnowledgeSettings};
#[cfg(test)]
use maestro_kernel::gateway::{ModelCard, Url};
use maestro_kernel::gateway::{ModelPort, RouterClient};
use maestro_knowledge::index::Qdrant;
use std::{future::Future, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

/// A read-only local knowledge server with bounded workers and shared search ports.
pub(in crate::mcp) struct KnowledgeServer<P = RouterClient> {
    /// Hard limit on operations running against the local kernel.
    pub(super) workers: Arc<Semaphore>,
    /// One fresh local kernel per call, injectable only inside this module's tests.
    pub(super) open_kernel: KernelOpener,
    /// Bounds how long a caller waits for one local operation.
    pub(super) call_deadline: Duration,
    /// The model port shared by retrieval and startup warming.
    pub(super) model_port: Arc<P>,
    /// Qdrant bound to the configured search service.
    pub(super) qdrant: Arc<Qdrant>,
    /// The session's settings: what every tool call runs with, fixed for the
    /// server's life.
    pub(super) settings: Arc<KnowledgeSettings>,
    /// Path-free session origin, fixed at process startup.
    pub(super) preference_context: String,
    /// Test-only startup cards to exercise warming without a model registry fixture.
    #[cfg(test)]
    pub(super) warmup_cards: Option<Vec<ModelCard>>,
}

impl<P: ModelPort + Send + Sync + 'static> KnowledgeServer<P> {
    /// Creates the read-only local knowledge server with its configured ports
    /// and the session's `settings`.
    pub(in crate::mcp) fn new(model_port: P, qdrant: Qdrant, settings: KnowledgeSettings) -> Self {
        Self {
            workers: Arc::new(Semaphore::new(WORKER_LIMIT)),
            open_kernel: Arc::new(Kernel::open),
            call_deadline: CALL_DEADLINE,
            model_port: Arc::new(model_port),
            qdrant: Arc::new(qdrant),
            settings: Arc::new(settings),
            preference_context: String::new(),
            #[cfg(test)]
            warmup_cards: None,
        }
    }

    /// Attach local preference provenance without any filesystem paths.
    pub(in crate::mcp) fn with_preferences(mut self, context: String) -> Self {
        self.preference_context = context;
        self
    }

    /// Creates an unpolled startup task; callers spawn it only after serving begins.
    pub(in crate::mcp) fn warmup(&self) -> impl Future<Output = ()> + Send + 'static {
        #[cfg(test)]
        let test_cards = self.warmup_cards.clone();
        #[cfg(not(test))]
        let test_cards = None;
        warmup::run(
            self.open_kernel.clone(),
            self.model_port.clone(),
            test_cards,
        )
    }

    /// Creates an isolated generic server for background and port tests.
    #[cfg(test)]
    pub(super) fn with_test_ports(
        open_kernel: impl Fn() -> Result<Kernel, Failure> + Send + Sync + 'static,
        call_deadline: Duration,
        model_port: P,
        qdrant: Qdrant,
        warmup_cards: Vec<ModelCard>,
    ) -> Self {
        Self {
            workers: Arc::new(Semaphore::new(WORKER_LIMIT)),
            open_kernel: Arc::new(open_kernel),
            call_deadline,
            model_port: Arc::new(model_port),
            qdrant: Arc::new(qdrant),
            settings: Arc::default(),
            preference_context: String::new(),
            warmup_cards: Some(warmup_cards),
        }
    }
}

#[cfg(test)]
impl KnowledgeServer<RouterClient> {
    /// Creates a server with closed local service ports for protocol-only tests.
    pub(super) fn for_tests() -> Self {
        Self::new(
            RouterClient::new(Url::parse("http://127.0.0.1:8080").expect("default router URL"))
                .expect("default router client"),
            Qdrant::new("http://127.0.0.1:6334").expect("default Qdrant client"),
            KnowledgeSettings::default(),
        )
    }

    /// Creates a bounded server with an isolated opener and default local ports.
    pub(super) fn with_kernel_opener(
        open_kernel: impl Fn() -> Result<Kernel, Failure> + Send + Sync + 'static,
        call_deadline: Duration,
    ) -> Self {
        let mut server = Self::for_tests();
        server.open_kernel = Arc::new(open_kernel);
        server.call_deadline = call_deadline;
        server
    }
}
