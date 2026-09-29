//! Deterministic page, section, table, procedure and code ancestry.
use super::{
    group_helpers::{
        block_below, normalize, part_maps_under, preceding_lead_in, row_parts, section_group_id,
        sha, structural_group_id, unit_maps_under,
    },
    group_links::{
        assign_family_occurrences, assign_unit_occurrences, group_parts, order_context_relations,
        wire_parentage,
    },
    types::{
        ContextRelation, ContextRelationRecord, DeliveryUnit, FamilyKey, Group, GroupKind,
        PartRole, SourcePart, UnitGraphInput, UnitKind,
    },
};
use crate::{
    content::{Block, BlockType},
    error::Error,
    source_units::MappedDocument,
};
use std::collections::BTreeMap;

/// Groups and explicit typed context edges from one mapped source.
pub(super) struct GroupRecords {
    /// Nonembedded groups in source order.
    pub groups: Vec<Group>,
    /// Required-context relations with group-local ordinals.
    pub context_relations: Vec<ContextRelationRecord>,
}

/// Stable source-family fields reused for every group in one page.
struct FamilyContext {
    /// Collection owning this source family.
    collection_id: String,
    /// Namespace separating otherwise equal source paths.
    source_namespace: String,
    /// Final source path component.
    path_segment: String,
    /// Normalized document title.
    page_title: String,
}

impl FamilyContext {
    /// Build one family key with normalized heading components.
    fn key(&self, heading_path: &[String], occurrence: usize) -> FamilyKey {
        FamilyKey {
            collection_id: self.collection_id.clone(),
            source_namespace: self.source_namespace.clone(),
            path_segment: self.path_segment.clone(),
            page_title: self.page_title.clone(),
            heading_path: heading_path.iter().map(|part| normalize(part)).collect(),
            occurrence,
        }
    }
}

/// Shared mapped input for structural group builders.
struct GroupBuilder<'a, 'doc> {
    /// Canonical document and source identity.
    input: &'a UnitGraphInput<'doc>,
    /// Primary delivery ownership.
    units: &'a [DeliveryUnit],
    /// Mapped source units used to resolve canonical block ancestry.
    mapped: &'a MappedDocument,
    /// Root page group ID.
    page_id: String,
    /// Stable group IDs keyed by canonical section ID.
    section_ids: BTreeMap<String, String>,
    /// Stable source-family fields.
    family: FamilyContext,
}

impl GroupBuilder<'_, '_> {
    /// Build root and nested section groups.
    fn section_groups(&self) -> Result<Vec<Group>, Error> {
        let mut groups = Vec::with_capacity(self.input.document.sections.len());
        let mut occurrences = BTreeMap::<Vec<String>, usize>::new();
        for section in &self.input.document.sections {
            let heading_key = section
                .heading_path
                .iter()
                .map(|part| normalize(part))
                .collect::<Vec<_>>();
            let occurrence = occurrences.entry(heading_key).or_default();
            let group_id = section_group_id(self.input, &section.section_id)?;
            let parent_id = section
                .parent_section_id
                .as_ref()
                .and_then(|id| self.section_ids.get(id))
                .cloned()
                .or_else(|| Some(self.page_id.clone()));
            let children: Vec<_> = self
                .units
                .iter()
                .filter(|unit| {
                    unit.section_id.as_deref() == Some(&section.section_id)
                        || unit.source_block_id == section.section_id
                })
                .collect();
            groups.push(Group {
                group_id,
                kind: GroupKind::Section,
                section_id: Some(section.section_id.clone()),
                parent_id,
                children: children.iter().map(|unit| unit.unit_id.clone()).collect(),
                parts: group_parts(
                    children
                        .iter()
                        .flat_map(|unit| unit.parts.iter())
                        .filter(|part| part.role == PartRole::Primary)
                        .cloned(),
                ),
                family: self.family.key(&section.heading_path, *occurrence),
            });
            *occurrence += 1;
        }
        Ok(groups)
    }

