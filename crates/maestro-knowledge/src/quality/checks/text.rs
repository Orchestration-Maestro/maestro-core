//! The checks of the characters of a document's text, the text of its
//! innermost blocks but its front matter: replacement characters, and
//! extraction markers left unrestored.

use super::{
    body::innermost,
    flag::{Flag, counted, number},
};
use maestro_canonicalization::{BlockType, CanonicalDocument};
use maestro_kernel::document::Outcome;

/// The character a conversion puts where it lost one: U+FFFD. Markdown puts
/// it in place of NUL too.
const REPLACEMENT: char = '\u{fffd}';

/// From one replacement character in this many characters, text is garbled.
const GARBLED_ONE_IN: u64 = 100;

/// The markers an extraction leaves where it did not restore what it set
/// aside, as canonicalization's `extraction_artifact` finding names them.
const MARKERS: [&str; 2] = ["DOCLINGCODE", "DOCLINGBREAK"];

/// The text of each innermost block of `document` but its front matter.
fn texts(document: &CanonicalDocument) -> impl Iterator<Item = &str> {
    document
        .blocks
        .iter()
        .filter(|block| innermost(block) && block.block_type != BlockType::Metadata)
        .map(|block| block.retrieval_text.as_str())
}

/// `text.replacement-characters`: U+FFFD in the text, a warning, since the
/// rest still reads; from one in [`GARBLED_ONE_IN`] characters, the text is
/// garbled and needs another extraction. Text in any script, accents and
/// emoji included, holds none.
pub(super) fn replacement_characters(document: &CanonicalDocument) -> Option<Flag> {
    let (replaced, characters) = texts(document).fold((0, 0), |(replaced, characters), text| {
        let found = text.chars().filter(|character| *character == REPLACEMENT);
        (
            replaced + number(found.count()),
            characters + number(text.chars().count()),
        )
    });
    if replaced == 0 {
        return None;
    }
    let found = format!("{} (U+FFFD)", counted(replaced, "replacement character"));
    Some(if replaced * GARBLED_ONE_IN >= characters {
        Flag {
            rule: "text.replacement-characters",
            outcome: Outcome::NeedsReextraction,
            reason: format!(
                "{found} in {}: from 1 in {GARBLED_ONE_IN}, its text is garbled",
                counted(characters, "character")
            ),
        }
    } else {
        Flag {
            rule: "text.replacement-characters",
            outcome: Outcome::AcceptedWithWarnings,
            reason: format!("{found}: characters its conversion lost"),
        }
    })
}

/// `text.extraction-artifacts`: an extraction marker left in the text, where
/// code or a line break was set aside and never restored. A page that names
/// its converter holds none.
pub(super) fn extraction_artifacts(document: &CanonicalDocument) -> Option<Flag> {
    let markers: u64 = texts(document)
        .map(|text| {
            MARKERS
                .iter()
                .map(|marker| number(text.matches(marker).count()))
                .sum::<u64>()
        })
        .sum();
    (markers > 0).then(|| Flag {
        rule: "text.extraction-artifacts",
        outcome: Outcome::NeedsReextraction,
        reason: format!(
            "{} left in its text (DOCLINGCODE or DOCLINGBREAK): code or line breaks its \
             extraction set aside and never restored",
            counted(markers, "extraction marker")
        ),
    })
}
