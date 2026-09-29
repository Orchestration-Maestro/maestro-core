//! Synthetic graph size and indexed candidate lookup, without row explosion.
use super::support::{Scratch, graph, scoped};
use crate::{
    artifact::Digest,
    store::Error,
    unit_graph::{
        DeliveryGraph, GraphKey, MappingContribution, MappingEntry, MappingLedger, Part,
        SourceRange,
    },
};
use rusqlite::params;
use std::time::Instant;

#[test]
fn graph_scale_keeps_one_sql_record_and_an_indexed_revision_lookup() {
    let mut graph = graph();
    let mut source = include_str!("../../../tests/fixtures/unit-graph-v1.txt").to_owned();
    let mut mapping = MappingLedger::from_bytes(
        include_bytes!("../../../tests/fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    for ordinal in 0..1000 {
        append(&mut graph, &mut mapping, &mut source, ordinal);
    }
    let scratch = Scratch::new();
    let db = scratch.open_source(source.as_bytes());
    graph.descriptor.original_markdown_digest = Digest::of(source.as_bytes());
    mapping.original_markdown_digest = graph.descriptor.original_markdown_digest.clone();
    let mapping_bytes = mapping.to_bytes().unwrap();
    graph.descriptor.mapping_digest = db.put(&mapping_bytes, "application/json").unwrap();
    db.put(b"value\n", "text/plain").unwrap();
    db.write::<_, Error>(|tx| {
        for view in graph.retrieval_views.iter().skip(1) {
            tx.execute(
                "INSERT INTO chunks VALUES ('set',?1,'revision',NULL,?2,1,0,1)",
                params![view.chunk_id, view.prepared_input_digest.as_str()],
            )?;
        }
        Ok(())
    })
    .unwrap();
    let graph_bytes = graph.to_bytes().unwrap();
    let digest = db.put(&graph_bytes, "application/json").unwrap();
    db.record_revision_graph(&scoped(), "set", &digest).unwrap();
    let started = Instant::now();
    let read = db
        .unit_graph(
            &scoped(),
            &GraphKey {
                collection_id: "collection",
                chunk_set_id: "set",
                revision_id: "revision",
            },
        )
        .unwrap()
        .unwrap();
    let elapsed = started.elapsed();
    assert_eq!(read, graph);
    let reader = db.reader().unwrap();
    let plan: String = reader
        .query_row(
            "EXPLAIN QUERY PLAN SELECT graph_digest FROM revision_unit_graphs
        WHERE collection_id='collection' AND chunk_set_id='set' AND revision_id='revision'",
            [],
            |row| row.get(3),
        )
        .unwrap();
    assert!(plan.contains("SEARCH") && plan.contains("INDEX"), "{plan}");
    let count: i64 = reader
        .query_row("SELECT count(*) FROM revision_unit_graphs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
    println!(
        "units={} graph_bytes={} mapping_bytes={} graph_rows={count} read_us={} plan={plan}",
        graph.units.len(),
        graph_bytes.len(),
        mapping_bytes.len(),
        elapsed.as_micros()
    );
}

/// Extends one page with an exact synthetic source contribution and ranked view.
fn append(
    graph: &mut DeliveryGraph,
    mapping: &mut MappingLedger,
    source: &mut String,
    ordinal: usize,
) {
    let start = source.len() as u64;
    source.push_str("value\n");
    let mut unit = graph.units[0].clone();
    unit.unit_id = format!("unit-{ordinal}");
    unit.parent = Some("page".into());
    let part_id = format!("part-extra-{ordinal}");
    let range = SourceRange {
        start,
        end: source.len() as u64,
    };
    let mapping_value = MappingContribution {
        unit_id: format!("canonical-extra-{ordinal}"),
        derived_range: (0, 6),
        mapping_mode: "exact".into(),
    };
    unit.part_ids = vec![part_id.clone()];
    mapping.contributions.push(MappingEntry {
        range,
        mapping: mapping_value.clone(),
    });
    graph.parts.push(Part {
        part_id: part_id.clone(),
        ranges: vec![range],
        mappings: vec![mapping_value],
    });
    graph.groups[0].children.push(unit.unit_id.clone());
    let mut view = graph.retrieval_views[0].clone();
    view.retrieval_view_id = format!("view-{ordinal}");
    view.chunk_id = format!("chunk-{ordinal}");
    view.prepared_input_digest = Digest::of(b"value\n");
    view.token_count = 1;
    view.memberships.truncate(1);
    let member = &mut view.memberships[0];
    member.unit_id = unit.unit_id.clone();
    member.primary_part_ids = vec![part_id];
    graph.units.push(unit);
    graph.retrieval_views.push(view);
}
