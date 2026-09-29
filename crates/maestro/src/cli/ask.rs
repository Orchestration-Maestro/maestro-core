//! CLI adapter for evidence-grounded knowledge answers.

use super::output::Output;
use crate::{
    failure::Failure,
    kernel::Kernel,
    knowledge::operations::{KnowledgeError, ask::run::ask_with, ensure_current_scopes},
    settings::KnowledgeSettings,
};
use maestro_kernel::evidence::RouteStatus;
use maestro_knowledge::answer::{Answer, AskRequest, Rejection};
use serde::Serialize;
use std::{collections::BTreeMap, process::ExitCode};

/// Runs one ask under `settings` and prints the same versioned answer as
/// the MCP tool; with `explain`, it also prints on stderr the prompt's
/// presentation, each search route's status and why each rejected attempt
/// failed.
pub(super) fn run(
    output: Output,
    request: &AskRequest,
    explain: bool,
    settings: &KnowledgeSettings,
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
) -> Result<ExitCode, Failure> {
    let mut scoped = match ask_with(open_kernel, request, settings) {
        Ok(scoped) => scoped,
        Err(error) => return refusal(output, ASK_ERROR, error),
    };
    if let Err(error) = ensure_current_scopes(&mut scoped.kernel, &scoped.scopes) {
        return refusal(output, ASK_ERROR, error);
    }
    if explain {
        eprintln!(
            "explain: prompt {}",
            settings.prompt.presentation().identity()
        );
        eprint!(
            "{}",
            explanation(
                &scoped.data.routes,
                scoped.data.reply_cap,
                &scoped.data.rejections
            )
        );
    }
    let text = answer_text(&scoped.data)?;
    output.result(&scoped.data, &text)?;
    Ok(ExitCode::SUCCESS)
}

/// The schema of an ask's error document.
const ASK_ERROR: &str = "maestro-cli/knowledge-ask-error/1";

/// Prints one safe error document of `schema` and applies the CLI's refusal
/// exit mapping; prepare and publish refuse this way too.
pub(super) fn refusal(
    output: Output,
    schema: &str,
    error: KnowledgeError,
) -> Result<ExitCode, Failure> {
    let (code, message, exit) = match error {
        KnowledgeError::Refused { code, message } => (code, message, ExitCode::from(2)),
        KnowledgeError::Failed { code, message } => (code, message, ExitCode::from(1)),
    };
    output.refusal(
        &AskErrorEnvelope {
            schema,
            error: AskErrorBody { code, message },
        },
        message,
    )?;
    Ok(exit)
}

/// The bounded public shape for an ask input or execution failure.
#[derive(Debug, Serialize)]
struct AskErrorEnvelope<'a> {
    /// Versioned CLI error contract.
    schema: &'a str,
    /// Privacy-safe error code and reason.
    error: AskErrorBody,
}

/// Public failure fields, never an error-chain body.
#[derive(Debug, Serialize)]
struct AskErrorBody {
    /// Stable public error code.
    code: &'static str,
    /// Safe explanation.
    message: &'static str,
}

/// Prints a checked answer with only its host-resolved citation metadata.
fn answer_text(answer: &Answer) -> Result<String, Failure> {
    use std::fmt::Write as _;

    let Some(refusal) = &answer.refusal else {
        let mut text = answer.answer.clone();
        for citation in &answer.citations {
            write!(
                text,
                "\n\n[{}] {} — {}",
                citation.n, citation.title, citation.source_ref
            )
            .map_err(|_| Failure::failed("answer text could not be formatted"))?;
        }
        return Ok(text);
    };
    Ok(refusal.message.clone())
}

