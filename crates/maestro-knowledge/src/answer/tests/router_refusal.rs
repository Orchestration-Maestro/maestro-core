//! A refusal the router client cannot read in full, through the answer path:
//! it stays the answerer's refusal, so no repair prompt follows it.

use super::*;
use maestro_kernel::gateway::{RouterClient, Url};
use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};

/// The llama.cpp build the canonical answerer card records.
const BUILD: &str = "b1234-abcdef";

/// One byte more than the router client reads of a refusal.
const OVERSIZED: usize = 65_537;

/// A loopback router that serves the answerer's `/props` as its card records
/// and refuses every other request with a 503 of [`OVERSIZED`] bytes; it
/// records the path of each request.
fn refusing_router() -> (Url, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let base = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let paths = Arc::default();
    let recorded = Arc::clone(&paths);
    thread::spawn(move || {
        for stream in listener.incoming() {
            refuse(&stream.expect("accept a request"), &recorded);
        }
    });
    (base, paths)
}

/// Reads one request from `stream`, records its path, and answers it.
fn refuse(stream: &TcpStream, recorded: &Mutex<Vec<String>>) {
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
    reader.read_exact(&mut vec![0; length]).unwrap();
    let (status, body) = if path.ends_with("/props") {
        (200, format!(r#"{{"build_info":"{BUILD}"}}"#))
    } else {
        (503, " ".repeat(OVERSIZED))
    };
    recorded.lock().unwrap().push(path);
    // The client may hang up once it reads the declared length.
    drop(write!(
        &mut &*stream,
        "HTTP/1.1 {status} Stub\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ));
}

#[tokio::test]
async fn an_oversized_refusal_stays_a_backend_failure_without_a_repair_call() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("Which default port does the service use?");
    let (base, paths) = refusing_router();
    let port = RouterClient::new(base).expect("router client");

    let outcome = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        bundle(
            &request.question,
            "en",
            "The service listens on port 8080 by default.",
        ),
        &PromptVersion::V1.into(),
    )
    .await;

    match outcome {
        Err(AskError::Backend(Error::Refused {
            status: 503,
            code: None,
            ..
        })) => {}
        other => panic!("not the answerer's refusal: {other:?}"),
    }
    assert_eq!(
        *paths.lock().unwrap(),
        [
            "/models/qwen3-4b/props",
            "/models/qwen3-4b/v1/chat/completions"
        ]
    );
}
