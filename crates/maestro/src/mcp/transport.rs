//! Stdio JSON-RPC framing with complete-line and complete-response byte bounds.

use crate::knowledge::RESPONSE_LIMIT_BYTES;
use maestro_kernel::json::canonical;
use rmcp::{
    ErrorData,
    model::{JsonRpcMessage, RequestId},
    service::{RoleServer, RxJsonRpcMessage, TxJsonRpcMessage},
    transport::Transport,
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    future::Future,
    io,
    mem::take,
    sync::{Arc, Mutex as StdMutex, PoisonError},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    sync::Mutex,
    time::sleep,
};

/// Maximum serialized UTF-8 bytes in a JSON-RPC input line, including its newline.
pub(crate) const INPUT_LIMIT_BYTES: usize = RESPONSE_LIMIT_BYTES;
/// Maximum UTF-8 bytes in a string request ID.
const REQUEST_ID_LIMIT_BYTES: usize = 256;

/// The bounded stdio byte stream used by the MCP service.
#[derive(Debug)]
pub(crate) struct BoundedStdio<R, W> {
    /// Incoming byte stream.
    reader: R,
    /// Serialized protocol output, shared between concurrent requests.
    writer: Arc<Mutex<W>>,
    /// Bytes read beyond the current complete line.
    pending: Vec<u8>,
    /// Whether the current input line exceeded its byte limit and is being discarded.
    discarding: bool,
    /// Protocol errors queued for independent flushing or close.
    pending_errors: Arc<StdMutex<VecDeque<Vec<u8>>>>,
    /// Whether the input stream returned EOF.
    eof: bool,
}

impl<R, W> BoundedStdio<R, W> {
    /// Creates a stdio transport over the supplied streams.
    pub(crate) fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer: Arc::new(Mutex::new(writer)),
            pending: Vec::new(),
            discarding: false,
            pending_errors: Arc::new(StdMutex::new(VecDeque::new())),
            eof: false,
        }
    }
}

impl<R, W> BoundedStdio<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Send + Unpin + 'static,
{
    /// Reads one bounded frame, including a final valid frame without a newline at EOF.
    async fn line(&mut self) -> Option<Vec<u8>> {
        loop {
            match self.pending.iter().position(|byte| *byte == b'\n') {
                Some(newline) if self.discarding || newline >= INPUT_LIMIT_BYTES => {
                    self.pending.drain(..=newline);
                    self.invalid_request(-32600, "MCP input line exceeds 65536 bytes");
                    self.discarding = false;
                    continue;
                }
                Some(newline) => {
                    let mut frame = self.pending.drain(..=newline).collect::<Vec<_>>();
                    frame.pop();
                    frame.truncate(frame.len() - usize::from(frame.ends_with(b"\r")));
                    return Some(frame);
                }
                None => {}
            }
            if !self.discarding && self.pending.len() > INPUT_LIMIT_BYTES {
                self.pending.clear();
                self.discarding = true;
            }
            if self.eof && self.discarding {
                self.pending.clear();
                self.discarding = false;
                self.invalid_request(-32600, "MCP input line exceeds 65536 bytes");
                return None;
            }
            if self.eof {
                return (!self.pending.is_empty()).then(|| take(&mut self.pending));
            }
            if self.discarding {
                self.pending.clear();
            }
            let mut buffer = [0_u8; 8192];
            match self.reader.read(&mut buffer).await {
                Ok(0) => self.eof = true,
                Ok(read) => self.pending.extend(buffer.into_iter().take(read)),
                Err(_) => return None,
            }
        }
    }

    /// Queues a protocol-safe invalid-request error without echoing an invalid ID.
    fn invalid_request(&mut self, code: i64, message: &'static str) {
        let response = canonical(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": {"code": code, "message": message}
        }));
        if let Ok(mut bytes) = serde_json::to_vec(&response) {
            bytes.push(b'\n');
            self.pending_errors
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push_back(bytes);
            spawn_error_flush(self.writer.clone(), self.pending_errors.clone());
        }
    }
}

