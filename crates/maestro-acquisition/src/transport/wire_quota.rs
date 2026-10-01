//! Gate header read-ahead and cap every raw post-header read before I/O.
use super::failure::Failure;
use maestro_kernel::retrieval::Clock;
use std::{
    io::{Error, ErrorKind, Result as IoResult},
    mem::take,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    time::Instant,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Maximum informational header blocks before the final response in one hop.
pub const MAX_INTERIM_RESPONSES: u8 = 8;
/// Cumulative bounds for interim and final response headers, before body reads.
#[derive(Debug, Clone, Copy)]
pub(super) struct HeaderLimits {
    /// Total bytes across all header blocks.
    pub(super) bytes: usize,
    /// Total field lines across all header blocks.
    pub(super) fields: usize,
}
/// Shared driver/consumer quota; no response bytes or fields are retained here.
#[derive(Debug, Default, Clone)]
pub(super) struct Quota(Arc<Mutex<State>>);
/// One freshly admitted connection, never reused for another response.
#[derive(Debug, Default)]
struct State {
    /// None holds the body until status and Content-Length preflight complete.
    remaining: Option<u64>,
    /// Raw bytes read but not yet transferred to cumulative accounting.
    received: u64,
    /// Fixed-length completion needs no EOF read beyond its payload.
    fixed: bool,
    /// Preserve the precise bound refusal through hyper's socket error mapping.
    failure: Option<Failure>,
    /// Wake the held driver after the response consumer grants its allowance.
    reader: Option<Waker>,
}
impl Quota {
    /// Release the body gate only after the effective envelope is checked.
    pub(super) fn open(&self, remaining: u64, length: Option<u64>) -> Result<(), Failure> {
        let mut state = self.0.lock().map_err(|_| Failure::Configuration)?;
        state.remaining = Some(length.map_or(remaining, |length| remaining.min(length)));
        state.fixed = length.is_some();
        if let Some(reader) = state.reader.take() {
            reader.wake();
        }
        Ok(())
    }
    /// Charge actual reads, including parser read-ahead, chunk framing and trailers.
    pub(super) fn take_received(&self) -> Result<(u64, Option<Failure>), Failure> {
        let mut state = self.0.lock().map_err(|_| Failure::Configuration)?;
        Ok((take(&mut state.received), state.failure.clone()))
    }
}
/// Constant-size delimiter/status detector; hyper still parses every header block.
#[derive(Debug)]
struct HeaderGate {
    /// Independently composed cumulative header envelope.
    limits: HeaderLimits,
    /// Only enough status-line bytes to classify interim responses.
    prefix: [u8; 12],
    /// Received prefix bytes; incomplete status lines refuse.
    prefix_len: usize,
    /// Constant-size CRLFCRLF detector.
    tail: u32,
    /// Status lines are not header fields.
    first_line: bool,
    /// Distinguish a field line from the empty terminator.
    line_bytes: usize,
    /// Cumulative received header bytes.
    bytes: usize,
    /// Cumulative field lines, never reset for a new block.
    fields: usize,
    /// Informational responses preceding the final response.
    interim: u8,
    /// Only the final response can open the body gate.
    done: bool,
}
impl HeaderGate {
    /// Start with no consumed header allowance.
    fn new(limits: HeaderLimits) -> Self {
        Self {
            limits,
            prefix: [0; 12],
            prefix_len: 0,
            tail: 0,
            first_line: true,
            line_bytes: 0,
            bytes: 0,
            fields: 0,
            interim: 0,
            done: false,
        }
    }
    /// Count bytes and field lines cumulatively, not separately per interim block.
    fn push(&mut self, byte: u8) -> Result<(), Failure> {
        self.bytes += 1;
        self.line_bytes += 1;
        if self.first_line
            && let Some(slot) = self.prefix.get_mut(self.prefix_len)
        {
            *slot = byte;
            self.prefix_len += 1;
        }
        if byte == b'\n' {
            if !self.first_line && self.line_bytes > 2 {
                self.fields += 1;
            }
            self.first_line = false;
            self.line_bytes = 0;
            if self.fields > self.limits.fields {
                return Err(Failure::Content);
            }
        }
        self.tail = (self.tail << 8) | u32::from(byte);
        if self.tail != u32::from_be_bytes(*b"\r\n\r\n") {
            return Ok(());
        }
        let status = self.status()?;
        if status == 101 {
            return Err(Failure::Content);
        }
        if (100..200).contains(&status) {
            self.interim += 1;
            if self.interim > MAX_INTERIM_RESPONSES {
                return Err(Failure::Content);
            }
            self.prefix_len = 0;
            self.tail = 0;
            self.first_line = true;
        } else {
            self.done = true;
        }
        Ok(())
    }
    /// Read only the three status digits; version/line/field syntax stays with hyper.
    fn status(&self) -> Result<u16, Failure> {
        if self.prefix_len != self.prefix.len() {
            return Err(Failure::Content);
        }
        let digits = self.prefix.get(9..12).ok_or(Failure::Content)?;
        if !digits.iter().all(u8::is_ascii_digit) {
            return Err(Failure::Content);
        }
        Ok(digits
            .iter()
            .fold(0, |code, byte| code * 10 + u16::from(byte - b'0')))
    }
}
/// Thin owned I/O wrapper; dropping the HTTP future drops the wrapped connection.
#[derive(Debug)]
pub(super) struct BoundedIo<T> {
    /// Checked socket owned by this hop.
    inner: T,
    /// Body allowance shared with the response consumer.
    quota: Quota,
    /// Bounded interim/final header detector.
    headers: HeaderGate,
    /// Same trusted clock as cumulative document accounting.
    clock: Arc<dyn Clock>,
    /// Same effective document/run cutoff as the owning HTTP future.
    deadline: Instant,
}
impl<T> BoundedIo<T> {
    /// Start with the body held and no implicit resource allowance.
    pub(super) fn new(
        inner: T,
        quota: Quota,
        headers: HeaderLimits,
        timing: (Arc<dyn Clock>, Instant),
    ) -> Self {
        Self {
            inner,
            quota,
            headers: HeaderGate::new(headers),
            clock: timing.0,
            deadline: timing.1,
        }
    }
}
impl<T: AsyncRead + Unpin> AsyncRead for BoundedIo<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        let this = self.get_mut();
        let mut state = this
            .quota
            .0
            .lock()
            .map_err(|_| Error::from(ErrorKind::Other))?;
        if this.clock.now() >= this.deadline {
            state.failure = Some(Failure::Timeout);
            return Poll::Ready(Err(Error::from(ErrorKind::TimedOut)));
        }
        let allowance = if this.headers.done {
            let Some(left) = state.remaining else {
                state.reader = Some(context.waker().clone());
                return Poll::Pending;
            };
            if left == 0 && state.fixed {
                state.reader = Some(context.waker().clone());
                return Poll::Pending;
            }
            if left == 0 {
                state.failure = Some(Failure::EncodedBytes);
                return Poll::Ready(Err(Error::from(ErrorKind::FileTooLarge)));
            }
            usize::try_from(left).unwrap_or(usize::MAX)
        } else {
            if this.headers.bytes >= this.headers.limits.bytes {
                state.failure = Some(Failure::Content);
                return Poll::Ready(Err(Error::from(ErrorKind::InvalidData)));
            }
            // Generic AsyncRead has no peek: one-byte header reads are the only
            // way to guarantee no payload read before Content-Length preflight.
            1
        };
        let capacity = allowance.min(buffer.remaining());
        let mut limited = ReadBuf::new(buffer.initialize_unfilled_to(capacity));
        match Pin::new(&mut this.inner).poll_read(context, &mut limited) {
            Poll::Ready(Ok(())) => {
                let received = limited.filled();
                let count = received.len();
                if this.headers.done {
                    state.remaining = state.remaining.map(|left| left - count as u64);
                    state.received += count as u64;
                } else if let Some(byte) = received.first()
                    && let Err(failure) = this.headers.push(*byte)
                {
                    state.failure = Some(failure);
                    return Poll::Ready(Err(Error::from(ErrorKind::InvalidData)));
                }
                buffer.advance(count);
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
impl<T: AsyncWrite + Unpin> AsyncWrite for BoundedIo<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<IoResult<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, bytes)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
