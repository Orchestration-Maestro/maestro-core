//! Label identity, question and review constraints.
use super::{
    label_format::{Disposition, Label, OwnerRuling},
    label_types::{CheckError, LabelCode, LabelError, QuestionKind, Stage},
};

/// Construct a safe label refusal.
pub(super) fn error<E>(code: LabelCode, line: usize, item: Option<String>) -> CheckError<E> {
    CheckError::Label(LabelError { code, line, item })
}
/// Reject unsafe identifiers before including an item ID in an error.
pub(super) fn validate_identity<E>(label: &Label, line: usize) -> Result<(), CheckError<E>> {
    if !safe_id(&label.id) || !safe_id(&label.family) {
        return Err(error(LabelCode::UnsafeId, line, None));
    }
    Ok(())
}
/// Whether an identifier is safe to include in a public refusal.
pub(super) fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-.:".contains(&byte))
}
/// Validate suite agreement and required proof/review fields.
pub(super) fn validate_question<E>(
    label: &Label,
    answerable: bool,
    language: &str,
    stage: Stage,
    line: usize,
) -> Result<QuestionKind, CheckError<E>> {
    if language != label.language {
        return Err(error(
            LabelCode::LanguageMismatch,
            line,
            Some(label.id.clone()),
        ));
    }
    let kind = match label.kind.as_str() {
        "dependency" => QuestionKind::Dependency,
        "relationship" => QuestionKind::Relationship,
        "multi_hop" => QuestionKind::MultiHop,
        "version_difference" => QuestionKind::VersionDifference,
        "unanswerable" => QuestionKind::Unanswerable,
        _ => {
            return Err(error(LabelCode::Vocabulary, line, Some(label.id.clone())));
        }
    };
    if answerable == (kind == QuestionKind::Unanswerable) {
        return Err(error(
            LabelCode::Answerability,
            line,
            Some(label.id.clone()),
        ));
    }
    if kind == QuestionKind::Unanswerable && !label.proofs.is_empty() {
        return Err(error(
            LabelCode::Answerability,
            line,
            Some(label.id.clone()),
        ));
    }
    if kind == QuestionKind::Unanswerable
        && label
            .unanswerable_reason
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return Err(error(
            LabelCode::UnanswerableRationale,
            line,
            Some(label.id.clone()),
        ));
    }
    validate_review(label, stage, line)?;
    if kind != QuestionKind::Unanswerable && label.proofs.is_empty() {
        return Err(error(
            LabelCode::Answerability,
            line,
            Some(label.id.clone()),
        ));
    }
    Ok(kind)
}
/// Frozen labels need independent review and resolved owner decisions.
fn validate_review<E>(label: &Label, stage: Stage, line: usize) -> Result<(), CheckError<E>> {
    let review = label.review.as_ref();
    if review.is_none() && stage == Stage::Frozen {
        return Err(error(
            LabelCode::MissingReview,
            line,
            Some(label.id.clone()),
        ));
    }
    if let Some(review) = review {
        if review.reviewer_card.is_empty() {
            return Err(error(
                LabelCode::MissingReview,
                line,
                Some(label.id.clone()),
            ));
        }
        if review.disposition == Disposition::Flagged && review.owner.is_none() {
            return Err(error(
                LabelCode::UnresolvedReview,
                line,
                Some(label.id.clone()),
            ));
        }
        if review.owner == Some(OwnerRuling::Rejected) {
            return Err(error(
                LabelCode::RejectedReview,
                line,
                Some(label.id.clone()),
            ));
        }
    }
    Ok(())
}
