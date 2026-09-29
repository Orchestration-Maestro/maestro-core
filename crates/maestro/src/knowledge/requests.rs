//! Strict arguments shared by the CLI and MCP tools.

use maestro_kernel::{evidence::RequestBudget, scope::check_name};
use maestro_knowledge::query::understand;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;

/// Returns the evidence contract's default passage count for the wire schema.
fn default_k() -> u32 {
    RequestBudget::default().k
}

/// Returns the evidence contract's default token budget for the wire schema.
fn default_evidence_bytes() -> u32 {
    RequestBudget::default().evidence_bytes
}

/// Returns the evidence contract's default deadline for the wire schema.
fn default_deadline_ms() -> u32 {
    RequestBudget::default().deadline_ms
}

/// `arguments` with each of `defaults` a call leaves out inserted: the
/// session's settings fill what a tool call omits, and what it gives wins.
/// Anything but an object is left as it is, for the strict parse to refuse.
pub(crate) fn with_defaults(mut arguments: Value, defaults: &[(&str, Value)]) -> Value {
    if let Value::Object(object) = &mut arguments {
        for (name, value) in defaults {
            object.entry(*name).or_insert_with(|| value.clone());
        }
    }
    arguments
}

/// The arguments for exact retrieval through one scoped identifier.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetRequest {
    /// The chunk ID, scoped by collection or visible published membership.
    #[schemars(length(min = 1))]
    #[schemars(description = "Nonblank chunk ID, at most 256 UTF-8 bytes.")]
    pub(crate) chunk_id: Option<String>,
    /// The canonical section ID, scoped by collection or visible publication.
    #[schemars(length(min = 1))]
    #[schemars(description = "Nonblank section ID, at most 256 UTF-8 bytes.")]
    pub(crate) section_id: Option<String>,
    /// Limit lookup to this collection.
    #[schemars(regex(pattern = r"^[a-z0-9][a-z0-9._-]{0,63}$"))]
    #[schemars(length(min = 1, max = 64))]
    pub(crate) collection: Option<String>,
    /// Pin this published or retained generation; requires `collection`.
    #[schemars(range(min = 1))]
    pub(crate) generation: Option<i64>,
}

/// The arguments for a scoped search through the evidence pipeline.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchRequest {
    /// The collection whose current published generation is searched.
    #[schemars(regex(pattern = r"^[a-z0-9][a-z0-9._-]{0,63}$"))]
    #[schemars(length(min = 1, max = 64))]
    pub(crate) collection: String,
    /// The original question, at most 8192 UTF-8 bytes.
    #[schemars(length(min = 1))]
    #[schemars(description = "Nonblank question, at most 8192 UTF-8 bytes.")]
    pub(crate) query: String,
    /// Restrict results to this exact documented version.
    #[schemars(length(min = 1, max = 256))]
    pub(crate) version: Option<String>,
    /// Maximum final passage count, default 10.
    #[serde(rename = "k", default = "default_k")]
    #[schemars(range(min = 1, max = 50))]
    pub(crate) max_passages: u32,
    /// Maximum evidence size in UTF-8 bytes, not tokens, default 6000.
    #[serde(default = "default_evidence_bytes")]
    #[schemars(range(min = 1, max = RequestBudget::MAX_EVIDENCE_BUDGET))]
    pub(crate) evidence_bytes: u32,
    /// Search deadline in milliseconds, default 30000.
    #[serde(default = "default_deadline_ms")]
    #[schemars(range(min = 1, max = RequestBudget::MAX_DEADLINE_MS))]
    pub(crate) deadline_ms: u32,
}

impl SearchRequest {
    /// A CLI request, validated by the same rules as MCP input.
    pub(crate) fn from_cli(request: Self) -> Result<Self, RequestError> {
        request.validate()?;
        Ok(request)
    }

    /// Parses a strict MCP object, refusing unknown fields before any kernel work.
    pub(crate) fn parse(value: Value) -> Result<Self, RequestError> {
        let request: Self =
            serde_json::from_value(value).map_err(|_| RequestError::InvalidArguments)?;
        request.validate()?;
        Ok(request)
    }

    /// The accepted bounds echoed into `maestro-evidence/1`.
    pub(crate) fn budget(&self) -> RequestBudget {
        RequestBudget {
            k: self.max_passages,
            evidence_bytes: self.evidence_bytes,
            deadline_ms: self.deadline_ms,
        }
    }

