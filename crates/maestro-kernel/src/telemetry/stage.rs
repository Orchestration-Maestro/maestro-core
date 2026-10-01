//! One traced stage of a knowledge operation: its span, the outcome it ends
//! with, how long it ran, and what it counted.
//!
//! A stage records identities and counts, never content: no question,
//! passage, title, source reference, path or prompt reaches its span. Its
//! span is opened by a function of the `span` module, under a pinned name.
//! A stage dropped before it finishes, by an early return, a panic or a
//! cancelled future, ends as [`Outcome::Error`].

use std::{future::Future, time::Instant};
use tracing::{Dispatch, Span, dispatcher, instrument::Instrumented};

/// The attribute of a stage's [`Outcome`], by its name.
pub const OUTCOME: &str = "outcome";

/// The attribute of how long a stage ran, in microseconds.
pub const DURATION_US: &str = "duration_us";

/// The attribute of the collection a stage works on, by its ID.
pub const COLLECTION_ID: &str = "collection_id";

/// The attribute of the generation a stage works on, by its number.
pub const GENERATION: &str = "generation";

/// How a stage ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It did its work, written `ok`.
    Ok,
    /// It declined the request by its contract, written `refused`.
    Refused,
    /// A dependency it needs could not serve it, written `unavailable`.
    Unavailable,
    /// Its deadline elapsed, written `timeout`.
    Timeout,
    /// It failed, written `error`.
    Error,
}

impl Outcome {
    /// The outcome's name, as a span records it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Refused => "refused",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::Error => "error",
        }
    }
}

/// A count a stage records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Count {
    /// Candidates: route hits, fused or ranked chunks.
    Candidates,
    /// Chunks of a chunk set or of a batch.
    Chunks,
    /// Passages selected for a bundle.
    Passages,
    /// Points of a Qdrant collection.
    Points,
    /// Source revisions read.
    Sources,
}

impl Count {
    /// The attribute the count is recorded under.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Candidates => "candidates",
            Self::Chunks => "chunks",
            Self::Passages => "passages",
            Self::Points => "points",
            Self::Sources => "sources",
        }
    }
}

/// A stage in progress: its span, and when it started.
#[derive(Debug)]
pub struct Stage {
    /// The span every call of the stage runs in.
    span: Span,
    /// When the stage was opened.
    started: Instant,
    /// Whether its outcome is recorded.
    finished: bool,
}

impl Stage {
    /// Starts the stage of `span`, now.
    pub(super) fn open(span: Span) -> Self {
        Self {
            span,
            started: Instant::now(),
            finished: false,
        }
    }

    /// Records `value` as the stage's `count`.
    pub fn count(&self, count: Count, value: usize) {
        self.span
            .record(count.field(), u64::try_from(value).unwrap_or(u64::MAX));
    }

    /// Records the collection the stage works on, by its ID.
    pub fn collection(&self, id: &str) {
        self.span.record(COLLECTION_ID, id);
    }

    /// Records the generation the stage works on, by its number.
    pub fn generation(&self, id: i64) {
        self.span.record(GENERATION, id);
    }

    /// Runs `future` inside the stage's span, so that the stages it opens
    /// are the stage's children.
    pub fn instrument<F: Future>(&self, future: F) -> Instrumented<F> {
        tracing::Instrument::instrument(future, self.span.clone())
    }

    /// Runs `work` inside the stage's span.
    pub fn in_scope<T>(&self, work: impl FnOnce() -> T) -> T {
        self.span.in_scope(work)
    }

    /// The stage's span and the current subscriber, to carry to another
    /// thread, such as a blocking worker.
    #[must_use]
    pub fn carry(&self) -> Carried {
        Carried {
            dispatch: dispatcher::get_default(Dispatch::clone),
            span: self.span.clone(),
        }
    }

    /// Ends the stage with `outcome` and its duration.
    pub fn finish(mut self, outcome: Outcome) {
        self.end(outcome);
    }

    /// Records `outcome` and the duration, once.
    fn end(&mut self, outcome: Outcome) {
        if self.finished {
            return;
        }
        self.finished = true;
        let elapsed = u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX);
        self.span.record(OUTCOME, outcome.as_str());
        self.span.record(DURATION_US, elapsed);
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        self.end(Outcome::Error);
    }
}

/// A stage's span and the subscriber current where it was carried from,
/// so that the stages opened on another thread reach the same subscriber
/// as its children.
#[derive(Debug, Clone)]
pub struct Carried {
    /// The subscriber current where it was taken.
    dispatch: Dispatch,
    /// The stage's span.
    span: Span,
}

impl Carried {
    /// Runs `work` with the carried subscriber and span current.
    pub fn in_scope<T>(&self, work: impl FnOnce() -> T) -> T {
        dispatcher::with_default(&self.dispatch, || self.span.in_scope(work))
    }
}
