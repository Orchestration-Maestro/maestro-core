//! The ignored live reranker selection and the checks of its helper.

use super::{
    rung_selection::{RungEvidence, select_reranker},
    support::{
        Scratch, card_for_role, collection, collection_scopes, generation_in, live_kernel, required,
    },
};
use crate::{
    artifact::Digest,
    gateway::Role,
    model::{EvaluationDisposition, EvaluationMode, NewModelCard, SelectionRecord},
    scope::ScopeSet,
    store::Database,
};
use serde_json::{Value, json};
use std::{error::Error as StdError, fs};

const COLLECTION: &str = "selection-test";

#[test]
#[ignore = "selects the reranker in the local kernel; run only with owner approval"]
fn select_reranker_live() {
    let manifest = fs::read(required("MAESTRO_RUNG_MANIFEST")).expect("read the ladder manifest");
    let report = fs::read(required("MAESTRO_RUNG_REPORT")).expect("read the rung report");
    let collection_id = required("MAESTRO_CARD_COLLECTION");
    let card = Digest::parse(&required("MAESTRO_CARD_DIGEST")).expect("a card digest");
    let (database, scopes, _) = live_kernel();
    let published = database
        .published_generation(&scopes, &collection_id)
        .expect("read the published generation")
        .expect("a published generation");
    let selection = select_reranker(
        &database,
        &scopes,
        &RungEvidence {
            collection: &collection_id,
            card: &card,
            published_generation: published.id,
            manifest: &manifest,
            report: &report,
            selected_by: &required("MAESTRO_SELECTED_BY"),
            reason: &required("MAESTRO_SELECTION_REASON"),
        },
    )
    .expect("select the reranker");
    println!("{} {}", selection.id, selection.evaluation_id);
}

#[test]
fn a_rung_whose_retrieval_floors_pass_selects_its_reranker_whatever_its_answer_floors() {
    let fixture = Fixture::new();
    let inputs = fixture.inputs();

    let selection = fixture
        .select(&inputs, &fixture.card, fixture.generation)
        .unwrap();

    let selected = fixture
        .database
        .selected_model_card(&fixture.scopes, COLLECTION, Role::Reranker)
        .unwrap()
        .unwrap();
    assert_eq!(selected.card.digest(), &fixture.card);
    assert_eq!(selected.selection, selection);
    assert_eq!(selection.selected_by, "owner");
    assert_eq!(selection.reason, "measured");
    let evaluation = selected.evaluation;
    let report = bytes(&inputs["report"]);
    assert_eq!(
        evaluation.run_id,
        format!("ladder:v2-base:{}", Digest::of(&report).as_str())
    );
    assert_eq!(evaluation.mode, EvaluationMode::Real);
    assert_eq!(evaluation.disposition, EvaluationDisposition::Eligible);
    assert_eq!(evaluation.generation_id, Some(fixture.generation));
    assert_eq!(
        fixture.database.get(&evaluation.report_digest).unwrap(),
        report
    );
    assert_eq!(
        fixture.database.get(&evaluation.manifest_digest).unwrap(),
        bytes(&inputs["manifest"])
    );
    assert_eq!(fixture.writes(), (1, 1));
}

