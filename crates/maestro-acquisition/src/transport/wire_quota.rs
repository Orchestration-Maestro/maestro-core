//! Gate header read-ahead and cap every raw post-header read before I/O.
use super::failure::Failure;
use std::{
    io::{Error, ErrorKind, Result as IoResult},
    mem::take,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

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
    /// Distinguish quota exhaustion from an ordinary socket failure.
    exhausted: bool,
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
    pub(super) fn take_received(&self) -> Result<(u64, bool), Failure> {
        let mut state = self.0.lock().map_err(|_| Failure::Configuration)?;
        Ok((take(&mut state.received), state.exhausted))
    }
}
/// Thin owned I/O wrapper; dropping the HTTP future drops the wrapped connection.
#[derive(Debug)]
pub(super) struct BoundedIo<T> {
    /// Socket-only adapter selected by admission.
    inner: T,
    /// Shared read allowance set by the header consumer.
    quota: Quota,
    /// Constant-size header terminator detector, not a second header parser.
    tail: u32,
    /// Never permit body bytes in a header read, even in the same socket packet.
    headers_done: bool,
}
impl<T> BoundedIo<T> {
    /// Start with the body held and no implicit resource allowance.
    pub(super) fn new(inner: T, quota: Quota) -> Self {
        Self {
            inner,
            quota,
            tail: 0,
            headers_done: false,
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
        let allowance = if this.headers_done {
            let Some(left) = state.remaining else {
                state.reader = Some(context.waker().clone());
                return Poll::Pending;
            };
            if left == 0 && state.fixed {
                state.reader = Some(context.waker().clone());
                return Poll::Pending;
            }
            if left == 0 {
                state.exhausted = true;
                return Poll::Ready(Err(Error::from(ErrorKind::FileTooLarge)));
            }
            usize::try_from(left).unwrap_or(usize::MAX)
        } else {
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
                if this.headers_done {
                    state.remaining = state.remaining.map(|left| left - count as u64);
                    state.received += count as u64;
                } else if let Some(byte) = received.first() {
                    this.tail = (this.tail << 8) | u32::from(*byte);
                    this.headers_done = this.tail == u32::from_be_bytes(*b"\r\n\r\n");
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
