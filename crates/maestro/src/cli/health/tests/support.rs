//! What the health tests share: a scratch directory for the kernel's data
//! and configuration, a stub HTTP server with one canned answer, an address
//! nothing answers at, and a failed check's parts.

use super::super::check::{Check, Outcome};
use maestro_test_scratch::scratch_directory;
use qdrant_client::qdrant::{
    HealthCheckReply, HealthCheckRequest,
    qdrant_server::{Qdrant as QdrantService, QdrantServer},
};
use std::{
    fs,
    io::{BufRead as _, BufReader, Write as _},
    net::TcpListener,
    path::PathBuf,
    thread,
};
use tokio::runtime::Builder;
use tonic::{
    Request, Response, Status,
    transport::{Server, server::TcpIncoming},
};

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: `data` is the kernel's data
/// directory and `config` its configuration directory.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        let path = scratch_directory().unwrap();
        fs::create_dir_all(path.join("data")).unwrap();
        fs::create_dir_all(path.join("config")).unwrap();
        Self(path)
    }

    /// The kernel's data directory.
    pub(super) fn data(&self) -> PathBuf {
        self.0.join("data")
    }

    /// The kernel's configuration directory.
    pub(super) fn config(&self) -> PathBuf {
        self.0.join("config")
    }

    /// Writes `text` as `name` in the configuration directory.
    pub(super) fn configure(&self, name: &str, text: &str) {
        fs::write(self.config().join(name), text).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// A stub Qdrant health service on a new loopback port, on a thread of its
/// own, that answers with `version`; returns its gRPC address.
pub(super) fn serve_qdrant(version: &str) -> String {
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let incoming =
        runtime.block_on(async { TcpIncoming::bind("127.0.0.1:0".parse().unwrap()).unwrap() });
    let address = format!("http://{}", incoming.local_addr().unwrap());
    let server = Server::builder()
        .add_service(QdrantServer::new(StubQdrant(version.to_owned())))
        .serve_with_incoming(incoming);
    thread::spawn(move || drop(runtime.block_on(server)));
    address
}

/// The health reply from the stub Qdrant.
struct StubQdrant(String);

#[tonic::async_trait]
impl QdrantService for StubQdrant {
    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckReply>, Status> {
        Ok(Response::new(HealthCheckReply {
            title: "qdrant - vector search engine".to_owned(),
            version: self.0.clone(),
            commit: None,
        }))
    }
}

/// A stub HTTP server on a new loopback port, on a thread of its own, that
/// answers every request with `status` and `body`; returns its address.
pub(super) fn serve(status: u16, body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let body = body.to_owned();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            write!(
                &mut &stream,
                "HTTP/1.1 {status} Stub\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    address
}

/// An address on the loopback that nothing answers at: a port just freed.
pub(super) fn nothing_at() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

/// The problem and next action of `check`, which failed.
pub(super) fn failure(check: &Check) -> (&str, &str) {
    match &check.outcome {
        Outcome::Failed { problem, next } => (problem, next),
        Outcome::Passed(detail) => panic!("{} passed: {detail}", check.name),
    }
}

/// What `check` saw, which passed.
pub(super) fn detail(check: &Check) -> &str {
    match &check.outcome {
        Outcome::Passed(detail) => detail,
        Outcome::Failed { problem, .. } => panic!("{} failed: {problem}", check.name),
    }
}
