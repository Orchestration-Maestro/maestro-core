//! Validate every link and exact authority anchor in each alternative.
use super::{
    label_format::{Anchor, Label, Link, Proof},
    label_types::{CheckError, LabelCode, Located, Original, QuestionKind},
    label_validation::error,
};
use maestro_kernel::artifact::Digest;
use std::str::from_utf8;

/// Resolved alternatives, relation count and anchor count.
type Proofs = (Vec<Vec<Located>>, usize, usize);

/// Check alternatives and count their links and anchors.
pub(super) fn check_proofs<E>(
    label: &Label,
    kind: QuestionKind,
    line: usize,
    source: &mut impl FnMut(&str, &Digest) -> Result<Option<Original>, E>,
) -> Result<Proofs, CheckError<E>> {
    let mut links_n = 0;
    let mut anchors_n = 0;
    let mut alternatives = Vec::new();
    for proof in &label.proofs {
        check_shape(proof, kind, label, line)?;
        let mut proof_anchors = Vec::new();
        for link in &proof.links {
            check_link(link, kind, label, line)?;
            links_n += 1;
            for anchor in &link.anchors {
                proof_anchors.push(check_anchor(anchor, label, line, source)?);
                anchors_n += 1;
            }
        }
        alternatives.push(proof_anchors);
    }
    Ok((alternatives, links_n, anchors_n))
}
/// Validate the topology required by the question kind.
fn check_shape<E>(
    proof: &Proof,
    kind: QuestionKind,
    label: &Label,
    line: usize,
) -> Result<(), CheckError<E>> {
    if proof.links.is_empty() {
        return Err(error(LabelCode::Vocabulary, line, Some(label.id.clone())));
    }
    if kind == QuestionKind::MultiHop && proof.links.len() < 2 {
        return Err(error(LabelCode::Vocabulary, line, Some(label.id.clone())));
    }
    if kind == QuestionKind::MultiHop
        && proof
            .links
            .iter()
            .zip(proof.links.iter().skip(1))
            .any(|(before, after)| before.object != after.subject)
    {
        return Err(error(LabelCode::Vocabulary, line, Some(label.id.clone())));
    }
    if kind == QuestionKind::VersionDifference
        && proof.links.iter().all(|link| link.version.is_none())
    {
        return Err(error(LabelCode::Vocabulary, line, Some(label.id.clone())));
    }
    Ok(())
}
/// Validate the closed relation vocabulary and nonempty anchors.
fn check_link<E>(
    link: &Link,
    kind: QuestionKind,
    label: &Label,
    line: usize,
) -> Result<(), CheckError<E>> {
    let allowed: &[&str] = match kind {
        QuestionKind::Dependency => &["DEPENDS_ON", "REQUIRES"],
        QuestionKind::MultiHop => &["DEPENDS_ON", "REQUIRES", "PART_OF"],
        _ => &[
            "DEPENDS_ON",
            "REQUIRES",
            "PART_OF",
            "DEFAULTS_TO",
            "INTRODUCED_IN",
            "DEPRECATED_IN",
            "REPLACES",
            "APPLIES_TO",
        ],
    };
    if !allowed.contains(&link.predicate.as_str())
        || link.subject.is_empty()
        || link.object.is_empty()
        || link.conditions.iter().any(String::is_empty)
    {
        return Err(error(LabelCode::Vocabulary, line, Some(label.id.clone())));
    }
    if link.anchors.is_empty() {
        return Err(error(
            LabelCode::MissingAnchor,
            line,
            Some(label.id.clone()),
        ));
    }
    Ok(())
}
/// Resolve and verify a digest-pinned UTF-8 span against original bytes.
fn check_anchor<E>(
    anchor: &Anchor,
    label: &Label,
    line: usize,
    source: &mut impl FnMut(&str, &Digest) -> Result<Option<Original>, E>,
) -> Result<Located, CheckError<E>> {
    let original = Digest::parse(&anchor.original)
        .map_err(|_| error(LabelCode::UnknownSource, line, Some(label.id.clone())))?;
    let Some(src) = source(&anchor.source_ref, &original).map_err(CheckError::Source)? else {
        return Err(error(
            LabelCode::UnknownSource,
            line,
            Some(label.id.clone()),
        ));
    };
    if Digest::of(&src.bytes) != original {
        return Err(error(
            LabelCode::UnknownSource,
            line,
            Some(label.id.clone()),
        ));
    }
    let [start, end] = anchor.span;
    if start >= end || end > src.bytes.len() {
        return Err(error(
            LabelCode::SpanOutOfRange,
            line,
            Some(label.id.clone()),
        ));
    }
    if !from_utf8(&src.bytes)
        .is_ok_and(|text| text.is_char_boundary(start) && text.is_char_boundary(end))
    {
        return Err(error(
            LabelCode::SpanOffBoundary,
            line,
            Some(label.id.clone()),
        ));
    }
    let quote = Digest::parse(&anchor.quote)
        .map_err(|_| error(LabelCode::QuoteMismatch, line, Some(label.id.clone())))?;
    if Digest::of(
        src.bytes
            .get(start..end)
            .ok_or_else(|| error(LabelCode::SpanOutOfRange, line, Some(label.id.clone())))?,
    ) != quote
    {
        return Err(error(
            LabelCode::QuoteMismatch,
            line,
            Some(label.id.clone()),
        ));
    }
    Ok(Located {
        revision_id: src.revision_id.clone(),
        span: [start, end],
    })
}
