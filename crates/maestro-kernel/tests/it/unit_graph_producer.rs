//! Kernel conformance against the canonicalization producer snapshot.
#![cfg(test)]

use maestro_kernel::unit_graph::{DeliveryGraph, MappingLedger};

#[test]
fn b06_built_snapshot_is_accepted_by_kernel() {
    let graph_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-canonicalization/tests/fixtures/unit-graph-v1.built.json"
    ))
    .strip_suffix(b"\n")
    .unwrap();
    let mapping_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-canonicalization/tests/fixtures/unit-mapping-v1.built.json"
    ))
    .strip_suffix(b"\n")
    .unwrap();
    let graph = DeliveryGraph::from_bytes(graph_bytes).unwrap();
    let mapping = MappingLedger::from_bytes(mapping_bytes).unwrap();
    graph
        .validate(
            &mapping,
            include_str!("../../../maestro-canonicalization/tests/fixtures/unit-graph-v1.built.md"),
        )
        .unwrap();
}
