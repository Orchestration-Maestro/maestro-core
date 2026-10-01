//! Synthetic admitted HTTP/1.1 connections, never a network listener.
#![expect(clippy::indexing_slicing, reason = "authored synthetic fixture keys")]
use super::{n07_parse_url_identity_and_denial_precedence as n07, support};
use maestro_acquisition::{
    CheckedPolicy, Refusal,
    policy::{
        authority::{Authority, AuthorityRefusal, Operation, Permit, Target},
        decision::{AdmissionControls, RequestKind},
    },
    transport::{
        connect::{CheckedDestination, PinnedTransport, Resolver},
        http::{Fetch, Http},
    },
};
use std::{
    cell::Cell,
    collections::VecDeque,
    future::Future,
    io::Result as IoResult,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{
        AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, DuplexStream, ReadBuf, duplex,
    },
    runtime::Builder,
    time::pause,
};

/// Explicit generous transport data, not an engine default.
pub(super) fn policy() -> CheckedPolicy {
    policy_with(|_| {})
}
/// Mutate reviewed fixture data before rebinding its immutable digest.
pub(super) fn policy_with(edit: impl FnOnce(&mut serde_json::Value)) -> CheckedPolicy {
    let (mut collection, mut catalog) = n07::fixture();
    let mut wire = support::value(&catalog, "policy");
    let mut other = wire["sources"][0]["origins"][0].clone();
    other["id"] = "other".into();
    other["host"] = "other.example".into();
    wire["sources"][0]["origins"]
        .as_array_mut()
        .unwrap()
        .push(other);
    let mut selector = wire["sources"][0]["selectors"][0].clone();
    selector["origin"] = "other".into();
    wire["sources"][0]["selectors"]
        .as_array_mut()
        .unwrap()
        .push(selector);
    for limits in [wire["sources"][0]["limits"].as_object_mut().unwrap()] {
        for key in [
            "requests",
            "redirects",
            "wire_bytes",
            "memory_bytes",
            "asset_bytes",
            "dom_bytes",
            "staging_bytes",
        ] {
            limits.insert(key.into(), 1_000_000.into());
        }
        limits.insert("redirects".into(), 5.into());
        limits.insert("memory_bytes".into(), 10_000_000.into());
        limits.insert("retries".into(), 1.into());
    }
    wire["aggregate_limits"] = wire["sources"][0]["limits"].clone();
    wire["sources"][0]["robots"]["rules_max_bytes"] = 512_000.into();
    edit(&mut wire);
    support::put(&mut catalog, "policy", &wire);
    support::rebind(&mut collection, &mut catalog);
    n07::checked(collection, &catalog).unwrap()
}
/// Live grant decisions are observable independently of admission controls.
#[derive(Debug, Default)]
pub(super) struct Grants {
    pub(super) calls: Cell<usize>,
    pub(super) refuse_after: Cell<usize>,
    pub(super) targets: Mutex<Vec<String>>,
}
impl Authority for Grants {
    fn decide(
        &self,
        _: &str,
        _: Operation,
        target: &Target,
        _: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        let calls = self.calls.get();
        self.calls.set(calls + 1);
        self.targets.lock().unwrap().push(target.resource.clone());
        if self.refuse_after.get() != 0 && calls >= self.refuse_after.get() {
            return Err(Refusal::Access.into());
        }
        Ok(Permit {
            grant_id: "synthetic".into(),
        })
    }
}
/// DNS returns only public addresses, with an optional rebinding refusal.
#[derive(Debug, Default)]
pub(super) struct Dns {
    pub(super) calls: Cell<usize>,
    pub(super) deny_after: Cell<usize>,
}
impl Resolver for Dns {
    fn resolve(&self, _: &str) -> Result<Vec<String>, Refusal> {
        let calls = self.calls.get();
        self.calls.set(calls + 1);
        Ok(vec![
            if self.deny_after.get() != 0 && calls >= self.deny_after.get() {
                "127.0.0.1"
            } else {
                "8.8.8.8"
            }
            .into(),
        ])
    }
}
/// Raw responses permit hostile protocol bytes; requests are captured at the wire.
#[derive(Debug, Default)]
pub(super) struct Wire {
    /// Number of initial socket failures for freshly admitted retry tests.
    pub(super) failures: Cell<usize>,
    pub(super) responses: Mutex<VecDeque<Vec<u8>>>,
    pub(super) requests: Arc<Mutex<Vec<String>>>,
    pub(super) closed: Arc<Mutex<usize>>,
    /// Immediate client drop, independent of server scheduling.
    pub(super) client_dropped: Arc<Mutex<usize>>,
}
impl Wire {
    pub(super) fn new(responses: Vec<Vec<u8>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            ..Self::default()
        }
    }
}
impl PinnedTransport for Wire {
    type Connection = ObservedStream;
    fn connect(
        &self,
        destination: CheckedDestination,
    ) -> Pin<Box<dyn Future<Output = Result<ObservedStream, Refusal>> + Send + '_>> {
        assert_eq!(destination.socket().ip().to_string(), "8.8.8.8");
        if self.failures.get() > 0 {
            self.failures.set(self.failures.get() - 1);
            return Box::pin(async { Err(Refusal::Access) });
        }
        let response = self.responses.lock().unwrap().pop_front();
        let requests = self.requests.clone();
        let closed = self.closed.clone();
        let client_dropped = self.client_dropped.clone();
        Box::pin(async move {
            let (client, server) = duplex(1024);
            tokio::spawn(serve(server, response, requests, closed));
            Ok(ObservedStream {
                inner: client,
                dropped: client_dropped,
            })
        })
    }
}
/// Complete valid response without requiring a server implementation.
pub(super) fn response(status: u16, headers: &str, body: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 {status} Synthetic\r\nContent-Length: {}\r\n{headers}\r\n",
        body.len()
    )
    .into_bytes();
    bytes.extend_from_slice(body);
    bytes
}
/// One runtime; paused Tokio time makes stalled work deterministic.
pub(super) fn run(work: impl Future<Output = ()>) {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            pause();
            work.await;
        });
}
/// Bind all replaceable ports to the same immutable source policy.
pub(super) fn http<'a>(
    policy: &'a CheckedPolicy,
    controls: &'a dyn AdmissionControls,
    grants: &'a Grants,
    dns: &'a Dns,
    wire: &'a Wire,
) -> Http<'a, Wire> {
    Http {
        policy,
        controls,
        authority: grants,
        resolver: dns,
        transport: wire,
    }
}
/// Explicit current host identity and query-free authority binding.
pub(super) fn fetch(url: &str) -> Fetch<'_> {
    Fetch {
        request: n07::request(url, RequestKind::Seed),
        principal: "synthetic",
        scope: "workspace/default/collection/garden",
        account: "public",
        authority_time: UNIX_EPOCH,
        credentials: None,
        robots: false,
    }
}

