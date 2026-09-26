//! The checks of a page that is not the document: an application's error
//! message where content should be, and a sign-in or challenge prompt.
//!
//! Both read the paragraphs and headings at the top of the document's
//! structure, never one quoted, listed or in a table, and compare their
//! text lowercased, each run of characters that are not letters or digits
//! made one space: a page that explains an error, or quotes one, is content.
//! A heading names what its page is about, so an error message as a heading
//! counts only over a body too short to explain it.

use super::{
    body::Body,
    flag::{Flag, counted, number},
};
use maestro_canonicalization::{Block, BlockType, CanonicalDocument};
use maestro_kernel::document::Outcome;

/// The error messages web applications show in place of their content.
const APPLICATION_ERRORS: [&str; 19] = [
    "something went wrong",
    "something went wrong please try again later",
    "oops something went wrong",
    "oops something went wrong please try again later",
    "an error occurred",
    "an error has occurred",
    "an unexpected error occurred",
    "an unexpected error has occurred",
    "internal server error",
    "500 internal server error",
    "service unavailable",
    "503 service unavailable",
    "bad gateway",
    "502 bad gateway",
    "page not found",
    "404 not found",
    "404 page not found",
    "sorry to interrupt",
    "css error",
];

/// The sign-in and challenge prompts a page is when its text is one of them.
const PROMPTS: [&str; 8] = [
    "sign in",
    "sign on",
    "log in",
    "login",
    "access denied",
    "just a moment",
    "attention required",
    "authentication required",
];

/// The prompts a page's text may go on from, after a space.
const PROMPT_OPENINGS: [&str; 9] = [
    "sign in to continue",
    "log in to continue",
    "please sign in",
    "please log in",
    "you must be signed in",
    "checking your browser",
    "verify you are human",
    "please enable javascript",
    "enable javascript and cookies",
];

/// A sign-in or challenge page holds fewer words than this, and so does an
/// error's page under an error message as its heading; a guide about signing
/// in, or a page that explains an error, holds more.
const PAGE_WORDS: u64 = 50;

/// `page.application-error`: a paragraph that is an application's error
/// message, or a heading that is one over a body of fewer than
/// [`PAGE_WORDS`] words.
pub(super) fn application_error(document: &CanonicalDocument, body: &Body) -> Option<Flag> {
    let errors = document
        .blocks
        .iter()
        .filter(|block| on_top(block))
        .filter(|block| block.block_type != BlockType::Heading || body.words < PAGE_WORDS)
        .filter(|block| APPLICATION_ERRORS.contains(&normalized(&block.retrieval_text).as_str()))
        .count();
    (errors > 0).then(|| Flag {
        rule: "page.application-error",
        outcome: Outcome::NeedsReextraction,
        reason: format!(
            "an application's error message stands where content should be, in {} of its \
             paragraphs and headings",
            number(errors)
        ),
    })
}

/// `page.sign-in`: the first heading or the first paragraph is a sign-in or
/// challenge prompt, and the body holds fewer than [`PAGE_WORDS`] words.
pub(super) fn sign_in(document: &CanonicalDocument, body: &Body) -> Option<Flag> {
    if body.words >= PAGE_WORDS {
        return None;
    }
    let first = |kind: BlockType| {
        document
            .blocks
            .iter()
            .find(|block| block.block_type == kind && block.parent_block_id.is_none())
    };
    let prompt = [
        ("first heading", first(BlockType::Heading)),
        ("first paragraph", first(BlockType::Paragraph)),
    ]
    .into_iter()
    .find_map(|(which, block)| {
        block
            .filter(|block| is_prompt(&normalized(&block.retrieval_text)))
            .map(|_| which)
    })?;
    Some(Flag {
        rule: "page.sign-in",
        outcome: Outcome::NeedsReextraction,
        reason: format!(
            "its {prompt} is a sign-in or challenge prompt, and its body holds {}: fewer than \
             {PAGE_WORDS} is the prompt's page, not the document",
            counted(body.words, "word")
        ),
    })
}

/// Whether `block` is a paragraph or a heading at the top of its document's
/// structure.
fn on_top(block: &Block) -> bool {
    block.parent_block_id.is_none()
        && matches!(block.block_type, BlockType::Paragraph | BlockType::Heading)
}

/// Whether the normalized `text` is a sign-in or challenge prompt.
fn is_prompt(text: &str) -> bool {
    PROMPTS.contains(&text)
        || PROMPT_OPENINGS.iter().any(|opening| {
            text.strip_prefix(opening)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
        })
}

/// `text` lowercased, each run of characters that are not letters or digits
/// made one space, and none at either end.
fn normalized(text: &str) -> String {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
