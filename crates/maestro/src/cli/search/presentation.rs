//! The CLI's bounded evidence document and text view.

use super::super::output::Output;
use crate::{
    failure::Failure,
    knowledge::{
        RESPONSE_LIMIT_BYTES,
        operations::KnowledgeError,
        output::{SearchOutputError, SearchTruncation, truncate_search_bundle},
    },
};
use maestro_kernel::evidence::{Bundle, Passage};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    process::ExitCode,
};

/// The stable CLI search document schema.
const CLI_SCHEMA: &str = "maestro-cli/knowledge-search/1";
/// Public fields for a bounded CLI result or refusal.
#[derive(Debug, Serialize)]
pub(super) struct CliEnvelope {
    /// Versioned CLI contract.
    schema: &'static str,
    /// The complete bundle shared with MCP, when successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) data: Option<Value>,
    /// A privacy-safe refusal or failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<CliError>,
    /// Whether semantic units were omitted to fit the line limit.
    pub(super) truncated: bool,
    /// Maximum complete response line in UTF-8 bytes.
    limit_bytes: usize,
    /// Categories omitted instead of slicing serialized evidence.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) omitted: Vec<&'static str>,
    /// Counts of complete semantic units omitted, when the result was reduced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) dropped: Option<SearchTruncation>,
}

/// A bounded public error without a source chain.
#[derive(Debug, Serialize)]
pub(super) struct CliError {
    /// Stable error code.
    pub(super) code: &'static str,
    /// Privacy-safe explanation.
    pub(super) message: &'static str,
}

/// Creates a refusal envelope with no source or backend details.
fn error_document(error: CliError, truncated: bool, omitted: Vec<&'static str>) -> CliEnvelope {
    CliEnvelope {
        schema: CLI_SCHEMA,
        data: None,
        error: Some(error),
        truncated,
        limit_bytes: RESPONSE_LIMIT_BYTES,
        omitted,
        dropped: None,
    }
}

/// Writes a typed CLI error with the matching process exit code.
pub(super) fn write_error(
    output: Output,
    error: CliError,
    exit_code: u8,
) -> Result<ExitCode, Failure> {
    let diagnostic = format!("{}: {}", error.code, error.message);
    output.refusal(&error_document(error, false, Vec::new()), &diagnostic)?;
    Ok(ExitCode::from(exit_code))
}

/// Writes the stable accepted-deadline refusal.
pub(super) fn deadline_error(output: Output) -> Result<ExitCode, Failure> {
    write_error(
        output,
        CliError {
            code: "deadline_exceeded",
            message: "search exceeded its accepted deadline",
        },
        1,
    )
}

/// Fits the same reduced bundle as MCP by dropping whole ranked passages and inventory groups.
pub(super) fn search_document(bundle: &Bundle) -> Result<CliEnvelope, KnowledgeError> {
    let bounded = match truncate_search_bundle(bundle.clone()) {
        Ok(bounded) => bounded,
        Err(SearchOutputError::Format) => return Err(format_failure()),
        Err(SearchOutputError::TooLarge) => {
            return Ok(error_document(
                CliError {
                    code: "response_too_large",
                    message: "the search response exceeds the response limit",
                },
                true,
                vec!["bundle"],
            ));
        }
    };
    let mut envelope =
        success(serde_json::to_value(&bounded.bundle).map_err(|_| format_failure())?);
    envelope.truncated = bounded.truncation.is_truncated();
    envelope.omitted = bounded.truncation.omitted();
    envelope.dropped = envelope.truncated.then_some(bounded.truncation);
    if fits(&envelope)? {
        return Ok(envelope);
    }
    Ok(error_document(
        CliError {
            code: "response_too_large",
            message: "the search response exceeds the response limit",
        },
        true,
        vec!["bundle"],
    ))
}

