//! The envelope: an event read back from the journal as the `CloudEvents` 1.0
//! envelope it leaves the kernel in, and the machine its source names.

use super::support::{SCOPE, Scratch, imported, whole};
use crate::journal::{Envelope, Event, GenerationPublished, InvalidMachine, Machine, NewEvent};
use serde_json::{Value, json};

/// The stream the tests record their events on.
const STREAM: &str = "collection/demo";

/// The machine the tests' events come from.
fn machine() -> Machine {
    Machine::parse("test-machine").unwrap()
}

#[test]
fn an_event_reads_back_as_a_cloudevents_envelope_with_its_sequence_and_scope() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let published = GenerationPublished {
        collection: "demo".to_owned(),
        generation: 8,
        point_count: 81_234,
    };
    let data = serde_json::to_value(&published).unwrap();
    database
        .record(&imported(STREAM, STREAM, &Value::Null))
        .unwrap();
    database
        .record(&NewEvent {
            stream: STREAM,
            r#type: GenerationPublished::TYPE,
            subject: STREAM,
            scope: SCOPE,
            data: &data,
        })
        .unwrap();
    let [_, event] = <[Event; 2]>::try_from(whole(&database, STREAM)).unwrap();
    let (id, time) = (event.id.to_string(), event.time.clone());
    let envelope = Envelope::new(event, &machine());
    assert_eq!(
        serde_json::to_value(&envelope).unwrap(),
        json!({
            "specversion": "1.0",
            "id": id,
            "source": "maestro://test-machine/kernel",
            "type": "maestro.knowledge.generation.published.v1",
            "subject": "collection/demo",
            "time": time,
            "datacontenttype": "application/json",
            "dataschema": "maestro://schemas/events/knowledge.generation.published/1",
            "maestroscope": "workspace/default/collection/demo",
            "maestrosequence": 2,
            "data": {"collection": "demo", "generation": 8, "point_count": 81_234}
        })
    );
    assert_eq!(envelope.traceparent, None);
    let read: GenerationPublished = serde_json::from_value(envelope.data).unwrap();
    assert_eq!(read, published);
}

#[test]
fn an_event_outside_the_public_catalogue_carries_no_dataschema() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let data = json!({"done": 2});
    for kind in [
        "maestro.test.internal.v1",
        "maestro.knowledge.generation.published.v2",
    ] {
        database
            .record(&NewEvent {
                stream: STREAM,
                r#type: kind,
                subject: "job/7",
                scope: SCOPE,
                data: &data,
            })
            .unwrap();
    }
    let events = whole(&database, STREAM);
    assert_eq!(events.len(), 2);
    for event in events {
        let (id, time) = (event.id.to_string(), event.time.clone());
        let (kind, sequence) = (event.r#type.clone(), event.sequence);
        let envelope = serde_json::to_value(Envelope::new(event, &machine())).unwrap();
        assert_eq!(
            envelope,
            json!({
                "specversion": "1.0",
                "id": id,
                "source": "maestro://test-machine/kernel",
                "type": kind,
                "subject": "job/7",
                "time": time,
                "datacontenttype": "application/json",
                "maestroscope": "workspace/default/collection/demo",
                "maestrosequence": sequence,
                "data": {"done": 2}
            })
        );
    }
}

#[test]
fn a_machine_is_named_by_the_unreserved_characters_of_a_uri_host() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let event = database
        .record(&imported(STREAM, STREAM, &Value::Null))
        .unwrap();
    for name in [
        "test-machine",
        "WS7",
        "build.example.org",
        "build_2",
        "a~b",
        "0",
    ] {
        let machine = Machine::parse(name).unwrap();
        let envelope = Envelope::new(event.clone(), &machine);
        assert_eq!(envelope.source, format!("maestro://{name}/kernel"));
    }
    for name in [
        "",
        "a/b",
        "a b",
        "host:8080",
        "user@host",
        "machine-\u{3c0}",
        "a%20b",
        "a?b",
        "a#b",
        "[::1]",
    ] {
        let refused = Machine::parse(name);
        assert_eq!(refused, Err(InvalidMachine(name.to_owned())), "{name:?}");
    }
    assert_eq!(
        InvalidMachine("a/b".to_owned()).to_string(),
        "\"a/b\" cannot name the machine of an event's source: use letters, digits, '-', '.', \
         '_' and '~' only"
    );
}
