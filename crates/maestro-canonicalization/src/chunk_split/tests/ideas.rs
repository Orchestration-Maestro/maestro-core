//! The complete-ideas profile: page chrome left out of the indexed text, and a section's blocks
//! packed together, a chunk ending only between whole steps and rows.
use super::*;
use crate::source_units::OriginMode;
use std::fmt::Write as _;

/// A document's chunks under the complete-ideas profile, the fake counter counting: each replays,
/// and together they cover every unit's kept text exactly once.
pub(super) fn idea_chunks(markdown: &str) -> Result<(MappedDocument, Vec<ChunkContent>), Error> {
    let doc = canonicalize(CanonicalizeInput::new(markdown, "ideas-test"))?;
    let mapped = map_document(&doc, markdown)?;
    let layout = Layout::new(&doc, markdown, &mapped, ChunkProfile::CompleteIdeas)?;
    let mut fake_counter = |text: &str| Ok(text.chars().count() + 2);
    let chunks = build_drafts(&layout, &mut fake_counter)?;
    validate_preparation(&layout, &chunks, &mut fake_counter)?;
    for (index, _) in mapped
        .units
        .iter()
        .enumerate()
        .filter(|(_, unit)| unit.primary)
    {
        let mut ranges: Vec<_> = chunks
            .iter()
            .flat_map(|chunk| &chunk.fragments)
            .filter(|fragment| fragment.contribution.unit_index == index)
            .map(|fragment| fragment.contribution.range)
            .collect();
        ranges.sort_by_key(|range| range.start);
        let kept = layout.kept(index);
        let mut cursor = kept.map_or(0, |kept| kept.start);
        for range in ranges {
            assert_eq!(range.start, cursor, "body gap or overlap");
            cursor = range.end;
        }
        assert_eq!(cursor, kept.map_or(0, |kept| kept.end), "missing kept text");
    }
    Ok((mapped, chunks))
}

/// The prepared inputs of a document's chunks under the complete-ideas profile.
pub(super) fn prepared(markdown: &str) -> Vec<String> {
    let (_, chunks) = idea_chunks(markdown).unwrap();
    chunks
        .into_iter()
        .map(|chunk| chunk.prepared_input)
        .collect()
}

#[test]
fn an_introduction_shares_a_chunk_with_its_steps() {
    let markdown = "# Restarting a Service\n\nThis procedure restarts a service.\n\nBegin\n\n\
                    1. Select the service.\n2. Click **Restart**.\n";
    assert_eq!(
        prepared(markdown),
        [
            "Restarting a Service\n\nThis procedure restarts a service.\n\nBegin\n\
          1. Select the service.\n2. Click Restart."
        ]
    );
    // The structural profile keeps the introduction and the steps apart.
    assert_eq!(structural_chunks(markdown).unwrap().1.len(), 2);
}

#[test]
fn page_chrome_leaves_the_indexed_text_and_the_rest_keeps_exact_source_spans() {
    let markdown = "# Service Tasks\n\n## Restarting a Service Link copied to clipboard\n\n\
                    Restart the service.\n\nClosed\n\n<!-- image -->\n\n<a id=\"step\"></a>\n\n\
                    Copy Copied to clipboard\n\nClosed doors stay closed.\n";
    let (_, chunks) = idea_chunks(markdown).unwrap();
    let texts: Vec<_> = chunks
        .iter()
        .map(|chunk| chunk.prepared_input.as_str())
        .collect();
    assert_eq!(
        texts,
        [
            "Service Tasks",
            "Service Tasks\n\nRestarting a Service\n\nRestart the service.\n\n\
             Closed doors stay closed."
        ]
    );
    assert_eq!(
        chunks[1].heading_path,
        ["Service Tasks", "Restarting a Service"]
    );
    let heading = chunks[1]
        .input_parts
        .iter()
        .find(|part| part.text == "Restarting a Service")
        .unwrap();
    let origin = &heading.mappings[0].origins[0];
    assert_eq!(
        &markdown[origin.span.start..origin.span.end],
        "Restarting a Service"
    );
    for part in chunks.iter().flat_map(|chunk| &chunk.input_parts) {
        for run in part
            .mappings
            .iter()
            .filter(|run| run.mode == OriginMode::ExactCopy)
        {
            let span = run.origins[0].span;
            assert_eq!(
                &markdown[span.start..span.end],
                &part.text[run.range.start..run.range.end]
            );
        }
    }
}

#[test]
fn a_heading_label_is_left_out_of_every_continuation_context() {
    let markdown = format!(
        "# Settings Link copied to clipboard\n\n{}\n\n{}\n",
        "a".repeat(600),
        "b".repeat(600)
    );
    let texts = prepared(&markdown);
    assert_eq!(texts.len(), 2);
    assert!(texts[0].starts_with("Settings\n\naaa"));
    assert!(texts[1].starts_with("Settings\n\nbbb"));
    assert!(texts.iter().all(|text| !text.contains("clipboard")));
}