    /// Build table, procedure and code groups with their direct typed context.
    fn structural_groups(&self) -> Result<GroupRecords, Error> {
        let mut records = GroupRecords {
            groups: Vec::new(),
            context_relations: Vec::new(),
        };
        for (index, block) in self
            .input
            .document
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| {
                matches!(
                    block.block_type,
                    BlockType::Table | BlockType::List | BlockType::Code
                )
            })
        {
            let children = self.children_for(block);
            if children.is_empty() {
                continue;
            }
            if block.block_type == BlockType::Table {
                let (table, rows, relations) = self.table_group(block, index, &children)?;
                records.groups.push(table);
                records.groups.extend(rows);
                records.context_relations.extend(relations);
            } else {
                let (group, relations) = self.lead_in_group(block, index, &children)?;
                records.groups.push(group);
                records.context_relations.extend(relations);
            }
        }
        Ok(records)
    }

    /// Build the aggregate table group and its row children.
    fn table_group(
        &self,
        block: &Block,
        index: usize,
        children: &[&DeliveryUnit],
    ) -> Result<(Group, Vec<Group>, Vec<ContextRelationRecord>), Error> {
        let group_id = structural_group_id(self.input, block)?;
        let section_id = block.parent_section_id.clone();
        let parent_id = self.section_parent(section_id.as_deref());
        let rows: Vec<_> = self
            .input
            .document
            .blocks
            .iter()
            .filter(|candidate| {
                candidate.block_type == BlockType::TableRow
                    && block_below(self.input, candidate, &block.block_id)
            })
            .collect();
        let table_children: Vec<_> = children
            .iter()
            .filter(|unit| {
                unit.kind == UnitKind::Table
                    || !rows
                        .iter()
                        .any(|row| unit_maps_under(unit, row, self.input, self.mapped))
            })
            .map(|unit| unit.unit_id.clone())
            .collect();
        let table_parts: Vec<_> = children
            .iter()
            .flat_map(|unit| unit.parts.iter().cloned())
            .filter(|part| part.role == PartRole::Primary)
            .collect();
        let relations = self.header_relations(block, &group_id, &table_parts);
        let row_groups = self.row_groups(&group_id, &rows, children, &table_parts)?;
        let row_ids = row_groups.iter().map(|group| group.group_id.clone());
        let group = Group {
            group_id,
            kind: GroupKind::Table,
            section_id,
            parent_id,
            children: table_children.into_iter().chain(row_ids).collect(),
            parts: group_parts(table_parts.into_iter()),
            family: self.family.key(&block.heading_path, index),
        };
        Ok((group, row_groups, relations))
    }

    /// Build row groups and their row-unit children.
    fn row_groups(
        &self,
        table_id: &str,
        rows: &[&Block],
        table_children: &[&DeliveryUnit],
        table_parts: &[SourcePart],
    ) -> Result<Vec<Group>, Error> {
        let mut groups = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let row_units: Vec<_> = table_children
                .iter()
                .filter(|unit| {
                    unit.kind != UnitKind::Table
                        && unit_maps_under(unit, row, self.input, self.mapped)
                })
                .copied()
                .collect();
            let row_parts = row_parts(&row_units, row, table_parts, self.input, self.mapped);
            if row_parts.is_empty() {
                continue;
            }
            let group_id = format!(
                "row-{}",
                sha(&(
                    &self.input.document.document_id,
                    &self.input.document.revision_id,
                    &row.block_id
                ))?
            );
            groups.push(Group {
                group_id,
                kind: GroupKind::Row,
                section_id: row.parent_section_id.clone(),
                parent_id: Some(table_id.to_owned()),
                children: row_units.iter().map(|unit| unit.unit_id.clone()).collect(),
                parts: group_parts(row_parts.into_iter()),
                family: self.family.key(&row.heading_path, index),
            });
        }
        Ok(groups)
    }

    /// Build a procedure or code group and associate its nearest real paragraph lead-in.
    fn lead_in_group(
        &self,
        block: &Block,
        index: usize,
        children: &[&DeliveryUnit],
    ) -> Result<(Group, Vec<ContextRelationRecord>), Error> {
        let group_id = structural_group_id(self.input, block)?;
        let section_id = block.parent_section_id.clone();
        let mut parts: Vec<_> = children
            .iter()
            .flat_map(|unit| unit.parts.iter().cloned())
            .collect();
        let mut relations = Vec::new();
        if let Some(introduction) = preceding_lead_in(self.input, block) {
            for part in self
                .units
                .iter()
                .filter(|unit| unit.source_block_id == introduction.block_id)
                .flat_map(|unit| &unit.parts)
                .filter(|part| part.role == PartRole::Primary)
            {
                relations.push(ContextRelationRecord {
                    group_id: group_id.clone(),
                    kind: ContextRelation::LeadIn,
                    part_id: part.part_id.clone(),
                    ordinal: 0,
                });
                parts.push(part.clone());
            }
        }
        let group = Group {
            group_id,
            kind: if block.block_type == BlockType::List {
                GroupKind::Procedure
            } else {
                GroupKind::Code
            },
            section_id: section_id.clone(),
            parent_id: self.section_parent(section_id.as_deref()),
            children: children.iter().map(|unit| unit.unit_id.clone()).collect(),
            parts: group_parts(parts.into_iter()),
            family: self.family.key(&block.heading_path, index),
        };
        Ok((group, relations))
    }

    /// Relate each actual table-header part to its table group.
    fn header_relations(
        &self,
        table: &Block,
        group_id: &str,
        parts: &[SourcePart],
    ) -> Vec<ContextRelationRecord> {
        parts
            .iter()
            .filter(|part| self.is_header_part(part, table))
            .map(|part| ContextRelationRecord {
                group_id: group_id.to_owned(),
                kind: ContextRelation::HeaderToTable,
                part_id: part.part_id.clone(),
                ordinal: 0,
            })
            .collect()
    }

    /// Check if a primary part maps into this table's header block.
    fn is_header_part(&self, part: &SourcePart, table: &Block) -> bool {
        self.input.document.blocks.iter().any(|header| {
            header.block_type == BlockType::TableHead
                && block_below(self.input, header, &table.block_id)
                && part_maps_under(part, header, self.input, self.mapped)
        })
    }

    /// Return source-order delivery units belonging below a structural block.
    fn children_for(&self, block: &Block) -> Vec<&DeliveryUnit> {
        self.units
            .iter()
            .filter(|unit| unit_maps_under(unit, block, self.input, self.mapped))
            .collect()
    }

    /// Resolve a section parent, falling back to the page group.
    fn section_parent(&self, section_id: Option<&str>) -> Option<String> {
        section_id
            .and_then(|id| self.section_ids.get(id))
            .cloned()
            .or_else(|| Some(self.page_id.clone()))
    }
}

