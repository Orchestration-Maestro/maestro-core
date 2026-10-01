//! The checks of what a canonical document records beside its body: its
//! verdict, its provenance, its assets, its tables and the characters of its
//! text, each with a document it flags and one it must not.

use super::support::{PAGE, canonical, document, flag, flags, flags_of, reason, rules};
use maestro_canonicalization::AssetStatus;
use maestro_kernel::document::Outcome;
use std::collections::BTreeMap;

/// A paragraph of about 300 characters, `quote` standing around one word.
fn quoted(quote: char) -> String {
    format!(
        "# Owners\n\nThe {quote}owner{quote} of the files is the account that runs the agent. \
         It must stay the same after an upgrade of the host, or the agent cannot read the \
         files it wrote before, and every job that reads them ends in error until someone \
         gives the files back to the account that runs the agent now.\n"
    )
}

#[test]
fn a_failed_canonical_document_needs_another_extraction() {
    let found = flags("---\ntitle: Another page\n---\n# Page\n\nWhat the page says.\n");
    let failure = flag(&found, "canonicalization.failed");
    assert_eq!(failure.outcome, Outcome::NeedsReextraction);
    assert_eq!(
        failure.reason,
        "canonicalization failed: metadata_conflict: conflicting title; supplied and Markdown \
         values retained"
    );
}

#[test]
fn a_document_valid_with_warnings_did_not_fail() {
    let page = "# Page\n\nA line<br>with raw HTML in it, which the parser keeps as it is.\n";
    assert_eq!(rules(page), [""; 0]);
}

#[test]
fn a_document_without_a_source_reference_or_a_title_is_quarantined() {
    let markdown = "# Page\n\nWhat the page says about the backups of the database.\n";
    let found = flags_of(&canonical(markdown, None, None, BTreeMap::new()), markdown);
    let missing = flag(&found, "metadata.missing");
    assert_eq!(missing.outcome, Outcome::Quarantined);
    assert_eq!(
        missing.reason,
        "it has no source reference and no title: its provenance is unresolved until someone \
         reviews it"
    );
    let untitled = flags_of(
        &canonical(markdown, Some(PAGE), None, BTreeMap::new()),
        markdown,
    );
    assert_eq!(
        flag(&untitled, "metadata.missing").reason,
        "it has no title: its provenance is unresolved until someone reviews it"
    );
}

#[test]
fn an_unknown_language_extraction_or_access_policy_is_not_missing_metadata() {
    let page = "# Page\n\nWhat the page says about the backups of the database.\n";
    let known = document(page);
    assert_eq!(known.source_metadata.language, None);
    assert_eq!(known.access_policy, None);
    assert_eq!(rules(page), [""; 0]);
}

#[test]
fn a_missing_asset_or_one_outside_its_root_is_a_warning() {
    let markdown = "# Flow\n\n![The flow](images/flow.png) and ![The map](../map.png)\n\n\
                    The two pictures show how a job moves from one state to the next.\n";
    let assets = BTreeMap::from([
        ("images/flow.png".to_owned(), AssetStatus::Missing),
        ("../map.png".to_owned(), AssetStatus::OutsideRoot),
    ]);
    let found = flags_of(
        &canonical(markdown, Some(PAGE), Some("Page"), assets),
        markdown,
    );
    let missing = flag(&found, "assets.missing");
    assert_eq!(missing.outcome, Outcome::AcceptedWithWarnings);
    assert_eq!(
        missing.reason,
        "2 assets it refers to: missing, or outside the root it may be read from"
    );
}

#[test]
fn an_asset_nobody_checked_or_a_remote_one_is_not_missing() {
    let page = "# Flow\n\n![The flow](images/flow.png) and \
                ![The map](https://example.org/map.png)\n\n\
                The two pictures show how a job moves from one state to the next.\n";
    assert_eq!(rules(page), [""; 0]);
}

