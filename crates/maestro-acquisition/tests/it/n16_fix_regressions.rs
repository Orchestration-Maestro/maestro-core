//! Review regressions for shared reservations, provenance and exact fidelity.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls,
    n09_support::{Dns, Grants, Wire, fetch, http, policy, response, run},
    n16_define_extraction_fidelity_and_cumulative_decode_contracts::{
        accounting, document, request,
    },
};
use maestro_acquisition::{
    extraction::{
        contract::{
            ByteSpan, Content, MappedUnit, Measured, Measurement, SourceUnit, StructureKind,
        },
        decode::{DecodeReceipt, DecodeStage, ParserDecode},
        fidelity::{Finding, evaluate},
    },
    transport::stream::{Accounting, Failure},
};
use maestro_kernel::{artifact::Digest, document::Outcome};
use std::{mem::size_of, time::Duration};
use tokio::time::advance;

#[test]
fn n16_parser_reservations_survive_http_setup() {
    run(async {
        for over in [0_u64, 1] {
            let checked = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![response(200, "", b"x")]);
            let mut limits = checked.policy().sources[0].limits.clone();
            limits.decode.memory_bytes = 100_000.try_into().unwrap();
            let mut ledger = Accounting::new(limits);
            ledger.encoded(100).unwrap();
            let mut first = request();
            first.memory_bytes = 30_000;
            ParserDecode::new(&mut ledger).admit(first).unwrap();
            http(&checked, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut ledger)
                .await
                .unwrap();
            let mut next = request();
            // HTTP parser + trailers = 49,152; wire/output copies = 244.
            next.memory_bytes =
                100_000 - 49_152 - 244 - 30_000 - 2 * size_of::<DecodeReceipt>() as u64 + over;
            let mut parser = ParserDecode::new(&mut ledger);
            let result = parser.admit(next);
            if over == 0 {
                result.unwrap();
                assert!(parser.finish().is_ok());
            } else {
                let refusal = result.unwrap_err();
                assert_eq!(refusal.reason, Failure::Memory);
                assert_eq!(parser.finish(), Err(refusal));
            }
        }
    });
}

#[test]
fn n16_review_mixed_kind_order() {
    let mut doc = document();
    doc.source = Digest::of(b"notyes");
    doc.source_length = 6;
    doc.markdown = "notyes".into();
    doc.units[0].source.kind = StructureKind::Negation;
    let second = SourceUnit {
        kind: StructureKind::Literal,
        span: Some(ByteSpan { start: 3, end: 6 }),
        content: Content::Text("yes".into()),
    };
    doc.units.push(MappedUnit {
        source: second.clone(),
        output: Some(ByteSpan { start: 3, end: 6 }),
    });
    doc.measurements = vec![
        Measurement {
            kind: StructureKind::Negation,
            source: Measured::Known(vec![doc.units[0].source.clone()]),
        },
        Measurement {
            kind: StructureKind::Literal,
            source: Measured::Known(vec![second]),
        },
    ];
    doc.required = vec![StructureKind::Negation, StructureKind::Literal];
    assert_eq!(evaluate(doc.clone()).outcome(), Outcome::Accepted);
    // Inventories and claimed source order stay intact; physical output reverses.
    doc.markdown = "yesnot".into();
    doc.units[0].output = Some(ByteSpan { start: 3, end: 6 });
    doc.units[1].output = Some(ByteSpan { start: 0, end: 3 });
    assert_eq!(evaluate(doc.clone()).outcome(), Outcome::NeedsReextraction);
    // Reordering the claims must not hide the reversal either.
    doc.units.swap(0, 1);
    doc.measurements.reverse();
    assert_eq!(evaluate(doc).outcome(), Outcome::NeedsReextraction);
}

#[test]
fn n16_nested_output_neighbour_is_accepted() {
    let mut doc = document();
    doc.units[0].source.kind = StructureKind::Code;
    let inner = SourceUnit {
        kind: StructureKind::Literal,
        span: Some(ByteSpan { start: 0, end: 2 }),
        content: Content::Text("no".into()),
    };
    doc.units.push(MappedUnit {
        source: inner.clone(),
        output: Some(ByteSpan { start: 0, end: 2 }),
    });
    doc.measurements = vec![
        Measurement {
            kind: StructureKind::Code,
            source: Measured::Known(vec![doc.units[0].source.clone()]),
        },
        Measurement {
            kind: StructureKind::Literal,
            source: Measured::Known(vec![inner]),
        },
    ];
    doc.required = vec![StructureKind::Code, StructureKind::Literal];
    assert_eq!(evaluate(doc).outcome(), Outcome::Accepted);
}