/// Build page, nested section and structural ancestry with source-order children.
pub(super) fn build_groups(
    input: &UnitGraphInput<'_>,
    units: &mut [DeliveryUnit],
    mapped: &MappedDocument,
) -> Result<GroupRecords, Error> {
    let title = input
        .document
        .blocks
        .iter()
        .find(|block| block.block_type == BlockType::Heading)
        .map(|block| normalize(&block.retrieval_text))
        .unwrap_or_default();
    let source_namespace = if input.source_namespace.trim().is_empty() {
        format!(
            "local:{}:{}",
            input.document.document_id, input.document.revision_id
        )
    } else {
        input.source_namespace.to_owned()
    };
    let source_path = input
        .document
        .source_reference
        .as_deref()
        .unwrap_or(&input.document.document_id);
    let family = FamilyContext {
        collection_id: input.collection_id.to_owned(),
        source_namespace,
        path_segment: source_path
            .rsplit('/')
            .next()
            .unwrap_or(source_path)
            .to_owned(),
        page_title: title,
    };
    let page_id = format!(
        "page-{}",
        sha(&(&input.document.document_id, &input.document.revision_id))?
    );
    let section_ids: BTreeMap<_, _> = input
        .document
        .sections
        .iter()
        .map(|section| {
            Ok((
                section.section_id.clone(),
                section_group_id(input, &section.section_id)?,
            ))
        })
        .collect::<Result<_, Error>>()?;
    let builder = GroupBuilder {
        input,
        units,
        mapped,
        page_id: page_id.clone(),
        section_ids,
        family,
    };
    let mut groups = vec![page_group(input, units, &page_id, &builder.family)?];
    groups.extend(builder.section_groups()?);
    let mut structural = builder.structural_groups()?;
    groups.append(&mut structural.groups);
    wire_parentage(&mut groups, units);
    groups.sort_by_key(|group| {
        let start = group
            .parts
            .iter()
            .flat_map(|part| &part.ranges)
            .map(|range| range.start)
            .min()
            .unwrap_or(usize::MAX);
        (start, group.group_id.clone())
    });
    assign_family_occurrences(&mut groups);
    assign_unit_occurrences(units);
    order_context_relations(&groups, &mut structural.context_relations);
    Ok(GroupRecords {
        groups,
        context_relations: structural.context_relations,
    })
}

/// Create the root page group and collect all primary descendants.
fn page_group(
    input: &UnitGraphInput<'_>,
    units: &[DeliveryUnit],
    page_id: &str,
    family: &FamilyContext,
) -> Result<Group, Error> {
    Ok(Group {
        group_id: page_id.to_owned(),
        kind: GroupKind::Page,
        section_id: None,
        parent_id: None,
        children: input
            .document
            .sections
            .iter()
            .map(|section| section_group_id(input, &section.section_id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .chain(
                units
                    .iter()
                    .filter(|unit| unit.section_id.is_none())
                    .map(|unit| unit.unit_id.clone()),
            )
            .collect(),
        parts: group_parts(
            units
                .iter()
                .flat_map(|unit| unit.parts.iter())
                .filter(|part| part.role == PartRole::Primary)
                .cloned(),
        ),
        family: family.key(&[], 0),
    })
}
