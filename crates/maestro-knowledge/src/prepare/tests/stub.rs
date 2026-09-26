//! A stub of the model router for the router client's tests: a loopback HTTP
//! server, on a thread of its own, that serves the entry `embed` as a model
//! of the build [`BUILD`] and tokenizes each parity fixture into the native
//! counter's IDs. It records the path and the room of every request.

use super::{super::parity::fixtures, support::BUILD};
use maestro_kernel::gateway::Url;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

/// One request as the stub read it: its path, and the value of
/// `X-Model-Router-Room` when it carries one.
pub(super) type Request = (String, Option<String>);

/// A running stub.
pub(super) struct StubRouter {
    /// Where it listens.
    base: Url,
    /// The requests it read, in order.
    requests: Arc<Mutex<Vec<Request>>>,
}

impl StubRouter {
    /// A stub on a new loopback port.
    pub(super) fn serve() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let goldens: HashMap<String, Vec<u32>> = fixtures()
            .unwrap()
            .into_iter()
            .map(|fixture| (fixture.input, fixture.ids))
            .collect();
        let requests = Arc::default();
        let recorded = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming() {
                reply(&stream.unwrap(), &goldens, &recorded);
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

/// Reads one request from `stream`, records it, and answers it: `/props`
/// with the build, `/tokenize` with the golden of the text, or none.
fn reply(stream: &TcpStream, goldens: &HashMap<String, Vec<u32>>, recorded: &Mutex<Vec<Request>>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let path = line.split_whitespace().nth(1).unwrap().to_owned();
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
    recorded.lock().unwrap().push((path.clone(), room));
    let answer = match path.as_str() {
        "/models/embed/props" => json!({"build_info": BUILD}),
        "/models/embed/tokenize" => {
            let asked: Value = serde_json::from_slice(&body).unwrap();
            let text = asked["content"].as_str().unwrap();
            json!({"tokens": goldens.get(text).cloned().unwrap_or_default()})
        }
        other => panic!("the stub serves no {other}"),
    }
    .to_string();
    write!(
        &mut &*stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{answer}",
        answer.len()
    )
    .unwrap();
}