/// Test-owned raw server: response closure and dropped-client evidence are distinct.
async fn serve(
    mut server: DuplexStream,
    response: Option<Vec<u8>>,
    requests: Arc<Mutex<Vec<String>>>,
    closed: Arc<Mutex<usize>>,
) {
    let mut request = Vec::new();
    let mut byte = [0];
    while !request.ends_with(b"\r\n\r\n") && server.read_exact(&mut byte).await.is_ok() {
        request.push(byte[0]);
    }
    requests
        .lock()
        .unwrap()
        .push(String::from_utf8(request).unwrap());
    if let Some(response) = response {
        drop(server.write_all(&response).await);
        drop(server.shutdown().await);
    }
    let mut remaining = Vec::new();
    drop(server.read_to_end(&mut remaining).await);
    *closed.lock().unwrap() += 1;
}

/// Observe ownership without requiring the remote peer to run after cancellation.
#[derive(Debug)]
pub(super) struct ObservedStream {
    /// Test-owned duplex endpoint.
    inner: DuplexStream,
    /// Synchronous endpoint destruction count.
    dropped: Arc<Mutex<usize>>,
}
impl Drop for ObservedStream {
    fn drop(&mut self) {
        *self.dropped.lock().unwrap() += 1;
    }
}
impl AsyncRead for ObservedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buffer)
    }
}
impl AsyncWrite for ObservedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<IoResult<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
