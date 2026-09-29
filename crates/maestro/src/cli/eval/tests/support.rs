//! A synthetic suite, rungs, and a fake engine that records what it is asked.

use super::super::rank_settings::{Context, Prior};
use super::super::{
    manifest::{AskSettings, Rerank, Routes, Rung, RungConfiguration, Weights},
    runner::{Asked, Engine, Provenance, RejectedCheck, SearchDiagnostic, Searched},
};
use crate::failure::Failure;
use maestro_knowledge::search::RerankHeader;
use maestro_knowledge::{
    answer::RefusalCode,
    eval::{AskOutcome, SearchOutcome, SectionRef},
    search::{IntentExpansion, IntentTrigger, SearchConfiguration, evidence::Anchor},
    suite::Suite,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    num::{NonZeroU32, NonZeroUsize},
};

/// A reranker card digest.
pub(super) const RERANKER: &str =
    "1111111111111111111111111111111111111111111111111111111111111111";

/// A suite of `answerable` answerable questions, `a0`, `a1`..., then
/// `unanswerable` unanswerable ones, `u0`, `u1`...
pub(super) fn suite(answerable: usize, unanswerable: usize) -> Suite {
    let answerable = (0..answerable).map(|index| {
        json!({
            "schema": "maestro-suite/1", "id": format!("a{index}"), "language": "en",
            "question": format!("What is secret {index}?"), "answerable": true,
            "expected": [{"source_ref": format!("doc:a{index}"), "heading_path": ["Title"]}],
        })
    });
    let unanswerable = (0..unanswerable).map(|index| {
        json!({
            "schema": "maestro-suite/1", "id": format!("u{index}"), "language": "fr",
            "question": format!("Quel est le secret {index} ?"), "answerable": false,
            "expected": [],
        })
    });
    answerable
        .chain(unanswerable)
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .parse()
        .unwrap()
}

/// A rung named `name` that runs every route, reranks with [`RERANKER`] and
/// asks.
pub(super) fn rung(name: &str) -> Rung {
    Rung {
        name: name.to_owned(),
        configuration: RungConfiguration {
            intent_expansion: IntentExpansion::Off,
            intent_trigger: IntentTrigger::Always,
            intent_card: None,
            intent_deadline_ms: 4000,
            intent_weight: 1.0,
            intent_rerank_additions: 10,
            routes: Routes {
                dense: true,
                lexical: true,
                identifier: true,
                structured: false,
            },
            identifier_noise_guard: false,
            rrf_k: NonZeroU32::new(20).unwrap(),
            weights: Weights {
                dense: 2.0,
                lexical: 1.0,
                identifier: 1.0,
                structured: 1.0,
            },
            rerank: Some(Rerank {
                card: RERANKER.to_owned(),
                depth: NonZeroUsize::new(30).unwrap(),
                blend: None,
                demotion_cap: None,
                candidate_context: Context::default(),
                header: RerankHeader::default(),
            }),
            min_rerank_score: None,
            section_prior: Prior::default(),
            stage_window_ms: None,
            source_prior: None,
        },
        ask: Some(AskSettings::default()),
    }
}

/// The manifest JSON of [`rung`] `name`, which asks with the default
/// settings.
pub(super) fn rung_json(name: &str) -> Value {
    json!({
        "name": name,
        "configuration": serde_json::to_value(rung(name).configuration).unwrap(),
        "ask": true,
    })
}

/// One call to the fake engine.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Call {
    /// `search` or `ask`.
    pub(super) operation: &'static str,
    /// The rung's name.
    pub(super) rung: String,
    /// The question.
    pub(super) question: String,
    /// The configuration it ran under.
    pub(super) configuration: SearchConfiguration,
}

/// An engine whose searches rank each answerable question's document first,
/// with evidence of another document, and whose asks cite its section, pinned,
/// from a bundle that holds it, after one attempt the answer check refused,
/// and refuse the others.
#[derive(Debug, Default)]
pub(super) struct FakeEngine {
    /// Every search and ask, in order.
    pub(super) calls: RefCell<Vec<Call>>,
    /// The published generation.
    pub(super) generation: Cell<i64>,
    /// After this many searches, the generation changes.
    pub(super) drift_after: Option<usize>,
    /// The rung whose cards cannot run.
    pub(super) refused_rung: Option<String>,
    /// The rung at whose start the generation changes.
    pub(super) drift_at_start_of: Option<String>,
    /// Whether the generation has no embedder card.
    pub(super) no_embedder: bool,
    /// Whether each search ranks the right document 7th, and its evidence
    /// holds 3 other documents only.
    pub(super) right_document_at_7: bool,
    /// Whether the collection has no answerer card.
    pub(super) no_answerer: bool,
    /// After this many searches, what a rung runs against cannot be read.
    pub(super) unreadable_after: Option<usize>,
    /// How many of the last questions' expected sections are missing.
    pub(super) expectations_missing: usize,
}

impl FakeEngine {
    /// The questions of the calls of `operation`, in order.
    pub(super) fn questions(&self, operation: &str) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .filter(|call| call.operation == operation)
            .map(|call| call.question.clone())
            .collect()
    }

    /// Records a call.
    fn record(&self, operation: &'static str, rung: &Rung, question: &str) {
        self.calls.borrow_mut().push(Call {
            operation,
            rung: rung.name.clone(),
            question: question.to_owned(),
            configuration: rung.configuration.search(),
        });
    }
}

