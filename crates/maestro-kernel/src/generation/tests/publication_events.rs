//! Publication events are committed with each generation lifecycle change.

use super::support::{Scratch, generation_in};
use crate::{
    generation::GenerationState::Verified,
    journal::{Event, Filter, GenerationPublished, GenerationRetired},
    scope::ScopeSet,
    store::Database,
};
use serde_json::json;

/// The knowledge events on `ctm`'s stream.
fn events(database: &Database) -> Vec<Event> {
    database
        .events(
            &ScopeSet::default_workspace(),
            &Filter {
                stream: "collection/ctm",
                after: 0,
                r#type: None,
            },
        )
        .unwrap()
}

#[test]
fn publishing_journals_each_change_once_and_failed_publications_journal_nothing() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let first = generation_in(&database, "ctm", Verified);
    assert_eq!(database.publish_generation(first).unwrap(), None);
    let first_events = events(&database);
    assert_eq!(first_events.len(), 1, "one change, one event");
    assert_eq!(first_events[0].r#type, GenerationPublished::TYPE);
    assert_eq!(
        first_events[0].data,
        json!({"collection": "ctm", "generation": first, "point_count": 3})
    );

    let second = generation_in(&database, "ctm", Verified);
    assert_eq!(database.publish_generation(second).unwrap(), Some(first));
    let published = events(&database);
    assert_eq!(
        published
            .iter()
            .filter(|event| event.r#type == GenerationPublished::TYPE)
            .count(),
        2
    );
    let retired: Vec<_> = published
        .iter()
        .filter(|event| event.r#type == GenerationRetired::TYPE)
        .collect();
    assert_eq!(retired.len(), 1);
    assert_eq!(
        retired[0].data,
        json!({"collection": "ctm", "generation": first})
    );

    let failed = generation_in(&database, "ctm", Verified);
    database.fail_generation(failed).unwrap();
    let before = events(&database);
    assert!(database.publish_generation(failed).is_err());
    assert_eq!(
        events(&database),
        before,
        "a refused publication journals none"
    );
}
