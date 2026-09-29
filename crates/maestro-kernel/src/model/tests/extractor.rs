//! The extractor role in the model registry, and the upgrade that adds it.
//!
//! FR-S2-016: the migration keeps every S1 registration, evaluation,
//! selection, pin and guard, and only a real eligible evaluation of an
//! extractor's own card selects it.

use super::support::{Scratch, card_for_role, collection, collection_scopes, generation_in};
use crate::{
    gateway::Role,
    model::{
        CardRecord, Error, EvaluationDisposition, EvaluationMode, EvaluationRecord, NewModelCard,
        NewModelEvaluation, NewModelSelection, SelectedModelCard, SelectionRecord,
    },
    scope::ScopeSet,
    store::{self, Database, pending_migrations},
};
use rusqlite::{Connection, params, types::Value};
use ulid::Ulid;

/// The migration that adds the extractor role.
const EXTRACTOR_ROLE: &str = "0016_extractor_role";
/// The role constraint of each S1 model table.
const S1_ROLES: &str = "role IN ('embedder', 'reranker', 'answerer')";
/// The same constraint once the extractor role exists.
const ROLES: &str = "role IN ('embedder', 'reranker', 'answerer', 'extractor')";
/// The outcome of an evaluation that qualifies its card for selection.
const QUALIFIED: (EvaluationMode, EvaluationDisposition) =
    (EvaluationMode::Real, EvaluationDisposition::Eligible);
/// What the selection trigger says when it refuses.
const SELECTION_GUARD: &str =
    "selection requires an eligible real evaluation of the same v2 card and role";

/// Registers the card of `role` in collection `one`.
fn register(database: &Database, scratch: &Scratch, scopes: &ScopeSet, role: Role) -> CardRecord {
    let card = card_for_role(database, scratch, role);
    database
        .record_model_card(
            scopes,
            &NewModelCard {
                collection_id: "one",
                card: &card,
            },
        )
        .unwrap()
}

/// Records an evaluation of `card` as `role`, which may differ from the
/// card's, with the mode and disposition of the outcome.
fn evaluate(
    database: &Database,
    scopes: &ScopeSet,
    card: &CardRecord,
    role: Role,
    outcome: (EvaluationMode, EvaluationDisposition),
) -> Result<EvaluationRecord, Error> {
    evaluate_on(database, scopes, (card, role), outcome, None)
}

/// Records an evaluation as [`evaluate`] does, against `generation_id`.
fn evaluate_on(
    database: &Database,
    scopes: &ScopeSet,
    (card, role): (&CardRecord, Role),
    (mode, disposition): (EvaluationMode, EvaluationDisposition),
    generation_id: Option<i64>,
) -> Result<EvaluationRecord, Error> {
    database.record_model_evaluation(
        scopes,
        &NewModelEvaluation {
            run_id: "run",
            collection_id: "one",
            card_id: card.id,
            role,
            mode,
            generation_id,
            disposition,
            manifest: b"manifest",
            report: b"report",
        },
    )
}

/// Selects `card` for `role` on the strength of `evaluation`.
fn select(
    database: &Database,
    scopes: &ScopeSet,
    role: Role,
    card: &CardRecord,
    evaluation: &EvaluationRecord,
) -> Result<SelectionRecord, Error> {
    database.record_model_selection(
        scopes,
        &NewModelSelection {
            collection_id: "one",
            role,
            card_id: card.id,
            evaluation_id: evaluation.id,
            selected_by: "owner",
            reason: "qualified",
        },
    )
}

/// A registry an S1 binary left: a selected card of each S1 role, each
/// with a real evaluation against a generation, then a failed synthetic
/// evaluation against none.
fn s1_registry(scratch: &Scratch) -> Vec<(Role, SelectedModelCard)> {
    let database = Database::open_before(&scratch.0, EXTRACTOR_ROLE).unwrap();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let generation = Some(generation_in(&database, "one"));
    let failed = (EvaluationMode::Synthetic, EvaluationDisposition::Failed);
    let mut selected = Vec::new();
    for role in [Role::Embedder, Role::Reranker, Role::Answerer] {
        let card = register(&database, scratch, &scopes, role);
        let evaluation =
            evaluate_on(&database, &scopes, (&card, role), QUALIFIED, generation).unwrap();
        assert_eq!(evaluation.generation_id, generation);
        select(&database, &scopes, role, &card, &evaluation).unwrap();
        evaluate(&database, &scopes, &card, role, failed).unwrap();
        let record = database.selected_model_card(&scopes, "one", role).unwrap();
        selected.push((role, record.unwrap()));
    }
    selected
}

