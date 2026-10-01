//! Kernel conformance for nested procedure, code and table producer graphs.
use maestro_kernel::unit_graph::{DeliveryGraph, GroupKind, MappingLedger, UnitKind};

type Fixture = (
    &'static str,
    &'static [u8],
    &'static [u8],
    &'static str,
    fn(&DeliveryGraph),
);

#[test]
fn nested_producer_fixtures_are_accepted_by_kernel() {
    let cases: [Fixture; 3] = [
        (
            "nested list",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../maestro-canonicalization/tests/fixtures/unit-graph-v1.nested_list.json"
            )),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../maestro-canonicalization/tests/fixtures/unit-mapping-v1.nested_list.json"
            )),
            include_str!(
                "../../../maestro-canonicalization/tests/fixtures/unit-graph-v1.nested_list.md"
            ),
            assert_nested_list,
        ),
        (
            "code in list",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../maestro-canonicalization/tests/fixtures/unit-graph-v1.code_in_list.json"
            )),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../maestro-canonicalization/tests/fixtures/unit-mapping-v1.code_in_list.json"
            )),
            include_str!(
                "../../../maestro-canonicalization/tests/fixtures/unit-graph-v1.code_in_list.md"
            ),
            assert_code_in_list,
        ),
        (
            "table in list",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../maestro-canonicalization/tests/fixtures/unit-graph-v1.table_in_list.json"
            )),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../maestro-canonicalization/tests/fixtures/unit-mapping-v1.table_in_list.json"
            )),
            include_str!(
                "../../../maestro-canonicalization/tests/fixtures/unit-graph-v1.table_in_list.md"
            ),
            assert_table_in_list,
        ),
    ];
    for (name, graph_bytes, mapping_bytes, source, assert_shape) in cases {
        let graph = DeliveryGraph::from_bytes(graph_bytes.strip_suffix(b"\n").unwrap()).unwrap();
        let mapping =
            MappingLedger::from_bytes(mapping_bytes.strip_suffix(b"\n").unwrap()).unwrap();
        graph
            .validate(&mapping, source)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_shape(&graph);
    }
}

fn assert_nested_list(graph: &DeliveryGraph) {
    assert_eq!(
        graph
            .groups
            .iter()
            .filter(|group| group.kind == GroupKind::Procedure)
            .count(),
        1
    );
}

fn assert_code_in_list(graph: &DeliveryGraph) {
    assert!(
        graph
            .units
            .iter()
            .filter(|unit| unit.kind == UnitKind::Code)
            .any(|code| {
                graph.groups.iter().any(|group| {
                    group.kind == GroupKind::Code && group.children.contains(&code.unit_id)
                })
            })
    );
}

fn assert_table_in_list(graph: &DeliveryGraph) {
    assert!(
        graph
            .units
            .iter()
            .filter(|unit| unit.kind == UnitKind::Table)
            .any(|table| {
                graph.groups.iter().any(|group| {
                    group.kind == GroupKind::Table && group.children.contains(&table.unit_id)
                })
            })
    );
}
