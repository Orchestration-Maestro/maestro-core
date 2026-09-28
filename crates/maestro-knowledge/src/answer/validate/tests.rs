use super::{Reply, ValidationFailure, unsupported_literals, validate_reply};
use crate::answer::{AskBudget, AskRequest};
use maestro_kernel::{
    artifact::Digest,
    evidence::{Budget, Bundle, Passage, RequestBudget, RouteStatus, Schema, Span, Trace},
};
use std::collections::BTreeMap;

fn bundle(text: &str) -> Bundle {
    Bundle {
        schema: Schema::V1,
        collection: "docs".to_owned(),
        generation: 1,
        query: "How does this work?".to_owned(),
        lang: "en".to_owned(),
        routes: BTreeMap::from([("bm25".to_owned(), RouteStatus::Ok)]),
        passages: vec![Passage {
            n: 1,
            section_id: None,
            document_id: "document".to_owned(),
            revision_id: "revision".to_owned(),
            title: "Documentation".to_owned(),
            section_path: Vec::new(),
            version: None,
            source_ref: "https://example.org/docs".to_owned(),
            span: Span {
                start: 0,
                end: text.len(),
            },
            digest: Digest::of(text.as_bytes()),
            text: text.to_owned(),
            windowed: false,
            alternates: Vec::new(),
        }],
        conflicts: Vec::new(),
        known_gaps: Vec::new(),
        budget: Budget {
            evidence_tokens: u32::try_from(text.len()).expect("short passage"),
            limit: 6000,
            counter: Some("evidence-utf8-bytes/1".to_owned()),
            estimated: true,
        },
        request_budget: Some(RequestBudget::default()),
        inventory: None,
        trace: vec![Trace {
            n: 1,
            score: None,
            routes: vec!["bm25".to_owned()],
            chunk_ids: vec!["chunk".to_owned()],
            procedural: false,
        }],
    }
}

#[test]
fn c1_rejects_unsubstantiated_commands_flags_paths_names_and_versions() {
    let cases: [(&str, &[&str]); 5] = [
        ("Run `ctmpsm` now safely please.", &["ctmpsm"]),
        (
            "Set EM_HOME, then run ctmpsm -FORCEALL",
            &["EM_HOME", "ctmpsm", "-FORCEALL"],
        ),
        ("Use -FORCEALL now safely please.", &["-FORCEALL"]),
        ("Use /opt/ctm now safely please.", &["/opt/ctm"]),
        ("Use model-4.2 safely now please.", &["model-4.2"]),
    ];
    let question = "How does the service work?";

    for (answer, literals) in cases {
        assert!(
            unsupported_literals(answer, question, &bundle("The service is documented here.")),
            "unsupported literals {literals:?} passed"
        );
        assert!(
            !unsupported_literals(
                answer,
                question,
                &bundle(&format!("The manual documents {}.", literals.join(" "))),
            ),
            "supported literals {literals:?} were rejected"
        );
    }
}

#[test]
fn c1_allows_literals_that_appear_in_the_question() {
    assert!(!unsupported_literals(
        "Set EM_HOME, then run ctmpsm -FORCEALL now.",
        "How do I use EM_HOME with ctmpsm -FORCEALL?",
        &bundle("The service uses a local environment setting."),
    ));
}

#[test]
fn i1_accepts_verbatim_fenced_code_and_digit_mixed_tokens() {
    let question = "How do I check the build configuration?";
    for (answer, evidence) in [
        (
            "Use x86_64 with 64-bit builds.",
            "Set the target to x86_64 for 64-bit builds.",
        ),
        (
            "```bash\nctmpsm -LISTALL\n```",
            "Run ctmpsm -LISTALL to display the jobs.",
        ),
        (
            "Use tool run --force now.",
            "The exact command is tool\n  run   --force.",
        ),
    ] {
        assert!(
            !unsupported_literals(answer, question, &bundle(evidence)),
            "verbatim answer was rejected: {answer:?}"
        );
    }
}

#[test]
fn i2_accepts_an_answer_whose_language_is_undetected() {
    let request = AskRequest {
        collection: "docs".to_owned(),
        question: "How does the service work?".to_owned(),
        model: "qwen3-4b".to_owned(),
        version: None,
        budget: AskBudget::default(),
    };
    let reply = validate_reply(
        "- a - b - c [1]",
        &request,
        &bundle("The evidence describes the service configuration."),
    );
    assert!(matches!(reply, Ok(Reply::Answer(_))));
}

#[test]
fn m3_literals_from_passage_titles_and_section_paths_are_supported() {
    let question = "How does the service work?";
    let mut titled = bundle("The service uses documented defaults.");
    titled.passages[0].title = "EM_HOME".to_owned();
    assert!(!unsupported_literals(
        "Use EM_HOME with the documented service settings.",
        question,
        &titled,
    ));

    let mut sectioned = bundle("The service uses documented defaults.");
    sectioned.passages[0].section_path = vec!["ctmpsm -FORCEALL".to_owned()];
    assert!(!unsupported_literals(
        "Run ctmpsm -FORCEALL during maintenance.",
        question,
        &sectioned,
    ));
}

#[test]
fn i8_thinking_markup_is_stripped_or_rejected_before_output() {
    let request = AskRequest {
        collection: "docs".to_owned(),
        question: "How does the service work?".to_owned(),
        model: "qwen3-4b".to_owned(),
        version: None,
        budget: AskBudget::default(),
    };
    let evidence = bundle("The service uses documented defaults.");
    let reply = validate_reply(
        "<think>private reasoning</think>The service uses documented defaults [1].",
        &request,
        &evidence,
    );
    let Ok(Reply::Answer(reply)) = reply else {
        panic!("leading think block should leave a valid answer");
    };
    assert_eq!(reply.text, "The service uses documented defaults [1].");
    assert!(matches!(
        validate_reply("<think>unfinished reasoning", &request, &evidence),
        Err(ValidationFailure::ThinkMarkup)
    ));
    assert!(matches!(
        validate_reply(
            "The service uses documented defaults [1]. <think>hidden</think>",
            &request,
            &evidence,
        ),
        Err(ValidationFailure::ThinkMarkup)
    ));
}

#[test]
fn i8_fake_missing_and_malformed_citations_are_rejected() {
    let request = AskRequest {
        collection: "docs".to_owned(),
        question: "How does the service work?".to_owned(),
        model: "qwen3-4b".to_owned(),
        version: None,
        budget: AskBudget::default(),
    };
    let evidence = bundle("The service uses documented defaults.");

    for reply in [
        "The service uses documented defaults.",
        "The service uses documented defaults [2].",
        "The service uses documented defaults [x].",
        "The service uses documented defaults [0].",
        "The service uses documented defaults [1]. ]",
        "The service uses documented defaults [1.",
    ] {
        assert!(
            matches!(
                validate_reply(reply, &request, &evidence),
                Err(ValidationFailure::Citation)
            ),
            "invalid citation passed: {reply:?}"
        );
    }
}

#[test]
fn i8_citation_only_and_short_answers_are_rejected() {
    let request = AskRequest {
        collection: "docs".to_owned(),
        question: "How does the service work?".to_owned(),
        model: "qwen3-4b".to_owned(),
        version: None,
        budget: AskBudget::default(),
    };
    let evidence = bundle("The service uses documented defaults.");

    for reply in ["[1]", "Only two [1]", "- - - [1]"] {
        assert!(
            matches!(
                validate_reply(reply, &request, &evidence),
                Err(ValidationFailure::TooShort)
            ),
            "short answer passed: {reply:?}"
        );
    }
}
