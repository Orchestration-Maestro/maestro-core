use super::super::{
    build::{ViewBuildContext, chunk_views, retain_ordered},
    prepared::{GraphIndex, MappedTextIndex},
    profile::{RankedUnit, UnitProfile},
    types::{PartRole, UnitGraphInput},
};
use super::*;
use crate::{chunk_mapping::map_document, chunks::RetrievalChunk};
use std::collections::BTreeSet;

struct TestCounter;

impl TokenCounter for TestCounter {
    fn contract_id(&self) -> &'static str {
        "synthetic-counter/1"
    }
    fn verify(&self) -> Result<(), Error> {
        Ok(())
    }
    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        Ok(input.chars().map(u32::from).collect())
    }
}
use crate::{
    CanonicalizeInput, RevisionKey, canonicalize,
    prepared_inputs::{ChunkContent, Contribution, Fragment, SplitKind},
    source_units::TextRange,
    unit_documents,
};

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "The synthetic one-fragment graph must exercise the actual view-building seam."
)]
fn chunk_membership_includes_only_primary_parts_for_its_fragments() {
    let markdown = concat!(
        "# Operations\n\nOverview.\n\n",
        "## Install\n\nSet the deployment target.\n\n",
        "| Setting | Value |\n|---|---|\n| mode | safe |\n| tier | core |\n\n",
        "1. Prepare.\n   - Check access.\n2. Deploy.\n\n",
        "```sh\nrun deploy\n```\n"
    );
    let document = canonicalize(CanonicalizeInput::new(markdown, "membership.md")).unwrap();
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
    let batch = unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        UnitProfile::new(RankedUnit::CompleteIdeas),
        &TestCounter,
    )
    .unwrap();
    let graph = &batch.graphs[0];
    let mapped = map_document(&document, markdown).unwrap();
    let graph_index = GraphIndex::new(&graph.units, &graph.groups, &graph.context_relations);
    let (unit, source_index) = graph
        .units
        .iter()
        .find_map(|unit| {
            let source_ids: BTreeSet<_> = unit
                .parts
                .iter()
                .filter(|part| part.role == PartRole::Primary)
                .flat_map(|part| part.mappings.iter().map(|mapping| mapping.unit_id.as_str()))
                .collect();
            (source_ids.len() > 1).then(|| {
                let source_id = *source_ids.iter().next().unwrap();
                let index = mapped
                    .units
                    .iter()
                    .position(|source| source.unit_id == source_id)
                    .unwrap();
                (unit, index)
            })
        })
        .expect("fixture has a delivery unit composed from multiple mapped source units");
    let source = &mapped.units[source_index];
    let fragment = Fragment {
        contribution: Contribution {
            unit_index: source_index,
            range: TextRange {
                start: 0,
                end: source.text.len(),
            },
        },
        part_ordinal: 0,
        split: SplitKind::Whole,
    };
    let chunk = RetrievalChunk {
        chunk_id: "partial-membership".into(),
        occurrence_index: 0,
        ordinal: 0,
        content: ChunkContent {
            section_id: None,
            container_ids: Vec::new(),
            heading_path: Vec::new(),
            fragments: vec![fragment],
            table_windows: Vec::new(),
            body_text: source.text.clone(),
            input_parts: Vec::new(),
            prepared_input: source.text.clone(),
            token_count: source.text.len(),
        },
        retrieval_input_fingerprint: "sha256:fixture".into(),
    };
    let text_index = MappedTextIndex::new(&mapped);
    let context = ViewBuildContext {
        scope: &scope,
        input: &input,
        mapped: &mapped,
        units: &graph.units,
        graph_index: &graph_index,
        mapped_text_index: &text_index,
        warning_policy: WarningPolicy::Preserve,
    };
    let views = chunk_views(&context, &[chunk]).unwrap();
    let membership = views
        .iter()
        .flat_map(|view| &view.memberships)
        .find(|membership| membership.unit_id == unit.unit_id)
        .unwrap();
    let expected: BTreeSet<_> = unit
        .parts
        .iter()
        .filter(|part| part.role == PartRole::Primary)
        .filter(|part| {
            part.mappings
                .iter()
                .any(|mapping| mapping.unit_id == source.unit_id)
        })
        .map(|part| part.part_id.as_str())
        .collect();
    assert_eq!(
        membership
            .primary_part_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        expected
    );
}

#[test]
fn ordered_retention_keeps_first_occurrences() {
    let mut values = vec![3, 1, 3, 2, 1];
    retain_ordered(&mut values);
    assert_eq!(values, [3, 1, 2]);
}