#[test]
fn a_table_shares_a_chunk_with_its_caption_and_code_before_it() {
    let markdown = "# Delay Setting\n\nThe example sets a delay.\n\n\
                    ```\n{\"Delay\": \"5\"}\n```\n\nWhere:\n\n\
                    | Parameter | Description |\n|---|---|\n| Delay | Minutes to wait. |\n";
    assert_eq!(
        prepared(markdown),
        [
            "Delay Setting\n\nThe example sets a delay.\n\n{\"Delay\": \"5\"}\n\n\nWhere:\n\n\
          Parameter\tDescription\nDelay\tMinutes to wait."
        ]
    );
    assert_eq!(structural_chunks(markdown).unwrap().1.len(), 4);
}

#[test]
fn later_rows_of_a_long_table_repeat_its_heading_and_header_and_no_row_is_split() {
    let mut rows = String::new();
    for row in 0..12 {
        writeln!(rows, "| key{row} | {} |", "v".repeat(80)).unwrap();
    }
    let markdown = format!("# Keys\n\nThe keys:\n\n| Key | Value |\n|---|---|\n{rows}");
    let (_, chunks) = idea_chunks(&markdown).unwrap();
    assert!(chunks.len() > 1);
    assert!(
        chunks[0]
            .prepared_input
            .starts_with("Keys\n\nThe keys:\n\nKey\tValue\n")
    );
    for chunk in &chunks[1..] {
        assert!(chunk.prepared_input.starts_with("Keys\nKey\tValue\n\nkey"));
    }
    for row in 0..12 {
        let holding: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.body_text.contains(&format!("key{row}\t")))
            .collect();
        assert_eq!(holding.len(), 1, "row {row}");
        assert!(
            holding[0]
                .body_text
                .contains(&format!("key{row}\t{}", "v".repeat(80)))
        );
    }
}

#[test]
fn a_step_stays_with_its_substeps_where_the_target_would_have_ended_the_chunk() {
    let markdown = format!(
        "{}\n\n1. Step one text.\n   - Sub one.\n   - Sub two.\n",
        "a".repeat(490)
    );
    let texts = prepared(&markdown);
    assert_eq!(texts.len(), 1);
    assert!(texts[0].ends_with("1. Step one text.\n  - Sub one.\n  - Sub two."));
    assert_eq!(structural_chunks(&markdown).unwrap().1.len(), 3);
}

#[test]
fn an_oversized_step_continues_under_its_parent_between_whole_substeps() {
    let mut substeps = String::new();
    for sub in 0..6 {
        writeln!(substeps, "   - Sub {sub} {}", "s".repeat(150)).unwrap();
    }
    let markdown = format!("1. Parent step.\n{substeps}");
    let (_, chunks) = idea_chunks(&markdown).unwrap();
    assert!(chunks.len() > 1);
    for chunk in &chunks[1..] {
        assert!(chunk.prepared_input.starts_with("1. Parent step."));
    }
    for sub in 0..6 {
        let whole = format!("Sub {sub} {}", "s".repeat(150));
        assert_eq!(
            chunks
                .iter()
                .filter(|chunk| chunk.body_text.contains(&whole))
                .count(),
            1,
            "sub-step {sub}"
        );
    }
}

#[test]
fn a_small_chunk_joins_the_chunk_before_it_in_its_section() {
    let markdown = format!(
        "{}\n\nSee the guide for more.\n\nThe settings apply at once.\n",
        "a".repeat(490)
    );
    assert_eq!(prepared(&markdown).len(), 1);
    assert_eq!(structural_chunks(&markdown).unwrap().1.len(), 2);
}

#[test]
fn sections_never_share_a_chunk() {
    let (_, chunks) = idea_chunks("# One\n\na\n\n# Two\n\nb\n").unwrap();
    assert_eq!(chunks.len(), 2);
    assert_ne!(chunks[0].section_id, chunks[1].section_id);
}

#[test]
fn identifiers_versions_and_accents_stay_verbatim() {
    let markdown = "# Réglages de relay_x\n\nVersion 4.2.17 : exécuter `relay-x send batch`.\n";
    assert_eq!(
        prepared(markdown),
        ["Réglages de relay_x\n\nVersion 4.2.17 : exécuter relay-x send batch."]
    );
}

#[test]
fn a_mixed_chunk_replays_only_under_its_own_profile() {
    let markdown = "Where:\n\n| A | B |\n|---|---|\n| one | two |\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "mixed")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let ideas = Layout::new(&doc, markdown, &mapped, ChunkProfile::CompleteIdeas).unwrap();
    let chunks = build_drafts(&ideas, &mut |text: &str| Ok(text.chars().count() + 2)).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].prepared_input, "Where:\n\nA\tB\none\ttwo");
    let mut count = |text: &str| Ok(text.chars().count() + 2);
    validate_preparation(&ideas, &chunks, &mut count).unwrap();
    let structural = structural(&doc, markdown, &mapped);
    assert!(validate_preparation(&structural, &chunks, &mut count).is_err());
}

