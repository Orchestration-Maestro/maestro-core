//! Counter-specific provenance and exact worst-ratio completion regressions.
#![expect(clippy::unreachable, reason = "closed synthetic guard table")]
use super::n16_define_extraction_fidelity_and_cumulative_decode_contracts::{accounting, request};
use maestro_acquisition::{
    extraction::decode::{DecodeStage, ParserDecode},
    transport::stream::Failure,
};
use std::time::Duration;
use tokio::time::advance;

#[test]
fn n16_completion_keeps_worst_fractional_step_ratio() {
    let mut ledger = accounting();
    let mut first = request();
    first.input_bytes = 3;
    let mut parser = ParserDecode::new(&mut ledger);
    parser.admit(first).unwrap(); // 10/3, not rounded to 3.
    let mut second = request();
    second.input_bytes = 1;
    second.expanded_bytes = 2; // Smaller later ratio cannot replace the worst.
    parser.admit(second).unwrap();
    drop(parser);
    let mut limits = ledger.limits().clone();
    limits.decode.expansion_ratio = 4.try_into().unwrap();
    ledger.tighten(&limits);
    assert!(ParserDecode::new(&mut ledger).finish().is_ok());
    limits.decode.expansion_ratio = 3.try_into().unwrap();
    ledger.tighten(&limits);
    assert_eq!(
        ParserDecode::new(&mut ledger).finish().unwrap_err().reason,
        Failure::ExpansionRatio
    );
}

#[test]
fn n16_completion_reports_counter_charging_stage() {
    for (field, reason, expected) in [
        ("expanded", Failure::ExpandedBytes, DecodeStage::Image),
        ("ratio", Failure::ExpansionRatio, DecodeStage::Image),
        ("levels", Failure::Nesting, DecodeStage::Image),
        ("members", Failure::Members, DecodeStage::Image),
        ("pixels", Failure::Pixels, DecodeStage::Image),
        ("memory", Failure::Memory, DecodeStage::Office),
        ("wire", Failure::EncodedBytes, DecodeStage::Http),
    ] {
        let mut ledger = accounting();
        let mut first = request();
        first.stage = DecodeStage::Image;
        first.input_bytes = 1;
        first.levels = 2;
        first.members = 2;
        let mut parser = ParserDecode::new(&mut ledger);
        parser.admit(first).unwrap();
        let mut second = request();
        second.expanded_bytes = 0;
        second.levels = 0;
        second.members = 0;
        second.pixels = 0;
        second.memory_bytes = 0;
        parser.admit(second).unwrap();
        drop(parser);
        let mut limits = ledger.limits().clone();
        match field {
            "expanded" => limits.decode.expanded_bytes = 9.try_into().unwrap(),
            "ratio" => limits.decode.expansion_ratio = 9.try_into().unwrap(),
            "levels" => limits.decode.nested_levels = 1.try_into().unwrap(),
            "members" => limits.decode.members = 1.try_into().unwrap(),
            "pixels" => limits.decode.decoded_pixels = 9.try_into().unwrap(),
            "memory" => limits.decode.memory_bytes = 1.try_into().unwrap(),
            "wire" => limits.wire_bytes = 99.try_into().unwrap(),
            _ => unreachable!(),
        }
        ledger.tighten(&limits);
        let refusal = ParserDecode::new(&mut ledger).finish().unwrap_err();
        assert_eq!(refusal.reason, reason, "{field}");
        assert_eq!(refusal.stage, Some(expected), "{field}");
    }
}

#[tokio::test(start_paused = true)]
async fn n16_timeout_reports_latest_charged_stage_after_handoff() {
    let mut ledger = accounting();
    let elapsed = ledger
        .limits()
        .elapsed_ms
        .min(ledger.limits().decode.elapsed_ms)
        .get();
    let mut parser = ParserDecode::new(&mut ledger);
    let mut first = request();
    first.stage = DecodeStage::Image;
    parser.admit(first).unwrap();
    parser.admit(request()).unwrap();
    drop(parser);
    advance(Duration::from_millis(elapsed)).await;
    let refusal = ParserDecode::new(&mut ledger).finish().unwrap_err();
    assert_eq!(refusal.reason, Failure::Timeout);
    assert_eq!(refusal.stage, Some(DecodeStage::Office));
}
