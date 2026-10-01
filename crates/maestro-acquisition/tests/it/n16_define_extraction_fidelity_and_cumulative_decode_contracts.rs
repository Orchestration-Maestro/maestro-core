//! Synthetic source-correspondence and core-owned parser preflight contracts.
#![expect(
    clippy::indexing_slicing,
    reason = "synthetic fixture vectors have fixed known positions"
)]
#![expect(clippy::unreachable, reason = "closed synthetic guard table")]

use super::n09_support::policy;
use maestro_acquisition::{
    Ref,
    extraction::{
        contract::{
            ByteSpan, Content, Extraction, MappedUnit, Measured, Measurement, SourceUnit,
            StructureKind, Warning,
        },
        decode::{DecodeReceipt, DecodeRequest, DecodeStage, ParserDecode},
        fidelity::{Finding, evaluate},
        model::Processing,
    },
    transport::stream::{Accounting, Failure},
};
use maestro_kernel::{acquisition::Handle, artifact::Digest, document::Outcome};
use std::{mem::size_of, time::Duration};
use tokio::time::advance;

/// Stable synthetic immutable pin.
fn pin(id: &str) -> Ref {
    Ref {
        id: id.into(),
        digest: Digest::of(id.as_bytes()),
    }
}
/// One source cell and its exact Markdown correspondence.
fn document() -> Extraction {
    let unit = SourceUnit {
        kind: StructureKind::Cell,
        span: Some(ByteSpan { start: 0, end: 3 }),
        content: Content::Text("not".into()),
    };
    Extraction {
        attempt: Handle::new(),
        capture: Handle::new(),
        source: Digest::of(b"not"),
        source_length: 3,
        method: "synthetic".into(),
        profile: pin("profile"),
        tool: pin("tool"),
        models: vec![],
        processing: Processing {
            cleanup: pin("cleanup"),
            chunk: pin("chunk"),
            dedup: pin("dedup"),
        },
        markdown: "not".into(),
        units: vec![MappedUnit {
            source: unit.clone(),
            output: Some(ByteSpan { start: 0, end: 3 }),
        }],
        measurements: vec![Measurement {
            kind: StructureKind::Cell,
            source: Measured::Known(vec![unit]),
        }],
        required: vec![StructureKind::Cell],
        assets: vec![],
        warnings: vec![],
        missing: vec![],
    }
}

#[test]
fn n16_equal_cell_counts_do_not_prove_order_or_content() {
    let mut doc = document();
    let second = SourceUnit {
        kind: StructureKind::Cell,
        span: Some(ByteSpan { start: 3, end: 6 }),
        content: Content::Text("yes".into()),
    };
    doc.source_length = 6;
    doc.markdown = "notyes".into();
    doc.units.push(MappedUnit {
        source: second.clone(),
        output: Some(ByteSpan { start: 3, end: 6 }),
    });
    doc.measurements[0].source = Measured::Known(vec![doc.units[0].source.clone(), second]);
    assert_eq!(evaluate(doc.clone()).outcome(), Outcome::Accepted);
    let mut output_reordered = doc.clone();
    output_reordered.markdown = "yesnot".into();
    output_reordered.units[0].output = Some(ByteSpan { start: 3, end: 6 });
    output_reordered.units[1].output = Some(ByteSpan { start: 0, end: 3 });
    assert_eq!(
        evaluate(output_reordered).outcome(),
        Outcome::NeedsReextraction
    );
    doc.units.swap(0, 1);
    let held = evaluate(doc);
    assert_eq!(held.outcome(), Outcome::NeedsReextraction);
    assert!(
        held.receipt()
            .findings
            .contains(&Finding::Correspondence(StructureKind::Cell))
    );
    assert_eq!(held.document().units.len(), 2);
    assert_eq!(held.receipt().units.len(), 2);
    assert!(
        held.receipt()
            .units
            .iter()
            .all(|unit| unit.content_preserved)
    );
    assert_eq!(held.receipt().measurements, held.document().measurements);
    let mut changed = document();
    changed.markdown = "yes".into();
    changed.units[0].source.content = Content::Text("yes".into());
    assert_eq!(evaluate(changed).outcome(), Outcome::NeedsReextraction);
}

