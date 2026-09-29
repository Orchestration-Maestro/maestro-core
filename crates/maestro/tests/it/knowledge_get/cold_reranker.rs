//! A cold reranker costs CLI and MCP search only the rerank.
//!
//! When the selected reranker's model never finishes loading, both return
//! the fused order, with the rerank reported unavailable, not an error. The
//! searches ask for a 3 s deadline, not the 30 s default, to stay quick.

use super::super::{
    knowledge_prepare_v2::identity,
    support::{Home, Running, local},
};
use super::knowledge_search::{cli_data, mcp_search_result_of, published_identifier_source};
use maestro_kernel::{
    artifact::Store,
    gateway::{
        ModelCard, Role, RouterEntry,
        card_v2::{Capability, Dimensions, EmbeddingFormat},
    },
    model::{
        EvaluationDisposition, EvaluationMode, NewModelCard, NewModelEvaluation, NewModelSelection,
    },
};
use serde_json::{Value, json};
use std::{
    net::{TcpListener, TcpStream},
    process::Command,
    thread,
};

const QUERY: &str = "What does --force do?";
/// The deadline the searches ask for, in milliseconds.
const DEADLINE_MS: u32 = 3000;

/// Serves a loopback router whose models never finish loading: it takes
/// every request and never answers. Returns its URL.
fn cold_router() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        let mut held: Vec<TcpStream> = Vec::new();
        for stream in listener.incoming().flatten() {
            held.push(stream);
        }
    });
    url
}

/// Registers a v2 reranker card in `synthetic`, evaluates it on
/// `generation` and selects it.
pub(super) fn select_reranker(home: &Home, generation: i64) {
    let database = home.database();
    let scopes = local(&database);
    let model = database
        .put(b"reranker weights", "application/octet-stream")
        .unwrap();
    let qualification = database.put(b"{}", "application/json").unwrap();
    let mut reranker = identity(&model, qualification);
    reranker.role = Role::Reranker;
    reranker.router_entry = RouterEntry::parse("rerank").unwrap();
    reranker.invocation.dimensions = Dimensions::NotApplicable;
    reranker.formats.embedding = EmbeddingFormat::NotApplicable;
    reranker.formats.document = Capability::NotApplicable;
    reranker.formats.query = Capability::NotApplicable;
    let card = ModelCard::record_v2(&Store::new(home.data().join("artifacts")), &reranker).unwrap();
    let card_id = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "synthetic",
                card: &card,
            },
        )
        .unwrap()
        .id;
    let evaluation = database
        .record_model_evaluation(
            &scopes,
            &NewModelEvaluation {
                run_id: "run",
                collection_id: "synthetic",
                card_id,
                role: Role::Reranker,
                mode: EvaluationMode::Real,
                generation_id: Some(generation),
                disposition: EvaluationDisposition::Eligible,
                manifest: b"manifest",
                report: b"report",
            },
        )
        .unwrap();
    database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "synthetic",
                role: Role::Reranker,
                card_id,
                evaluation_id: evaluation.id,
                selected_by: "test",
                reason: "a reranker the router never loads",
            },
        )
        .unwrap();
}

/// The binary with `arguments` in `home`, reaching the router at `router`
/// and no Qdrant.
fn command(home: &Home, router: &str, arguments: &[&str]) -> Command {
    let mut command = home.command(arguments);
    command
        .env("MAESTRO_ROUTER_URL", router)
        .env("MAESTRO_QDRANT_URL", "http://127.0.0.1:1");
    command
}

#[test]
fn a_cold_reranker_degrades_cli_and_mcp_search_to_the_fused_order() {
    let home = Home::new();
    let generation = published_identifier_source(&home);
    select_reranker(&home, generation);
    let router = cold_router();
    let unavailable: Value = json!({"unavailable": "deadline_exceeded"});

    let cli = Running::of(command(
        &home,
        &router,
        &[
            "--json",
            "knowledge",
            "search",
            "--collection",
            "synthetic",
            "--query",
            QUERY,
            "--deadline-ms",
            &DEADLINE_MS.to_string(),
        ],
    ))
    .finish();
    assert_eq!(cli.code, Some(0), "{cli:?}");
    let bundle = cli_data(&cli.stdout);
    assert_eq!(bundle["routes"]["rerank"], unavailable);
    assert_eq!(bundle["passages"].as_array().unwrap().len(), 1);

    let mcp = mcp_search_result_of(
        command(&home, &router, &["mcp"]),
        &json!({"collection": "synthetic", "query": QUERY, "deadline_ms": DEADLINE_MS}),
    );
    assert_eq!(mcp["isError"], false, "{mcp}");
    assert_eq!(mcp["structuredContent"]["routes"]["rerank"], unavailable);
    assert_eq!(
        mcp["structuredContent"]["passages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
