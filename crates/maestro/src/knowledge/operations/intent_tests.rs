//! Production searches take their query expander from the collection's
//! selected answerer, never from a hard-coded choice.

use super::{
    SearchCards,
    ask::tests::{register_reasoning_answerer, select_card},
    local_search_context,
    search::selected_answerer,
    tests::Scratch,
};
use maestro_kernel::gateway::{Role, RouterClient, Url};
use maestro_knowledge::index::Qdrant;

/// An address where nothing listens.
const NOWHERE: &str = "http://127.0.0.1:1";

#[test]
fn the_selected_answerer_expands_queries_unless_it_thinks() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).expect("open test kernel");
    let generation = kernel
        .database
        .published_generation(&kernel.scopes, "collection")
        .expect("read published generation")
        .expect("published generation");
    let selected = || {
        selected_answerer(&kernel.database, &kernel.scopes, "collection")
            .expect("read the selected answerer")
            .map(|card| card.digest().clone())
    };
    assert_eq!(selected(), None);
    let (id, answerer) = register_reasoning_answerer(&kernel, b"answerer", false);
    select_card(&kernel, &generation, Role::Answerer, &id);
    assert_eq!(selected(), Some(answerer.digest().clone()));

    let (_, thinking) = register_reasoning_answerer(&kernel, b"thinking", true);
    let port = RouterClient::new(Url::parse(NOWHERE).expect("router URL")).expect("router");
    let qdrant = Qdrant::new(NOWHERE).expect("Qdrant client");
    let expands = |intent| {
        local_search_context(
            &kernel.database,
            &qdrant,
            &port,
            SearchCards {
                intent,
                ..SearchCards::default()
            },
        )
        .intent_expander
        .is_some()
    };
    assert!(expands(Some(&answerer)));
    assert!(!expands(Some(&thinking)));
    assert!(!expands(None));
}
