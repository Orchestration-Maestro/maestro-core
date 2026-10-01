//! Direct tests for immutable model-registry row guards.

use super::support::{
    Scratch, card, collection, collection_scopes, identity, new_eval, populated_model_records,
};
use crate::{
    artifact::Digest,
    gateway::ModelCard,
    generation::NewGeneration,
    model::{CardRecord, EvaluationDisposition, EvaluationMode, NewModelCard},
    store::Database,
};
use rusqlite::{Connection, params};

fn assert_error_message(error: &rusqlite::Error, expected: &str) {
    let message = error.to_string();
    assert!(
        message.contains(expected),
        "expected SQLite error containing {expected:?}, got {message:?}"
    );
}

fn assert_insert_refusal<P: rusqlite::Params>(
    connection: &Connection,
    statement: &str,
    parameters: P,
    expected: &str,
) {
    let error = connection.execute(statement, parameters).unwrap_err();
    assert_error_message(&error, expected);
}

struct ImmutableRow<'a> {
    table: &'a str,
    id: &'a str,
    update: &'a str,
    replacements: &'a [&'a str],
    immutable_message: &'a str,
    replacement_message: &'a str,
}

fn assert_immutable_row(connection: &Connection, row: &ImmutableRow<'_>) {
    let error = connection.execute(row.update, [row.id]).unwrap_err();
    assert_error_message(&error, row.immutable_message);
    let error = connection
        .execute(&format!("DELETE FROM {} WHERE id=?1", row.table), [row.id])
        .unwrap_err();
    assert_error_message(&error, row.immutable_message);
    for replacement in row.replacements {
        let error = connection.execute(replacement, [row.id]).unwrap_err();
        assert_error_message(&error, row.replacement_message);
    }
}

#[test]
fn each_model_registry_table_rejects_update_delete_and_each_replace_key() {
    let scratch = Scratch::new();
    let (card, evaluation, selection) = populated_model_records(&scratch);
    let connection = scratch.outside();
    let card_id = card.id.to_string();
    let evaluation_id = evaluation.id.to_string();
    let selection_id = selection.id.to_string();

    let card_replacements = [
        concat!(
            "INSERT OR REPLACE INTO model_cards (id,collection_id,role,digest,card_json) ",
            "SELECT id,collection_id,role,lower(hex(randomblob(32))),card_json ",
            "FROM model_cards WHERE id=?1"
        ),
        concat!(
            "INSERT OR REPLACE INTO model_cards ",
            "(rowid,id,collection_id,role,digest,card_json) ",
            "SELECT rowid,'new-rowid',collection_id,role,lower(hex(randomblob(32))),card_json ",
            "FROM model_cards WHERE id=?1"
        ),
        concat!(
            "INSERT OR REPLACE INTO model_cards (id,collection_id,role,digest,card_json) ",
            "SELECT 'new-digest',collection_id,role,digest,card_json ",
            "FROM model_cards WHERE id=?1"
        ),
    ];
    let evaluation_replacements = [
        concat!(
            "INSERT OR REPLACE INTO model_evaluations ",
            "(id,run_id,collection_id,card_id,role,mode,generation_id,disposition, ",
            "manifest_digest,report_digest) ",
            "SELECT id,run_id,collection_id,card_id,role,mode,generation_id,disposition, ",
            "manifest_digest,report_digest FROM model_evaluations WHERE id=?1"
        ),
        concat!(
            "INSERT OR REPLACE INTO model_evaluations ",
            "(rowid,id,run_id,collection_id,card_id,role,mode,generation_id,disposition, ",
            "manifest_digest,report_digest) ",
            "SELECT rowid,'new-evaluation',run_id,collection_id,card_id,role,mode, ",
            "generation_id,disposition,manifest_digest,report_digest ",
            "FROM model_evaluations WHERE id=?1"
        ),
    ];
    let selection_replacements = [
        concat!(
            "INSERT OR REPLACE INTO model_selections ",
            "(id,collection_id,role,card_id,evaluation_id,selected_by,reason) ",
            "SELECT id,collection_id,role,card_id,evaluation_id,selected_by,reason ",
            "FROM model_selections WHERE id=?1"
        ),
        concat!(
            "INSERT OR REPLACE INTO model_selections ",
            "(rowid,id,collection_id,role,card_id,evaluation_id,selected_by,reason) ",
            "SELECT rowid,'new-selection',collection_id,role,card_id,evaluation_id, ",
            "selected_by,reason FROM model_selections WHERE id=?1"
        ),
    ];

    for (table, id, update, replacements, immutable_message, replacement_message) in [
        (
            "model_cards",
            card_id.as_str(),
            "UPDATE model_cards SET role='reranker' WHERE id=?1",
            card_replacements.as_slice(),
            "a model card registration is immutable",
            "a model card registration is never replaced",
        ),
        (
            "model_evaluations",
            evaluation_id.as_str(),
            "UPDATE model_evaluations SET disposition='blocked' WHERE id=?1",
            evaluation_replacements.as_slice(),
            "a model evaluation is immutable",
            "model evaluation does not match its collection, role, card, or generation",
        ),
        (
            "model_selections",
            selection_id.as_str(),
            "UPDATE model_selections SET reason='changed' WHERE id=?1",
            selection_replacements.as_slice(),
            "a model selection is immutable",
            "selection requires an eligible real evaluation of the same v2 card and role",
        ),
    ] {
        assert_immutable_row(
            &connection,
            &ImmutableRow {
                table,
                id,
                update,
                replacements,
                immutable_message,
                replacement_message,
            },
        );
    }
}

struct InsertFixture {
    scratch: Scratch,
    one_card: String,
    two_card: String,
    foreign_generation: i64,
    synthetic_evaluation: String,
    ineligible_evaluation: String,
    other_card_evaluation: String,
}

