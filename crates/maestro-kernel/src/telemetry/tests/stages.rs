//! Tests of the stages: the spans they open under their pinned names, the
//! outcome, duration and counts they record, and the parent they keep on
//! another thread.

use super::super::{
    span,
    stage::{Count, Outcome, Stage},
};
use std::{
    cell::RefCell,
    fmt,
    pin::pin,
    sync::Mutex,
    task::{Context, Poll, Waker},
    thread,
};
use tracing::{
    Dispatch, Event, Metadata, Subscriber, dispatcher,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

/// A span as the recording subscriber saw it.
#[derive(Debug, PartialEq, Eq)]
struct Recorded {
    /// The span's name.
    name: &'static str,
    /// The name of the span it was opened in, if any.
    parent: Option<&'static str>,
    /// Its fields, in the order they were given a value, each as text.
    fields: Vec<(String, String)>,
}

impl Recorded {
    /// The value of `field`, as text.
    fn field(&self, field: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, value)| value.as_str())
    }
}

/// A function that opens a stage.
type Opener = fn() -> Stage;

thread_local! {
    /// The spans this thread is inside, innermost last.
    static ENTERED: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

/// A subscriber that records every span opened while it is the default, and
/// every value later recorded on it.
#[derive(Debug, Default)]
struct Recorder {
    /// The spans, in the order they opened; a span's ID is its position + 1.
    spans: Mutex<Vec<Recorded>>,
}

/// The fields of one span, as text.
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

impl Subscriber for Recorder {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let mut fields = Fields::default();
        span.record(&mut fields);
        let mut spans = self.spans.lock().unwrap();
        let parent = if span.is_contextual() {
            ENTERED.with(|entered| entered.borrow().last().copied())
        } else {
            span.parent().map(Id::into_u64)
        };
        let parent = parent.map(|id| spans[usize::try_from(id - 1).unwrap()].name);
        spans.push(Recorded {
            name: span.metadata().name(),
            parent,
            fields: fields.0,
        });
        Id::from_u64(spans.len().try_into().unwrap())
    }

    fn record(&self, span: &Id, values: &Record<'_>) {
        let mut fields = Fields::default();
        values.record(&mut fields);
        let mut spans = self.spans.lock().unwrap();
        let index = usize::try_from(span.into_u64() - 1).unwrap();
        spans[index].fields.extend(fields.0);
    }

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {}

    fn enter(&self, span: &Id) {
        ENTERED.with(|entered| entered.borrow_mut().push(span.into_u64()));
    }

    fn exit(&self, _span: &Id) {
        ENTERED.with(|entered| entered.borrow_mut().pop());
    }
}

/// Runs `work` with a fresh recorder as the default, and returns what it
/// recorded.
fn recorded(work: impl FnOnce()) -> Vec<Recorded> {
    let dispatch = Dispatch::new(Recorder::default());
    dispatcher::with_default(&dispatch, work);
    let recorder = dispatch.downcast_ref::<Recorder>().unwrap();
    recorder.spans.lock().unwrap().drain(..).collect()
}

#[test]
fn a_tool_call_opens_a_span_carrying_the_pinned_names() {
    let spans = recorded(|| span::tool_call("knowledge_search").finish(Outcome::Refused));

    let [call] = spans.as_slice() else {
        panic!("one span, not {spans:?}");
    };
    assert_eq!(call.name, "gen_ai.execute_tool");
    assert_eq!(call.parent, None);
    assert_eq!(
        call.fields[..3],
        [
            (
                "gen_ai.operation.name".to_owned(),
                "execute_tool".to_owned()
            ),
            ("gen_ai.tool.name".to_owned(), "knowledge_search".to_owned()),
            ("outcome".to_owned(), "refused".to_owned()),
        ]
    );
    assert_eq!(call.fields[3].0, "duration_us");
    assert_eq!(call.fields.len(), 4);
}

#[test]
fn each_stage_opens_its_span_under_its_pinned_name() {
    let stages: [(Opener, &str); 21] = [
        (span::publish, "knowledge.publish"),
        (span::publish_load_inputs, "knowledge.publish.load_inputs"),
        (span::publish_project, "knowledge.publish.project"),
        (span::publish_embed, "knowledge.publish.embed"),
        (span::publish_verify, "knowledge.publish.verify"),
        (span::publish_switch_alias, "knowledge.publish.switch_alias"),
        (span::publish_kernel, "knowledge.publish.kernel"),
        (span::search, "retrieval.search"),
        (span::route_dense, "retrieval.route.dense"),
        (span::route_lexical, "retrieval.route.lexical"),
        (span::route_identifier, "retrieval.route.identifier"),
        (span::route_structured, "retrieval.route.structured"),
        (span::fuse, "retrieval.fuse"),
        (span::rerank, "retrieval.rerank"),
        (span::assemble, "retrieval.assemble"),
        (span::assemble_ledger, "retrieval.assemble.ledger"),
        (span::assemble_candidates, "retrieval.assemble.candidates"),
        (span::assemble_sources, "retrieval.assemble.sources"),
        (span::assemble_sections, "retrieval.assemble.sections"),
        (span::assemble_selection, "retrieval.assemble.selection"),
        (span::assemble_output, "retrieval.assemble.output"),
    ];
    let spans = recorded(|| {
        for (open, _) in stages {
            open().finish(Outcome::Ok);
        }
    });

    let names: Vec<_> = spans.iter().map(|span| span.name).collect();
    let expected: Vec<_> = stages.iter().map(|(_, name)| *name).collect();
    assert_eq!(names, expected);
}

