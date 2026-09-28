//! Evidence-grounded answering with bounded generation and one validation retry.

/// Search, assemble, generate and render one evidence-grounded answer.
mod generate;
/// Build the bounded system and evidence messages sent to the answerer.
mod prompt;
#[cfg(test)]
mod tests;
/// Public request, response and error contracts for answering.
mod types;
/// Check citations, language and supported literals in a buffered reply.
mod validate;

pub use generate::{ask, ask_configured};
pub use types::{
    Answer, AnswerCitation, AnswerContext, AnswerModel, AnswerPrompt, AnswerRefusal, AskBudget,
    AskError, AskRequest, CHAT_DEADLINE, DATA_SLOT, DEFAULT_MODEL, PromptText, PromptVersion,
    RefusalCode, RegisteredAnswerer, Rejection,
};
