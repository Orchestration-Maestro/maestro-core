//! Test-owned timing and segmented I/O instrumentation for N09 review probes.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls,
    n09_support::{Grants, Wire},
};
use maestro_acquisition::{
    Refusal,
    policy::{
        authority::{Authority, AuthorityRefusal, Operation, Permit, Target},
        decision::{AdmissionControls, Request},
        identity::FetchIdentity,
        source::Source,
    },
    transport::connect::{CheckedDestination, PinnedTransport},
};
use maestro_kernel::retrieval::Clock;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::Future,
    io::Result as IoResult,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
    time::{Duration, Instant as StdInstant, SystemTime},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    time::{Instant, advance},
};
/// Header bytes, payload bytes and trusted connection latency.
type Segments = (Vec<u8>, Vec<u8>, Duration);
/// Separate header and payload reads, optionally delaying connection completion.
#[derive(Debug)]
pub(super) struct SegmentedWire {
    /// Headers, payload, and pre-response wait for each owned connection.
    responses: Mutex<VecDeque<Segments>>,
    /// Direct connection count, independent of accounting or HTTP requests.
    pub(super) dials: Cell<usize>,
    /// Payload bytes actually returned to hyper's read buffer.
    pub(super) payload_read: Arc<AtomicUsize>,
    /// Optional trusted-clock advance at actual receive completion.
    pub(super) clock_on_eof: Option<Arc<ManualClock>>,
    /// Optional advance when the last header byte is received (unread robots).
    pub(super) clock_on_headers: Option<Arc<ManualClock>>,
    pub(super) completion_elapsed: Duration,
}
impl SegmentedWire {
    /// Explicit response segments with no network listener.
    pub(super) fn new(responses: Vec<Segments>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            dials: Cell::new(0),
            payload_read: Arc::new(AtomicUsize::new(0)),
            clock_on_eof: None,
            clock_on_headers: None,
            completion_elapsed: Duration::from_mins(2),
        }
    }
}
impl PinnedTransport for SegmentedWire {
    type Connection = SegmentedStream;
    fn connect(
        &self,
        destination: CheckedDestination,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Connection, Refusal>> + Send + '_>> {
        assert_eq!(destination.socket().ip().to_string(), "8.8.8.8");
        self.dials.set(self.dials.get() + 1);
        let response = self.responses.lock().unwrap().pop_front();
        let payload_read = self.payload_read.clone();
        let clock_on_eof = self.clock_on_eof.clone();
        let clock_on_headers = self.clock_on_headers.clone();
        let completion_elapsed = self.completion_elapsed;
        Box::pin(async move {
            let (headers, payload, delay) = response.ok_or(Refusal::Access)?;
            advance(delay).await;
            let header_end = headers
                .windows(4)
                .position(|bytes| bytes == b"\r\n\r\n")
                .map_or(headers.len(), |index| index + 4);
            Ok(SegmentedStream {
                header_end,
                request_written: false,
                reader: None,
                clock_on_eof,
                clock_on_headers,
                completion_elapsed,
                headers,
                payload,
                header_offset: 0,
                payload_offset: 0,
                payload_read,
            })
        })
    }
}
/// Reads never combine a header segment with payload bytes.
#[derive(Debug)]
pub(super) struct SegmentedStream {
    /// A synthetic server cannot send a response before HTTP dispatch.
    request_written: bool,
    reader: Option<Waker>,
    /// Exact boundary for measuring contiguous header/body read-ahead.
    header_end: usize,
    /// Trusted completion hooks, consumed exactly once.
    clock_on_eof: Option<Arc<ManualClock>>,
    clock_on_headers: Option<Arc<ManualClock>>,
    completion_elapsed: Duration,
    /// First read segment.
    headers: Vec<u8>,
    /// Later read segment, measured separately.
    payload: Vec<u8>,
    /// Bytes already read from the headers.
    header_offset: usize,
    /// Bytes already read from the payload.
    payload_offset: usize,
    /// Shared actual payload read count.
    pub(super) payload_read: Arc<AtomicUsize>,
}
impl AsyncRead for SegmentedStream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        let stream = self.get_mut();
        if !stream.request_written {
            stream.reader = Some(context.waker().clone());
            return Poll::Pending;
        }
        if stream.header_offset < stream.headers.len() {
            let bytes = stream.headers.get(stream.header_offset..).unwrap();
            let count = bytes.len().min(buffer.remaining());
            buffer.put_slice(bytes.get(..count).unwrap());
            let before = stream.header_offset.saturating_sub(stream.header_end);
            stream.header_offset += count;
            let after = stream.header_offset.saturating_sub(stream.header_end);
            stream
                .payload_read
                .fetch_add(after - before, Ordering::SeqCst);
            if stream.header_offset >= stream.header_end
                && let Some(clock) = stream.clock_on_headers.take()
            {
                *clock.0.lock().unwrap() += stream.completion_elapsed;
            }
        } else {
            let bytes = stream.payload.get(stream.payload_offset..).unwrap();
            let count = bytes.len().min(buffer.remaining());
            buffer.put_slice(bytes.get(..count).unwrap());
            stream.payload_offset += count;
            stream.payload_read.fetch_add(count, Ordering::SeqCst);
            if count == 0
                && let Some(clock) = stream.clock_on_eof.take()
            {
                *clock.0.lock().unwrap() += stream.completion_elapsed;
            }
        }
        Poll::Ready(Ok(()))
    }
}
impl AsyncWrite for SegmentedStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<IoResult<usize>> {
        let stream = self.get_mut();
        stream.request_written = true;
        if let Some(reader) = stream.reader.take() {
            reader.wake();
        }
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Retain the Controls fixture while observing per-hop policy and monotonic time.
#[derive(Debug)]
pub(super) struct TimedControls {
    /// Existing synthetic admission fixture.
    pub(super) inner: Controls,
    /// Monotonic document start.
    pub(super) started: Instant,
    /// Policy timestamp supplied at each admission, paired with elapsed millis.
    pub(super) stages: RefCell<Vec<(String, u128)>>,
}
impl AdmissionControls for TimedControls {
    fn caller(&self, source: &Source, request: &Request<'_>) -> Result<(), Refusal> {
        self.stages
            .borrow_mut()
            .push((request.now.into(), self.started.elapsed().as_millis()));
        self.inner.caller(source, request)
    }
    fn network(&self, identity: &FetchIdentity) -> Result<(), Refusal> {
        self.inner.network(identity)
    }
    fn robots(&self, identity: &FetchIdentity) -> Result<(), Refusal> {
        self.inner.robots(identity)
    }
}
/// Retain the grant fixture while recording the actual authority wall times.
#[derive(Debug, Default)]
pub(super) struct TimedGrants {
    /// Existing synthetic grant fixture.
    pub(super) inner: Grants,
    /// Actual authority timestamp for each invocation.
    pub(super) times: RefCell<Vec<SystemTime>>,
}
impl Authority for TimedGrants {
    fn decide(
        &self,
        principal: &str,
        operation: Operation,
        target: &Target,
        now: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        self.times.borrow_mut().push(now);
        self.inner.decide(principal, operation, target, now)
    }
}

/// Trusted clock advanced by the async transport without real sleeps.
#[derive(Debug)]
pub(super) struct ManualClock(pub(super) Mutex<StdInstant>);
impl Clock for ManualClock {
    fn now(&self) -> StdInstant {
        *self.0.lock().unwrap()
    }
}
/// Existing Wire fixture with an async connection completing at the cutoff.
#[derive(Debug)]
pub(super) struct CutoffWire<'a> {
    /// Existing instrumented connection fixture.
    pub(super) wire: &'a Wire,
    /// Same trusted clock bound to the document accounting.
    pub(super) clock: Arc<ManualClock>,
    pub(super) elapsed: Duration,
}
impl PinnedTransport for CutoffWire<'_> {
    type Connection = <Wire as PinnedTransport>::Connection;
    fn connect(
        &self,
        destination: CheckedDestination,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Connection, Refusal>> + Send + '_>> {
        let connecting = self.wire.connect(destination);
        let clock = self.clock.clone();
        let elapsed = self.elapsed;
        Box::pin(async move {
            let connection = connecting.await?;
            *clock.0.lock().unwrap() += elapsed;
            Ok(connection)
        })
    }
}
