//! Retrieval preparation from ancestor headings and typed context relations.
use super::{
    profile::UnitProfile,
    types::{DeliveryUnit, SourcePart, UnitKind},
};
use crate::{error::Error, source_units::MappedDocument, tokenizer::TokenCounter};
use std::collections::{BTreeMap, BTreeSet};

/// Resolve typed heading parts from section and page ancestors.
pub(super) fn unit_heading_parts(
    unit: &DeliveryUnit,
    units: &[DeliveryUnit],
    groups: &[super::types::Group],
) -> Result<Vec<SourcePart>, Error> {
    let groups_by_id: BTreeMap<_, _> = groups
        .iter()
        .map(|group| (group.group_id.as_str(), group))
        .collect();
    let mut ancestors = Vec::new();
    let mut parent = unit.parent_id.as_deref();
    while let Some(group_id) = parent {
        let group = groups_by_id
            .get(group_id)
            .ok_or_else(|| Error("delivery unit has an unknown group ancestor".into()))?;
        ancestors.push(*group);
        parent = group.parent_id.as_deref();
    }
    ancestors.reverse();
    let mut headings = Vec::new();
    for group in ancestors {
        if group.kind == super::types::GroupKind::Section && group.family.heading_path.len() == 1 {
            continue;
        }
        let heading_unit = units.iter().find(|candidate| match group.kind {
            super::types::GroupKind::Page => {
                candidate.kind == UnitKind::Section && candidate.heading_path.len() == 1
            }
            super::types::GroupKind::Section => {
                candidate.kind == UnitKind::Section
                    && candidate.source_block_id == group.section_id.as_deref().unwrap_or_default()
            }
            _ => false,
        });
        if let Some(heading_unit) = heading_unit {
            headings.extend(heading_unit.parts.iter().cloned());
        }
    }
    Ok(headings)
}

/// Resolve context only from typed relations on structural ancestors.
pub(super) fn unit_context_parts(
    unit: &DeliveryUnit,
    units: &[DeliveryUnit],
    groups: &[super::types::Group],
    relations: &[super::types::ContextRelationRecord],
) -> Result<Vec<SourcePart>, Error> {
    let group_ids: BTreeMap<_, _> = groups
        .iter()
        .map(|group| (group.group_id.as_str(), group))
        .collect();
    let parts: BTreeMap<_, _> = units
        .iter()
        .flat_map(|unit| &unit.parts)
        .map(|part| (part.part_id.as_str(), part))
        .collect();
    let primary_ids: BTreeSet<_> = unit
        .parts
        .iter()
        .map(|part| part.part_id.as_str())
        .collect();
    let mut context_ids = BTreeSet::new();
    let mut contexts = Vec::new();
    let mut parent = unit.parent_id.as_deref();
    while let Some(group_id) = parent {
        let group = group_ids
            .get(group_id)
            .ok_or_else(|| Error("delivery unit has an unknown group ancestor".into()))?;
        for relation in relations
            .iter()
            .filter(|relation| relation.group_id == group_id)
        {
            if primary_ids.contains(relation.part_id.as_str())
                || !context_ids.insert(relation.part_id.as_str())
            {
                continue;
            }
            let part = parts.get(relation.part_id.as_str()).ok_or_else(|| {
                Error("context relation references an unowned source part".into())
            })?;
            contexts.push((*part).clone());
        }
        parent = group.parent_id.as_deref();
    }
    contexts.sort_by_key(|part| {
        (
            part.ranges
                .iter()
                .map(|range| range.start)
                .min()
                .unwrap_or(usize::MAX),
            part.part_id.clone(),
        )
    });
    Ok(contexts)
}

/// Source data needed to prepare and measure all delivery units.
pub(super) struct PreparedInputs<'a> {
    /// Delivery units.
    pub units: &'a [DeliveryUnit],
    /// Mapped canonical source.
    pub mapped: &'a MappedDocument,
    /// Structural groups.
    pub groups: &'a [super::types::Group],
    /// Typed context relations.
    pub context_relations: &'a [super::types::ContextRelationRecord],
}

/// Count each complete prepared unit with the verified counter and refuse beyond its pinned kind.
pub(super) fn validate_unit_sizes(
    input: &PreparedInputs<'_>,
    profile: UnitProfile,
    counter: &(impl TokenCounter + ?Sized),
) -> Result<(), Error> {
    for unit in input.units {
        let contexts =
            unit_context_parts(unit, input.units, input.groups, input.context_relations)?;
        let headings = unit_heading_parts(unit, input.units, input.groups)?;
        let prepared = prepared_text(unit, input.mapped, &contexts, &headings)?;
        let count = counter.token_ids(&prepared)?.len();
        if count > size_limit(profile, unit.kind) {
            return Err(Error(format!(
                "oversized_unit_refusal: {} counter tokens exceed the provisional {:?} limit",
                unit.unit_id, unit.kind
            )));
        }
    }
    Ok(())
}

/// Render one unit's actual heading and source-mapped context plus primary contributions.
pub(super) fn prepared_text(
    unit: &DeliveryUnit,
    mapped: &MappedDocument,
    contexts: &[SourcePart],
    heading_parts: &[SourcePart],
) -> Result<String, Error> {
    let mut primary_ids = BTreeSet::new();
    let mut context_ids = BTreeSet::new();
    for part in &unit.parts {
        primary_ids.extend(part.mappings.iter().map(|mapping| mapping.unit_id.as_str()));
    }
    for part in contexts {
        context_ids.extend(part.mappings.iter().map(|mapping| mapping.unit_id.as_str()));
    }
    let text_for = |ids: &BTreeSet<&str>| -> Result<String, Error> {
        ids.iter()
            .map(|id| {
                mapped
                    .units
                    .iter()
                    .find(|source| source.unit_id == *id)
                    .map(|source| source.text.as_str())
                    .ok_or_else(|| {
                        Error("delivery unit mapping names a missing source unit".into())
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|pieces| pieces.join("\n"))
    };
    let body = text_for(&primary_ids)?;
    let context = text_for(&context_ids)?;
    let heading_text: Vec<_> = heading_parts
        .iter()
        .flat_map(|part| &part.mappings)
        .map(|mapping| {
            mapped
                .units
                .iter()
                .find(|source| source.unit_id == mapping.unit_id)
                .map(|source| source.text.as_str())
                .ok_or_else(|| Error("heading mapping names a missing source unit".into()))
        })
        .collect::<Result<_, _>>()?;
    let headings = if heading_text.is_empty() {
        unit.heading_path.join("\n")
    } else {
        heading_text.join(" / ")
    };
    Ok([headings, context, body]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n"))
}

/// Select the profile's provisional verified-counter limit for one unit kind.
pub(super) fn size_limit(profile: UnitProfile, kind: UnitKind) -> usize {
    let limits = profile.size_limits();
    match kind {
        UnitKind::Section => limits.section_tokens,
        UnitKind::Table => limits.table_tokens,
        UnitKind::Row => limits.row_tokens,
        UnitKind::Procedure => limits.procedure_tokens,
        UnitKind::Code => limits.code_tokens,
        UnitKind::Paragraphs | UnitKind::Block => limits.paragraphs_tokens,
    }
}
