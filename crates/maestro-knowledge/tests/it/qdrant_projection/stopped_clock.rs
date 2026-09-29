//! A stopped clock, for the searches whose deadlines a loaded host must not
//! decide: tokio's clock is paused and held still while a search computes and
//! waits on the fake Qdrant, and it moves only once the test lets it.

use maestro_kernel::telemetry::stage::OUTCOME;
use std::{
    collections::HashSet,
    fmt,
    future::Future,
    pin::pin,
    sync::{Arc, LazyLock, Mutex, mpsc},
};
use tokio::{sync::Notify, task, time};
use tracing::{
    Dispatch, Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
    subscriber::{self, DefaultGuard, NoSubscriber},
};

/// Runs `work` on tokio's paused clock, held still until `release` ends;
/// from then on the clock moves only to the runtime's next timer, when every
/// task waits. Real time resumes when `work` ends.
///
/// Paused alone, tokio advances the clock to the next timer whenever the
/// runtime waits, and a loopback gRPC reply in flight is such a wait: its
/// readiness does not count as a wake, so a route's deadline could pass at
/// once. Tokio does not auto-advance while a blocking task runs ("Preventing
/// auto-advance" in the documentation of `tokio::time::pause`), so one
/// blocking task waiting for the release holds the clock still, however
/// loaded the host.
pub(super) async fn on_stopped_clock<T>(
    release: impl Future<Output = ()>,
    work: impl Future<Output = T>,
) -> T {
    time::pause();
    let (unhold, held) = mpsc::channel::<()>();
    let hold = task::spawn_blocking(move || held.recv());
    let mut unhold = Some(unhold);
    let mut release = pin!(release);
    let mut work = pin!(work);
    let output = loop {
        tokio::select! {
            output = &mut work => break output,
            () = &mut release, if unhold.is_some() => unhold = None,
        }
    };
    drop(unhold);
    hold.await.unwrap().unwrap_err();
    time::resume();
    output
}

/// A dispatcher registered for the life of the test process, so that
/// `tracing` asks each thread's default subscriber about a callsite instead
/// of caching it as never wanted (see `synthetic_gate::span_recorder`).
static SECOND_DISPATCHER: LazyLock<Dispatch> =
    LazyLock::new(|| Dispatch::new(NoSubscriber::default()));

/// The end of one pipeline stage on this thread, watched from its creation
/// until it drops, as the thread's default subscriber meanwhile: a release
/// for [`on_stopped_clock`] once a route no longer waits on the fake Qdrant.
pub(super) struct StageEnd {
    /// Told when the stage records its outcome.
    ended: Arc<Notify>,
    /// Keeps the watcher the thread's default subscriber.
    _default: DefaultGuard,
}

impl StageEnd {
    /// Watches for the stage span named `stage`, such as
    /// `retrieval.route.lexical`, to record its outcome.
    pub(super) fn watch(stage: &'static str) -> Self {
        Self::watch_all(&[stage])
    }

    /// Watches each `stage` until its outcome has been recorded.
    pub(super) fn watch_all(stages: &[&'static str]) -> Self {
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
    pub(super) async fn ended(&self) {
        self.ended.notified().await;
    }
}

/// The subscriber: it keeps each span's name, and tells `ended` when every
/// watched stage records its outcome.
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
#[derive(Default)]
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

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let mut names = self.names.lock().unwrap();
        names.push(span.metadata().name());
        Id::from_u64(names.len().try_into().unwrap())
    }

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