/// A search's diagnostic whose evidence holds `bundle_documents`: a top
/// reranker score of 0.75 when `rung` reranks, and a top fused score of 0.05.
fn diagnostic(rung: &Rung, bundle_documents: Vec<String>) -> SearchDiagnostic {
    SearchDiagnostic {
        intent_status: None,
        intent_displaced: None,
        bundle_documents,
        top_rerank_score: rung.configuration.rerank.as_ref().map(|_| 0.75),
        top_fused_score: Some(0.05),
        ..SearchDiagnostic::default()
    }
}

/// The pinned revision of every section the fake engine expects.
const REVISION: &str = "rev";

/// An anchor of `document`, pinned, over the bytes 0 to 10 of its section
/// `section`.
pub(super) fn anchor(document: &str, section: &str) -> Anchor {
    Anchor {
        source_ref: format!("doc:{document}"),
        doc_id: document.to_owned(),
        revision_id: REVISION.to_owned(),
        section_id: Some(section.to_owned()),
        span: [0, 10],
        digest: format!("sha256:{}", "0".repeat(64)),
    }
}

/// The index of an answerable question from its text, if it is one.
fn answerable_index(question: &str) -> Option<String> {
    question
        .strip_prefix("What is secret ")
        .and_then(|rest| rest.strip_suffix('?'))
        .map(str::to_owned)
}

impl Engine for FakeEngine {
    fn provenance(&self, rung: &Rung) -> Result<Provenance, Failure> {
        if self.refused_rung.as_deref() == Some(rung.name.as_str()) {
            return Err(Failure::refused("the reranker card is not registered"));
        }
        if self
            .unreadable_after
            .is_some_and(|searches| self.questions("search").len() >= searches)
        {
            return Err(Failure::failed(
                "the collection has no published generation",
            ));
        }
        Ok(Provenance {
            intent: None,
            generation: self.generation.get(),
            chunk_set: "chunk-set".to_owned(),
            embedder: (!self.no_embedder).then(|| "e".repeat(64)),
            reranker: rung
                .configuration
                .rerank
                .as_ref()
                .map(|rerank| rerank.card.clone()),
            answerer: (!self.no_answerer).then(|| "a".repeat(64)),
            prompt: None,
            source_classes: None,
        })
    }

    fn start(
        &mut self,
        rung: &Rung,
        suite: &Suite,
    ) -> Result<(Provenance, Vec<Vec<SectionRef>>), Failure> {
        if self.drift_at_start_of.as_deref() == Some(rung.name.as_str()) {
            self.generation.set(self.generation.get() + 1);
        }
        let expected = suite
            .questions
            .iter()
            .map(|question| {
                if question.answerable {
                    vec![SectionRef {
                        revision_id: Some(REVISION.to_owned()),
                        span: Some([0, 10]),
                        ..SectionRef::section(
                            &format!("doc-{}", question.id),
                            &format!("section-{}", question.id),
                        )
                    }]
                } else {
                    Vec::new()
                }
            })
            .collect::<Vec<_>>();
        let kept = expected.len() - self.expectations_missing;
        Ok((self.provenance(rung)?, expected[..kept].to_vec()))
    }

    fn search(&self, rung: &Rung, question: &str) -> Searched {
        self.record("search", rung, question);
        let searches = self.questions("search").len();
        if self.drift_after == Some(searches) {
            self.generation.set(self.generation.get() + 1);
        }
        let right = answerable_index(question)
            .map_or_else(|| "doc-other".to_owned(), |index| format!("doc-a{index}"));
        if self.right_document_at_7 {
            let others: Vec<String> = (0..9).map(|index| format!("doc-other-{index}")).collect();
            let mut ranked = others[..6].to_vec();
            ranked.push(right);
            ranked.extend_from_slice(&others[6..]);
            return Searched {
                outcome: SearchOutcome::Ranked(ranked),
                delivered: Vec::new(),
                diagnostic: diagnostic(rung, others[..3].to_vec()),
            };
        }
        Searched {
            outcome: SearchOutcome::Ranked(vec![right.clone()]),
            delivered: vec![anchor("doc-other", "section-other")],
            diagnostic: diagnostic(rung, vec![right]),
        }
    }

    fn ask(&self, rung: &Rung, question: &str) -> Asked {
        self.record("ask", rung, question);
        answerable_index(question).map_or_else(
            || Asked {
                outcome: AskOutcome::Refused(RefusalCode::NotFound),
                delivered: Vec::new(),
                rejections: Vec::new(),
                reply_cap: Some(2048),
            },
            |index| Asked {
                outcome: AskOutcome::Answered {
                    citations: vec![SectionRef {
                        revision_id: Some(REVISION.to_owned()),
                        ..SectionRef::section(
                            &format!("doc-a{index}"),
                            &format!("section-a{index}"),
                        )
                    }],
                    invented_literals: 0,
                },
                delivered: vec![anchor(
                    &format!("doc-a{index}"),
                    &format!("section-a{index}"),
                )],
                rejections: vec![RejectedCheck {
                    attempt: 1,
                    check: "unsupported_literal",
                }],
                reply_cap: Some(2048),
            },
        )
    }
}
