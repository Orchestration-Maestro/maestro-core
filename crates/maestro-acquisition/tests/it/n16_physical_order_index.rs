//! Indexed physical-order matching preserves exact identity and first-match semantics.
use super::n16_define_extraction_fidelity_and_cumulative_decode_contracts::document;
use maestro_acquisition::extraction::{
    contract::{ByteSpan, Content, MappedUnit, Measured, Measurement, SourceUnit, StructureKind},
    fidelity::{Finding, evaluate},
};
use maestro_kernel::document::Outcome;

#[test]
fn n16_physical_order_accepts_large_inventories() {
    let mut doc = document();
    doc.units.clear();
    doc.markdown.clear();
    let mut source = Vec::new();
    let prefix = "x".repeat(32);
    // Equal spans force full content comparisons for every unit; no wall-clock bound
    // (a timing assertion flakes under load), the index keeps this linear.
    for number in 0..2_000 {
        let text = format!("{prefix}{number:05}");
        let start = doc.markdown.len() as u64;
        doc.markdown.push_str(&text);
        let unit = SourceUnit {
            kind: StructureKind::Cell,
            span: Some(ByteSpan { start: 0, end: 3 }),
            content: Content::Text(text),
        };
        source.push(unit.clone());
        doc.units.push(MappedUnit {
            source: unit,
            output: Some(ByteSpan {
                start,
                end: doc.markdown.len() as u64,
            }),
        });
    }
    doc.measurements.first_mut().unwrap().source = Measured::Known(source);
    let checked = evaluate(doc);
    assert_eq!(checked.outcome(), Outcome::Accepted);
}

#[test]
fn n16_physical_order_index_keeps_the_first_exact_match_even_without_output() {
    let mut doc = document();
    doc.source_length = 6;
    doc.markdown = "notyesx".into();
    let mut duplicate = doc.units.first().unwrap().clone();
    duplicate.output = Some(ByteSpan { start: 4, end: 7 });
    doc.units.first_mut().unwrap().output = None;
    // Same span and kind, but different content: this is not the source identity.
    let mut other = duplicate.clone();
    other.source.content = Content::Text("other".into());
    doc.units.insert(0, other);
    doc.units.push(duplicate);
    let heading = SourceUnit {
        kind: StructureKind::Heading,
        span: Some(ByteSpan { start: 3, end: 6 }),
        content: Content::Text("yes".into()),
    };
    doc.measurements.push(Measurement {
        kind: StructureKind::Heading,
        source: Measured::Known(vec![heading.clone()]),
    });
    doc.units.push(MappedUnit {
        source: heading,
        output: Some(ByteSpan { start: 3, end: 6 }),
    });
    let checked = evaluate(doc);
    // Per-kind comparison rejects the extras; physical order adds no finding.
    let correspondence: Vec<_> = checked
        .receipt()
        .findings
        .iter()
        .filter(|finding| matches!(finding, Finding::Correspondence(_)))
        .collect();
    assert_eq!(
        correspondence,
        vec![&Finding::Correspondence(StructureKind::Cell)]
    );
}