#[test]
fn a_finished_stage_records_its_identities_counts_outcome_and_duration() {
    let spans = recorded(|| {
        let stage = span::publish();
        stage.collection("synthetic");
        stage.generation(7);
        stage.count(Count::Candidates, 1);
        stage.count(Count::Chunks, 2);
        stage.count(Count::Passages, 3);
        stage.count(Count::Points, 4);
        stage.count(Count::Sources, 5);
        stage.finish(Outcome::Ok);
    });

    let [publish] = spans.as_slice() else {
        panic!("one span, not {spans:?}");
    };
    assert_eq!(publish.name, "knowledge.publish");
    let fields: Vec<_> = publish.fields.iter().map(|(name, _)| name).collect();
    assert_eq!(
        fields,
        [
            "collection_id",
            "generation",
            "candidates",
            "chunks",
            "passages",
            "points",
            "sources",
            "outcome",
            "duration_us",
        ]
    );
    assert_eq!(publish.field("collection_id"), Some("synthetic"));
    assert_eq!(publish.field("generation"), Some("7"));
    assert_eq!(publish.field("candidates"), Some("1"));
    assert_eq!(publish.field("chunks"), Some("2"));
    assert_eq!(publish.field("passages"), Some("3"));
    assert_eq!(publish.field("points"), Some("4"));
    assert_eq!(publish.field("sources"), Some("5"));
    assert_eq!(publish.field("outcome"), Some("ok"));
    assert!(publish.field("duration_us").unwrap().parse::<u64>().is_ok());
}

#[test]
fn each_outcome_is_recorded_by_its_name() {
    let outcomes = [
        (Outcome::Ok, "ok"),
        (Outcome::Refused, "refused"),
        (Outcome::Unavailable, "unavailable"),
        (Outcome::Timeout, "timeout"),
        (Outcome::Error, "error"),
    ];
    let spans = recorded(|| {
        for (outcome, _) in outcomes {
            span::search().finish(outcome);
        }
    });

    let recorded: Vec<_> = spans.iter().map(|span| span.field("outcome")).collect();
    let expected: Vec<_> = outcomes.iter().map(|(_, name)| Some(*name)).collect();
    assert_eq!(recorded, expected);
}

#[test]
fn a_stage_dropped_before_it_finishes_ends_as_an_error_once() {
    let spans = recorded(|| drop(span::rerank()));

    let [rerank] = spans.as_slice() else {
        panic!("one span, not {spans:?}");
    };
    assert_eq!(rerank.field("outcome"), Some("error"));
    let outcomes = rerank.fields.iter().filter(|(name, _)| name == "outcome");
    assert_eq!(outcomes.count(), 1);
}

#[test]
fn stages_opened_inside_a_stage_are_its_children() {
    let spans = recorded(|| {
        let search = span::search();
        search.in_scope(|| span::route_dense().finish(Outcome::Unavailable));
        let fused = futures_free_poll(search.instrument(async { span::fuse() }));
        fused.finish(Outcome::Ok);
        search.finish(Outcome::Ok);
    });

    let tree: Vec<_> = spans.iter().map(|span| (span.name, span.parent)).collect();
    assert_eq!(
        tree,
        [
            ("retrieval.search", None),
            ("retrieval.route.dense", Some("retrieval.search")),
            ("retrieval.fuse", Some("retrieval.search")),
        ]
    );
}

#[test]
fn a_carried_context_opens_stages_under_its_span_on_another_thread() {
    let spans = recorded(|| {
        let assemble = span::assemble();
        let carried = assemble.carry();
        thread::scope(|scope| {
            scope.spawn(|| {
                carried.in_scope(|| span::assemble_ledger().finish(Outcome::Ok));
                span::assemble_output().finish(Outcome::Ok);
            });
        });
        assemble.finish(Outcome::Ok);
    });

    let tree: Vec<_> = spans.iter().map(|span| (span.name, span.parent)).collect();
    assert_eq!(
        tree,
        [
            ("retrieval.assemble", None),
            ("retrieval.assemble.ledger", Some("retrieval.assemble")),
        ]
    );
}

/// Polls a future that is ready at once to its output, with no runtime.
fn futures_free_poll<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the future is not ready at once"),
    }
}

#[test]
fn rerank_header_missing_is_stage_telemetry() {
    let spans = recorded(|| {
        let stage = span::rerank();
        stage.count(Count::HeaderMissing, 3);
        stage.finish(Outcome::Ok);
    });
    assert_eq!(spans[0].field("header_missing"), Some("3"));
}
