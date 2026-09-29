//! `ask` keeps the collection's selected reranker after a republish, as
//! search does, and its explanation names each route's status, so a
//! degraded ask is visible.

use super::super::support::{Home, Running, local};
use super::cold_reranker::select_reranker;
use super::knowledge_search::published_identifier_source;
use maestro_kernel::{generation::NewGeneration, retrieval::IDENTIFIER_PROFILE};
use std::{
    io::{Read as _, Write as _},
    net::TcpListener,
    thread,
};

/// Serves a loopback router that refuses every request at once with 503, so
/// a rerank that runs ends `model_unavailable` long before the deadline, and
/// a slow machine leaves evidence assembly its time. Returns its URL.
fn refusing_router() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = [0; 4096];
            let _read = stream.read(&mut request);
            let _written = stream.write_all(
                b"HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\n\
                  connection: close\r\n\r\n",
            );
        }
    });
    url
}

/// Publishes a new generation of the published chunk set, as a republish
/// does, and returns its ID.
fn republish(home: &Home, published: i64) -> i64 {
    let database = home.database();
    let scopes = local(&database);
    let current = database
        .published_generation(&scopes, "synthetic")
        .unwrap()
        .unwrap();
    assert_eq!(current.id, published);
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: current.collection_id,
            chunk_set_id: current.chunk_set_id,
            embedding_profile: current.embedding_profile,
            sparse_profile: current.sparse_profile,
        })
        .unwrap();
    database
        .begin_generation_search(&scopes, generation.id, IDENTIFIER_PROFILE)
        .unwrap();
    database
        .complete_generation_search(&scopes, generation.id)
        .unwrap();
    database.verify_generation(generation.id, 1).unwrap();
    database.publish_generation(generation.id).unwrap();
    generation.id
}

#[test]
fn ask_still_reranks_after_a_republish_and_explains_each_route() {
    let home = Home::new();
    let first = published_identifier_source(&home);
    select_reranker(&home, first);
    let second = republish(&home, first);
    assert_ne!(second, first);
    let router = refusing_router();

    let mut command = home.command(&[
        "knowledge",
        "ask",
        "--collection",
        "synthetic",
        "--question",
        "What does --force do?",
        "--explain",
    ]);
    command
        .env("MAESTRO_ROUTER_URL", &router)
        .env("MAESTRO_QDRANT_URL", "http://127.0.0.1:1");
    let ended = Running::of(command).finish();

    assert_eq!(ended.code, Some(0), "{ended:?}");
    let routes: Vec<&str> = ended
        .stderr
        .lines()
        .filter(|line| line.starts_with("explain: route "))
        .collect();
    assert!(
        routes.contains(&"explain: route rerank unavailable: model_unavailable"),
        "the selected reranker was called: {}",
        ended.stderr
    );
    let names: Vec<&str> = routes
        .iter()
        .filter_map(|line| line.split(' ').nth(2))
        .collect();
    assert_eq!(names, ["dense", "identifier", "lexical", "rerank"]);
}