#[test]
fn n16_negation_code_and_literals_are_exact_content_not_counts() {
    for kind in [
        StructureKind::Negation,
        StructureKind::Code,
        StructureKind::Literal,
    ] {
        let mut doc = document();
        doc.required = vec![kind];
        doc.measurements[0].kind = kind;
        doc.units[0].source.kind = kind;
        doc.measurements[0].source = Measured::Known(vec![doc.units[0].source.clone()]);
        assert_eq!(evaluate(doc.clone()).outcome(), Outcome::Accepted);
        doc.markdown = "yes".into();
        let receipt = evaluate(doc.clone());
        assert_eq!(receipt.outcome(), Outcome::NeedsReextraction);
        assert!(!receipt.receipt().units[0].content_preserved);
        assert!(receipt.receipt().findings.contains(&Finding::Content(kind)));
        doc.units.clear();
        assert_eq!(evaluate(doc).outcome(), Outcome::NeedsReextraction);
    }
}

#[test]
fn n16_unknown_required_measurement_is_not_known_zero() {
    let mut doc = document();
    doc.units.clear();
    doc.markdown.clear();
    doc.measurements[0].source = Measured::Known(vec![]);
    assert_eq!(evaluate(doc.clone()).outcome(), Outcome::Accepted);
    doc.measurements[0].source = Measured::Unknown;
    let held = evaluate(doc.clone());
    assert_eq!(held.outcome(), Outcome::Quarantined);
    assert!(
        held.receipt()
            .findings
            .contains(&Finding::Unknown(StructureKind::Cell))
    );
    assert_eq!(held.document(), &doc);
    doc.measurements.clear();
    assert_eq!(evaluate(doc).outcome(), Outcome::Quarantined);
}

#[test]
fn n16_assets_missing_indicators_and_warn_rule_have_neighbours() {
    let mut doc = document();
    let asset = pin("asset");
    doc.required = vec![StructureKind::Asset];
    doc.units[0].source.kind = StructureKind::Asset;
    doc.units[0].source.content = Content::Asset(asset.clone());
    doc.units[0].output = None;
    doc.measurements[0].kind = StructureKind::Asset;
    doc.measurements[0].source = Measured::Known(vec![doc.units[0].source.clone()]);
    doc.assets.push(asset);
    assert_eq!(evaluate(doc.clone()).outcome(), Outcome::Accepted);
    doc.assets.clear();
    assert_eq!(evaluate(doc).outcome(), Outcome::NeedsReextraction);
    let mut doc = document();
    doc.warnings.push(Warning {
        rule: Some("synthetic-warning".into()),
        message: "visible warning".into(),
    });
    assert_eq!(
        evaluate(doc.clone()).outcome(),
        Outcome::AcceptedWithWarnings
    );
    for rule in [None, Some(String::new()), Some("../invalid".into())] {
        doc.warnings[0].rule = rule;
        assert_eq!(evaluate(doc.clone()).outcome(), Outcome::Quarantined);
    }
    doc.warnings.clear();
    doc.missing.push("literal".into());
    assert_eq!(evaluate(doc).outcome(), Outcome::NeedsReextraction);
}

#[test]
fn n16_mapping_ranges_provenance_and_receipt_survive_holds() {
    let mut doc = document();
    doc.units[0].output = Some(ByteSpan { start: 0, end: 4 });
    assert_eq!(evaluate(doc).outcome(), Outcome::NeedsReextraction);
    let mut doc = document();
    doc.units[0].source.span = None;
    doc.measurements[0].source = Measured::Known(vec![doc.units[0].source.clone()]);
    assert_eq!(evaluate(doc).outcome(), Outcome::Quarantined);
    let doc = document();
    let bytes = serde_json::to_vec(&doc).unwrap();
    assert_eq!(serde_json::from_slice::<Extraction>(&bytes).unwrap(), doc);
    let evaluated = evaluate(doc);
    let receipt = serde_json::to_vec(evaluated.receipt()).unwrap();
    assert!(String::from_utf8(receipt).unwrap().contains("capture"));
}

/// Shared envelope with room for independent one-guard tests.
fn accounting() -> Accounting {
    let mut limits = policy().policy().sources[0].limits.clone();
    limits.decode.expanded_bytes = 100.try_into().unwrap();
    limits.decode.expansion_ratio = 10.try_into().unwrap();
    limits.decode.nested_levels = 2.try_into().unwrap();
    limits.decode.members = 2.try_into().unwrap();
    limits.decode.decoded_pixels = 100.try_into().unwrap();
    limits.decode.memory_bytes = 10_000.try_into().unwrap();
    let mut accounting = Accounting::new(limits);
    accounting.encoded(100).unwrap();
    accounting
}
/// IPC proposal, not a replacement serialized accounting counter.
fn request() -> DecodeRequest {
    DecodeRequest {
        stage: DecodeStage::Office,
        input_bytes: 10,
        expanded_bytes: 10,
        levels: 1,
        members: 1,
        entities: 0,
        pixels: 10,
        memory_bytes: 10,
    }
}

