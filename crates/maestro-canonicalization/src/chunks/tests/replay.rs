//! Replay validation: coverage and prepared parts must rebuild from the mapped source.
use super::*;
use crate::chunk_mapping::map_document;
use crate::chunk_split::{MAX_TOKENS, build_drafts};
use std::mem;

#[test]
fn primary_ranges_not_broad_origins_prove_coverage() {
    let markdown = "`alpha beta`\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "coverage")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let drafts = build_drafts(&doc, markdown, &mapped, &mut |input| Ok(fake_count(input))).unwrap();
    assert_eq!(validate_coverage(&mapped, &drafts).unwrap().len(), 1);
    assert_eq!(
        mapped.units[0].mappings[0].mode,
        OriginMode::CanonicalTransformation
    );
    let mut missing = drafts.clone();
    missing[0].fragments.clear();
    assert!(validate_coverage(&mapped, &missing).is_err());
    let mut overlap = drafts.clone();
    let repeated = overlap[0].fragments[0].clone();
    overlap[0].fragments.push(repeated);
    assert!(validate_coverage(&mapped, &overlap).is_err());
    let mut clipped = drafts.clone();
    clipped[0].fragments[0].contribution.range.end = 1;
    assert!(validate_coverage(&mapped, &clipped).is_err());
}

#[test]
fn prepared_parts_counts_and_table_header_associations_are_validated() {
    let markdown = format!(
        "| Name | Value |\n|---|---|\n| alpha | {} |\n",
        "x".repeat(1500)
    );
    let doc = canonicalize(CanonicalizeInput::new(&markdown, "parts")).unwrap();
    let mapped = map_document(&doc, &markdown).unwrap();
    let drafts =
        build_drafts(&doc, &markdown, &mapped, &mut |input| Ok(fake_count(input))).unwrap();
    validate_chunks(&doc, &markdown, &mapped, &drafts, &mut |input| {
        Ok(fake_count(input))
    })
    .unwrap();
    let target = drafts
        .iter()
        .position(|chunk| {
            chunk
                .input_parts
                .iter()
                .any(|part| part.role == InputRole::TableHeaderContext)
        })
        .unwrap();
    let mut broken = drafts.clone();
    broken[target].input_parts[0].text.push('!');
    assert!(
        validate_chunks(&doc, &markdown, &mapped, &broken, &mut |input| Ok(
            fake_count(input)
        ))
        .is_err()
    );
    let mut broken = drafts.clone();
    for chunk in &mut broken {
        chunk.token_count = 701;
    }
    assert!(validate_chunks(&doc, &markdown, &mapped, &broken, &mut |_| Ok(701)).is_err());
    let mut missing_ledger = mapped.clone();
    missing_ledger.accounting.pop();
    assert!(
        validate_chunks(&doc, &markdown, &missing_ledger, &drafts, &mut |input| Ok(
            fake_count(input)
        ))
        .is_err()
    );
    let mut broken = drafts.clone();
    broken[target].table_windows[0].columns = vec![99];
    assert!(
        validate_chunks(&doc, &markdown, &mapped, &broken, &mut |input| Ok(
            fake_count(input)
        ))
        .is_err()
    );
    let mut broken = drafts.clone();
    let header = broken[target]
        .input_parts
        .iter_mut()
        .find(|part| part.role == InputRole::TableHeaderContext)
        .unwrap();
    header.role = InputRole::HeadingContext;
    assert!(
        validate_chunks(&doc, &markdown, &mapped, &broken, &mut |input| Ok(
            fake_count(input)
        ))
        .is_err()
    );
    let mut broken = drafts.clone();
    broken[target].prepared_input.push('!');
    assert!(
        validate_chunks(&doc, &markdown, &mapped, &broken, &mut |input| Ok(
            fake_count(input)
        ))
        .is_err()
    );
}