/// Creates an untruncated successful response envelope.
fn success(data: Value) -> CliEnvelope {
    CliEnvelope {
        schema: CLI_SCHEMA,
        data: Some(data),
        error: None,
        truncated: false,
        limit_bytes: RESPONSE_LIMIT_BYTES,
        omitted: Vec::new(),
        dropped: None,
    }
}

/// Checks the complete JSON line, including its framing newline.
fn fits(envelope: &CliEnvelope) -> Result<bool, KnowledgeError> {
    let bytes = serde_json::to_vec(envelope).map_err(|_| format_failure())?;
    Ok(bytes.len().saturating_add(1) <= RESPONSE_LIMIT_BYTES)
}

/// Lists the documents of the passages retained in the bounded JSON
/// envelope, one line each at its best passage's place: its source label,
/// the version it documents and its title, then each of its passages, by
/// number and section, with its text. `labels` holds the source label of
/// each classified document by id; any other document is named by its
/// origin host or first corpus-path segment.
pub(super) fn search_text(
    bundle: &Bundle,
    envelope: &CliEnvelope,
    labels: &BTreeMap<String, String>,
) -> String {
    use std::fmt::Write as _;
    let mut text = format!(
        "Search {} generation {}: {}\n",
        bundle.collection, bundle.generation, bundle.query
    );
    let included = envelope
        .data
        .as_ref()
        .and_then(|data| data["passages"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|passage| passage["n"].as_u64())
        .collect::<BTreeSet<_>>();
    let mut documents: Vec<(&Passage, Vec<&Passage>)> = Vec::new();
    for passage in bundle
        .passages
        .iter()
        .filter(|passage| included.contains(&u64::from(passage.n)))
    {
        match documents
            .iter_mut()
            .find(|(first, _)| first.document_id == passage.document_id)
        {
            Some((_, passages)) => passages.push(passage),
            None => documents.push((passage, vec![passage])),
        }
    }
    for (rank, (first, passages)) in documents.iter().enumerate() {
        let _ = writeln!(
            text,
            "\n{}. {} — {}",
            rank + 1,
            source_label(first, labels),
            first.title
        );
        for passage in passages {
            let _ = write!(text, "[{}]", passage.n);
            if !passage.section_path.is_empty() {
                let _ = write!(text, " {}", passage.section_path.join(" › "));
            }
            let _ = writeln!(text, "\n{}", passage.text);
        }
    }
    if included.is_empty() {
        text.push_str("No evidence passages were returned.\n");
    }
    if let Some(gaps) = envelope
        .data
        .as_ref()
        .and_then(|data| data.get("known_gaps"))
        .and_then(Value::as_array)
    {
        for gap in gaps.iter().filter_map(Value::as_str) {
            let _ = writeln!(text, "Known gap: {gap}");
        }
    }
    text
}

/// The label of `passage`'s document, followed by its version, if any.
fn source_label(passage: &Passage, labels: &BTreeMap<String, String>) -> String {
    let label = labels
        .get(&passage.document_id)
        .map_or_else(|| origin(&passage.source_ref), String::as_str);
    match &passage.version {
        Some(version) => format!("{label} {version}"),
        None => label.to_owned(),
    }
}

/// The host of a URL `source_ref`, or the first path segment of a
/// `corpus-path:` one.
fn origin(source_ref: &str) -> &str {
    let rest = source_ref
        .split_once("://")
        .or_else(|| source_ref.split_once(':'))
        .map_or(source_ref, |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest)
}

/// Creates the safe failure for a bundle that cannot be serialized.
fn format_failure() -> KnowledgeError {
    KnowledgeError::Failed {
        code: "response_format_error",
        message: "the response could not be safely formatted",
    }
}

#[cfg(test)]
mod tests {
    use super::{RESPONSE_LIMIT_BYTES, fits, success};
    use serde_json::Value;

    #[test]
    fn cli_envelope_fit_check_includes_all_serialized_bytes() {
        let envelope = success(Value::String("x".repeat(RESPONSE_LIMIT_BYTES)));
        assert!(!fits(&envelope).expect("serialize CLI envelope"));
    }
}
