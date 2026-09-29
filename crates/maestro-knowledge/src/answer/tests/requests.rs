use super::*;
use maestro_kernel::gateway::MAX_CHAT_OUTPUT_TOKENS;
use std::error::Error as _;

/// A change to a valid request, and the error it must cause, if any.
type BoundCase = (fn(&mut AskRequest), Option<&'static str>);

/// The error `answer_bundle` returns for `request`, rendered, or `None` when
/// the request passes its bounds and reaches the empty-bundle refusal.
async fn bound_error(request: &AskRequest) -> Option<String> {
    let port = ScriptedPort::new(&[]);
    match answer_bundle(
        &port,
        request,
        None,
        empty_bundle(&request.question),
        &PromptVersion::V1.into(),
    )
    .await
    {
        Ok(answer) => {
            assert_eq!(
                answer.refusal.expect("refusal").code,
                RefusalCode::NoEvidence
            );
            None
        }
        Err(error) => Some(error.to_string()),
    }
}

#[tokio::test]
async fn request_bounds_accept_their_limits_and_refuse_one_past_them() {
    let question = "invalid ask request: question must contain 1 to 8192 UTF-8 bytes";
    let version = "invalid ask request: version must contain 1 to 256 UTF-8 bytes";
    let budget = "invalid ask request: ask budget is outside accepted limits";
    let evidence = "invalid ask request: max_tokens must be between 1 and 24000";
    let cases: [BoundCase; 14] = [
        (|_| {}, None),
        (|request| request.question = " ".to_owned(), Some(question)),
        (|request| request.question = "q".repeat(8192), None),
        (
            |request| request.question = "q".repeat(8193),
            Some(question),
        ),
        (
            |request| request.version = Some(String::new()),
            Some(version),
        ),
        (|request| request.version = Some("v".repeat(256)), None),
        (
            |request| request.version = Some("v".repeat(257)),
            Some(version),
        ),
        (|request| request.budget.k = 0, Some(budget)),
        (|request| request.budget.max_tokens = 0, Some(evidence)),
        (|request| request.budget.max_tokens = 24_000, None),
        (|request| request.budget.max_tokens = 24_001, Some(evidence)),
        (
            |request| request.budget.search_deadline_ms = 0,
            Some(budget),
        ),
        (
            |request| request.budget.output_tokens = Some(0),
            Some(budget),
        ),
        (
            |request| request.collection = " ".to_owned(),
            Some("invalid ask request: collection must not be blank"),
        ),
    ];
    for (index, (change, expected)) in cases.into_iter().enumerate() {
        let mut request = request("How does the service work?");
        change(&mut request);
        assert_eq!(
            bound_error(&request).await.as_deref(),
            expected,
            "case {index}"
        );
    }
}

#[test]
fn an_ask_budget_leaves_the_reply_cap_to_the_answerer_card() {
    assert_eq!(AskBudget::default().output_tokens, None);
}

#[test]
fn an_ask_budget_is_within_limits_up_to_each_bound_and_not_past_it() {
    let at_bounds = AskBudget {
        k: 50,
        max_tokens: 24_000,
        search_deadline_ms: 30_000,
        output_tokens: Some(MAX_CHAT_OUTPUT_TOKENS),
    };
    let past: [fn(&mut AskBudget); 4] = [
        |budget| budget.k = 51,
        |budget| budget.max_tokens = 24_001,
        |budget| budget.search_deadline_ms = 30_001,
        |budget| budget.output_tokens = Some(MAX_CHAT_OUTPUT_TOKENS + 1),
    ];

    assert!(AskBudget::default().is_within_limits());
    assert!(at_bounds.is_within_limits());
    for (index, change) in past.into_iter().enumerate() {
        let mut budget = at_bounds;
        change(&mut budget);
        assert!(!budget.is_within_limits(), "case {index}");
    }
}

/// The outcome of one ask whose answerer card allows `limit` output tokens.
async fn answerer_outcome(model: &str, output_tokens: u32, limit: u32) -> Result<(), String> {
    let scratch = Scratch::new();
    let answerer = scratch.answerer_with_output_limit(limit);
    let mut request = request("How does the service work?");
    request.model = model.to_owned();
    request.budget.output_tokens = Some(output_tokens);
    let port = ScriptedPort::new(&["The service uses verified instructions. [1]"]);
    let evidence = bundle(
        &request.question,
        "en",
        "The service uses verified instructions.",
    );
    answer_bundle(
        &port,
        &request,
        Some(&answerer),
        evidence,
        &PromptVersion::V1.into(),
    )
    .await
    .map(|answer| assert!(answer.refusal.is_none()))
    .map_err(|error| error.to_string())
}

#[tokio::test]
async fn the_answerer_must_match_the_model_and_allow_the_output_budget() {
    assert_eq!(answerer_outcome("qwen3-4b", 100, 100).await, Ok(()));
    assert_eq!(
        answerer_outcome("qwen3-4b", 101, 100).await,
        Err("invalid ask request: output limit exceeds the registered answerer card".to_owned())
    );
    assert_eq!(
        answerer_outcome("other-model", 100, 100).await,
        Err(
            "invalid ask request: registered answerer does not match the requested model entry"
                .to_owned()
        )
    );
}

#[tokio::test]
async fn the_retry_carries_the_rejected_reply_and_its_repair_code() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("How does the service work?");
    let evidence = bundle(
        &request.question,
        "en",
        "The service uses verified instructions.",
    );
    let port = ScriptedPort::new(&["[1]", "The service uses verified instructions. [1]"]);

    answer_bundle(
        &port,
        &request,
        Some(&answerer),
        evidence,
        &PromptVersion::V1.into(),
    )
    .await
    .expect("answer after one repair");

    let calls = port.chat_calls.lock().expect("chat-call lock");
    let (first, retry) = (&calls[0].2.messages, &calls[1].2.messages);
    assert_eq!(retry.len(), first.len() + 2);
    assert_eq!(retry[..first.len()], first[..]);
    assert_eq!(retry[first.len()].speaker, Speaker::Assistant);
    assert_eq!(retry[first.len()].content, "[1]");
    assert_eq!(retry[first.len() + 1].speaker, Speaker::User);
    assert_eq!(
        retry[first.len() + 1].content,
        "Repair the answer once. Validation code: too_short. Follow the original evidence-only \
         instructions."
    );
}