/// A document, its mapped units and its drafts under the fake counter.
fn drafted(markdown: &str) -> (CanonicalDocument, MappedDocument, Vec<ChunkContent>) {
    let doc = canonicalize(CanonicalizeInput::new(markdown, "tamper")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let drafts = build_drafts(&doc, markdown, &mapped, &mut |input| Ok(fake_count(input))).unwrap();
    (doc, mapped, drafts)
}

/// Why chunks fail validation under the fake counter, or `None` when they pass.
fn refusal(
    doc: &CanonicalDocument,
    markdown: &str,
    mapped: &MappedDocument,
    chunks: &[ChunkContent],
) -> Option<String> {
    validate_chunks(doc, markdown, mapped, chunks, &mut |input| {
        Ok(fake_count(input))
    })
    .err()
    .map(|error| error.to_string())
}

#[test]
fn each_coverage_fault_is_refused_and_a_split_unit_is_accepted() {
    let (_, mapped, drafts) = drafted("é alpha beta\n\nSecond paragraph.\n");
    let whole = drafts[0].fragments[0].clone();
    let end = whole.contribution.range.end;
    let piece = |start, end, part_ordinal| {
        let mut fragment = whole.clone();
        fragment.contribution.range = TextRange { start, end };
        fragment.part_ordinal = part_ordinal;
        fragment
    };
    let covered = |pieces: Vec<Fragment>| {
        let mut chunks = drafts.clone();
        chunks[0].fragments.splice(0..1, pieces);
        validate_coverage(&mapped, &chunks).is_ok()
    };
    // `é ` is three bytes: a split there is two valid parts of one unit.
    assert!(covered(vec![piece(0, 3, 0), piece(3, end, 1)]));
    assert!(!covered(vec![piece(0, 0, 0), piece(0, end, 1)]));
    assert!(!covered(vec![piece(0, 1, 0), piece(1, end, 1)]));
    assert!(!covered(vec![piece(0, end, 1)]));
}

#[test]
fn a_chunk_of_exactly_the_maximum_tokens_is_valid() {
    let markdown = "Body\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "maximum")).unwrap();
    let mapped = map_document(&doc, markdown).unwrap();
    let mut at_maximum = |_: &str| Ok(MAX_TOKENS);
    let drafts = build_drafts(&doc, markdown, &mapped, &mut at_maximum).unwrap();
    assert_eq!(drafts[0].token_count, MAX_TOKENS);
    validate_chunks(&doc, markdown, &mapped, &drafts, &mut at_maximum).unwrap();
}

#[test]
fn each_replayed_fault_is_refused_by_its_own_check() {
    // The final rebuild refuses these too, with another error: each check must fire first.
    let markdown = "# A\n\n## B\n\nBody one.\n\nBody two.\n";
    let (doc, mapped, drafts) = drafted(markdown);
    assert_eq!(refusal(&doc, markdown, &mapped, &drafts), None);
    let target = drafts
        .iter()
        .position(|chunk| chunk.prepared_input.contains("Body one"))
        .unwrap();
    let parts = &drafts[target].input_parts;
    let source = parts
        .iter()
        .position(|part| part.role == InputRole::SourceContent)
        .unwrap();
    let separator = parts
        .iter()
        .position(|part| part.role == InputRole::FormattingSeparator)
        .unwrap();
    for fault in 0..7 {
        let mut chunks = drafts.clone();
        let chunk = &mut chunks[target];
        match fault {
            0 => chunk.input_parts[source].prepared_range.end += 1,
            // Same length, so the same count: only the prepared text differs.
            1 => chunk.prepared_input = chunk.prepared_input.replacen("Body", "Bodz", 1),
            2 => chunk.body_text.push('!'),
            3 => chunk.input_parts[separator].mappings[0].mode = OriginMode::ExactCopy,
            4 => chunk.input_parts[source].body_layout = true,
            5 => chunk.input_parts[source].mappings[0].mode = OriginMode::Formatting,
            _ => chunk.fragments[0].contribution.range.start += 1,
        }
        assert_eq!(
            refusal(&doc, markdown, &mapped, &chunks),
            Some(invalid_chunks().to_string()),
            "fault {fault}"
        );
    }
}

#[test]
fn a_part_replays_each_contribution_at_its_own_offset() {
    let markdown = "Body one.\n\nBody two.\n";
    let (doc, mapped, drafts) = drafted(markdown);
    let mut chunk = drafts[0].clone();
    let parts = mem::take(&mut chunk.input_parts);
    assert_eq!(parts.len(), 3);
    // One part holding both sources, without the separator: consistent, so replay accepts it,
    // and the rebuild then refuses a layout the preparation never makes.
    let (mut merged, second) = (parts[0].clone(), &parts[2]);
    let shift = merged.text.len();
    merged.text.push_str(&second.text);
    merged
        .contributions
        .extend(second.contributions.iter().copied());
    merged
        .mappings
        .extend(second.mappings.iter().cloned().map(|mut run| {
            run.range.start += shift;
            run.range.end += shift;
            run
        }));
    merged.prepared_range = TextRange {
        start: 0,
        end: merged.text.len(),
    };
    chunk.prepared_input.clone_from(&merged.text);
    chunk.body_text.clone_from(&merged.text);
    chunk.token_count = fake_count(&merged.text);
    chunk.input_parts = vec![merged];
    let refused = refusal(&doc, markdown, &mapped, &[chunk]).unwrap();
    assert_ne!(refused, invalid_chunks().to_string());
}
