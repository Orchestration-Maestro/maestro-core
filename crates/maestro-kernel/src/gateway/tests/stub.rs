//! A stub of the model router: a loopback HTTP server, on a thread of its
//! own, that answers each path with its canned reply and records every
//! request it reads.

use reqwest::Url;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

/// What the stub answers a path with.
#[derive(Debug, Clone)]
pub(super) enum Reply {
    /// A status and a body.
    Answer(u16, String),
    /// The connection closed before any answer.
    HangUp,
}

/// A reply of `status` with `body` as its JSON.
pub(super) fn answer(status: u16, body: &Value) -> Reply {
    Reply::Answer(status, body.to_string())
}

/// One request as the stub read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Request {
    /// `GET` or `POST`.
    pub(super) method: String,
    /// The path, as the request line carries it.
    pub(super) path: String,
    /// The value of `X-Model-Router-Room`, when the request carries one.
    pub(super) room: Option<String>,
    /// The body read as JSON; `Null` when there is none.
    pub(super) body: Value,
}

/// A request of `method` to `path` asking for free room, with `body`.
pub(super) fn free(method: &str, path: &str, body: Value) -> Request {
    Request {
        method: method.to_owned(),
        path: path.to_owned(),
        room: Some("free".to_owned()),
        body,
    }
}

/// A running stub.
pub(super) struct StubRouter {
    /// Where it listens.
    base: Url,
    /// The requests it read, in order.
    requests: Arc<Mutex<Vec<Request>>>,
}

impl StubRouter {
    /// A stub on a new loopback port answering each path of `replies` with
    /// its reply, and any other path as the router does, `404
    /// path_not_found`.
    pub(super) fn serve(replies: Vec<(&str, Reply)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let replies: HashMap<String, Reply> = replies
            .into_iter()
            .map(|(path, reply)| (path.to_owned(), reply))
            .collect();
        let requests = Arc::default();
        let recorded = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming() {
                reply(&stream.unwrap(), &replies, &recorded);
            }
        });
        Self { base, requests }
    }

    /// The stub's address, as a router client takes it.
    pub(super) fn base(&self) -> Url {
        self.base.clone()
    }

    /// The requests read so far, in order.
    pub(super) fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

/// Reads one request from `stream`, records it, and answers it.
fn reply(stream: &TcpStream, replies: &HashMap<String, Reply>, recorded: &Mutex<Vec<Request>>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let mut words = line.split_whitespace();
    let method = words.next().unwrap().to_owned();
    let path = words.next().unwrap().to_owned();
    let (mut length, mut room) = (0, None);
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let Some((name, value)) = header.trim_end().split_once(':') else {
            break;
        };
        match name.to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().unwrap(),
            "x-model-router-room" => room = Some(value.trim().to_owned()),
            _ => {}
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let body = serde_json::from_slice(&body).unwrap_or(Value::Null);
    recorded.lock().unwrap().push(Request {
        method,
        path: path.clone(),
        room,
        body,
    });
    let not_found = json!({"error": {"code": "path_not_found", "message": path,
        "type": "invalid_request_error"}});
    match replies
        .get(&path)
        .cloned()
        .unwrap_or_else(|| answer(404, &not_found))
    {
        Reply::Answer(status, body) => write!(
            &mut &*stream,
            "HTTP/1.1 {status} Stub\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap(),
        Reply::HangUp => {}
    }
}