#[test]
fn a_heading_whose_last_unit_is_the_label_keeps_its_other_text() {
    let markdown = format!(
        "# **Bold** Link copied to clipboard\n\n{}\n\n{}\n",
        "a".repeat(600),
        "b".repeat(600)
    );
    let texts = prepared(&markdown);
    assert_eq!(texts.len(), 2);
    assert!(texts[0].starts_with("Bold\n\naaa"), "{}", &texts[0][..20]);
    assert!(texts[1].starts_with("Bold\n\nbbb"), "{}", &texts[1][..20]);
}

#[test]
fn a_heading_that_is_only_the_label_or_holds_it_inside_stays_whole() {
    assert_eq!(
        prepared("# Link copied to clipboard\n\nBody.\n"),
        ["Link copied to clipboard\n\nBody."]
    );
    assert_eq!(
        prepared("# Copy Link copied to clipboard here\n\nBody.\n"),
        ["Copy Link copied to clipboard here\n\nBody."]
    );
}

#[test]
fn a_paragraph_after_a_table_shares_its_chunk() {
    let markdown = "# Keys\n\n| Key | Value |\n|---|---|\n| one | 1 |\n\nSee the guide.\n";
    assert_eq!(
        prepared(markdown),
        ["Keys\n\nKey\tValue\none\t1\n\nSee the guide."]
    );
}

#[test]
fn a_small_chunk_of_exactly_the_minimum_stays_apart() {
    // The first paragraph fills a draft to the target, alone; the second makes a chunk of 150
    // tokens with its separators and counted specials, and 149 joins the first.
    for (length, chunks) in [(148, 2), (147, 1)] {
        let markdown = format!("{}\n\n{}\n", "a".repeat(510), "b".repeat(length));
        assert_eq!(prepared(&markdown).len(), chunks, "length {length}");
    }
}

#[test]
fn a_step_that_fits_alone_starts_a_chunk_rather_than_splitting_to_fill_one() {
    let markdown = format!(
        "{}\n\n1. {}\n   - {}\n   - {}\n",
        "a".repeat(300),
        "s".repeat(100),
        "t".repeat(150),
        "u".repeat(150)
    );
    let texts = prepared(&markdown);
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0], "a".repeat(300));
    assert!(texts[1].starts_with(&format!("1. {}", "s".repeat(100))));
}

#[test]
fn an_oversized_steps_first_atoms_join_the_introduction_before_it() {
    let mut markdown = format!("{}\n\n1. Parent step.\n", "a".repeat(100));
    for sub in 0..6 {
        writeln!(markdown, "   - Sub {sub} {}", "s".repeat(150)).unwrap();
    }
    let texts = prepared(&markdown);
    assert!(texts.len() > 1);
    assert!(texts[0].starts_with(&format!("{}\n1. Parent step.\n  - Sub 0", "a".repeat(100))));
}

#[test]
fn the_steps_of_a_long_list_each_stay_whole() {
    let mut markdown = String::new();
    for step in 1..=5 {
        writeln!(markdown, "{step}. Step {step} {}", "x".repeat(40)).unwrap();
        for sub in 0..2 {
            writeln!(markdown, "   - Sub {step}.{sub} {}", "y".repeat(50)).unwrap();
        }
    }
    let (_, chunks) = idea_chunks(&markdown).unwrap();
    assert!(chunks.len() > 1);
    for step in 1..=5 {
        // Bodies only: a continuation repeats its parent step as context.
        let holding: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.body_text.contains(&format!("Step {step} ")))
            .collect();
        assert_eq!(holding.len(), 1, "step {step}");
        for sub in 0..2 {
            assert!(
                holding[0].body_text.contains(&format!("Sub {step}.{sub} ")),
                "step {step}, substep {sub}"
            );
        }
    }
}

#[test]
fn a_step_of_exactly_the_maximum_after_an_introduction_is_its_own_chunk_whole() {
    let step = |length: usize| {
        format!(
            "1. Step {}\n   - Sub {}\n",
            "s".repeat(length),
            "t".repeat(40)
        )
    };
    let alone = idea_chunks(&step(STEP_OF_MAXIMUM)).unwrap().1;
    assert_eq!(
        alone
            .iter()
            .map(|chunk| chunk.token_count)
            .collect::<Vec<_>>(),
        [MAX_TOKENS]
    );
    let markdown = format!("{}\n\n{}", "a".repeat(100), step(STEP_OF_MAXIMUM));
    let texts = prepared(&markdown);
    assert_eq!(texts, ["a".repeat(100), alone[0].prepared_input.clone()]);
}

/// The length of the step's filler that makes it exactly the maximum alone, counted by the fake
/// counter.
const STEP_OF_MAXIMUM: usize = 641;
