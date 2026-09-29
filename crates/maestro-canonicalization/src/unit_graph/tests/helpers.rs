use super::*;
use crate::{
    model::SourceSpan,
    source_units::{MappingRun, OriginMode, SourceOrigin, SourceUnit, TextRange, UnitField},
    unit_graph::types::{MappingContribution, SplitMarker},
};

pub(super) fn part(part_id: &str, role: PartRole, start: usize) -> SourcePart {
    SourcePart {
        part_id: part_id.into(),
        role,
        ordinal: 0,
        ranges: vec![SourceRange {
            start,
            end: start + 1,
        }],
        mappings: vec![MappingContribution {
            unit_id: part_id.into(),
            derived_range: (0, 1),
            mapping_mode: "exact".into(),
        }],
    }
}

pub(super) fn delivery_unit(unit_id: &str, role: PartRole, start: usize) -> DeliveryUnit {
    DeliveryUnit {
        unit_id: unit_id.into(),
        kind: UnitKind::Block,
        source_block_id: "block".into(),
        section_id: None,
        heading_path: Vec::new(),
        parent_id: None,
        parts: vec![part(unit_id, role, start)],
        split: SplitMarker::Whole,
    }
}

pub(super) fn graph_for(markdown: &str) -> DeliveryGraph {
    graph_for_profile(markdown, UnitProfile::new(RankedUnit::V2Unit))
}

pub(super) fn graph_for_profile(markdown: &str, profile: UnitProfile) -> DeliveryGraph {
    let document = canonicalize(CanonicalizeInput::new(markdown, "groups.md")).unwrap();
    let scope = DedupScope {
        tenant_id: "tenant".into(),
        workspace_id: "collection".into(),
        authorized_revisions: [RevisionKey {
            document_id: document.document_id.clone(),
            revision_id: document.revision_id.clone(),
        }]
        .into(),
    };
    let input = UnitGraphInput {
        document: &document,
        markdown,
        collection_id: "collection",
        source_namespace: "synthetic",
    };
    unit_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        profile,
        &FixtureCounter,
    )
    .unwrap()
    .graphs[0]
        .clone()
}

pub(super) fn source_unit(spans: Vec<(usize, usize)>) -> SourceUnit {
    SourceUnit {
        unit_id: "unit".into(),
        block_id: "block".into(),
        field: UnitField::Inline {
            child_path: vec![0],
        },
        primary: true,
        text: "ab".into(),
        mappings: spans
            .into_iter()
            .enumerate()
            .map(|(index, (start, end))| MappingRun {
                range: TextRange {
                    start: index,
                    end: index + 1,
                },
                mode: OriginMode::ExactCopy,
                origins: vec![SourceOrigin {
                    block_id: "block".into(),
                    span: SourceSpan { start, end },
                }],
            })
            .collect(),
        envelopes: Vec::new(),
    }
}
