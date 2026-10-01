//! Regression checks for source offsets used by diversity signatures.

use super::super::sections::SectionIndex;
use super::{DiversityFeatures, diversity_features, diversity_similarity};
use maestro_canonicalization::{CanonicalizeInput, canonicalize};

fn section_features(markdown: &str) -> Result<DiversityFeatures, String> {
    let document = canonicalize(CanonicalizeInput::new(markdown, "fixture.md"))
        .map_err(|error| error.to_string())?;
    let guide = document
        .sections
        .iter()
        .find(|section| section.title == "Guide")
        .ok_or_else(|| "fixture has no Guide section".to_owned())?;
    let extent = SectionIndex::new(&document, markdown)?
        .section_extent(&guide.section_id)
        .ok_or_else(|| "Guide section has no extent".to_owned())?;
    diversity_features(markdown, &document, extent, None)
}

#[test]
fn identical_section_signatures_ignore_their_absolute_source_offsets() -> Result<(), String> {
    let section = "## Guide\n\nUse 7005 then `tool`.\n";
    let shifted = format!("## Intro\n\nLead text.\n\n{section}");

    assert!(diversity_similarity(&section_features(section)?, &section_features(&shifted)?)? > 0.0);
    Ok(())
}
