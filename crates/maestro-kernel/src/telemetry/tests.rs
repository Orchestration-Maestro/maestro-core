//! Tests of telemetry: component health and the names of a tool call's span.
//!
//! Every registered component is reported, whatever its check does: answers,
//! panics or keeps silent. Checks run beside each other, and a check still
//! running is never started beside itself.

use super::{Components, DuplicateComponent, Report, Status, span};
use std::{
    fmt, panic,
    sync::{Arc, Barrier, Mutex, mpsc},
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

/// Registers `component` with a check that answers only once the test drops
/// the sender returned; the receiver returned hears of each start of the
/// check, and ends once the check and every thread running it are gone.
fn register_held(
    components: &mut Components,
    component: &str,
) -> (mpsc::Sender<()>, mpsc::Receiver<()>) {
    let (release, held) = mpsc::channel::<()>();
    let (started, starts) = mpsc::channel::<()>();
    let held = Mutex::new(held);
    components
        .register(component, move || {
            let _ = started.send(());
            let _ = held.lock().unwrap().recv();
            Status::Ready
        })
        .unwrap();
    (release, starts)
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
        .register("database", || {
            panic::panic_any("kernel.sqlite3 is locked".to_owned())
        })
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
    let mut components = Components::default();
    let (release, _starts) = register_held(&mut components, "router");

    let reports = components.health(Duration::from_millis(20));
    drop(release);

    assert_eq!(reports, [report("router", down("no answer within 20ms"))]);
}

#[test]
fn a_check_still_running_from_an_earlier_call_is_down_and_not_started_again() {
    let mut components = Components::default();
    let (release, starts) = register_held(&mut components, "router");

    let first = components.health(Duration::from_millis(20));
    let second = components.health(Duration::from_millis(20));
    drop(release);
    drop(components);

    assert_eq!(first, [report("router", down("no answer within 20ms"))]);
    assert_eq!(
        second,
        [report(
            "router",
            down("its check from an earlier call has not answered yet")
        )]
    );
    assert_eq!(starts.iter().count(), 1);
}

#[test]
fn a_check_that_answered_is_asked_again_at_the_next_call() {
    let mut components = Components::default();
    components.register("database", || Status::Ready).unwrap();

    let first = components.health(AMPLE);
    let second = components.health(AMPLE);

    assert_eq!(first, [report("database", Status::Ready)]);
    assert_eq!(second, [report("database", Status::Ready)]);
}

#[test]
fn a_check_that_waits_delays_no_other_check() {
    // Each check answers only once both have started: run one after the
    // other, neither would answer.
    let both_started = Arc::new(Barrier::new(2));
    let mut components = Components::default();
    for name in ["database", "router"] {
        let both_started = Arc::clone(&both_started);
        components
            .register(name, move || {
                both_started.wait();
                Status::Ready
            })
            .unwrap();
    }

    assert_eq!(
        components.health(AMPLE),
        [
            report("database", Status::Ready),
            report("router", Status::Ready)
        ]
    );
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