    /// Applies public bounds before model or index work.
    fn validate(&self) -> Result<(), RequestError> {
        check_name(&self.collection).map_err(|_| RequestError::InvalidCollection)?;
        if self.query.len() > 8192 {
            return Err(RequestError::InvalidQuery);
        }
        let understood = understand(&self.query);
        if understood.normalized.is_empty() {
            return Err(RequestError::InvalidQuery);
        }
        if BTreeSet::from_iter(
            understood
                .identifiers
                .iter()
                .map(|identifier| identifier.text.as_str()),
        )
        .len()
            > 64
        {
            return Err(RequestError::TooManyIdentifiers);
        }
        if self
            .version
            .as_deref()
            .is_some_and(|version| version.trim().is_empty() || version.len() > 256)
        {
            return Err(RequestError::InvalidVersion);
        }
        if !(1..=50).contains(&self.max_passages) {
            return Err(RequestError::InvalidK);
        }
        if !(1..=RequestBudget::MAX_EVIDENCE_BUDGET).contains(&self.evidence_bytes) {
            return Err(RequestError::InvalidMaxTokens);
        }
        if !(1..=RequestBudget::MAX_DEADLINE_MS).contains(&self.deadline_ms) {
            return Err(RequestError::InvalidDeadline);
        }
        Ok(())
    }
}

/// Which exact identifier the request selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GetSelector<'a> {
    /// A prepared chunk ID.
    Chunk(&'a str),
    /// A canonical section ID.
    Section(&'a str),
}

/// A safe input refusal, separated from runtime failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestError {
    /// Arguments do not match the strict request type.
    InvalidArguments,
    /// The chunk ID is blank or exceeds 256 UTF-8 bytes.
    InvalidChunkId,
    /// The section ID is blank or exceeds 256 UTF-8 bytes.
    InvalidSectionId,
    /// The collection ID is not a kernel scope name.
    InvalidCollection,
    /// The caller selected a nonpositive generation.
    InvalidGeneration,
    /// A generation selector requires its collection.
    GenerationNeedsCollection,
    /// The query is blank or exceeds 8192 UTF-8 bytes.
    InvalidQuery,
    /// The query identifies more than 64 distinct values.
    TooManyIdentifiers,
    /// The explicit version is blank or exceeds 256 UTF-8 bytes.
    InvalidVersion,
    /// `k` is outside 1..=50.
    InvalidK,
    /// `evidence_bytes` is outside 1..=[`RequestBudget::MAX_EVIDENCE_BUDGET`].
    InvalidMaxTokens,
    /// `deadline_ms` is outside 1..=30000.
    InvalidDeadline,
}

impl RequestError {
    /// Stable public error code.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::InvalidArguments => "invalid_arguments",
            Self::InvalidChunkId => "invalid_chunk_id",
            Self::InvalidSectionId => "invalid_section_id",
            Self::InvalidCollection => "invalid_collection",
            Self::InvalidGeneration => "invalid_generation",
            Self::GenerationNeedsCollection => "generation_needs_collection",
            Self::InvalidQuery => "invalid_query",
            Self::TooManyIdentifiers => "too_many_identifiers",
            Self::InvalidVersion => "invalid_version",
            Self::InvalidK => "invalid_k",
            Self::InvalidMaxTokens => "invalid_evidence_bytes",
            Self::InvalidDeadline => "invalid_deadline_ms",
        }
    }

    /// The privacy-safe explanation returned at a public boundary.
    pub(crate) fn message(self) -> &'static str {
        const _: () = assert!(RequestBudget::MAX_EVIDENCE_BUDGET == 24_000);
        match self {
            Self::InvalidArguments => "arguments do not match the knowledge tool schema",
            Self::InvalidChunkId => "chunk_id must be nonblank and at most 256 UTF-8 bytes",
            Self::InvalidSectionId => "section_id must be nonblank and at most 256 UTF-8 bytes",
            Self::InvalidCollection => "collection is not a valid kernel name",
            Self::InvalidGeneration => "generation must be positive",
            Self::GenerationNeedsCollection => "generation requires collection",
            Self::InvalidQuery => "query must be nonblank and at most 8192 UTF-8 bytes",
            Self::TooManyIdentifiers => "query must contain at most 64 distinct identifiers",
            Self::InvalidVersion => "version must contain 1 to 256 UTF-8 bytes",
            Self::InvalidK => "k must be between 1 and 50",
            Self::InvalidMaxTokens => "evidence_bytes must be between 1 and 24000",
            Self::InvalidDeadline => "deadline_ms must be between 1 and 30000",
        }
    }
}

