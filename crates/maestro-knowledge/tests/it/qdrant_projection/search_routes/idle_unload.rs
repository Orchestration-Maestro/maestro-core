//! A long-lived router client reloads a model the router unloaded.
//!
//! The MCP server shares one client; its setup, before the route's window,
//! asks for the model again after the router unloaded it while idle.

use super::configured_search::{clean, published};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    gateway::{RouterClient, Url},
};
use maestro_knowledge::search::evidence::EvidenceSettings;
use maestro_knowledge::search::{
    SearchConfiguration, SearchContext, SearchRequest, routes::dense::Embedder, search,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

/// A router serving the entry `embed`, which loads its model only when asked
/// for its `/props`: an embedding of the unloaded model never answers, as
/// one whose load outlasts the route's window.
struct UnloadingRouter {
    /// Where it answers.
    url: String,
    /// Whether its model is loaded.
    loaded: Arc<AtomicBool>,
}

impl UnloadingRouter {
    /// Serves on a new loopback port, with the model not loaded.
    fn serve() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let loaded = Arc::new(AtomicBool::new(false));
        let model = loaded.clone();
        thread::spawn(move || {
            let held: Vec<TcpStream> = listener
                .incoming()
                .flatten()
                .filter_map(|stream| reply(stream, &model))
                .collect();
            drop(held);
        });
        Self { url, loaded }
    }

    /// Unloads the model, as the router does once it is idle.
    fn unload(&self) {
        self.loaded.store(false, Ordering::Relaxed);
    }
}

/// Answers one request, or returns its stream unanswered when it embeds
/// with the model unloaded.
fn reply(stream: TcpStream, loaded: &AtomicBool) -> Option<TcpStream> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let path = line.split_whitespace().nth(1).unwrap().to_owned();
    let mut length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let Some((name, value)) = header.trim_end().split_once(':') else {
            break;
        };
        if name.eq_ignore_ascii_case("content-length") {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let answer = if path == "/models/embed/props" {
        loaded.store(true, Ordering::Relaxed);
        json!({"build_info": "b6500-3f2c9a1b"})
    } else if path == "/models/embed/v1/embeddings" {
        if !loaded.load(Ordering::Relaxed) {
            return Some(reader.into_inner());
        }
        let request: Value = serde_json::from_slice(&body).unwrap();
        let data = request["input"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, _)| json!({"index": index, "embedding": [1.0, 0.0, 0.0]}))
            .collect::<Vec<_>>();
        json!({ "data": data })
    } else {
        panic!("the test router has no route for {path}");
    }
    .to_string();
    let mut stream = reader.into_inner();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{answer}",
        answer.len()
    )
    .unwrap();
    None
}

#[tokio::test]
async fn dense_runs_after_the_router_unloads_its_model_between_two_searches() {
    let fixture = published().await;
    let router = UnloadingRouter::serve();
    let client = RouterClient::new(Url::parse(&router.url).unwrap()).unwrap();
    let context = SearchContext {
        intent_expander: None,
        database: fixture.kernel.database.clone(),
        principal: "tester",
        qdrant: &fixture.qdrant,
        embedder: Some(Embedder {
            port: &client,
            card: &fixture.embedder_card,
        }),
        reranker: None,
        source_classes: None,
    };
    let request = SearchRequest {
        evidence: EvidenceSettings::default(),
        collection: &fixture.kernel.collection,
        text: "scheduler",
        version: None,
        budget: RequestBudget {
            deadline_ms: 1500,
            ..RequestBudget::default()
        },
        configuration: SearchConfiguration {
            rerank_enabled: false,
            ..SearchConfiguration::default()
        },
    };

    let mut dense = Vec::new();
    for unload in [false, true] {
        if unload {
            router.unload();
        }
        let input = Box::pin(search(&context, &request)).await.unwrap();
        dense.push(input.routes["dense"].clone());
    }

    assert_eq!(dense, [RouteStatus::Ok, RouteStatus::Ok]);
    clean(&fixture).await;
}
