//! Tests of telemetry: component health and the names of a tool call's span.
//!
//! Every registered component is reported, whatever its check does: answers,
//! panics or keeps silent.

use super::{Components, DuplicateComponent, Report, Status, span};
use std::{
    fmt, panic,
    sync::{Mutex, mpsc},
    time::Duration,
};
use tracing::{
    Dispatch, Event, Metadata, Subscriber, dispatcher,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

/// Long enough for any check that answers at once, however loaded the
/// machine; the report comes as soon as every check has answered.
const AMPLE: Duration = Duration::from_secs(60);

/// The report of `component` with `status`.
fn report(component: &str, status: Status) -> Report {
    Report {
        component: component.to_owned(),
        status,
    }
}

/// A down status with `reason`.
fn down(reason: &str) -> Status {
    Status::Down {
        reason: reason.to_owned(),
    }
}

#[test]
fn health_reports_each_component_ready_degraded_or_down_with_its_reason() {
    let mut components = Components::default();
    components
        .register("search", || down("connection refused"))
        .unwrap();
    components.register("database", || Status::Ready).unwrap();
    components
        .register("router", || Status::Degraded {
            reason: "the reranker is not loaded".to_owned(),
        })
        .unwrap();

    assert_eq!(
        components.health(AMPLE),
        [
            report("database", Status::Ready),
            report(
                "router",
                Status::Degraded {
                    reason: "the reranker is not loaded".to_owned()
                }
            ),
            report("search", down("connection refused")),
        ]
    );
}

#[test]
fn a_check_that_panics_leaves_its_component_down_with_the_panic_message() {
    let mut components = Components::default();
    components
        .register("artifacts", || panic!("the directory is gone"))
        .unwrap();
    components
        .register("database", || panic!("{} is locked", "kernel.sqlite3"))
        .unwrap();
    components
        .register("router", || panic::panic_any(503_u16))
        .unwrap();

    assert_eq!(
        components.health(AMPLE),
        [
            report(
                "artifacts",
                down("its check panicked: the directory is gone")
            ),
            report(
                "database",
                down("its check panicked: kernel.sqlite3 is locked")
            ),
            report("router", down("its check panicked")),
        ]
    );
}

#[test]
fn a_check_that_gives_no_answer_in_time_leaves_its_component_down() {
    let (release, held) = mpsc::channel::<()>();
    let held = Mutex::new(held);
    let mut components = Components::default();
    components
        .register("router", move || {
            // Answers only once the test drops the sender, after the report.
            let _ = held.lock().unwrap().recv();
            Status::Ready
        })
        .unwrap();

    let reports = components.health(Duration::from_millis(20));
    drop(release);

    assert_eq!(reports, [report("router", down("no answer within 20ms"))]);
}

#[test]
fn a_name_registered_twice_is_refused_naming_the_component() {
    let mut components = Components::default();
    components.register("router", || Status::Ready).unwrap();

    let refused: DuplicateComponent = components
        .register("router", || down("unreachable"))
        .unwrap_err();

    assert_eq!(refused.name(), "router");
    assert_eq!(
        refused.to_string(),
        "a component named \"router\" is already registered"
    );
    assert_eq!(components.health(AMPLE), [report("router", Status::Ready)]);
}

#[test]
fn the_debug_form_of_the_components_names_them() {
    let mut components = Components::default();
    components.register("router", || Status::Ready).unwrap();
    components.register("database", || Status::Ready).unwrap();

    assert_eq!(
        format!("{components:?}"),
        "Components { names: [\"database\", \"router\"] }"
    );
}

/// A span as the recording subscriber saw it open.
#[derive(Debug, PartialEq, Eq)]
struct Opened {
    /// The span's name.
    name: &'static str,
    /// Its fields, in order, each with its value as text.
    fields: Vec<(String, String)>,
}

/// A subscriber that records every span opened while it is the default.
#[derive(Debug, Default)]
struct Recorder {
    /// The spans opened, in order.
    opened: Mutex<Vec<Opened>>,
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
        let mut opened = self.opened.lock().unwrap();
        opened.push(Opened {
            name: span.metadata().name(),
            fields: fields.0,
        });
        Id::from_u64(opened.len().try_into().unwrap())
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {}

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[test]
fn a_tool_call_opens_a_span_carrying_the_pinned_names() {
    let dispatch = Dispatch::new(Recorder::default());

    dispatcher::with_default(&dispatch, || {
        let _call = span::tool_call("knowledge_search");
    });

    let recorder = dispatch.downcast_ref::<Recorder>().unwrap();
    assert_eq!(
        *recorder.opened.lock().unwrap(),
        [Opened {
            name: "gen_ai.execute_tool",
            fields: vec![
                (
                    "gen_ai.operation.name".to_owned(),
                    "execute_tool".to_owned()
                ),
                ("gen_ai.tool.name".to_owned(), "knowledge_search".to_owned()),
            ],
        }]
    );
}