/// Each search route's status, so a degraded ask is visible; the reply cap
/// the chat calls ran with, when any ran; then one line per rejected
/// attempt: its failed check and offending tokens.
fn explanation(
    routes: &BTreeMap<String, RouteStatus>,
    reply_cap: Option<u32>,
    rejections: &[Rejection],
) -> String {
    use std::fmt::Write as _;

    let mut text = String::new();
    for (route, status) in routes {
        // Writing to a String cannot fail.
        let _written = match status {
            RouteStatus::Ok => writeln!(text, "explain: route {route} ok"),
            RouteStatus::Unavailable(reason) => {
                writeln!(text, "explain: route {route} unavailable: {reason}")
            }
        };
    }
    if let Some(tokens) = reply_cap {
        let _written = writeln!(text, "explain: reply cap {tokens} tokens");
    }
    if rejections.is_empty() {
        text.push_str("explain: no attempt was rejected\n");
        return text;
    }
    for rejection in rejections {
        let tokens: Vec<String> = rejection
            .tokens
            .iter()
            .map(|token| format!("{token:?}"))
            .collect();
        // Writing to a String cannot fail.
        let _written = writeln!(
            text,
            "explain: attempt {} failed {}: {}",
            rejection.attempt,
            rejection.check,
            tokens.join(" ")
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{answer_text, explanation};
    use maestro_kernel::evidence::RouteStatus;
    use maestro_knowledge::answer::{
        Answer, AnswerCitation, AnswerModel, AnswerRefusal, RefusalCode, Rejection,
    };
    use std::collections::BTreeMap;

    #[test]
    fn answer_text_lists_each_citation_or_prints_only_the_refusal() {
        let mut answer = Answer {
            schema: "maestro-answer/1".to_owned(),
            collection: "docs".to_owned(),
            generation: 1,
            question: "How is it configured?".to_owned(),
            lang: "en".to_owned(),
            answer: "Use the documented defaults [1].".to_owned(),
            citations: vec![AnswerCitation {
                n: 1,
                chunk_id: "chunk".to_owned(),
                section_id: None,
                source_ref: "https://example.org/docs".to_owned(),
                title: "Defaults".to_owned(),
                section_path: Vec::new(),
                span: [0, 1],
            }],
            model: AnswerModel {
                router_entry: "qwen3-4b".to_owned(),
                card_id: None,
            },
            uncalibrated: true,
            refusal: None,
            closest: Vec::new(),
            rejections: Vec::new(),
            routes: BTreeMap::new(),
            delivered: Vec::new(),
            reply_cap: None,
        };
        assert_eq!(
            answer_text(&answer).ok().as_deref(),
            Some("Use the documented defaults [1].\n\n[1] Defaults — https://example.org/docs")
        );
        answer.refusal = Some(AnswerRefusal {
            code: RefusalCode::NotFound,
            message: "No passage answers it.".to_owned(),
        });
        assert_eq!(
            answer_text(&answer).ok().as_deref(),
            Some("No passage answers it.")
        );
    }

    #[test]
    fn explanation_names_the_reply_cap_then_each_rejected_attempt_check_and_tokens() {
        let routes = BTreeMap::new();
        assert_eq!(
            explanation(&routes, None, &[]),
            "explain: no attempt was rejected\n"
        );
        assert_eq!(
            explanation(&routes, Some(2048), &[]),
            "explain: reply cap 2048 tokens\nexplain: no attempt was rejected\n"
        );
        assert_eq!(
            explanation(
                &routes,
                None,
                &[
                    Rejection {
                        attempt: 1,
                        check: "unsupported_literal",
                        tokens: vec!["-FORCEALL".to_owned(), "EM_HOME".to_owned()],
                    },
                    Rejection {
                        attempt: 2,
                        check: "too_short",
                        tokens: Vec::new(),
                    },
                ]
            ),
            "explain: attempt 1 failed unsupported_literal: \"-FORCEALL\" \"EM_HOME\"\n\
             explain: attempt 2 failed too_short: \n"
        );
    }

    #[test]
    fn explanation_names_each_route_status_first() {
        let routes = BTreeMap::from([
            ("identifier".to_owned(), RouteStatus::Ok),
            (
                "rerank".to_owned(),
                RouteStatus::Unavailable("disabled_by_configuration".to_owned()),
            ),
        ]);
        assert_eq!(
            explanation(&routes, Some(1024), &[]),
            "explain: route identifier ok\n\
             explain: route rerank unavailable: disabled_by_configuration\n\
             explain: reply cap 1024 tokens\n\
             explain: no attempt was rejected\n"
        );
    }
}
