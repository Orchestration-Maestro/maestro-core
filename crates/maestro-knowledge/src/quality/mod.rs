//! The quality gate (docs/architecture/01 §4, plan D6, FR-S1-002a): every
//! revision of a collection receives one explicit disposition before it may
//! be indexed. A document is eligible, for chunking first, by its latest
//! revision in record order alone, when that revision is not failed and is
//! `accepted` or `accepted_with_warnings` ([`eligible`]): an older revision
//! never stands in for it.
//!
//! [`gate`](fn@gate) reads every revision of a collection its caller reads,
//! failed ones included, in record order, and decides each one without a
//! disposition, in this order of precedence:
//!
//! 1. A disposition recorded before is kept, whoever decided it: the
//!    import's quarantine of the lines that give one `source_ref` different
//!    bytes, a person's, or an earlier run's. A rule or a check changed since
//!    decides only the revisions not yet decided.
//! 2. The first rule of the collection's quality ledger ([`Ledger`]) that
//!    matches the revision decides it, with the rule's reason and author:
//!    a person's decision outranks every automatic check. A failed canonical
//!    document is never indexed (01 §5, §13), so a rule that would accept one
//!    is set aside, and the checks decide it, saying so.
//! 3. The automatic checks, on the revision's canonical document and, for
//!    secrets, its original Markdown: the most severe outcome among the
//!    checks that flag it decides, and every one of them is named with its
//!    reason, the most severe first. From the most severe: `excluded`,
//!    `quarantined`, `needs_reextraction`, `accepted_with_warnings`. A
//!    revision no check flags is `accepted`.
//!
//! What the checks decide names the gate, `quality-gate/1`, as its decider.
//! A disposition that holds a revision back is journaled as
//! `maestro.knowledge.revision.held.v1` in the write that records it.
//!
//! The automatic checks: each of 01 §4's that a canonical document can show,
//! and 01 §3's scan for secrets, over the original Markdown, front matter and
//! code included. A word is a run of characters between whitespace that
//! holds a letter or a digit; the body is every innermost block but headings,
//! front matter, thematic breaks and link reference definitions. Every reason
//! counts and names, and never quotes the document.
//!
//! - `canonicalization.failed`, `needs_reextraction`: canonicalization
//!   found an error, so the canonical document is failed.
//! - `body.near-empty`, `needs_reextraction`: no code block, list item or
//!   table cell holds a word outside a link label, and the rest of the body
//!   holds fewer than 8.
//! - `body.navigation-heavy`, `needs_reextraction`: link labels are 90 % of
//!   the body's words or more, over 10 links or more.
//! - `text.replacement-characters`, `accepted_with_warnings`: U+FFFD in the
//!   text, which Markdown also puts for NUL; from 1 in 100 characters, the
//!   text is garbled, and `needs_reextraction`.
//! - `text.extraction-artifacts`, `needs_reextraction`: an extraction marker
//!   left unrestored, `DOCLINGCODE` or `DOCLINGBREAK`.
//! - `text.suspected-secret`, `quarantined`: a credential in a format whose
//!   shape alone identifies it: a PEM private key block, an AWS access key
//!   ID, or a GitHub, GitLab or Slack token. Its reason names each format and
//!   the lines it is on.
//! - `table.incomplete`, `accepted_with_warnings`: a table row with fewer or
//!   more cells than its table's header.
//! - `page.application-error`, `needs_reextraction`: a paragraph or heading,
//!   outside quotes, lists and tables, that is an application's error
//!   message.
//! - `page.sign-in`, `needs_reextraction`: the first heading or paragraph is
//!   a sign-in or challenge prompt, and the body holds fewer than 50 words.
//! - `metadata.missing`, `quarantined`: no source reference or no title, so
//!   the provenance is unresolved.
//! - `assets.missing`, `accepted_with_warnings`: an asset it refers to is
//!   missing, or outside the root it may be read from.
//!
//! The checks are format-aware. A list item, a code block or a table cell is
//! an entry on its own, so a short changelog or a two-line configuration
//! file is not near-empty; an index that says what each link holds is not
//! navigation; an error a page explains, quotes or shows in code is not an
//! application's error; a guide about signing in is not a sign-in page; a
//! cell written empty, or a table without rows, is not incomplete; text in
//! any script is not garbled; an unknown language, extraction or access
//! policy, as an import's, is not missing metadata; an asset nobody checked,
//! as an import's, is unknown rather than missing; and a documentation
//! placeholder, such as `password: <your password>` or a token of `x`s, is
//! not a secret.
//!
//! One check of 01 §4 has nothing to read here: a fidelity-receipt loss. A
//! receipt is the extractor's, and the corpora S1 imports carry none; the
//! check arrives with the extractors of S6.

mod checks;
mod decide;
mod error;
mod gate;
mod ledger;
mod outcome;
mod report;
#[cfg(test)]
mod tests;

pub use error::Error;
pub use gate::{eligible, gate};
pub use ledger::{Ledger, LedgerError, Match, Rule, Schema};
pub use report::{Held, Outcomes, Report};