#[test]
fn a_table_row_with_fewer_or_more_cells_than_its_header_is_incomplete() {
    let page = "# Settings\n\n| Name | Default | Meaning |\n| --- | --- | --- |\n\
                | retries | 3 |\n| delay | 10 | seconds | extra |\n| mode | fast | how |\n";
    let found = flags(page);
    let incomplete = flag(&found, "table.incomplete");
    assert_eq!(incomplete.outcome, Outcome::AcceptedWithWarnings);
    assert_eq!(
        incomplete.reason,
        "2 table rows: fewer or more cells than its table's header"
    );
}

#[test]
fn an_empty_cell_or_a_table_without_rows_is_not_incomplete() {
    let page = "# Settings\n\n| Name | Default | Meaning |\n| --- | --- | --- |\n\
                | retries |  | how often a job runs again |\n\n\
                | Date | Who | What |\n| --- | --- | --- |\n";
    assert_eq!(rules(page), [""; 0]);
}

#[test]
fn a_few_replacement_characters_are_a_warning_and_many_are_garbled_text() {
    let found = flags(&quoted('\u{fffd}'));
    let replaced = flag(&found, "text.replacement-characters");
    assert_eq!(replaced.outcome, Outcome::AcceptedWithWarnings);
    assert_eq!(
        replaced.reason,
        "2 replacement characters (U+FFFD): characters its conversion lost"
    );
    let garbled = "# Notes\n\n\u{fffd}\u{fffd}\u{fffd} \u{fffd}\u{fffd}\u{fffd}\u{fffd} files \
                   \u{fffd}\u{fffd} the agent \u{fffd}\u{fffd}\u{fffd}\n";
    let found = flags(garbled);
    let replaced = flag(&found, "text.replacement-characters");
    assert_eq!(replaced.outcome, Outcome::NeedsReextraction);
    assert_eq!(
        replaced.reason,
        "12 replacement characters (U+FFFD) in 36 characters: from 1 in 100, its text is \
         garbled"
    );
}

#[test]
fn text_is_garbled_from_one_replacement_character_in_a_hundred() {
    let outcome = |length: usize| {
        let page = format!("{}\u{fffd}\n", "x".repeat(length - 1));
        flag(&flags(&page), "text.replacement-characters").outcome
    };
    assert_eq!(outcome(100), Outcome::NeedsReextraction);
    assert_eq!(outcome(101), Outcome::AcceptedWithWarnings);
}

#[test]
fn each_character_of_the_text_is_counted_once_and_the_front_matter_not_at_all() {
    let quoted = quoted('\u{fffd}').replace("\n\nThe ", "\n\n> The ");
    assert_eq!(
        reason(&quoted, "text.replacement-characters"),
        "2 replacement characters (U+FFFD): characters its conversion lost",
        "a quote's text is its paragraph's, counted once"
    );
    let page = "---\nnote: lost \u{fffd} here\n---\n# Backups\n\n\
                The backup runs every night at two in the morning.\n";
    assert_eq!(rules(page), [""; 0]);
}

#[test]
fn text_in_other_scripts_holds_no_replacement_character() {
    let page = "# Travel\n\nZürich, São Paulo, Ærøskøbing and 東京 are on the list 🙂: \
                the agents there run the same jobs as the others do.\n";
    assert_eq!(rules(page), [""; 0]);
    assert_eq!(rules(&quoted('"')), [""; 0]);
}

#[test]
fn an_extraction_marker_left_in_the_text_needs_another_extraction() {
    let page = "# Setup\n\nRun the installer as the next block shows.\n\nDOCLINGCODE\n\n\
                Then restart the agent, DOCLINGBREAK and check its status.\n";
    let found = flags(page);
    let artifacts = flag(&found, "text.extraction-artifacts");
    assert_eq!(artifacts.outcome, Outcome::NeedsReextraction);
    assert_eq!(
        artifacts.reason,
        "2 extraction markers left in its text (DOCLINGCODE or DOCLINGBREAK): code or line \
         breaks its extraction set aside and never restored"
    );
}

#[test]
fn a_page_that_names_its_converter_holds_no_extraction_marker() {
    let page = "# Setup\n\nThis page was converted to Markdown with Docling, and its code \
                blocks were restored.\n";
    assert_eq!(rules(page), [""; 0]);
}