/// The stored values of the registry, its pins, its journal and the earlier
/// migration records, rows in stored order.
fn rows(connection: &Connection) -> Vec<Vec<Vec<Value>>> {
    [
        "SELECT rowid, * FROM model_cards ORDER BY rowid",
        "SELECT rowid, * FROM model_evaluations ORDER BY rowid",
        "SELECT rowid, * FROM model_selections ORDER BY rowid",
        "SELECT digest, pins FROM artifacts ORDER BY digest",
        "SELECT rowid, * FROM events ORDER BY rowid",
        "SELECT * FROM migrations WHERE name < '0016' ORDER BY name",
    ]
    .map(|sql| {
        let mut statement = connection.prepare(sql).unwrap();
        let columns = statement.column_count();
        statement
            .query_map([], |row| (0..columns).map(|index| row.get(index)).collect())
            .unwrap()
            .map(Result::unwrap)
            .collect()
    })
    .to_vec()
}

/// The tables, indexes and triggers of the registry, with their SQL.
fn schema(connection: &Connection) -> Vec<(String, String, Option<String>)> {
    let mut statement = connection
        .prepare(
            "SELECT type, name, sql FROM sqlite_schema
             WHERE tbl_name IN ('model_cards', 'model_evaluations', 'model_selections')
             ORDER BY type, name",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn the_extractor_upgrade_keeps_every_s1_record_digest_pin_and_selection() {
    let scratch = Scratch::new();
    let selected = s1_registry(&scratch);
    assert_eq!(pending_migrations(&scratch.0).unwrap(), [EXTRACTOR_ROLE]);
    let before = rows(&scratch.outside());
    for reopening in 0..2 {
        let database = scratch.open();
        assert!(pending_migrations(&scratch.0).unwrap().is_empty());
        assert_eq!(rows(&scratch.outside()), before, "reopening {reopening}");
        let scopes = collection_scopes(&database, "one");
        for (role, earlier) in &selected {
            let now = database.selected_model_card(&scopes, "one", *role).unwrap();
            assert_eq!(now.as_ref(), Some(earlier));
            let card = database
                .model_card(&scopes, "one", earlier.card.digest())
                .unwrap();
            assert_eq!(card.as_ref(), Some(&earlier.card));
        }
    }
    let violations: i64 = scratch
        .outside()
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
}

#[test]
fn the_extractor_upgrade_changes_only_the_three_role_constraints() {
    let scratch = Scratch::new();
    s1_registry(&scratch);
    let before = schema(&scratch.outside());
    let tables: Vec<_> = before.iter().filter(|(kind, ..)| kind == "table").collect();
    assert_eq!(tables.len(), 3, "{before:?}");
    for (_, name, sql) in tables {
        let sql = sql.as_deref().unwrap_or_default();
        assert_eq!(sql.matches(S1_ROLES).count(), 1, "{name}: {sql}");
    }
    let expected: Vec<_> = before
        .into_iter()
        .map(|(kind, name, sql)| {
            let sql = if kind == "table" {
                sql.map(|sql| sql.replace(S1_ROLES, ROLES))
            } else {
                sql
            };
            (kind, name, sql)
        })
        .collect();
    drop(scratch.open());
    assert_eq!(schema(&scratch.outside()), expected);
}

#[test]
fn a_failing_extractor_upgrade_changes_nothing() {
    let scratch = Scratch::new();
    s1_registry(&scratch);
    // A copy damaged behind the kernel's back: the answerer's evaluations
    // and selection name a card it no longer holds.
    scratch
        .outside()
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TRIGGER model_cards_are_never_deleted;
             DELETE FROM model_cards WHERE role = 'answerer';",
        )
        .unwrap();
    let before = (rows(&scratch.outside()), schema(&scratch.outside()));
    let error = Database::open_in(&scratch.0).unwrap_err();
    assert!(matches!(error, store::Error::Sqlite(_)), "{error}");
    assert_eq!(
        (rows(&scratch.outside()), schema(&scratch.outside())),
        before
    );
    assert_eq!(pending_migrations(&scratch.0).unwrap(), [EXTRACTOR_ROLE]);
}