#[test]
fn n16_parser_preflight_enforces_every_cumulative_hook() {
    let cases = [
        (Failure::ExpandedBytes, "bytes"),
        (Failure::ExpansionRatio, "ratio"),
        (Failure::Nesting, "depth"),
        (Failure::Members, "members"),
        (Failure::Entities, "entities"),
        (Failure::Pixels, "pixels"),
        (Failure::Memory, "memory"),
    ];
    for (reason, field) in cases {
        let mut accounting = accounting();
        let mut parser = ParserDecode::new(&mut accounting);
        parser.admit(request()).unwrap();
        let mut next = request();
        match field {
            "bytes" => {
                next.expanded_bytes = 91;
                next.input_bytes = 100;
            }
            "ratio" => {
                next.expanded_bytes = 11;
                next.input_bytes = 1;
            }
            "depth" => next.levels = 2,
            "members" => next.members = 2,
            "entities" => next.entities = 1,
            "pixels" => next.pixels = 91,
            "memory" => next.memory_bytes = 9791,
            _ => unreachable!(),
        }
        next.stage = DecodeStage::Attachment;
        let refusal = parser.admit(next).unwrap_err();
        assert_eq!(refusal.reason, reason, "{field}");
        assert_eq!(refusal.stage, DecodeStage::Attachment);
        parser.crashed(DecodeStage::Pdf);
        assert_eq!(parser.receipts().len(), 1);
        assert_eq!(parser.finish(), Err(refusal.clone()));
        drop(parser);
        let mut recreated = ParserDecode::new(&mut accounting);
        assert_eq!(recreated.admit(request()), Err(refusal.clone()));
        assert_eq!(recreated.finish(), Err(refusal));
    }
}

#[test]
fn n16_cumulative_ratio_cannot_reset_at_parser_ipc() {
    let mut limits = accounting().limits().clone();
    limits.decode.expansion_ratio = 1.try_into().unwrap();
    limits.decode.expanded_bytes = 1000.try_into().unwrap();
    let mut accounting = Accounting::new(limits);
    accounting.encoded(100).unwrap();
    accounting.decoded(60).unwrap();
    let mut parser = ParserDecode::new(&mut accounting);
    let mut next = request();
    next.input_bytes = 100;
    next.expanded_bytes = 40;
    parser.admit(next.clone()).unwrap();
    next.expanded_bytes = 1;
    assert_eq!(
        parser.admit(next).unwrap_err().reason,
        Failure::ExpansionRatio
    );
}

#[tokio::test(start_paused = true)]
async fn n16_parser_time_and_crash_hold_without_partial_completion() {
    let mut accounting = accounting();
    let elapsed = accounting
        .limits()
        .decode
        .elapsed_ms
        .min(accounting.limits().elapsed_ms)
        .get();
    let mut parser = ParserDecode::new(&mut accounting);
    parser.admit(request()).unwrap();
    advance(Duration::from_millis(elapsed)).await;
    assert_eq!(
        parser.admit(request()).unwrap_err().reason,
        Failure::Timeout
    );
    assert_eq!(parser.finish().unwrap_err().reason, Failure::Timeout);
    let mut accounting = self::accounting();
    let mut parser = ParserDecode::new(&mut accounting);
    parser.admit(request()).unwrap();
    parser.crashed(DecodeStage::Pdf);
    assert_eq!(parser.finish().unwrap_err().reason, Failure::ParserCrash);
    let mut neighbour = self::accounting();
    let mut parser = ParserDecode::new(&mut neighbour);
    parser.admit(request()).unwrap();
    assert!(parser.finish().is_ok());
}

#[test]
fn n16_parser_passing_boundaries_and_shared_handoff() {
    let mut accounting = accounting();
    {
        let mut parser = ParserDecode::new(&mut accounting);
        parser.admit(request()).unwrap();
        assert_eq!(parser.finish().unwrap().len(), 1);
    }
    let mut parser = ParserDecode::new(&mut accounting);
    parser.admit(request()).unwrap();
    assert!(parser.finish().is_ok());
    assert_eq!(accounting.expanded_bytes(), 20);
    assert_eq!(accounting.members(), 2);
}