impl<R, W> Transport<RoleServer> for BoundedStdio<R, W>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    type Error = io::Error;

    fn send(
        &mut self,
        message: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let writer = self.writer.clone();
        async move {
            let mut bytes = serde_json::to_vec(&message).map_err(serialization_error)?;
            if bytes.len().saturating_add(1) > RESPONSE_LIMIT_BYTES {
                let id = response_id(&message);
                let refusal = TxJsonRpcMessage::<RoleServer>::error(
                    ErrorData::internal_error("response_too_large", None),
                    id,
                );
                bytes = serde_json::to_vec(&refusal).map_err(serialization_error)?;
            }
            if bytes.len().saturating_add(1) > RESPONSE_LIMIT_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bounded MCP refusal exceeds the response limit",
                ));
            }
            let mut writer = writer.lock().await;
            writer.write_all(&bytes).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await
        }
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleServer>> {
        loop {
            let mut frame = self.line().await?;
            if frame.starts_with(b"\xEF\xBB\xBF") {
                frame.drain(..3);
            }
            if frame.iter().all(u8::is_ascii_whitespace) {
                sleep(Duration::from_millis(1)).await;
                continue;
            }
            let Ok(value) = serde_json::from_slice::<Value>(&frame) else {
                self.invalid_request(-32700, "parse error");
                sleep(Duration::from_millis(1)).await;
                continue;
            };
            if value
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id.len() > REQUEST_ID_LIMIT_BYTES)
            {
                self.invalid_request(-32600, "request ID exceeds 256 UTF-8 bytes");
                sleep(Duration::from_millis(1)).await;
                continue;
            }
            if let Ok(message) = serde_json::from_value(value) {
                return Some(message);
            }
            self.invalid_request(-32600, "invalid MCP request");
            sleep(Duration::from_millis(1)).await;
        }
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        let pending_errors = self.pending_errors.clone();
        let mut writer = self.writer.lock().await;
        flush_errors(&pending_errors, &mut *writer).await?;
        writer.flush().await
    }
}

/// Writes protocol errors queued while receiving, preserving each complete line.
async fn flush_errors<W: AsyncWrite + Unpin>(
    pending_errors: &StdMutex<VecDeque<Vec<u8>>>,
    writer: &mut W,
) -> io::Result<()> {
    let errors = take_pending_errors(pending_errors);
    for error in errors {
        writer.write_all(&error).await?;
    }
    Ok(())
}

/// Drains protocol errors synchronously so no async lock spans a write.
fn take_pending_errors(pending_errors: &StdMutex<VecDeque<Vec<u8>>>) -> Vec<Vec<u8>> {
    take(
        &mut *pending_errors
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
    )
    .into_iter()
    .collect()
}

/// Writes queued errors independently of the cancelable receive/send futures.
fn spawn_error_flush<W>(writer: Arc<Mutex<W>>, pending_errors: Arc<StdMutex<VecDeque<Vec<u8>>>>)
where
    W: AsyncWrite + Send + Unpin + 'static,
{
    drop(tokio::spawn(async move {
        let mut writer = writer.lock().await;
        flush_errors(&pending_errors, &mut *writer).await?;
        writer.flush().await
    }));
}

/// Returns the known JSON-RPC identifier to correlate a bounded fallback.
fn response_id(message: &TxJsonRpcMessage<RoleServer>) -> Option<RequestId> {
    match message {
        JsonRpcMessage::Response(response) => Some(response.id.clone()),
        JsonRpcMessage::Error(error) => error.id.clone(),
        JsonRpcMessage::Request(request) => Some(request.id.clone()),
        JsonRpcMessage::Notification(_) => None,
    }
}

/// A safe serialization error with no input or backend text.
fn serialization_error(_: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "cannot serialize MCP response")
}

#[cfg(test)]
mod tests;