#[tokio::test]
async fn the_answer_language_follows_the_question() {
    for (question, language) in [
        (
            "How does the service work with the documented defaults?",
            "en",
        ),
        (
            "Comment fonctionne le service avec les valeurs par défaut?",
            "fr",
        ),
    ] {
        let request = request(question);
        let answer = answer_bundle(
            &ScriptedPort::new(&[]),
            &request,
            None,
            empty_bundle(question),
            &PromptVersion::V1.into(),
        )
        .await
        .expect("no-evidence refusal");
        assert_eq!(answer.lang, language, "{question}");
    }
}

#[test]
fn ask_errors_render_their_reason_and_keep_their_source() {
    let backend = AskError::Backend(Error::Unavailable {
        reason: "router detail".to_owned(),
    });
    assert_eq!(backend.to_string(), "the answerer is unavailable");
    assert!(backend.source().is_some());
    assert_eq!(
        AskError::TimedOut.to_string(),
        "the answerer exceeded its 20-second deadline"
    );
    assert!(AskError::TimedOut.source().is_none());
}

#[test]
fn the_search_budget_copies_the_ask_budget() {
    let budget = AskBudget {
        k: 7,
        max_tokens: 900,
        search_deadline_ms: 1234,
        output_tokens: Some(5),
    };
    assert_eq!(
        RequestBudget::from(budget),
        RequestBudget {
            k: 7,
            max_tokens: 900,
            deadline_ms: 1234,
        }
    );
}