fn registered_cards(
    database: &Database,
    scratch: &Scratch,
) -> (CardRecord, CardRecord, CardRecord) {
    collection(database, "one");
    collection(database, "two");
    let scopes_one = collection_scopes(database, "one");
    let scopes_two = collection_scopes(database, "two");
    let model_card = card(database, scratch);
    let one = database
        .record_model_card(
            &scopes_one,
            &NewModelCard {
                collection_id: "one",
                card: &model_card,
            },
        )
        .unwrap();
    let two = database
        .record_model_card(
            &scopes_two,
            &NewModelCard {
                collection_id: "two",
                card: &model_card,
            },
        )
        .unwrap();
    let mut other_identity = identity(database);
    other_identity.weights.upstream_revision = "fedcba9876543210".to_owned();
    let other_card = ModelCard::record_v2(&scratch.store(), &other_identity).unwrap();
    let other = database
        .record_model_card(
            &scopes_one,
            &NewModelCard {
                collection_id: "one",
                card: &other_card,
            },
        )
        .unwrap();
    (one, two, other)
}

fn foreign_generation(database: &Database, scratch: &Scratch) -> i64 {
    let chunk_set = scratch.outside();
    chunk_set
        .execute(
            "INSERT INTO chunk_sets (id,collection_id,chunk_profile,counter_contract_id,state) \
             VALUES ('other-set','two','structural-500-700/1','native','building')",
            [],
        )
        .unwrap();
    chunk_set
        .execute(
            "UPDATE chunk_sets SET state='complete',manifest_digest=?1 WHERE id='other-set'",
            ["4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"],
        )
        .unwrap();
    drop(chunk_set);
    database
        .create_generation(&NewGeneration {
            collection_id: "two".to_owned(),
            chunk_set_id: "other-set".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap()
        .id
}

fn selection_evaluations(
    database: &Database,
    one: &CardRecord,
    other: &CardRecord,
) -> (String, String, String) {
    let scopes_one = collection_scopes(database, "one");
    let synthetic = database
        .record_model_evaluation(
            &scopes_one,
            &new_eval(
                one.id,
                "one",
                "synthetic",
                EvaluationMode::Synthetic,
                EvaluationDisposition::Eligible,
                b"synthetic report",
                b"synthetic manifest",
            ),
        )
        .unwrap();
    let ineligible = database
        .record_model_evaluation(
            &scopes_one,
            &new_eval(
                one.id,
                "one",
                "ineligible",
                EvaluationMode::Real,
                EvaluationDisposition::Ineligible,
                b"ineligible report",
                b"ineligible manifest",
            ),
        )
        .unwrap();
    let other_card = database
        .record_model_evaluation(
            &scopes_one,
            &new_eval(
                other.id,
                "one",
                "other-card",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                b"other-card report",
                b"other-card manifest",
            ),
        )
        .unwrap();
    (
        synthetic.id.to_string(),
        ineligible.id.to_string(),
        other_card.id.to_string(),
    )
}

fn insert_fixture() -> InsertFixture {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (one, two, other) = registered_cards(&database, &scratch);
    let foreign_generation = foreign_generation(&database, &scratch);
    let (synthetic_evaluation, ineligible_evaluation, other_card_evaluation) =
        selection_evaluations(&database, &one, &other);
    InsertFixture {
        scratch,
        one_card: one.id.to_string(),
        two_card: two.id.to_string(),
        foreign_generation,
        synthetic_evaluation,
        ineligible_evaluation,
        other_card_evaluation,
    }
}

#[test]
fn model_evaluation_insert_triggers_recheck_registration_eligibility_and_generation() {
    let fixture = insert_fixture();
    let connection = fixture.scratch.outside();
    let manifest = Digest::of(b"raw manifest").as_str().to_owned();
    let report = Digest::of(b"raw report").as_str().to_owned();
    let evaluation_insert = concat!(
        "INSERT INTO model_evaluations ",
        "(id,run_id,collection_id,card_id,role,mode,generation_id,disposition, ",
        "manifest_digest,report_digest) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
    );
    for (id, role, card_id, generation_id) in [
        (
            "foreign-generation",
            "embedder",
            fixture.one_card.as_str(),
            Some(fixture.foreign_generation),
        ),
        ("wrong-role", "reranker", fixture.one_card.as_str(), None),
        ("foreign-card", "embedder", fixture.two_card.as_str(), None),
    ] {
        assert_insert_refusal(
            &connection,
            evaluation_insert,
            params![
                id,
                format!("run-{id}"),
                "one",
                card_id,
                role,
                "real",
                generation_id,
                "eligible",
                manifest,
                report
            ],
            "model evaluation does not match its collection, role, card, or generation",
        );
    }
}

#[test]
fn model_selection_insert_triggers_recheck_evaluation_eligibility() {
    let fixture = insert_fixture();
    let connection = fixture.scratch.outside();
    let selection_insert = concat!(
        "INSERT INTO model_selections ",
        "(id,collection_id,role,card_id,evaluation_id,selected_by,reason) ",
        "VALUES (?1,?2,?3,?4,?5,?6,?7)",
    );
    for (id, evaluation_id) in [
        ("synthetic-selection", fixture.synthetic_evaluation.as_str()),
        (
            "ineligible-selection",
            fixture.ineligible_evaluation.as_str(),
        ),
        (
            "other-card-selection",
            fixture.other_card_evaluation.as_str(),
        ),
    ] {
        assert_insert_refusal(
            &connection,
            selection_insert,
            params![
                id,
                "one",
                "embedder",
                fixture.one_card,
                evaluation_id,
                "tester",
                "must be refused",
            ],
            "selection requires an eligible real evaluation of the same v2 card and role",
        );
    }
}