#[test]
fn each_refusal_names_its_cause_and_records_nothing() {
    let other = "b".repeat(64);
    let cases: [(&str, Value, &str); 20] = [
        ("/selected_by", json!(" "), "who selects and why"),
        ("/reason", json!(""), "who selects and why"),
        (
            "/report/schema",
            json!("maestro-eval-ladder-rung/2"),
            "not a ladder rung report",
        ),
        (
            "/report/collection",
            json!("other"),
            "rung searched another collection",
        ),
        ("/report/verdict", json!("INVALID"), "INVALID"),
        (
            "/report/provenance/reranker",
            json!(other),
            "another reranker",
        ),
        ("/report/end/reranker", json!(other), "another reranker"),
        (
            "/report/configuration/rerank",
            Value::Null,
            "another reranker",
        ),
        (
            "/report/configuration/rerank/card",
            json!(other),
            "another reranker",
        ),
        (
            "/report/provenance/generation",
            json!(0),
            "another generation",
        ),
        ("/report/end/generation", json!(0), "another generation"),
        ("/report/score/floors/0/status", json!("fail"), "top_10"),
        (
            "/report/score/floors/1/status",
            json!("unavailable"),
            "top_1",
        ),
        (
            "/report/score/floors/2/floor",
            json!("ask_p95"),
            "search_p95",
        ),
        ("/report/score/floors", json!([]), "top_10"),
        (
            "/manifest/schema",
            json!("maestro-ladder-manifest/2"),
            "not a ladder manifest",
        ),
        (
            "/manifest/collection",
            json!("other"),
            "manifest searches another collection",
        ),
        ("/manifest/rungs/1/name", json!("v1-base"), "no rung"),
        (
            "/manifest/rungs/1/configuration/rerank/depth",
            json!(80),
            "no rung",
        ),
        (
            "/manifest/rungs/1/configuration/rerank/card",
            json!(other),
            "no rung",
        ),
    ];
    let fixture = Fixture::new();

    for (pointer, value, cause) in cases {
        let mut inputs = fixture.inputs();
        *inputs.pointer_mut(pointer).unwrap() = value;
        let error = fixture
            .select(&inputs, &fixture.card, fixture.generation)
            .unwrap_err()
            .to_string();
        assert!(error.contains(cause), "{pointer}: {error}");
    }
    let unregistered = Digest::parse(&"c".repeat(64)).unwrap();
    let mut inputs = fixture.inputs();
    for pointer in [
        "/report/provenance/reranker",
        "/report/end/reranker",
        "/report/configuration/rerank/card",
        "/manifest/rungs/1/configuration/rerank/card",
    ] {
        *inputs.pointer_mut(pointer).unwrap() = json!(unregistered.as_str());
    }
    let error = fixture
        .select(&inputs, &unregistered, fixture.generation)
        .unwrap_err();
    assert!(error.to_string().contains("not a registered reranker"));
    let error = fixture
        .select(&fixture.inputs(), &fixture.card, fixture.generation + 1)
        .unwrap_err();
    assert!(error.to_string().contains("another generation"));
    assert_eq!(fixture.writes(), (0, 0));
}

struct Fixture {
    scratch: Scratch,
    database: Database,
    scopes: ScopeSet,
    card: Digest,
    generation: i64,
}

impl Fixture {
    fn new() -> Self {
        let scratch = Scratch::new();
        let database = scratch.open();
        collection(&database, COLLECTION);
        let scopes = collection_scopes(&database, COLLECTION);
        let generation = generation_in(&database, COLLECTION);
        let card = card_for_role(&database, &scratch, Role::Reranker);
        database
            .record_model_card(
                &scopes,
                &NewModelCard {
                    collection_id: COLLECTION,
                    card: &card,
                },
            )
            .unwrap();
        Self {
            card: card.digest().clone(),
            scratch,
            database,
            scopes,
            generation,
        }
    }

    /// Passing inputs in the shape of ladder run 3's rung `v2-base`, whose
    /// answer floors fail, and of the manifest that ran it.
    fn inputs(&self) -> Value {
        let ran = json!({"generation": self.generation, "reranker": self.card.as_str()});
        let rerank = json!({"card": self.card.as_str(), "depth": 30});
        json!({
            "selected_by": "owner",
            "reason": "measured",
            "report": {
                "schema": "maestro-eval-ladder-rung/1",
                "rung": "v2-base",
                "verdict": "FAIL",
                "collection": COLLECTION,
                "provenance": ran,
                "end": ran,
                "configuration": {"rerank": rerank},
                "score": {"floors": [
                    {"floor": "top_10", "status": "pass"},
                    {"floor": "top_1", "status": "pass"},
                    {"floor": "search_p95", "status": "pass"},
                    {"floor": "citation", "status": "fail"},
                    {"floor": "answered", "status": "fail"},
                ]},
            },
            "manifest": {
                "schema": "maestro-ladder-manifest/1",
                "collection": COLLECTION,
                "rungs": [
                    {"name": "v2-k8", "configuration": {"rerank": rerank}},
                    {"name": "v2-base", "configuration": {"rerank": rerank}},
                ],
            },
        })
    }

    fn select(
        &self,
        inputs: &Value,
        card: &Digest,
        published_generation: i64,
    ) -> Result<SelectionRecord, Box<dyn StdError + Send + Sync>> {
        select_reranker(
            &self.database,
            &self.scopes,
            &RungEvidence {
                collection: COLLECTION,
                card,
                published_generation,
                manifest: &bytes(&inputs["manifest"]),
                report: &bytes(&inputs["report"]),
                selected_by: inputs["selected_by"].as_str().unwrap(),
                reason: inputs["reason"].as_str().unwrap(),
            },
        )
    }

    /// The evaluations and selections recorded.
    fn writes(&self) -> (i64, i64) {
        let count = |table: &str| -> i64 {
            self.scratch
                .outside()
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap()
        };
        (count("model_evaluations"), count("model_selections"))
    }
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}
