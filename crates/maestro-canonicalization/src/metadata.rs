//! Merge supplied metadata without guessing provenance or permissions.
use crate::BlockAttributes;
use crate::parse::{Kind, Node, text};
use crate::{Finding, Severity, SourceMetadata, SourceSpan};
use serde_json::Value;
use std::collections::BTreeMap;

/// A finding that names no block, located at one source span when it has one.
pub(crate) fn finding(
    code: &str,
    message: impl Into<String>,
    severity: Severity,
    span: Option<SourceSpan>,
) -> Finding {
    Finding {
        severity,
        code: code.into(),
        message: message.into(),
        block_id: None,
        source_spans: span.into_iter().collect(),
    }
}

/// Merge YAML front matter into the supplied metadata: every original value is kept, and a conflict
/// is reported rather than resolved.
pub(crate) fn merge(nodes: &[Node], supplied: &SourceMetadata) -> (SourceMetadata, Vec<Finding>) {
    let mut merged = supplied.clone();
    let mut warnings = Vec::new();
    for node in nodes {
        if !matches!(node.kind, Kind::Block(BlockAttributes::Metadata)) {
            continue;
        }
        match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text(node))
            .and_then(serde_yaml_ng::from_value::<BTreeMap<String, Value>>)
        {
            Ok(map) => {
                // Preserve every original metadata value, even a conflicting one.
                if merged.extra.contains_key("markdown_frontmatter") {
                    warnings.push(finding(
                        "metadata_conflict",
                        "reserved markdown_frontmatter key supplied",
                        Severity::Error,
                        Some(node.span),
                    ));
                } else {
                    merged
                        .extra
                        .insert("markdown_frontmatter".into(), serde_json::json!(map));
                }
                for (key, value) in &map {
                    if key != "converter" {
                        merge_field(&mut merged, key, value, node.span, &mut warnings);
                    }
                }
                if let Some(converter) = map.get("converter").filter(|v| !v.is_null()) {
                    merge_converter(&mut merged, converter, node.span, &mut warnings);
                }
            }
            Err(_) => warnings.push(finding(
                "invalid_frontmatter",
                "frontmatter is not a valid YAML mapping; original bytes retained",
                Severity::Error,
                Some(node.span),
            )),
        }
    }
    for (name, missing) in [
        (
            "source_reference",
            merged
                .source_reference
                .as_deref()
                .is_none_or(|s| s.trim().is_empty()),
        ),
        (
            "title",
            merged.title.as_deref().is_none_or(|s| s.trim().is_empty()),
        ),
        (
            "language",
            merged
                .language
                .as_deref()
                .is_none_or(|s| s.trim().is_empty()),
        ),
        (
            "extraction",
            merged.extraction.as_ref().is_none_or(Value::is_null),
        ),
        (
            "access_policy",
            merged.access_policy.as_ref().is_none_or(Value::is_null),
        ),
    ] {
        if missing {
            warnings.push(finding(
                "missing_metadata",
                format!("{name} is unknown; not inferred"),
                Severity::Warning,
                None,
            ));
        }
    }
    // Empty strings/null are absence, not a provenance or permission claim.
    for field in [
        &mut merged.source_reference,
        &mut merged.title,
        &mut merged.language,
    ] {
        if field.as_deref().is_some_and(|s| s.trim().is_empty()) {
            *field = None;
        }
    }
    for field in [&mut merged.extraction, &mut merged.access_policy] {
        if field.as_ref().is_some_and(Value::is_null) {
            *field = None;
        }
    }
    (merged, warnings)
}

/// Merge one front-matter field into its typed metadata field; a conflicting or mistyped value is a
/// finding.
fn merge_field(
    merged: &mut SourceMetadata,
    key: &str,
    value: &Value,
    span: SourceSpan,
    warnings: &mut Vec<Finding>,
) {
    if value.is_null() || value.as_str().is_some_and(str::is_empty) {
        return;
    }
    match key {
        "source_url" | "source_reference" | "title" | "language" => {
            let Some(text) = value.as_str() else {
                warnings.push(finding(
                    "invalid_metadata_type",
                    format!("{key} must be a string or null"),
                    Severity::Error,
                    Some(span),
                ));
                return;
            };
            let field = match key {
                "source_url" | "source_reference" => &mut merged.source_reference,
                "title" => &mut merged.title,
                _ => &mut merged.language,
            };
            if field.as_deref().is_some_and(|s| s != text) {
                warnings.push(finding(
                    "metadata_conflict",
                    format!("conflicting {key}; supplied and Markdown values retained"),
                    Severity::Error,
                    Some(span),
                ));
            } else {
                *field = Some(text.to_owned());
            }
        }
        "access_policy" | "extraction" => {
            let field = if key == "access_policy" {
                &mut merged.access_policy
            } else {
                &mut merged.extraction
            };
            if field.as_ref().is_some_and(|v| v != value) {
                warnings.push(finding(
                    "metadata_conflict",
                    format!("conflicting {key}; no resolution inferred"),
                    Severity::Error,
                    Some(span),
                ));
            } else {
                *field = Some(value.clone());
            }
        }
        _ => {} // Original map above retains unknown fields without interpreting them.
    }
}

/// Record the front matter's converter in the extraction details, unless they already name another.
fn merge_converter(
    merged: &mut SourceMetadata,
    converter: &Value,
    span: SourceSpan,
    warnings: &mut Vec<Finding>,
) {
    match &mut merged.extraction {
        None | Some(Value::Null) => {
            merged.extraction = Some(serde_json::json!({"converter": converter}));
        }
        Some(Value::Object(details)) => {
            if details.get("converter").is_some_and(|v| v != converter) {
                warnings.push(finding(
                    "metadata_conflict",
                    "converter conflicts with extraction details",
                    Severity::Error,
                    Some(span),
                ));
            } else {
                details.insert("converter".into(), converter.clone());
            }
        }
        Some(_) => warnings.push(finding(
            "extraction_unmerged",
            "opaque extraction details and converter retained separately",
            Severity::Warning,
            Some(span),
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::{CanonicalDocument, CanonicalizeInput, canonicalize};

    /// The codes of a document's findings.
    fn codes(doc: &CanonicalDocument) -> Vec<&str> {
        doc.warnings.iter().map(|f| f.code.as_str()).collect()
    }

    #[test]
    fn null_and_empty_front_matter_values_are_absent_not_invalid() {
        let markdown = "---\ntitle: ''\nlanguage:\n---\nBody\n";
        let doc = canonicalize(CanonicalizeInput::new(markdown, "absent.md")).unwrap();
        assert!(
            !codes(&doc).contains(&"invalid_metadata_type"),
            "{:?}",
            codes(&doc)
        );
        assert_eq!(doc.source_metadata.title, None);
        assert_eq!(doc.source_metadata.language, None);
    }

    #[test]
    fn only_a_different_front_matter_value_conflicts_with_the_supplied_one() {
        let mut input = CanonicalizeInput::new("---\ntitle: Guide\n---\nBody\n", "title.md");
        input.metadata.title = Some("Guide".into());
        let agreeing = canonicalize(input.clone()).unwrap();
        assert!(!codes(&agreeing).contains(&"metadata_conflict"));
        input.metadata.title = Some("Manual".into());
        let conflicting = canonicalize(input).unwrap();
        assert!(codes(&conflicting).contains(&"metadata_conflict"));
        assert_eq!(conflicting.source_metadata.title.as_deref(), Some("Manual"));
    }
}
