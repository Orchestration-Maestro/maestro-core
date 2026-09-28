//! CLI adapter for evidence-grounded knowledge answers.

use super::output::Output;
use crate::{
    failure::Failure,
    kernel::Kernel,
    knowledge::operations::{KnowledgeError, ask::run::ask_with, ensure_current_scopes},
};
use maestro_knowledge::answer::{Answer, AskRequest};
use serde::Serialize;
use std::process::ExitCode;

/// Runs one ask and prints the same versioned answer as the MCP tool.
pub(super) fn run(
    output: Output,
    request: &AskRequest,
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
) -> Result<ExitCode, Failure> {
    let mut scoped = match ask_with(open_kernel, request) {
        Ok(scoped) => scoped,
        Err(error) => return operation_refusal(output, error),
    };
    if let Err(error) = ensure_current_scopes(&mut scoped.kernel, &scoped.scopes) {
        return operation_refusal(output, error);
    }
    let text = answer_text(&scoped.data)?;
    output.result(&scoped.data, &text)?;
    Ok(ExitCode::SUCCESS)
}

/// Prints one safe error document and applies the CLI's refusal exit mapping.
fn operation_refusal(output: Output, error: KnowledgeError) -> Result<ExitCode, Failure> {
    let (code, message, exit) = match error {
        KnowledgeError::Refused { code, message } => (code, message, ExitCode::from(2)),
        KnowledgeError::Failed { code, message } => (code, message, ExitCode::from(1)),
    };
    output.refusal(
        &AskErrorEnvelope {
            schema: "maestro-cli/knowledge-ask-error/1",
            error: AskErrorBody { code, message },
        },
        message,
    )?;
    Ok(exit)
}

/// The bounded public shape for an ask input or execution failure.
#[derive(Debug, Serialize)]
struct AskErrorEnvelope {
    /// Versioned CLI error contract.
    schema: &'static str,
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
