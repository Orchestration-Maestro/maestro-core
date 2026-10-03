//! Small public accounting checks for downstream reuse and exact boundaries.
use super::n09_support::policy;
use maestro_acquisition::transport::{http::Failure, stream::Accounting};

#[test]
fn n09_accounting_bounds_copies_before_retention() {
    let mut limits = policy().policy().sources[0].limits.clone();
    limits.memory_bytes = 30.try_into().unwrap();
    let mut accounting = Accounting::new(limits);
    accounting.encoded(10).unwrap();
    assert_eq!(accounting.decoded(8), Err(Failure::Memory));
    assert_eq!(accounting.expanded_bytes(), 0);
}
#[test]
fn n09_encoded_counter_overflow_refuses_without_wrapping() {
    let mut limits = policy().policy().sources[0].limits.clone();
    limits.wire_bytes = u64::MAX.try_into().unwrap();
    let mut accounting = Accounting::new(limits);
    accounting.encoded(u64::MAX).unwrap();
    assert_eq!(accounting.encoded(1), Err(Failure::EncodedBytes));
    assert_eq!(accounting.wire_bytes(), u64::MAX);
}
#[test]
fn n09_shared_decode_limits_remain_tighter() {
    let mut limits = policy().policy().sources[0].limits.clone();
    limits.decode.expanded_bytes = 5.try_into().unwrap();
    let mut accounting = Accounting::new(limits);
    let looser = policy().policy().sources[0].limits.clone();
    accounting.tighten(&looser);
    accounting.encoded(10).unwrap();
    accounting.decoded(3).unwrap();
    assert_eq!(accounting.decoded(3), Err(Failure::ExpandedBytes));
    assert_eq!(accounting.expanded_bytes(), 3);
}

#[test]
fn n09_cumulative_ratio_spans_shared_decode_stages() {
    let mut limits = policy().policy().sources[0].limits.clone();
    limits.decode.expansion_ratio = 1.try_into().unwrap();
    let mut accounting = Accounting::new(limits);
    accounting.encoded(100).unwrap();
    accounting.decoded(60).unwrap();
    assert_eq!(accounting.decoded(60), Err(Failure::ExpansionRatio));
    assert_eq!(accounting.expanded_bytes(), 60);
}

#[test]
fn n09_new_stricter_decode_envelope_is_applied() {
    let limits = policy().policy().sources[0].limits.clone();
    let mut stricter = limits.clone();
    stricter.decode.expanded_bytes = 5.try_into().unwrap();
    let mut accounting = Accounting::new(limits);
    accounting.encoded(10).unwrap();
    accounting.decoded(3).unwrap();
    accounting.tighten(&stricter);
    assert_eq!(accounting.decoded(3), Err(Failure::ExpandedBytes));
}
