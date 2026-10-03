//! A drafting endpoint that redirects fails the window, and the redirect's
//! target never receives the private prompt.
use super::{
    graph_draft::{authority, draft, manifest},
    support::Home,
};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

/// A loopback listener that answers every request with `reply(path)` and
/// counts the requests it read.
struct Listener {
    /// `http://127.0.0.1:<port>`.
    url: String,
    /// Set when the owning test ends.
    stop: Arc<AtomicBool>,
    /// Requests read so far.
    requests: Arc<AtomicUsize>,
    /// The accept loop.
    thread: Option<thread::JoinHandle<()>>,
}

impl Listener {
    fn new(reply: impl Fn(&str) -> String + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let (ending, counted) = (Arc::clone(&stop), Arc::clone(&requests));
        let thread = thread::spawn(move || accept(&listener, &ending, &counted, &reply));
        Self {
            url,
            stop,
            requests,
            thread: Some(thread),
        }
    }
}

/// Answers each connection until `stop` is set, counting every request read.
fn accept(
    listener: &TcpListener,
    stop: &AtomicBool,
    requests: &AtomicUsize,
    reply: &impl Fn(&str) -> String,
) {
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let mut stream = stream.unwrap();
        let path = read_request(&stream);
        requests.fetch_add(1, Ordering::SeqCst);
        stream.write_all(reply(&path).as_bytes()).unwrap();
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _wake = TcpStream::connect(self.url.strip_prefix("http://").unwrap()).unwrap();
        self.thread.take().unwrap().join().unwrap();
    }
}

/// Reads one request's head and body, and returns its path.
fn read_request(stream: &TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap();
        }
    }
    reader.read_exact(&mut vec![0; length]).unwrap();
    first.split(' ').nth(1).unwrap().to_owned()
}

#[test]
fn graph_draft_fails_on_a_redirect_and_its_target_receives_nothing() {
    for status in [307, 308] {
        let home = Home::bare();
        let (card, generation) = authority(&home);
        let target = Listener::new(|_| {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
        });
        let location = target.url.clone();
        let redirecting = Listener::new(move |path| {
            format!(
                "HTTP/1.1 {status} Moved\r\nLocation: {location}{path}\r\n\
                 Content-Length: 0\r\nConnection: close\r\n\r\n"
            )
        });
        let path = manifest(&home, &redirecting.url, &card, generation);
        let result = draft(&home, &path);
        assert_eq!(result.status.code(), Some(1), "{status}: {result:?}");
        let summary: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(summary["failed"], 1, "{status}");
        let receipt: Value = serde_json::from_slice(
            &fs::read(
                path.parent()
                    .unwrap()
                    .join("draft-receipt-00000000000000000001.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let outcome = &receipt["receipt"]["outcome"];
        assert_eq!(outcome["state"], "failed", "{status}: {receipt}");
        assert_eq!(outcome["value"], "gateway", "{status}: {receipt}");
        assert!(redirecting.requests.load(Ordering::SeqCst) > 0, "{status}");
        assert_eq!(target.requests.load(Ordering::SeqCst), 0, "{status}");
    }
}
