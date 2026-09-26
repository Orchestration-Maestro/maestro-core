//! A generation's lifecycle: created building, then verified, published and
//! retired in that order only; every other move is refused, naming both
//! states.

use super::support::{Scratch, execute, generation_in, new_generation, state};
use crate::{
    generation::{
        Error, Generation,
        GenerationState::{self, Building, Published, Retired, Verified},
    },
    store,
};
use rusqlite::ffi;
use std::error;

/// Each state with the name a refusal gives it.
const NAMES: [(GenerationState, &str); 4] = [
    (Building, "building"),
    (Verified, "verified"),
    (Published, "published"),
    (Retired, "retired"),
];

/// Moves the generation `id` to `to` through the call that makes that move.
fn move_to(database: &store::Database, id: i64, to: GenerationState) -> Result<(), Error> {
    match to {
        Verified => database.verify_generation(id, 3),
        Published => database.publish_generation(id).map(drop),
        Retired => database.retire_generation(id),
        Building => panic!("no call moves a generation back to building"),
    }
}

/// The name a refusal gives `state`.
fn name(state: GenerationState) -> &'static str {
    NAMES
        .iter()
        .find(|(named, _)| *named == state)
        .map(|(_, name)| *name)
        .unwrap()
}

#[test]
fn a_new_generation_is_building_without_points_or_publication() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let created = database.create_generation(&new_generation("ctm")).unwrap();
    let expected = Generation {
        id: 1,
        collection_id: "ctm".to_owned(),
        chunk_set_id: "ctm-set".to_owned(),
        embedding_profile: "embed:test".to_owned(),
        sparse_profile: "bm25-en-fr/1".to_owned(),
        state: Building,
        point_count: None,
        published_at: None,
    };
    assert_eq!(created, expected);
    assert_eq!(database.generation(1).unwrap(), Some(expected));
    assert_eq!(database.generation(2).unwrap(), None);
    assert_eq!(database.published_generation("ctm").unwrap(), None);
}

#[test]
fn a_generation_id_is_never_given_again_once_its_row_is_gone() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let first = database.create_generation(&new_generation("ctm")).unwrap();
    let second = database
        .create_generation(&new_generation("synthetic"))
        .unwrap();
    execute(
        &database,
        "DELETE FROM generations WHERE collection_id = ?1",
        "synthetic",
    )
    .unwrap();
    let third = database.create_generation(&new_generation("ctm")).unwrap();
    assert_eq!([first.id, second.id, third.id], [1, 2, 3]);
}

#[test]
fn a_generation_moves_only_from_building_to_verified_published_then_retired() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let legal = [
        (Building, Verified),
        (Verified, Published),
        (Published, Retired),
    ];
    for (from, _) in NAMES {
        for to in [Verified, Published, Retired] {
            let id = generation_in(&database, "ctm", from);
            let moved = move_to(&database, id, to);
            if legal.contains(&(from, to)) {
                assert!(moved.is_ok(), "{from:?} to {to:?}: {moved:?}");
                assert_eq!(state(&database, id), to);
                continue;
            }
            let error = moved.unwrap_err();
            assert!(
                matches!(
                    error,
                    Error::IllegalMove { generation, from: found, to: tried }
                        if generation == id && found == from && tried == to
                ),
                "{error:?}"
            );
            let states = format!("from {} to {}", name(from), name(to));
            assert!(error.to_string().contains(&states), "{states}: {error}");
            assert_eq!(state(&database, id), from, "a refused move changes nothing");
        }
    }
}

#[test]
fn an_illegal_move_names_the_generation_and_both_states() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let id = generation_in(&database, "synthetic", Building);
    let error = database.publish_generation(id).unwrap_err().to_string();
    assert!(
        error.contains("generation 1 cannot move from building to published"),
        "{error}"
    );
}

#[test]
fn verifying_a_generation_records_its_point_count() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let id = generation_in(&database, "ctm", Building);
    database.verify_generation(id, 1_234).unwrap();
    let verified = database.generation(id).unwrap().unwrap();
    assert_eq!(
        (verified.state, verified.point_count),
        (Verified, Some(1_234))
    );
}

#[test]
fn a_point_count_sqlite_cannot_hold_is_refused_unrecorded() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let id = generation_in(&database, "ctm", Building);
    let error = database.verify_generation(id, u64::MAX).unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Store(store::Error::Sqlite(
                rusqlite::Error::ToSqlConversionFailure(_)
            ))
        ),
        "{error:?}"
    );
    let kept = database.generation(id).unwrap().unwrap();
    assert_eq!((kept.state, kept.point_count), (Building, None));
}

#[test]
fn moving_an_unrecorded_generation_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for to in [Verified, Published, Retired] {
        let error = move_to(&database, 7, to).unwrap_err();
        assert!(
            matches!(error, Error::UnknownGeneration(7)),
            "{to:?}: {error:?}"
        );
    }
}

#[test]
fn a_generation_of_an_unrecorded_collection_or_chunk_set_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut unknown_collection = new_generation("ctm");
    unknown_collection.collection_id = "other".to_owned();
    let mut unknown_chunk_set = new_generation("ctm");
    unknown_chunk_set.chunk_set_id = "other-set".to_owned();
    for new in [unknown_collection, unknown_chunk_set] {
        let error = database.create_generation(&new).unwrap_err();
        assert!(
            matches!(
                &error,
                Error::Store(store::Error::Sqlite(rusqlite::Error::SqliteFailure(failure, _)))
                    if failure.extended_code == ffi::SQLITE_CONSTRAINT_FOREIGNKEY
            ),
            "{new:?}: {error:?}"
        );
    }
    assert_eq!(database.generation(1).unwrap(), None);
}

#[test]
fn every_refusal_says_what_went_wrong() {
    let unknown = Error::UnknownGeneration(7).to_string();
    assert!(unknown.contains('7'), "{unknown}");
    let illegal = Error::IllegalMove {
        generation: 7,
        from: Retired,
        to: Verified,
    }
    .to_string();
    assert!(
        illegal.contains("generation 7 cannot move from retired to verified"),
        "{illegal}"
    );
    for error in [
        Error::UnknownGeneration(7),
        Error::IllegalMove {
            generation: 7,
            from: Retired,
            to: Verified,
        },
    ] {
        assert!(error::Error::source(&error).is_none(), "{error}");
    }
    let inner = || store::Error::Sqlite(rusqlite::Error::InvalidQuery);
    let wrapped = Error::from(inner());
    assert_eq!(wrapped.to_string(), inner().to_string());
    let reason = error::Error::source(&wrapped).map(ToString::to_string);
    assert_eq!(reason, Some(rusqlite::Error::InvalidQuery.to_string()));
    assert!(
        matches!(
            Error::from(rusqlite::Error::InvalidQuery),
            Error::Store(store::Error::Sqlite(rusqlite::Error::InvalidQuery))
        ),
        "a SQLite error is the store's"
    );
}
