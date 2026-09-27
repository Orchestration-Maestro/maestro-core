//! CLI adapters for the shared, permission-scoped read operations.

use super::output::Output;
use crate::knowledge::{
    CollectionsData, GetData, GetRequest, KnowledgeError, RESPONSE_LIMIT_BYTES, RequestError,
    collections_with, ensure_current_scopes, get_with,
};
use crate::{failure::Failure, kernel::Kernel};
use serde::Serialize;
use serde_json::Value;
use std::process::ExitCode;

/// Lists visible collection metadata without exposing source-only parents.
pub(super) fn collections(
    output: Output,
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
) -> Result<ExitCode, Failure> {
    let mut scoped = collections_with(open_kernel).map_err(failure)?;
    let document = collections_document(&scoped.data).map_err(failure)?;
    let text = collections_text(&scoped.data, &document);
    ensure_current_scopes(&mut scoped.kernel, &scoped.scopes).map_err(failure)?;
    output.result(&document, &text)?;
    Ok(ExitCode::SUCCESS)
}

/// Gets an exact chunk from the collection and generation admitted by the request.
pub(super) fn get_chunk(
    output: Output,
    chunk_id: String,
    collection: Option<String>,
    generation: Option<i64>,
    open_kernel: impl FnOnce() -> Result<Kernel, Failure>,
) -> Result<ExitCode, Failure> {
    let request = GetRequest::from_cli(chunk_id, collection, generation)
        .map_err(|error: RequestError| Failure::refused(error.message()))?;
    let mut scoped = get_with(
        open_kernel,
        &request.chunk_id,
        request.collection.as_deref(),
        request.generation,
    )
    .map_err(failure)?;
    let data = &scoped.data;
    let (document, too_large) = get_document(data).map_err(failure)?;
    let text = if too_large {
        "response_too_large: the exact excerpt exceeds the response limit".to_owned()
    } else {
        format!(
            "{}\n{}\n\n{}",
            data.excerpt.title.as_deref().unwrap_or("Untitled source"),
            data.excerpt.source_ref,
            data.excerpt.text
        )
    };
    ensure_current_scopes(&mut scoped.kernel, &scoped.scopes).map_err(failure)?;
    if too_large {
        output.refusal(&document, &text)?;
        Ok(ExitCode::from(2))
    } else {
        output.result(&document, &text)?;
        Ok(ExitCode::SUCCESS)
    }
}

