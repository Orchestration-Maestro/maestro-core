//! Suite binding and aggregate label checking.
use super::{
    label_format::Label,
    label_proofs::check_proofs,
    label_types::{
        CheckError, CheckedItem, CheckedLabels, LabelCode, LabelSummary, Original, QuestionKind,
        Stage,
    },
    label_validation::{error, safe_id, validate_identity, validate_question},
};
use crate::suite::{Language, Suite};
use maestro_kernel::artifact::Digest;
use std::collections::{BTreeMap, BTreeSet};

/// Check exact frozen labels against the suite and digest-pinned source authority.
///
/// # Errors
/// Returns a safe label refusal or the source authority callback error.
pub fn check_labels<E>(
    suite: &Suite,
    text: &str,
    digest: &Digest,
    stage: Stage,
    mut source: impl FnMut(&str, &Digest) -> Result<Option<Original>, E>,
) -> Result<CheckedLabels, CheckError<E>> {
    if Digest::of(text.as_bytes()) != *digest {
        return Err(error(LabelCode::DigestMismatch, 0, None));
    }
    let mut ids = BTreeSet::new();
    let mut families = BTreeSet::new();
    let mut items = Vec::new();
    let mut links_n = 0;
    let mut anchors_n = 0;
    let mut unreviewed = 0;
    let mut kinds = BTreeMap::new();
    let mut languages = BTreeMap::new();
    let questions = &suite.questions;
    let mut expected: BTreeMap<String, (bool, String)> = BTreeMap::new();
    for question in questions {
        if !safe_id(&question.id) {
            return Err(error(LabelCode::UnsafeId, 0, None));
        }
        expected.insert(
            question.id.clone(),
            (
                question.answerable,
                match question.language {
                    Language::En => "en",
                    Language::Fr => "fr",
                }
                .to_owned(),
            ),
        );
    }
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            return Err(error(LabelCode::Malformed, idx + 1, None));
        }
        let label: Label =
            serde_json::from_str(line).map_err(|_| error(LabelCode::Malformed, idx + 1, None))?;
        if label.schema != "maestro-graph-labels/1" {
            return Err(error(LabelCode::Malformed, idx + 1, None));
        }
        validate_identity(&label, idx + 1)?;
        if !ids.insert(label.id.clone()) {
            return Err(error(LabelCode::DuplicateItem, idx + 1, Some(label.id)));
        }
        let Some((answerable, language)) = expected.get(&label.id) else {
            return Err(error(LabelCode::UnknownItem, idx + 1, Some(label.id)));
        };
        if !families.insert(label.family.clone()) {
            return Err(error(
                LabelCode::DuplicateFamily,
                idx + 1,
                Some(label.id.clone()),
            ));
        }
        let kind = validate_question(&label, *answerable, language, stage, idx + 1)?;
        if label.review.is_none() {
            unreviewed += 1;
        }
        let (alternatives, link_count, anchor_count) =
            check_proofs(&label, kind, idx + 1, &mut source)?;
        links_n += link_count;
        anchors_n += anchor_count;
        *kinds.entry(label.kind).or_default() += 1;
        *languages.entry(label.language).or_default() += 1;
        items.push(CheckedItem {
            id: label.id,
            family: label.family,
            kind,
            proofs: alternatives,
        });
    }
    for id in expected.keys() {
        if !ids.contains(id) {
            return Err(error(LabelCode::MissingLabel, 0, Some(id.clone())));
        }
    }
    let answerable = items
        .iter()
        .filter(|i| i.kind != QuestionKind::Unanswerable)
        .count();
    let unanswerable = items.len() - answerable;
    Ok(CheckedLabels {
        summary: LabelSummary {
            items: items.len(),
            answerable,
            unanswerable,
            families: families.len(),
            links: links_n,
            anchors: anchors_n,
            unreviewed,
            digest: digest.clone(),
            kinds,
            languages,
        },
        items,
    })
}
