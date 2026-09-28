use super::*;

#[tokio::test]
async fn each_rejected_attempt_is_explained_but_never_serialized() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("How do I list the registered sources?");
    let evidence = bundle(
        &request.question,
        "en",
        "Run `maestro knowledge collections` to see registered sources.",
    );
    let port = ScriptedPort::new(&[
        "[1]",
        "Run `maestro collection remove --all` to remove every source. [1]",
    ]);

    let answer = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        evidence,
        PromptVersion::V1,
    )
    .await
    .expect("safe unsupported refusal");

    assert_eq!(
        answer.rejections,
        [
            Rejection {
                attempt: 1,
                check: "too_short",
                tokens: Vec::new(),
            },
            Rejection {
                attempt: 2,
                check: "unsupported_literal",
                tokens: vec![
                    "--all".to_owned(),
                    "maestro collection remove --all".to_owned(),
                    "remove".to_owned(),
                ],
            },
        ]
    );
    let json = serde_json::to_value(&answer).expect("answer JSON");
    assert!(json.get("rejections").is_none());
}

#[tokio::test]
async fn an_answer_after_a_rejected_attempt_keeps_its_explanation() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("How do I list the registered sources?");
    let evidence = bundle(
        &request.question,
        "en",
        "Run `maestro knowledge collections` to see registered sources.",
    );
    let port = ScriptedPort::with_results(vec![
        Err(Error::InvalidAnswer {
            reason: "the reply was cut at its token limit".to_owned(),
        }),
        Ok("Run `maestro knowledge collections` to see registered sources [1].".to_owned()),
    ]);

    let answer = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        evidence,
        PromptVersion::V1,
    )
    .await
    .expect("answer");

    assert!(answer.refusal.is_none());
    assert_eq!(
        answer.rejections,
        [Rejection {
            attempt: 1,
            check: "invalid_answer",
            tokens: vec!["the reply was cut at its token limit".to_owned()],
        }]
    );
}

#[test]
fn the_prompt_asks_for_sentences_not_a_bare_marker() {
    let request = request("How do I list the registered sources?");
    let evidence = bundle(&request.question, "en", "Sources are listed.");
    let messages = prompt(&request, &evidence, PromptVersion::V1).expect("bounded prompt");

    assert!(
        messages[0]
            .content
            .contains("a marker alone is not an answer")
    );
    assert!(messages[1].content.starts_with(
        "Answer the question in full sentences, with passage markers such as [1] after the \
         sentences they support.\nQuestion and evidence data (JSON):\n"
    ));
}

#[test]
fn ask_searches_long_enough_to_load_a_cold_embedder() {
    assert_eq!(AskBudget::default().search_deadline_ms, 6000);
}
