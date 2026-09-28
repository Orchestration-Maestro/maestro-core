use super::{
    Invalid, Reply, ValidationFailure, inline_backtick_literals, trim_token_edges, unescaped,
    unsupported_literals, validate_reply,
};
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
            !unsupported_literals(answer, question, &bundle("The service is documented here."))
                .is_empty(),
            "unsupported literals {literals:?} passed"
        );
        assert!(
            unsupported_literals(
                answer,
                question,
                &bundle(&format!("The manual documents {}.", literals.join(" "))),
            )
            .is_empty(),
            "supported literals {literals:?} were rejected"
        );
    }
}

#[test]
fn c1_allows_literals_that_appear_in_the_question() {
    assert!(
        unsupported_literals(
            "Set EM_HOME, then run ctmpsm -FORCEALL now.",
            "How do I use EM_HOME with ctmpsm -FORCEALL?",
            &bundle("The service uses a local environment setting."),
        )
        .is_empty()
    );
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
            unsupported_literals(answer, question, &bundle(evidence)).is_empty(),
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
    assert!(
        unsupported_literals(
            "Use EM_HOME with the documented service settings.",
            question,
            &titled,
        )
        .is_empty()
    );

    let mut sectioned = bundle("The service uses documented defaults.");
    sectioned.passages[0].section_path = vec!["ctmpsm -FORCEALL".to_owned()];
    assert!(
        unsupported_literals(
            "Run ctmpsm -FORCEALL during maintenance.",
            question,
            &sectioned,
        )
        .is_empty()
    );
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
        Err(Invalid {
            failure: ValidationFailure::ThinkMarkup,
            ..
        })
    ));
    for leaked in ["<think>hidden</think>", "<think>", "</think>"] {
        assert!(matches!(
            validate_reply(
                &format!("The service uses documented defaults [1]. {leaked}"),
                &request,
                &evidence,
            ),
            Err(Invalid {
                failure: ValidationFailure::ThinkMarkup,
                ..
            })
        ));
    }
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
                Err(Invalid {
                    failure: ValidationFailure::Citation,
                    ..
                })
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
                Err(Invalid {
                    failure: ValidationFailure::TooShort,
                    ..
                })
            ),
            "short answer passed: {reply:?}"
        );
    }
}

fn ask(question: &str) -> AskRequest {
    AskRequest {
        collection: "docs".to_owned(),
        question: question.to_owned(),
        model: "qwen3-4b".to_owned(),
        version: None,
        budget: AskBudget::default(),
    }
}

/// The unsupported literals of `reply`, or `None` when it passes.
fn rejected_literals(reply: &str, evidence: &Bundle) -> Option<Vec<String>> {
    match validate_reply(reply, &ask("How is the pool named?"), evidence) {
        Ok(Reply::Answer(_)) => None,
        Err(Invalid {
            failure: ValidationFailure::UnsupportedLiteral,
            tokens,
        }) => Some(tokens),
        Ok(Reply::NotFound) | Err(_) => panic!("{reply:?} failed another check"),
    }
}

#[test]
fn html_entities_in_a_passage_support_the_characters_they_stand_for() {
    let evidence = bundle("Include the **-o &lt;output&gt;** option in each command.");
    assert_eq!(
        rejected_literals(
            "Include the `-o <output>` option in each command [1].",
            &evidence
        ),
        None
    );
    assert_eq!(
        rejected_literals(
            "Include the `-x <output>` option in each command [1].",
            &evidence
        ),
        Some(vec!["-x".to_owned(), "-x <output>".to_owned()])
    );
}

