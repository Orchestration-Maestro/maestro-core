use super::{super::KnowledgeServer, published_embedder_cards, warm_cards};
use crate::{failure::Failure, mcp::transport::BoundedStdio};
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        CardFields, ChatRequest, Error, Limits, ModelCard, ModelPort, Role, Room, RouterEntry,
    },
};
use maestro_knowledge::index::Qdrant;
use rmcp::{ServerHandler, ServiceExt};
use serde_json::Value;
use std::{
    env, fs,
    future::{self, Future},
    num::{NonZeroU32, NonZeroUsize},
    process,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, duplex},
    spawn,
    sync::Notify,
    time::timeout,
};

/// Generous upper bound for service startup and test shutdown.
const HANG_GUARD: Duration = Duration::from_secs(10);

#[test]
fn card_discovery_preserves_kernel_open_failures() {
    let opener: super::super::types::KernelOpener =
        Arc::new(|| Err(Failure::failed("kernel open failed")));
    assert!(published_embedder_cards(&opener).is_err());
}

#[test]
fn warning_marks_itself_emitted_before_any_later_failure() {
    let mut warned = false;
    super::warn_once(&mut warned, "first");
    assert!(warned);
    super::warn_once(&mut warned, "second");
    assert!(warned);
}

#[tokio::test]
async fn initialize_returns_while_the_background_embedder_warmup_is_blocked() {
    let card = card(b"blocked");
    let port = Port::gated();
    let server = KnowledgeServer::with_test_ports(
        || Err(Failure::failed("test opener must not run")),
        HANG_GUARD,
        port.clone(),
        qdrant(),
        vec![card],
    );
    let warmup = server.warmup();
    let (server_input, mut client_input) = duplex(4096);
    let (server_output, client_output) = duplex(16_384);
    let serving = spawn(server.serve(BoundedStdio::new(server_input, server_output)));
    client_input
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{",
                "\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},",
                "\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n"
            )
            .as_bytes(),
        )
        .await
        .expect("write MCP initialize");
    let service = timeout(HANG_GUARD, serving)
        .await
        .expect("initialize deadline")
        .expect("join service start")
        .expect("start test service");
    let waiting = spawn(service.waiting());
    let warming = spawn(warmup);
    timeout(HANG_GUARD, port.wait_started())
        .await
        .expect("warm-up started");

    let mut reader = BufReader::new(client_output);
    let mut line = String::new();
    timeout(Duration::from_secs(1), reader.read_line(&mut line))
        .await
        .expect("initialize response must not wait for warm-up")
        .expect("read initialize response");
    let response: Value = serde_json::from_str(&line).expect("initialize JSON");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "maestro");
    assert_eq!(port.rooms(), [Room::Free]);

    port.release();
    drop(client_input);
    timeout(HANG_GUARD, warming)
        .await
        .expect("warm-up completion deadline")
        .expect("join warm-up");
    timeout(HANG_GUARD, waiting)
        .await
        .expect("server shutdown deadline")
        .expect("join server")
        .expect("clean server shutdown");
}

#[tokio::test]
async fn a_warmup_failure_does_not_disable_search_or_fail_the_server() {
    let port = Port::failing_at(0);
    let server = KnowledgeServer::with_test_ports(
        || Err(Failure::failed("test opener must not run")),
        HANG_GUARD,
        port.clone(),
        qdrant(),
        vec![card(b"failure")],
    );
    server.warmup().await;
    assert!(server.get_tool("knowledge_search").is_some());
    assert_eq!(port.calls(), 1);
}

#[tokio::test]
async fn warming_deduplicates_cards_and_stops_after_insufficient_room() {
    let first = card(b"first");
    let port = Port::unavailable_at(1);
    warm_cards(
        vec![
            first.clone(),
            first.clone(),
            card(b"second"),
            card(b"third"),
        ],
        &port,
    )
    .await;
    assert_eq!(port.calls(), 2);
    assert_eq!(port.rooms(), [Room::Free, Room::Free]);
}

fn qdrant() -> Qdrant {
    Qdrant::new("http://127.0.0.1:6334").expect("default test Qdrant client")
}

fn card(weights: &[u8]) -> ModelCard {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = env::temp_dir().join(format!(
        "maestro-mcp-warmup-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let fields = CardFields {
        role: Role::Embedder,
        router_entry: RouterEntry::parse("embed").expect("router entry"),
        file_digest: Digest::of(weights),
        template_digest: None,
        server_build: "warmup-test".to_owned(),
        dimensions: Some(NonZeroUsize::new(2).expect("nonzero dimensions")),
        limits: Limits {
            context_tokens: NonZeroU32::new(1024).expect("nonzero context"),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    let card = ModelCard::record(&Store::new(&root), &fields).expect("record test card");
    fs::remove_dir_all(root).expect("remove temporary card store");
    card
}

#[derive(Clone)]
struct Port {
    calls: Arc<Mutex<Vec<Room>>>,
    unavailable_at: Option<usize>,
    failing_at: Option<usize>,
    gate: Option<Arc<Gate>>,
}

#[derive(Default)]
struct Gate {
    started: Notify,
    release: Notify,
}

impl Port {
    fn gated() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            unavailable_at: None,
            failing_at: None,
            gate: Some(Arc::new(Gate::default())),
        }
    }

    fn unavailable_at(call: usize) -> Self {
        Self {
            unavailable_at: Some(call),
            ..Self::gated_unavailable_defaults()
        }
    }

    fn failing_at(call: usize) -> Self {
        Self {
            failing_at: Some(call),
            ..Self::gated_unavailable_defaults()
        }
    }

    fn gated_unavailable_defaults() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            unavailable_at: None,
            failing_at: None,
            gate: None,
        }
    }

    async fn wait_started(&self) {
        self.gate
            .as_ref()
            .expect("gated port")
            .started
            .notified()
            .await;
    }

    fn release(&self) {
        self.gate.as_ref().expect("gated port").release.notify_one();
    }

    fn calls(&self) -> usize {
        self.calls.lock().expect("port calls").len()
    }

    fn rooms(&self) -> Vec<Room> {
        self.calls.lock().expect("port calls").clone()
    }
}

impl ModelPort for Port {
    async fn embed(
        &self,
        _card: &ModelCard,
        room: Room,
        _inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        let call = {
            let mut calls = self.calls.lock().expect("port calls");
            calls.push(room);
            calls.len() - 1
        };
        if let Some(gate) = &self.gate {
            gate.started.notify_one();
            gate.release.notified().await;
        }
        if self.unavailable_at == Some(call) {
            return Err(Error::Unavailable {
                reason: "insufficient free room".to_owned(),
            });
        }
        if self.failing_at == Some(call) {
            return Err(Error::InvalidAnswer {
                reason: "test failure".to_owned(),
            });
        }
        Ok(vec![vec![1.0, 0.0]])
    }

    fn rerank(
        &self,
        _card: &ModelCard,
        _room: Room,
        _query: &str,
        _documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send {
        future::ready(Err(Error::InvalidAnswer {
            reason: "unused test operation".to_owned(),
        }))
    }

    fn tokenize(
        &self,
        _card: &ModelCard,
        _room: Room,
        _text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> + Send {
        future::ready(Err(Error::InvalidAnswer {
            reason: "unused test operation".to_owned(),
        }))
    }

    fn chat(
        &self,
        _card: &ModelCard,
        _room: Room,
        _request: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> + Send {
        future::ready(Err(Error::InvalidAnswer {
            reason: "unused test operation".to_owned(),
        }))
    }
}