#[test]
fn n16_each_preflight_limit_has_an_exact_passing_boundary() {
    for field in [
        "bytes", "ratio", "depth", "members", "entities", "pixels", "memory",
    ] {
        let mut accounting = accounting();
        let mut parser = ParserDecode::new(&mut accounting);
        parser.admit(request()).unwrap();
        let mut next = request();
        match field {
            "bytes" => {
                next.expanded_bytes = 90;
                next.input_bytes = 100;
            }
            "ratio" => {
                next.input_bytes = 1;
                next.expanded_bytes = 10;
            }
            "depth" => next.levels = 1,
            "members" => next.members = 1,
            "entities" => next.entities = 0,
            "pixels" => next.pixels = 90,
            "memory" => {
                next.memory_bytes = 10_000 - 240 - 10 - 2 * size_of::<DecodeReceipt>() as u64;
            }
            _ => unreachable!(),
        }
        parser.admit(next).unwrap();
        assert_eq!(parser.finish().unwrap().len(), 2, "{field}");
    }
}

#[tokio::test(start_paused = true)]
async fn n16_completion_checks_time_and_reservations_without_a_final_request() {
    let mut accounting = accounting();
    let elapsed = accounting
        .limits()
        .decode
        .elapsed_ms
        .min(accounting.limits().elapsed_ms)
        .get();
    let mut parser = ParserDecode::new(&mut accounting);
    parser.admit(request()).unwrap();
    advance(Duration::from_millis(elapsed - 1)).await;
    assert!(parser.finish().is_ok());
    advance(Duration::from_millis(1)).await;
    assert_eq!(parser.finish().unwrap_err().reason, Failure::Timeout);
}

#[test]
fn n16_decode_proposals_are_strict_and_memory_overflow_holds() {
    let proposal = request();
    let bytes = serde_json::to_vec(&proposal).unwrap();
    assert_eq!(
        serde_json::from_slice::<DecodeRequest>(&bytes).unwrap(),
        proposal
    );
    let mut wire = serde_json::to_value(&proposal).unwrap();
    wire["expanded_total"] = serde_json::json!(0);
    assert!(serde_json::from_value::<DecodeRequest>(wire).is_err());
    let mut accounting = accounting();
    let mut parser = ParserDecode::new(&mut accounting);
    let mut proposal = request();
    proposal.memory_bytes = u64::MAX;
    assert_eq!(parser.admit(proposal).unwrap_err().reason, Failure::Memory);
    assert!(parser.finish().is_err());
}

#[test]
fn n16_source_spans_utf8_and_duplicate_measurements_hold() {
    for span in [ByteSpan { start: 3, end: 2 }, ByteSpan { start: 0, end: 4 }] {
        let mut doc = document();
        doc.units[0].source.span = Some(span);
        doc.measurements[0].source = Measured::Known(vec![doc.units[0].source.clone()]);
        assert_eq!(evaluate(doc).outcome(), Outcome::Quarantined);
    }
    let mut doc = document();
    doc.measurements.push(doc.measurements[0].clone());
    assert_eq!(evaluate(doc).outcome(), Outcome::Quarantined);
    let mut doc = document();
    doc.required.clear();
    doc.measurements.clear();
    assert_eq!(evaluate(doc).outcome(), Outcome::Quarantined);
    let mut doc = document();
    doc.markdown = "éx".into();
    doc.units[0].output = Some(ByteSpan { start: 1, end: 3 });
    assert_eq!(evaluate(doc).outcome(), Outcome::NeedsReextraction);
    let mut doc = document();
    doc.warnings.push(Warning {
        rule: Some("named-rule".into()),
        message: "   ".into(),
    });
    assert_eq!(evaluate(doc).outcome(), Outcome::Quarantined);
}

#[test]
fn n16_completion_rechecks_tightened_shared_limits() {
    for field in ["bytes", "depth", "members", "pixels", "memory", "encoded"] {
        let mut accounting = accounting();
        let mut parser = ParserDecode::new(&mut accounting);
        parser.admit(request()).unwrap();
        parser.admit(request()).unwrap();
        assert!(parser.finish().is_ok());
        drop(parser);
        let mut limits = accounting.limits().clone();
        match field {
            "bytes" => limits.decode.expanded_bytes = 19.try_into().unwrap(),
            "depth" => limits.decode.nested_levels = 1.try_into().unwrap(),
            "members" => limits.decode.members = 1.try_into().unwrap(),
            "pixels" => limits.decode.decoded_pixels = 19.try_into().unwrap(),
            "memory" => limits.decode.memory_bytes = 239.try_into().unwrap(),
            "encoded" => limits.wire_bytes = 99.try_into().unwrap(),
            _ => unreachable!(),
        }
        accounting.tighten(&limits);
        let mut parser = ParserDecode::new(&mut accounting);
        assert!(parser.finish().is_err(), "{field}");
    }
}