#[test]
fn markdown_and_copied_prompt_escapes_do_not_make_a_literal_unsupported() {
    let evidence = bundle(r#"Name it Queue\_Name@Host\_Name, then run tool "*42" to see why."#);
    for reply in [
        r#"Name it Queue_Name@Host_Name, then run tool "*42" to see why [1]."#,
        r#"Name it Queue\\_Name@Host\\_Name, then run tool \"*42\" to see why [1]."#,
    ] {
        assert_eq!(rejected_literals(reply, &evidence), None, "{reply}");
    }
    assert_eq!(
        rejected_literals(
            r#"Name it Queue_Name@Host_Name, then run tool "*43" to see why [1]."#,
            &evidence
        ),
        Some(vec!["43".to_owned()])
    );
}

#[test]
fn bracketed_link_text_is_text_and_digits_in_brackets_are_citations() {
    let mut evidence = bundle("Open an item with the [item::open](guide.htm#open) call.");
    let mut second = evidence.passages[0].clone();
    second.n = 2;
    evidence.passages.push(second);
    let request = ask("How do I open an item?");
    for (reply, citations) in [
        (
            "Open an item with the [item::open](guide.htm#open) call [1].",
            vec![1],
        ),
        ("Open an item with the documented call [2, 1].", vec![1, 2]),
        (
            "Open an item with the documented call [2,1][1].",
            vec![1, 2],
        ),
    ] {
        let Ok(Reply::Answer(valid)) = validate_reply(reply, &request, &evidence) else {
            panic!("{reply:?} was refused");
        };
        assert_eq!(valid.citations, citations, "{reply}");
    }
    for (reply, marker) in [
        ("Open an item with the documented call [1, ].", "[1, ]"),
        ("Open an item with the documented call [1, 3].", "[3]"),
    ] {
        assert!(
            matches!(
                validate_reply(reply, &request, &evidence),
                Err(Invalid { failure: ValidationFailure::Citation, tokens }) if tokens == [marker]
            ),
            "{reply}"
        );
    }
}

#[test]
fn unescaping_decodes_entities_once_and_drops_nested_escapes() {
    assert_eq!(
        unescaped(r"&quot;a&quot; &#39;b&#39; &amp;lt; Queue\\_Name C:\Program\ \\\\host"),
        r#""a" 'b' &lt; Queue_Name C:\Program\ \host"#
    );
}

#[test]
fn a_backslash_that_was_a_paths_only_marker_keeps_it_a_literal() {
    let reply = r"Store the key in %USERPROFILE%\.toolrc on the host [1].";
    assert_eq!(
        rejected_literals(reply, &bundle("Store the key in the user's profile.")),
        Some(vec!["USERPROFILE%.toolrc".to_owned()])
    );
    assert_eq!(
        rejected_literals(
            reply,
            &bundle(r"Store the key in %USERPROFILE%\.toolrc on each host.")
        ),
        None
    );
}

#[test]
fn a_bracket_with_a_number_is_a_citation_and_bracket_words_are_not_the_answer() {
    let request = ask("How do I open an item?");
    let evidence = bundle("Open an item from its menu with the [-NAME <item>] option.");
    assert!(matches!(
        validate_reply("Open an item from its menu [1]. More [passage 9].", &request, &evidence),
        Err(Invalid { failure: ValidationFailure::Citation, tokens }) if tokens == ["[passage 9]"]
    ));
    assert!(matches!(
        validate_reply("[See the passage] [1]", &request, &evidence),
        Err(Invalid {
            failure: ValidationFailure::TooShort,
            ..
        })
    ));
    assert!(matches!(
        validate_reply(
            "Open an item from its menu with the [-NAME <item>] option [1].",
            &request,
            &evidence
        ),
        Ok(Reply::Answer(_))
    ));
}

#[test]
fn each_literal_source_is_checked_once_in_its_own_region() {
    // `understand` joins the lines of case 4 and pairs its stray backtick
    // with the one before `halt`; this evidence supports that span.
    let evidence = bundle("Press the backtick key. Then type the command.");
    let cases: [(&str, &[&str]); 8] = [
        (
            "Run tool remove --all now.",
            &["--all", "remove", "tool remove --all"],
        ),
        (
            "List them with the command below.\n```\ntool list all\n```",
            &["tool list all"],
        ),
        (
            "Run the command below\n```\n--all\n```",
            &["--all", "below"],
        ),
        ("Press ` key.\nThen type `halt` now.", &["halt"]),
        ("Type ``a`b`` now.", &["a`b"]),
        ("Type `` `halt` `` now `x", &[" `halt` "]),
        ("Type `halt` then `x", &["halt"]),
        ("Start it with ./run.", &["./run"]),
    ];
    for (reply, expected) in cases {
        assert_eq!(
            unsupported_literals(reply, "How?", &evidence),
            expected,
            "{reply:?}"
        );
    }
}

#[test]
fn backtick_spans_and_token_edges_are_cut_on_character_boundaries() {
    assert_eq!(
        inline_backtick_literals("Type `` then `halt` now."),
        ["halt"]
    );
    assert_eq!(inline_backtick_literals("é`ü` ``a`b``"), ["ü", "a`b"]);
    assert_eq!(trim_token_edges("“/opt/café”."), "/opt/café");
    assert_eq!(trim_token_edges("‘./é’"), "./é");
    assert_eq!(trim_token_edges("—“”–"), "");
    assert_eq!(trim_token_edges(""), "");
}
