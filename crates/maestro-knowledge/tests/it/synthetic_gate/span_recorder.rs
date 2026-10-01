//! A minimal recording subscriber, written with the `tracing` API alone: it
//! keeps every span, its parent and each value recorded on it, and the fields
//! of every event, while it is the default.
#![cfg(test)]

use std::{
    cell::RefCell,
    fmt, mem,
    sync::{Arc, LazyLock, Mutex},
};
use tracing::{
    Dispatch, Event, Metadata, Subscriber, dispatcher,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
    subscriber::NoSubscriber,
};

/// A dispatcher that stays registered for the life of the test process.
///
/// `tracing` caches whether each callsite is wanted, once for the whole
/// process. While at most one dispatcher is registered, a callsite first
/// reached on a thread with no subscriber, such as a concurrent test that
/// publishes too, is cached as never wanted, and a recorder installed on
/// another thread never sees it. With a second dispatcher registered,
/// `tracing` caches "sometimes" instead, and asks each thread's own default.
static SECOND_DISPATCHER: LazyLock<Dispatch> =
    LazyLock::new(|| Dispatch::new(NoSubscriber::default()));

/// A span as the recorder saw it.
#[derive(Debug, Clone)]
pub(super) struct Recorded {
    /// The span's name.
    pub(super) name: &'static str,
    /// The position of the span it was opened in, if any.
    parent: Option<usize>,
    /// Its fields, in the order they were given a value, each as text.
    pub(super) fields: Vec<(String, String)>,
}

impl Recorded {
    /// The value of `field`, as text.
    pub(super) fn field(&self, field: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, value)| value.as_str())
    }
}

/// What a recorder saw: the spans in the order they opened, and the fields
/// of each event.
#[derive(Debug, Default)]
pub(super) struct Recording {
    /// The spans; a span's ID is its position + 1.
    pub(super) spans: Vec<Recorded>,
    /// The fields of each event.
    pub(super) events: Vec<Vec<(String, String)>>,
}

impl Recording {
    /// Runs `work` with a fresh recorder as the default, and returns what it
    /// saw.
    pub(super) fn of(work: impl FnOnce()) -> Self {
        LazyLock::force(&SECOND_DISPATCHER);
        let recorder = Recorder::default();
        let seen = Arc::clone(&recorder.seen);
        dispatcher::with_default(&Dispatch::new(recorder), work);
        mem::take(&mut *seen.lock().unwrap())
    }

    /// The name of the parent of `span`, if it has one.
    pub(super) fn parent(&self, span: &Recorded) -> Option<&'static str> {
        span.parent.map(|parent| self.spans[parent].name)
    }

    /// The stages of the knowledge pipeline, without the spans of the
    /// libraries beneath it: each as its name, its parent's and its outcome.
    pub(super) fn stages(&self) -> Vec<(&'static str, Option<&'static str>, Option<&str>)> {
        self.spans
            .iter()
            .filter(|span| is_stage(span.name))
            .map(|span| (span.name, self.parent(span), span.field("outcome")))
            .collect()
    }

    /// The spans named `name`.
    pub(super) fn named(&self, name: &str) -> Vec<&Recorded> {
        self.spans.iter().filter(|span| span.name == name).collect()
    }

    /// Every field recorded, on a span or an event, as its name and value.
    pub(super) fn fields(&self) -> impl Iterator<Item = &(String, String)> {
        self.spans
            .iter()
            .flat_map(|span| &span.fields)
            .chain(self.events.iter().flatten())
    }
}

/// Whether `name` is a stage of the knowledge pipeline.
fn is_stage(name: &str) -> bool {
    ["knowledge.", "retrieval.", "gen_ai."]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

thread_local! {
    /// The IDs of the spans this thread is inside, innermost last.
    static ENTERED: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

/// The subscriber: it records everything it sees into `seen`.
#[derive(Debug, Default)]
struct Recorder {
    /// What it saw, shared with the caller.
    seen: Arc<Mutex<Recording>>,
}

/// The fields of one span or event, as text.
#[derive(Default)]
struct Fields(Vec<(String, String)>);

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.push((field.name().to_owned(), format!("{value:?}")));
    }
}

/// The position of the span `id`.
fn position(id: u64) -> usize {
    usize::try_from(id - 1).unwrap()
}

impl Subscriber for Recorder {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let mut fields = Fields::default();
        span.record(&mut fields);
        let parent = if span.is_contextual() {
            ENTERED.with(|entered| entered.borrow().last().copied())
        } else {
            span.parent().map(Id::into_u64)
        };
        let mut seen = self.seen.lock().unwrap();
        seen.spans.push(Recorded {
            name: span.metadata().name(),
            parent: parent.map(position),
            fields: fields.0,
        });
        Id::from_u64(seen.spans.len().try_into().unwrap())
    }

    fn record(&self, span: &Id, values: &Record<'_>) {
        let mut fields = Fields::default();
        values.record(&mut fields);
        let mut seen = self.seen.lock().unwrap();
        seen.spans[position(span.into_u64())]
            .fields
            .extend(fields.0);
    }

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        self.seen.lock().unwrap().events.push(fields.0);
    }

    fn enter(&self, span: &Id) {
        ENTERED.with(|entered| entered.borrow_mut().push(span.into_u64()));
    }

    fn exit(&self, _span: &Id) {
        ENTERED.with(|entered| entered.borrow_mut().pop());
    }
}
