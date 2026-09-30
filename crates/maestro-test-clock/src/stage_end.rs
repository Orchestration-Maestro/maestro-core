//! Watches the outcome fields of tracing stages for a held-clock release.

use std::{
    collections::HashSet,
    fmt,
    sync::{Arc, LazyLock, Mutex},
};
use tokio::sync::Notify;
use tracing::{
    Dispatch, Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
    subscriber::{self, DefaultGuard, NoSubscriber},
};

/// The conventional tracing field by which a pipeline stage finishes.
const OUTCOME: &str = "outcome";

/// A dispatcher registered for the life of the test process, so that
/// `tracing` asks each thread's default subscriber about a callsite instead
/// of caching it as never wanted.
static SECOND_DISPATCHER: LazyLock<Dispatch> =
    LazyLock::new(|| Dispatch::new(NoSubscriber::default()));

/// The end of one pipeline stage on this thread, watched from its creation
/// until it drops, as the thread's default subscriber meanwhile: a release
/// for [`crate::on_stopped_clock`] once a route no longer waits on the fake Qdrant.
#[derive(Debug)]
pub struct StageEnd {
    /// Told when the stage records its outcome.
    ended: Arc<Notify>,
    /// Keeps the watcher the thread's default subscriber.
    _default: DefaultGuard,
}

impl StageEnd {
    /// Watches for the stage span named `stage`, such as
    /// `retrieval.route.lexical`, to record its outcome.
    ///
    /// # Panics
    ///
    /// If tracing's dispatcher registration fails.
    #[must_use]
    pub fn watch(stage: &'static str) -> Self {
        Self::watch_all(&[stage])
    }

    /// Watches each `stage` until its outcome has been recorded.
    ///
    /// # Panics
    ///
    /// If tracing's dispatcher registration fails.
    #[must_use]
    pub fn watch_all(stages: &[&'static str]) -> Self {
        LazyLock::force(&SECOND_DISPATCHER);
        let ended = Arc::new(Notify::new());
        let watcher = Watcher {
            stages: stages.to_vec(),
            ended: Arc::clone(&ended),
            names: Mutex::default(),
            seen: Mutex::default(),
        };
        Self {
            ended,
            _default: subscriber::set_default(watcher),
        }
    }

    /// Ends once the stage has recorded its outcome.
    pub async fn ended(&self) {
        self.ended.notified().await;
    }
}

/// The subscriber: it keeps each span's name, and tells `ended` when every
/// watched stage records its outcome.
#[derive(Debug)]
struct Watcher {
    /// The names of the watched stage spans.
    stages: Vec<&'static str>,
    /// Told when every watched stage records its outcome.
    ended: Arc<Notify>,
    /// The name of each span; a span's ID is its position + 1.
    names: Mutex<Vec<&'static str>>,
    /// The watched stages that recorded their outcome.
    seen: Mutex<HashSet<&'static str>>,
}

/// Whether a span's recorded values hold its outcome.
#[derive(Debug, Default)]
struct HasOutcome(bool);

impl Visit for HasOutcome {
    fn record_debug(&mut self, field: &Field, _value: &dyn fmt::Debug) {
        self.0 |= field.name() == OUTCOME;
    }
}

impl Subscriber for Watcher {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    #[expect(
        clippy::unwrap_used,
        reason = "poisoned test state or an invalid tracing span ID fails the test"
    )]
    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let mut names = self.names.lock().unwrap();
        names.push(span.metadata().name());
        Id::from_u64(names.len().try_into().unwrap())
    }

    #[expect(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "tracing must return the span IDs created by this test subscriber"
    )]
    fn record(&self, span: &Id, values: &Record<'_>) {
        let mut outcome = HasOutcome::default();
        values.record(&mut outcome);
        let position = usize::try_from(span.into_u64() - 1).unwrap();
        let name = self.names.lock().unwrap()[position];
        if outcome.0 && self.stages.contains(&name) {
            let mut seen = self.seen.lock().unwrap();
            seen.insert(name);
            if seen.len() == self.stages.len() {
                self.ended.notify_one();
            }
        }
    }

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {}

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}