#[test]
fn n16_review_tightened_per_step_ratio() {
    let mut ledger = accounting();
    let mut proposal = request();
    proposal.input_bytes = 1;
    {
        let mut parser = ParserDecode::new(&mut ledger);
        parser.admit(proposal).unwrap();
        assert!(parser.finish().is_ok());
    }
    let mut limits = ledger.limits().clone();
    limits.decode.expansion_ratio = 1.try_into().unwrap();
    ledger.tighten(&limits);
    let mut parser = ParserDecode::new(&mut ledger);
    assert_eq!(parser.finish().unwrap_err().reason, Failure::ExpansionRatio);
}

#[test]
fn n16_review_stage_after_handoff() {
    let mut ledger = accounting();
    {
        let mut parser = ParserDecode::new(&mut ledger);
        let mut proposal = request();
        proposal.stage = DecodeStage::Image;
        parser.admit(proposal).unwrap();
        assert!(parser.finish().is_ok());
    }
    let mut limits = ledger.limits().clone();
    limits.decode.decoded_pixels = 9.try_into().unwrap();
    ledger.tighten(&limits);
    let mut parser = ParserDecode::new(&mut ledger);
    let refusal = parser.finish().unwrap_err();
    assert_eq!(refusal.reason, Failure::Pixels);
    assert_eq!(
        refusal.stage,
        Some(DecodeStage::Image),
        "there was no HTTP dispatch"
    );
}

#[test]
fn n16_whitespace_and_case_loss_hold_exact_content() {
    for kind in [
        StructureKind::Code,
        StructureKind::Literal,
        StructureKind::Negation,
    ] {
        for (source, output) in [(" not ", "not"), ("NOT", "not")] {
            let mut doc = document();
            let span = ByteSpan {
                start: 0,
                end: source.len() as u64,
            };
            doc.source = Digest::of(source.as_bytes());
            doc.source_length = span.end;
            doc.markdown = source.into();
            doc.units[0].source = SourceUnit {
                kind,
                span: Some(span),
                content: Content::Text(source.into()),
            };
            doc.units[0].output = Some(span);
            doc.measurements = vec![Measurement {
                kind,
                source: Measured::Known(vec![doc.units[0].source.clone()]),
            }];
            doc.required = vec![kind];
            assert_eq!(evaluate(doc.clone()).outcome(), Outcome::Accepted);
            doc.markdown = output.into();
            doc.units[0].output = Some(ByteSpan {
                start: 0,
                end: output.len() as u64,
            });
            let held = evaluate(doc);
            assert_eq!(
                held.outcome(),
                Outcome::NeedsReextraction,
                "{kind:?}: {source:?}"
            );
            assert!(held.receipt().findings.contains(&Finding::Content(kind)));
            assert!(!held.receipt().units[0].content_preserved);
        }
    }
}

#[test]
fn n16_acknowledgement_memory_exact_boundary() {
    for over in [0_u64, 1] {
        let mut ledger = accounting();
        let mut parser = ParserDecode::new(&mut ledger);
        parser.admit(request()).unwrap();
        let mut next = request();
        next.memory_bytes = 10_000 - 240 - 10 - 2 * size_of::<DecodeReceipt>() as u64 + over;
        let result = parser.admit(next);
        if over == 0 {
            result.unwrap();
            assert_eq!(parser.finish().unwrap().len(), 2);
        } else {
            assert_eq!(result.unwrap_err().reason, Failure::Memory);
            assert_eq!(parser.receipts().len(), 1);
            assert_eq!(parser.finish().unwrap_err().reason, Failure::Memory);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn n16_uncharged_timeout_has_no_decode_stage() {
    let mut ledger = Accounting::new(accounting().limits().clone());
    let elapsed = ledger
        .limits()
        .elapsed_ms
        .min(ledger.limits().decode.elapsed_ms)
        .get();
    advance(Duration::from_millis(elapsed)).await;
    let refusal = ParserDecode::new(&mut ledger).finish().unwrap_err();
    assert_eq!(refusal.reason, Failure::Timeout);
    assert_eq!(refusal.stage, None);
}
