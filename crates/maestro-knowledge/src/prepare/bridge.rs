//! A model port's asynchronous `tokenize`, called synchronously: the port's
//! futures run to completion on a small runtime, on a thread of their own,
//! which answers over a channel. Tokio's `block_on` panics on a thread that
//! already runs a runtime; that thread never does, so a caller may count
//! from a plain thread or from inside a runtime alike.

use super::error::TokenizerError;
use maestro_kernel::gateway::{self, ModelCard, ModelPort, Room};
use std::{
    io,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};
use tokio::runtime::Builder;

/// One text to tokenize, and where its answer goes.
struct Request {
    /// The complete text.
    text: String,
    /// Where the port's answer goes.
    answer: Sender<Result<Vec<u32>, gateway::Error>>,
}

/// One card's `tokenize` through one port, answered on a thread of its own
/// in free room, until the bridge is dropped.
#[derive(Debug)]
pub(super) struct Bridge {
    /// Where requests go to the thread.
    requests: Sender<Request>,
}

impl Bridge {
    /// Starts the thread that calls `port` for `card`, once its runtime runs.
    ///
    /// # Errors
    ///
    /// [`TokenizerError::Start`] when the thread or its runtime cannot start.
    pub(super) fn start<P>(port: P, card: ModelCard) -> Result<Self, TokenizerError>
    where
        P: ModelPort + Send + 'static,
    {
        let (requests, received) = mpsc::channel();
        let (report, started) = mpsc::channel();
        thread::Builder::new()
            .name("router-tokenizer".to_owned())
            .spawn(move || answer(&port, &card, &received, &report))
            .map_err(TokenizerError::Start)?;
        started
            .recv()
            .map_err(|_| TokenizerError::Stopped)?
            .map_err(TokenizerError::Start)?;
        Ok(Self { requests })
    }

    /// The ordered token IDs the port gives `text`, loading its model only
    /// into free room.
    ///
    /// # Errors
    ///
    /// The port's refusal, and [`TokenizerError::Stopped`] when the thread
    /// stopped.
    pub(super) fn tokenize(&self, text: &str) -> Result<Vec<u32>, TokenizerError> {
        let (answer, answered) = mpsc::channel();
        let request = Request {
            text: text.to_owned(),
            answer,
        };
        self.requests
            .send(request)
            .map_err(|_| TokenizerError::Stopped)?;
        answered
            .recv()
            .map_err(|_| TokenizerError::Stopped)?
            .map_err(TokenizerError::from_port)
    }
}

/// The thread: builds its runtime there, so that the runtime is also dropped
/// there and never inside the caller's, reports whether it could, then
/// answers each request in turn until the bridge is dropped.
fn answer<P: ModelPort>(
    port: &P,
    card: &ModelCard,
    requests: &Receiver<Request>,
    report: &Sender<io::Result<()>>,
) {
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            drop(report.send(Err(error)));
            return;
        }
    };
    drop(report.send(Ok(())));
    for request in requests {
        let ids = runtime.block_on(port.tokenize(card, Room::Free, &request.text));
        // The caller waits for the answer; nobody else needs it.
        drop(request.answer.send(ids));
    }
}
