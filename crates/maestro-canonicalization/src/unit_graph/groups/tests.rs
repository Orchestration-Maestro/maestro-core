use super::*;
use crate::{
    CanonicalizeInput, DedupScope, Error, RevisionKey, SourceRange, SplitMarker, TokenCounter,
    WarningPolicy, canonicalize,
    chunk_mapping::map_document,
    unit_graph::{UnitProfile, profile::RankedUnit, unit_documents},
};
use std::cell::Cell;

#[test]
fn table_group_contains_exact_row_groups_and_not_their_units() {
    let markdown = "# Guide\n\n| Name | Value |\n|---|---|\n| first | 1 |\n| second | 2 |\n";
    let document = canonicalize(CanonicalizeInput::new(markdown, "table.md")).unwrap();
    let input = UnitGraphInput {
        document: &document,
        markdown,
        collection_id: "collection",
        source_namespace: "synthetic",
    };
    let scope = DedupScope {
        tenant_id: "tenant".into(),
        workspace_id: "collection".into(),
        authorized_revisions: [RevisionKey {
            document_id: document.document_id.clone(),
            revision_id: document.revision_id.clone(),
        }]
        .into(),
    };
    let mut profile = UnitProfile::new(RankedUnit::V2Unit);
    profile.size_limits.table_tokens = 1;
    let batch = unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        profile,
        &Counter(Cell::new(0)),
    )
    .unwrap();
    let graph = &batch.graphs[0];
    let mapped = map_document(&document, markdown).unwrap();
    let index = SourceIndex::new(&input, &mapped);
    let table_block = document
        .blocks
        .iter()
        .find(|block| block.block_type == BlockType::Table)
        .unwrap();
    let row_blocks: Vec<_> = index
        .descendants(&table_block.block_id)
        .iter()
        .copied()
        .filter(|block| block.block_type == BlockType::TableRow)
        .collect();
    assert_eq!(row_blocks.len(), 2);
    let mut units = graph.units.clone();
    units.push(synthetic_unit(
        "extra-row",
        UnitKind::Row,
        &row_blocks[0].block_id,
    ));
    units.push(synthetic_unit(
        "table-row",
        UnitKind::Table,
        &row_blocks[0].block_id,
    ));
    units.push(synthetic_unit(
        "table-extra",
        UnitKind::Block,
        &table_block.block_id,
    ));
    let mut units_by_ancestor = BTreeMap::<String, Vec<usize>>::new();
    for (position, unit) in units.iter().enumerate() {
        for ancestor in index.ancestors_of(&unit.source_block_id) {
            units_by_ancestor
                .entry(ancestor.to_owned())
                .or_default()
                .push(position);
        }
    }
    let builder = GroupBuilder {
        input: &input,
        units: &units,
        index: &index,
        units_by_ancestor,
        page_id: "page".into(),
        section_ids: BTreeMap::new(),
        family: FamilyContext {
            collection_id: "collection".into(),
            source_namespace: "synthetic".into(),
            path_segment: "table.md".into(),
            page_title: "guide".into(),
        },
    };
    let block = table_block;
    let children: Vec<_> = builder
        .unit_indices_under(&block.block_id)
        .iter()
        .filter_map(|position| units.get(*position))
        .collect();
    let (table, rows, _) = builder.table_group(block, &children).unwrap();
    let row_group_ids: BTreeSet<_> = rows.iter().map(|row| row.group_id.as_str()).collect();
    assert_eq!(row_group_ids.len(), 2);
    assert!(
        row_group_ids
            .iter()
            .all(|row_id| table.children.contains(&row_id.to_string()))
    );
    assert!(!table.children.contains(&"extra-row".to_owned()));
    assert!(table.children.contains(&"table-row".to_owned()));
    assert!(table.children.contains(&"table-extra".to_owned()));
}

fn synthetic_unit(unit_id: &str, kind: UnitKind, block_id: &str) -> DeliveryUnit {
    DeliveryUnit {
        unit_id: unit_id.into(),
        kind,
        source_block_id: block_id.into(),
        section_id: None,
        heading_path: Vec::new(),
        parent_id: None,
        parts: vec![SourcePart {
            part_id: format!("part-{unit_id}"),
            role: PartRole::Primary,
            ordinal: 0,
            ranges: vec![SourceRange { start: 1, end: 2 }],
            mappings: Vec::new(),
        }],
        split: SplitMarker::Whole,
    }
}

struct Counter(Cell<usize>);

impl TokenCounter for Counter {
    fn contract_id(&self) -> &'static str {
        "test/table-group"
    }

    fn verify(&self) -> Result<(), Error> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }

    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        Ok(input.chars().map(u32::from).collect())
    }
}
