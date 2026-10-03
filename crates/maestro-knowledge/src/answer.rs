//! Evidence-grounded answering with bounded generation and one validation retry.

/// Search, assemble, generate and render one evidence-grounded answer.
mod generate;
/// The session's language and tone of an answer.
mod presentation;
/// Build the bounded system and evidence messages sent to the answerer.
mod prompt;
#[cfg(test)]
pub(crate) mod tests;
/// Public request, response and error contracts for answering.
mod types;
/// Check citations and supported literals in a buffered reply.
mod validate;

pub use generate::{ask, ask_configured};
pub use presentation::{PRESENTATION_VERSION, Presentation, Tone};
pub use types::{
    Answer, AnswerCitation, AnswerContext, AnswerModel, AnswerPrompt, AnswerRefusal, AskBudget,
    AskError, AskRequest, CHAT_DEADLINE, DATA_SLOT, DEFAULT_MODEL, LanguageCheck, PromptText,
    PromptVersion, RefusalCode, RegisteredAnswerer, Rejection,
};