#[test]
fn every_role_including_the_extractor_registers_evaluates_and_is_selected() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    for role in Role::ALL {
        let card = register(&database, &scratch, &scopes, role);
        assert_eq!(card.role, role);
        let evaluation = evaluate(&database, &scopes, &card, role, QUALIFIED).unwrap();
        let selection = select(&database, &scopes, role, &card, &evaluation).unwrap();
        assert_eq!(selection.role, role);
        let selected = database.selected_model_card(&scopes, "one", role).unwrap();
        let selected = selected.unwrap();
        assert_eq!(selected.selection, selection);
        assert_eq!(selected.evaluation, evaluation);
        assert_eq!(selected.card.fields().role, role);
        let listed = database.model_cards(&scopes, "one", role).unwrap();
        assert_eq!(listed, [card]);
    }
    let evaluations = database.model_evaluations(&scopes, "one").unwrap();
    let roles: Vec<Role> = evaluations.iter().map(|record| record.role).collect();
    assert_eq!(roles, Role::ALL);
}

#[test]
fn an_answerer_card_or_evaluation_never_qualifies_or_selects_the_extractor() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let answerer = register(&database, &scratch, &scopes, Role::Answerer);
    let answered = evaluate(&database, &scopes, &answerer, Role::Answerer, QUALIFIED).unwrap();
    let extractor = register(&database, &scratch, &scopes, Role::Extractor);
    let refusals = [
        evaluate(&database, &scopes, &answerer, Role::Extractor, QUALIFIED).unwrap_err(),
        select(&database, &scopes, Role::Extractor, &answerer, &answered).unwrap_err(),
        select(&database, &scopes, Role::Extractor, &extractor, &answered).unwrap_err(),
        select(&database, &scopes, Role::Answerer, &extractor, &answered).unwrap_err(),
    ];
    for error in refusals {
        assert!(matches!(error, Error::Invalid(_)), "{error}");
    }
    let outside = scratch.outside();
    let refused = insert_selection(&outside, "extractor", &answerer, &answered).unwrap_err();
    assert!(refused.to_string().contains(SELECTION_GUARD), "{refused}");
    let refused = insert_selection(&outside, "extractor", &extractor, &answered).unwrap_err();
    assert!(refused.to_string().contains(SELECTION_GUARD), "{refused}");
    for role in [Role::Extractor, Role::Answerer] {
        let selected = database.selected_model_card(&scopes, "one", role).unwrap();
        assert_eq!(selected, None, "{role}");
    }
}

#[test]
fn only_a_real_eligible_extractor_evaluation_selects_the_extractor() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let extractor = register(&database, &scratch, &scopes, Role::Extractor);
    let outside = scratch.outside();
    for outcome in [
        (EvaluationMode::Synthetic, EvaluationDisposition::Eligible),
        (EvaluationMode::Real, EvaluationDisposition::Ineligible),
        (EvaluationMode::Real, EvaluationDisposition::Blocked),
    ] {
        let evaluation = evaluate(&database, &scopes, &extractor, Role::Extractor, outcome);
        let evaluation = evaluation.unwrap();
        let error = select(&database, &scopes, Role::Extractor, &extractor, &evaluation);
        assert!(matches!(error, Err(Error::Invalid(_))), "{error:?}");
        let refused = insert_selection(&outside, "extractor", &extractor, &evaluation);
        let refused = refused.unwrap_err().to_string();
        assert!(refused.contains(SELECTION_GUARD), "{refused}");
    }
    assert_eq!(
        database
            .selected_model_card(&scopes, "one", Role::Extractor)
            .unwrap(),
        None
    );
    let qualified = evaluate(&database, &scopes, &extractor, Role::Extractor, QUALIFIED);
    let qualified = qualified.unwrap();
    let selection = select(&database, &scopes, Role::Extractor, &extractor, &qualified).unwrap();
    let selected = database
        .selected_model_card(&scopes, "one", Role::Extractor)
        .unwrap()
        .unwrap();
    assert_eq!(selected.selection, selection);
    assert_eq!(selected.evaluation, qualified);
}

/// Inserts a selection of `card` for `role` from outside the kernel, as a
/// writer that skipped its checks would.
fn insert_selection(
    connection: &Connection,
    role: &str,
    card: &CardRecord,
    evaluation: &EvaluationRecord,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO model_selections
         (id, collection_id, role, card_id, evaluation_id, selected_by, reason)
         VALUES (?1, 'one', ?2, ?3, ?4, 'owner', 'unchecked')",
        params![
            Ulid::generate().to_string(),
            role,
            card.id.to_string(),
            evaluation.id.to_string()
        ],
    )
}