impl GetRequest {
    /// The sole selector, when this request has passed validation.
    pub(crate) fn selector(&self) -> Option<GetSelector<'_>> {
        match (self.chunk_id.as_deref(), self.section_id.as_deref()) {
            (Some(chunk_id), None) => Some(GetSelector::Chunk(chunk_id)),
            (None, Some(section_id)) => Some(GetSelector::Section(section_id)),
            _ => None,
        }
    }

    /// A CLI request, validated by the same rules as MCP input.
    pub(crate) fn from_cli(
        chunk_id: Option<String>,
        section_id: Option<String>,
        collection: Option<String>,
        generation: Option<i64>,
    ) -> Result<Self, RequestError> {
        let request = Self {
            chunk_id,
            section_id,
            collection,
            generation,
        };
        request.validate()?;
        Ok(request)
    }

    /// Parses a strict MCP object, refusing unknown and mutually selected fields.
    pub(crate) fn parse(value: Value) -> Result<Self, RequestError> {
        let request: Self =
            serde_json::from_value(value).map_err(|_| RequestError::InvalidArguments)?;
        request.validate()?;
        Ok(request)
    }

    /// Applies shared identifier and generation bounds before database work.
    fn validate(&self) -> Result<(), RequestError> {
        let (id, invalid_id) = match (&self.chunk_id, &self.section_id) {
            (Some(chunk_id), None) => (chunk_id, RequestError::InvalidChunkId),
            (None, Some(section_id)) => (section_id, RequestError::InvalidSectionId),
            _ => return Err(RequestError::InvalidArguments),
        };
        if id.trim().is_empty() || id.len() > 256 {
            return Err(invalid_id);
        }
        if let Some(collection) = &self.collection {
            check_name(collection).map_err(|_| RequestError::InvalidCollection)?;
        }
        if self.generation.is_some_and(|generation| generation <= 0) {
            return Err(RequestError::InvalidGeneration);
        }
        if self.generation.is_some() && self.collection.is_none() {
            return Err(RequestError::GenerationNeedsCollection);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{GetRequest, RequestError, SearchRequest};
    use maestro_kernel::evidence::RequestBudget;
    use serde_json::json;

    #[test]
    fn chunk_id_validation_uses_nonblank_utf8_byte_bounds() {
        assert_eq!(
            GetRequest::from_cli(Some(" \t ".to_owned()), None, None, None).expect_err("blank ID"),
            RequestError::InvalidChunkId
        );
        assert!(GetRequest::from_cli(Some("é".repeat(128)), None, None, None).is_ok());
        assert_eq!(
            GetRequest::from_cli(Some("x".repeat(257)), None, None, None)
                .expect_err("oversized ID"),
            RequestError::InvalidChunkId
        );
    }

    #[test]
    fn section_id_validation_uses_nonblank_utf8_byte_bounds() {
        assert_eq!(
            GetRequest::from_cli(None, Some(String::new()), None, None).expect_err("blank ID"),
            RequestError::InvalidSectionId
        );
        assert!(GetRequest::from_cli(None, Some("é".repeat(128)), None, None).is_ok());
        assert_eq!(
            GetRequest::from_cli(None, Some("x".repeat(257)), None, None)
                .expect_err("oversized ID"),
            RequestError::InvalidSectionId
        );
    }

    #[test]
    fn strict_get_parser_accepts_one_selector_and_rejects_both() {
        assert_eq!(
            GetRequest::from_cli(
                Some("chunk".to_owned()),
                Some("section".to_owned()),
                None,
                None,
            )
            .expect_err("mutually exclusive selectors"),
            RequestError::InvalidArguments
        );
        assert!(GetRequest::parse(json!({"chunk_id": "chunk"})).is_ok());
        assert!(GetRequest::parse(json!({"section_id": "section"})).is_ok());
        assert_eq!(
            GetRequest::parse(json!({"chunk_id": "chunk", "section_id": "section"}))
                .expect_err("mutually exclusive selectors"),
            RequestError::InvalidArguments
        );
        assert_eq!(
            GetRequest::parse(json!({"principal": "other"})).expect_err("unknown identity field"),
            RequestError::InvalidArguments
        );
    }

    fn search(
        query: impl Into<String>,
        version: Option<String>,
        max_passages: Option<u32>,
        evidence_bytes: Option<u32>,
        deadline_ms: Option<u32>,
    ) -> Result<SearchRequest, RequestError> {
        let defaults = RequestBudget::default();
        SearchRequest::from_cli(SearchRequest {
            collection: "collection".to_owned(),
            query: query.into(),
            version,
            max_passages: max_passages.unwrap_or(defaults.k),
            evidence_bytes: evidence_bytes.unwrap_or(defaults.evidence_bytes),
            deadline_ms: deadline_ms.unwrap_or(defaults.deadline_ms),
        })
    }

    #[test]
    fn search_request_uses_defaults_and_accepts_each_inclusive_budget_boundary() {
        assert_eq!(
            search("query", None, None, None, None)
                .expect("defaults")
                .budget(),
            RequestBudget::default()
        );
        assert!(search("query", Some("v".repeat(256)), Some(1), Some(1), Some(1)).is_ok());
        assert!(
            search(
                "query",
                Some("v".repeat(256)),
                Some(50),
                Some(24_000),
                Some(30_000)
            )
            .is_ok()
        );
    }

    #[test]
    fn search_request_refuses_blank_and_oversized_utf8_queries_and_versions() {
        assert_eq!(
            search(" \t ", None, None, None, None).expect_err("blank question"),
            RequestError::InvalidQuery
        );
        assert!(search("é".repeat(4096), None, None, None, None).is_ok());
        assert_eq!(
            search("é".repeat(4097), None, None, None, None).expect_err("query byte bound"),
            RequestError::InvalidQuery
        );
        assert_eq!(
            search("query", Some("v".repeat(257)), None, None, None)
                .expect_err("version byte bound"),
            RequestError::InvalidVersion
        );
        assert_eq!(
            search("query", Some(" \t ".to_owned()), None, None, None).expect_err("blank version"),
            RequestError::InvalidVersion
        );
    }

    #[test]
    fn search_request_accepts_exactly_64_distinct_identifiers() {
        let query = (0..64)
            .map(|number| format!("${{ITEM_{number}}}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(search(&query, None, None, None, None).is_ok());
    }

    #[test]
    fn search_request_refuses_more_than_64_distinct_identifiers() {
        let query = (0..65)
            .map(|number| format!("${{ITEM_{number}}}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            search(&query, None, None, None, None).expect_err("identifier bound"),
            RequestError::TooManyIdentifiers
        );
    }

    #[test]
    fn search_request_refuses_values_outside_each_budget_bound() {
        for (max_passages, evidence_bytes, deadline_ms, expected) in [
            (Some(0), None, None, RequestError::InvalidK),
            (Some(51), None, None, RequestError::InvalidK),
            (None, Some(0), None, RequestError::InvalidMaxTokens),
            (None, Some(24_001), None, RequestError::InvalidMaxTokens),
            (None, None, Some(0), RequestError::InvalidDeadline),
            (None, None, Some(30_001), RequestError::InvalidDeadline),
        ] {
            assert_eq!(
                search("query", None, max_passages, evidence_bytes, deadline_ms)
                    .expect_err("out of bounds"),
                expected
            );
        }
        assert_eq!(
            RequestError::InvalidMaxTokens.message(),
            "evidence_bytes must be between 1 and 24000"
        );
    }

    #[test]
    fn strict_search_parser_refuses_unknown_fields_and_invalid_collections() {
        assert!(
            SearchRequest::parse(json!({
                "collection": "collection",
                "query": "question"
            }))
            .is_ok()
        );
        assert_eq!(
            SearchRequest::parse(
                json!({"collection": "collection", "query": "question", "principal": "other"})
            )
            .expect_err("identity injection"),
            RequestError::InvalidArguments
        );
        assert_eq!(
            SearchRequest::from_cli(SearchRequest {
                collection: "bad/name".to_owned(),
                query: "query".to_owned(),
                version: None,
                max_passages: RequestBudget::default().k,
                evidence_bytes: RequestBudget::default().evidence_bytes,
                deadline_ms: RequestBudget::default().deadline_ms,
            })
            .expect_err("invalid collection"),
            RequestError::InvalidCollection
        );
    }

    #[test]
    fn search_schema_has_a_strict_object_root_and_bounds() {
        let schema = serde_json::to_value(schemars::schema_for!(SearchRequest))
            .expect("search argument schema");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["k"]["minimum"], 1);
        assert_eq!(schema["properties"]["k"]["maximum"], 50);
        assert_eq!(schema["properties"]["k"]["default"], 10);
        assert_eq!(schema["properties"]["evidence_bytes"]["maximum"], 24_000);
        assert_eq!(schema["properties"]["evidence_bytes"]["default"], 6000);
        assert_eq!(schema["properties"]["deadline_ms"]["maximum"], 30_000);
        assert_eq!(schema["properties"]["deadline_ms"]["default"], 30_000);
    }
}
