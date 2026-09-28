//! Interrupted work never reads as complete: a preparation its observer
//! stops, or whose router is unavailable, leaves its chunk set building, and
//! a rerun resumes it to the set an uninterrupted run gives. A counter that
//! changes under its card fails the set, for good.

use super::{
    port::Answer,
    scratch::{COLLECTION, Scratch, chunk_set_of, chunks_of, decide_all, tokenizer, words},
};
use crate::prepare::{
    ChunkProfile, Error, Preparation, TokenizerError, chunk_set_id, prepare, prepare_observed,
};
use maestro_kernel::{
    chunk_set::ChunkSetState, document::Outcome, scope::ScopeSet, store::Database,
};
use std::ops::ControlFlow;

/// A scratch kernel holding seventeen documents, more than one batch,
/// imported and accepted, with the scopes that read it.
fn imported(scratch: &Scratch) -> (Database, ScopeSet) {
    imported_with(scratch, "")
}

/// [`imported`], each heading ending with `label`.
fn imported_with(scratch: &Scratch, label: &str) -> (Database, ScopeSet) {
    let documents: Vec<(String, String)> = (1..=17)
        .map(|number| {
            (
                format!("note-{number}.md"),
                format!(
                    "# Note {number}{label}\n\n{}\n",
                    words(&format!("note{number}x"), 30)
                ),
            )
        })
        .collect();
    let borrowed: Vec<(&str, &str)> = documents
        .iter()
        .map(|(path, markdown)| (path.as_str(), markdown.as_str()))
        .collect();
    scratch.corpus(&borrowed);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    (database, scopes)
}

#[test]
fn a_preparation_stopped_after_its_first_batch_is_building_and_resumes_to_the_same_set() {
    let (stopped, uninterrupted) = (Scratch::new(), Scratch::new());
    let (database, scopes) = imported(&stopped);
    let (_, tokenizer) = tokenizer();
    let mut seen = Vec::new();
    let result = prepare_observed(
        &database,
        &scopes,
        Preparation::of(COLLECTION),
        &tokenizer,
        &mut |report| {
            seen.push(report.clone());
            ControlFlow::Break(())
        },
    );
    assert!(matches!(result, Err(Error::Stopped)), "{result:?}");
    let [first] = seen.as_slice() else {
        panic!("{seen:?}");
    };
    assert_eq!([first.eligible, first.prepared, first.chunks], [17, 16, 16]);
    let set = chunk_set_of(&database, &scopes, first);
    assert_eq!(
        (set.state, set.manifest_digest.clone()),
        (ChunkSetState::Building, None)
    );
    assert_eq!(chunks_of(&database, &scopes, first).len(), 16);
    let resumed = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(
        chunk_set_of(&database, &scopes, &resumed).state,
        ChunkSetState::Complete
    );
    let (other, other_scopes) = imported(&uninterrupted);
    let whole = prepare(&other, &other_scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(resumed, whole);
    assert_eq!(
        chunks_of(&database, &scopes, &resumed),
        chunks_of(&other, &other_scopes, &whole)
    );
    assert_eq!(
        Error::Stopped.to_string(),
        "the preparation was stopped by its caller: its chunk set stays building, and a rerun \
         resumes it"
    );
}

#[test]
fn a_resumed_preparation_counts_the_chrome_of_the_revisions_chunked_before() {
    let (stopped, uninterrupted) = (Scratch::new(), Scratch::new());
    let label = " Link copied to clipboard";
    let (database, scopes) = imported_with(&stopped, label);
    let (_, tokenizer) = tokenizer();
    let ideas = Preparation {
        collection: COLLECTION,
        profile: ChunkProfile::CompleteIdeas,
    };
    let mut stop = |_: &_| ControlFlow::Break(());
    let result = prepare_observed(&database, &scopes, ideas, &tokenizer, &mut stop);
    assert!(matches!(result, Err(Error::Stopped)), "{result:?}");
    let mut unobserved = |_: &_| ControlFlow::Continue(());
    let resumed = prepare_observed(&database, &scopes, ideas, &tokenizer, &mut unobserved).unwrap();
    let (other, other_scopes) = imported_with(&uninterrupted, label);
    let whole =
        prepare_observed(&other, &other_scopes, ideas, &tokenizer, &mut unobserved).unwrap();
    assert_eq!(resumed, whole);
    assert_eq!(
        serde_json::to_value(&resumed.chrome).unwrap(),
        serde_json::json!({"heading_suffix": {"units": 17, "bytes": 17 * label.len()}})
    );
    let error = maestro_canonicalization::Error("no unit".to_owned());
    assert_eq!(
        Error::Chrome(error).to_string(),
        "the chrome of a revision chunked before cannot be counted again: no unit"
    );
}

#[test]
fn a_router_without_free_room_leaves_the_set_building_and_a_rerun_completes_it() {
    let scratch = Scratch::new();
    let (database, scopes) = imported(&scratch);
    let (port, tokenizer) = tokenizer();
    port.answer("Hello world", Answer::Unavailable);
    let Err(Error::Counter(TokenizerError::Unavailable { reason })) =
        prepare(&database, &scopes, COLLECTION, &tokenizer)
    else {
        panic!("the preparation counted without room");
    };
    assert_eq!(reason, "no free room for 1280 MiB");
    let id = chunk_set_id(&database, &scopes, Preparation::of(COLLECTION), &tokenizer).unwrap();
    let set = database.chunk_set(&scopes, &id).unwrap().unwrap();
    assert_eq!(
        (set.state, set.manifest_digest),
        (ChunkSetState::Building, None)
    );
    assert_eq!(database.chunks(&scopes, &id).unwrap(), []);
    port.answer("Hello world", Answer::Ids(vec![0, 35378, 8999, 2]));
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(report.chunk_set, id);
    assert_eq!([report.prepared, report.chunks], [17, 17]);
    assert_eq!(chunks_of(&database, &scopes, &report).len(), 17);
    assert_eq!(
        scratch.rows("chunk_sets"),
        1,
        "the rerun resumed the set it began"
    );
    assert_eq!(
        chunk_set_of(&database, &scopes, &report).state,
        ChunkSetState::Complete
    );
}

#[test]
fn a_counter_that_changed_under_its_card_fails_the_set_for_good() {
    let scratch = Scratch::new();
    let (database, scopes) = imported(&scratch);
    let (port, tokenizer) = tokenizer();
    port.answer("Hello world", Answer::Ids(vec![0, 1, 2]));
    let Err(Error::Counter(TokenizerError::Disagreement { fixture, .. })) =
        prepare(&database, &scopes, COLLECTION, &tokenizer)
    else {
        panic!("the preparation trusted a changed counter");
    };
    assert_eq!(fixture, "ascii");
    port.answer("Hello world", Answer::Ids(vec![0, 35378, 8999, 2]));
    let calls = port.texts().len();
    let Err(Error::Failed(id)) = prepare(&database, &scopes, COLLECTION, &tokenizer) else {
        panic!("a failed set was resumed");
    };
    assert_eq!(port.texts().len(), calls, "nothing was counted");
    let set = database.chunk_set(&scopes, &id).unwrap().unwrap();
    assert_eq!(set.state, ChunkSetState::Failed);
    assert_eq!(database.chunks(&scopes, &id).unwrap(), []);
    assert_eq!(
        Error::Failed(id.clone()).to_string(),
        format!(
            "the chunk set {id} failed, which is final: its counts were not trusted; a new \
             model card makes a new set"
        )
    );
}