/// Builds bounded human collection output with an explicit semantic omission notice.
fn collections_text(data: &CollectionsData, document: &CliEnvelope) -> String {
    const WARNING: &str = "Some collection entries were omitted to fit the response limit.";
    let included = document
        .data
        .as_ref()
        .and_then(|data| data.get("collections"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let mut text = data
        .collections
        .iter()
        .take(included)
        .map(|collection| {
            format!(
                "{}: {} (published generation {})",
                collection.id,
                collection.title,
                collection
                    .published_generation
                    .map_or_else(|| "none".to_owned(), |generation| generation.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    if document.truncated {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(WARNING);
    }
    text
}

/// Maps internal failures to the CLI's existing 0/1/2 exit-code contract.
fn failure(error: KnowledgeError) -> Failure {
    match error {
        KnowledgeError::Refused { message, .. } => Failure::refused(message),
        KnowledgeError::Failed { message, .. } => Failure::failed(message),
    }
}

/// The CLI's result envelope, including explicit transport-bound disclosure.
#[derive(Debug, Serialize)]
struct CliEnvelope {
    /// Versioned CLI contract.
    schema: &'static str,
    /// The same versioned data MCP returns on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
    /// A bounded refusal when an indivisible get result does not fit.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<SafeError>,
    /// Whether semantic units were omitted to stay within the wire bound.
    truncated: bool,
    /// Maximum complete response line in UTF-8 bytes.
    limit_bytes: usize,
    /// Categories omitted instead of slicing serialized JSON or source text.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    omitted: Vec<&'static str>,
}

/// A public error without source-chain details.
#[derive(Debug, Serialize)]
struct SafeError {
    /// Stable public code.
    code: &'static str,
    /// Privacy-safe explanation.
    message: &'static str,
}

/// Fits collection data by omitting complete trailing entries in stable ID order.
fn collections_document(data: &CollectionsData) -> Result<CliEnvelope, KnowledgeError> {
    let data = serde_json::to_value(data).map_err(|_| format_failure())?;
    let mut envelope = success("maestro-cli/knowledge-collections/1", data);
    while !fits(&envelope)? {
        let Some(collections) = envelope
            .data
            .as_mut()
            .and_then(|data| data.get_mut("collections"))
            .and_then(Value::as_array_mut)
        else {
            return Err(format_failure());
        };
        if collections.pop().is_none() {
            return Err(format_failure());
        }
        envelope.truncated = true;
        envelope.omitted = vec!["collections"];
    }
    Ok(envelope)
}

/// Keeps exact get data whole, or returns a small explicit size refusal.
fn get_document(data: &GetData) -> Result<(CliEnvelope, bool), KnowledgeError> {
    if excerpt_text_exceeds_limit(&data.excerpt.text) {
        return Ok((response_too_large(), true));
    }
    let value = serde_json::to_value(data).map_err(|_| format_failure())?;
    let envelope = success("maestro-cli/knowledge-get/1", value);
    if fits(&envelope)? {
        return Ok((envelope, false));
    }
    let error = response_too_large();
    if fits(&error)? {
        Ok((error, true))
    } else {
        Err(format_failure())
    }
}

/// Whether the indivisible source text cannot fit in any complete response.
fn excerpt_text_exceeds_limit(text: &str) -> bool {
    text.len() > RESPONSE_LIMIT_BYTES
}

/// A compact exact-excerpt refusal for either CLI output mode.
fn response_too_large() -> CliEnvelope {
    CliEnvelope {
        schema: "maestro-cli/knowledge-get/1",
        data: None,
        error: Some(SafeError {
            code: "response_too_large",
            message: "the exact excerpt exceeds the response limit",
        }),
        truncated: true,
        limit_bytes: RESPONSE_LIMIT_BYTES,
        omitted: vec!["excerpt"],
    }
}

/// Creates the regular complete CLI envelope.
fn success(schema: &'static str, data: Value) -> CliEnvelope {
    CliEnvelope {
        schema,
        data: Some(data),
        error: None,
        truncated: false,
        limit_bytes: RESPONSE_LIMIT_BYTES,
        omitted: Vec::new(),
    }
}

/// Checks the serialized JSON line, including its framing newline.
fn fits(envelope: &impl Serialize) -> Result<bool, KnowledgeError> {
    let bytes = serde_json::to_vec(envelope).map_err(|_| format_failure())?;
    Ok(bytes.len().saturating_add(1) <= RESPONSE_LIMIT_BYTES)
}

/// A serialization failure or envelope too large to fit after safe omission.
fn format_failure() -> KnowledgeError {
    KnowledgeError::Failed {
        code: "response_format_error",
        message: "the response could not be safely formatted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{CollectionItem, GetExcerpt, RefreshScratch};

    #[test]
    fn collections_refuses_access_revoked_before_cli_delivery() {
        let scratch = RefreshScratch::new();
        let result = collections(Output::new(true), || scratch.kernel(Some(1)));
        assert!(matches!(
            result,
            Err(Failure::Refused(message))
                if message == "permissions changed during the request; no result was delivered"
        ));
    }

    #[test]
    fn get_refuses_access_revoked_before_cli_delivery() {
        let scratch = RefreshScratch::new();
        let result = get_chunk(
            Output::new(true),
            "chunk".to_owned(),
            Some("collection".to_owned()),
            None,
            || scratch.kernel(Some(2)),
        );
        assert!(matches!(
            result,
            Err(Failure::Refused(message))
                if message == "permissions changed during the request; no result was delivered"
        ));
    }

    #[test]
    fn cli_bounds_omit_whole_collection_entries_and_refuse_whole_oversized_excerpts()
    -> Result<(), KnowledgeError> {
        let collections = CollectionsData {
            schema: "maestro-knowledge-collections/1",
            collections: vec![
                CollectionItem {
                    id: "first".to_owned(),
                    title: "visible".to_owned(),
                    published_generation: None,
                },
                CollectionItem {
                    id: "second".to_owned(),
                    title: "x".repeat(70_000),
                    published_generation: None,
                },
            ],
        };
        let document = collections_document(&collections)?;
        let bytes = serde_json::to_vec(&document).map_err(|_| format_failure())?;
        assert!(bytes.len().saturating_add(1) <= RESPONSE_LIMIT_BYTES);
        assert!(document.truncated);
        assert_eq!(document.omitted, ["collections"]);
        let text = collections_text(&collections, &document);
        assert_eq!(
            text,
            concat!(
                "first: visible (published generation none)\n",
                "Some collection entries were omitted to fit the response limit.",
            )
        );
        assert!(!text.contains(&"x".repeat(100)));
        assert_eq!(
            document
                .data
                .as_ref()
                .and_then(|data| data["collections"].as_array())
                .map(Vec::len),
            Some(1)
        );

        let get = GetData {
            schema: "maestro-knowledge-get/1",
            collection: "collection".to_owned(),
            generation: 1,
            excerpt: GetExcerpt {
                chunk_id: "chunk".to_owned(),
                document_id: "document".to_owned(),
                revision_id: "revision".to_owned(),
                section_id: None,
                source_ref: "source".to_owned(),
                title: None,
                version: None,
                span: [0, 70_000],
                digest: "sha256:digest".to_owned(),
                text: "x".repeat(70_000),
            },
        };
        let (document, refused) = get_document(&get)?;
        let bytes = serde_json::to_vec(&document).map_err(|_| format_failure())?;
        assert!(refused);
        assert!(bytes.len().saturating_add(1) <= RESPONSE_LIMIT_BYTES);
        assert_eq!(
            document.error.as_ref().map(|error| error.code),
            Some("response_too_large")
        );
        assert!(document.truncated);
        assert_eq!(document.omitted, ["excerpt"]);
        assert!(document.data.is_none());
        Ok(())
    }

    #[test]
    fn cli_wire_limit_enforces_65535_65536_and_65537_bytes() -> Result<(), KnowledgeError> {
        assert!(fits(&"x".repeat(RESPONSE_LIMIT_BYTES - 4))?);
        assert!(fits(&"x".repeat(RESPONSE_LIMIT_BYTES - 3))?);
        assert!(!fits(&"x".repeat(RESPONSE_LIMIT_BYTES - 2))?);

        let below = format!("{}x", "é".repeat((RESPONSE_LIMIT_BYTES - 1) / 2));
        let at = "é".repeat(RESPONSE_LIMIT_BYTES / 2);
        let above = format!("{at}x");
        assert_eq!(
            (below.len(), at.len(), above.len()),
            (65_535, 65_536, 65_537)
        );
        assert!(!excerpt_text_exceeds_limit(&below));
        assert!(!excerpt_text_exceeds_limit(&at));
        assert!(excerpt_text_exceeds_limit(&above));
        Ok(())
    }
}
