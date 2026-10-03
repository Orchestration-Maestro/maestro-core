//! Isolated payload neighbours: the one total budget also covers stream overhead,
//! so an aggregate-exact payload cannot itself be a successful full tar stream.

use super::write::{entries, minimal, run};
use crate::source::{SourceTree as _, builtin};
use crate::{
    bundle::{manifest::Budget, write::charge},
    limits::Limits,
};

#[test]
fn manifest_bytes_use_archive_budget_not_source_file_budget() {
    let tree = minimal();
    let registry = builtin().unwrap();
    let original = run(&tree, &registry, &Limits::PRODUCTION).unwrap();
    let manifest_bytes = entries(&original.bytes)["bundle.json"].len() as u64;
    let source_bytes = tree.read("package.toml", u64::MAX).unwrap().len() as u64;
    assert!(manifest_bytes > source_bytes);
    let limits = Limits {
        source_file_bytes: source_bytes,
        archive_entry_bytes: manifest_bytes,
        ..Limits::PRODUCTION
    };
    assert_eq!(
        run(&tree, &registry, &limits).unwrap().bytes,
        original.bytes
    );
}

#[test]
fn aggregate_payload_accepts_exact_and_refuses_one_past() {
    let limits = Limits {
        archive_total_bytes: 10,
        archive_entry_bytes: 11,
        ..Limits::PRODUCTION
    };
    let (mut aggregate, mut stream) = (0, 1024);
    charge("entry", 10, &limits, &mut aggregate, &mut stream).unwrap();
    assert_eq!((aggregate, stream), (10, 2048));
    let error = charge("entry", 1, &limits, &mut aggregate, &mut stream).unwrap_err();
    assert!(error.to_string().contains("aggregate bytes"));
}

#[test]
fn payload_accounting_checks_all_arithmetic_overflows() {
    let limits = Limits {
        archive_total_bytes: u64::MAX,
        archive_entry_bytes: u64::MAX,
        ..Limits::PRODUCTION
    };
    for (length, mut aggregate, mut stream, reason) in [
        (1, u64::MAX, 0, "aggregate bytes overflow"),
        (u64::MAX, 0, 0, "stream bytes overflow"),
        (1, 0, u64::MAX, "stream bytes overflow"),
    ] {
        assert!(
            charge("entry", length, &limits, &mut aggregate, &mut stream)
                .unwrap_err()
                .to_string()
                .contains(reason)
        );
    }
}

#[test]
fn manifest_budget_counts_serialized_bytes_before_allocation() {
    Budget::new(3).charge(&"a").unwrap();
    assert!(
        Budget::new(2)
            .charge(&"a")
            .unwrap_err()
            .to_string()
            .contains("manifest entry bytes")
    );
    let mut budget = Budget::new(4);
    budget.charge(&"a").unwrap();
    assert!(budget.charge(&"b").is_err());
}
