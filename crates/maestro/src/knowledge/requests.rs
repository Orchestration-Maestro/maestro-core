//! Strict arguments shared by the CLI and MCP tools.

use maestro_kernel::scope::check_name;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

/// The arguments for exact retrieval through a visible chunk ID.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetRequest {
    /// The chunk ID, scoped by collection or visible published membership.
    #[schemars(length(min = 1))]
    #[schemars(description = "Nonblank chunk ID, at most 256 UTF-8 bytes.")]
    pub(crate) chunk_id: String,
    /// Limit lookup to this collection.
    #[schemars(regex(pattern = r"^[a-z0-9][a-z0-9._-]{0,63}$"))]
    #[schemars(length(min = 1, max = 64))]
    pub(crate) collection: Option<String>,
    /// Pin this published or retained generation; requires `collection`.
    #[schemars(range(min = 1))]
    pub(crate) generation: Option<i64>,
}

/// A safe input refusal, separated from runtime failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestError {
    /// Arguments do not match the strict request type.
    InvalidArguments,
    /// The chunk ID is blank or exceeds 256 UTF-8 bytes.
    InvalidChunkId,
    /// The collection ID is not a kernel scope name.
    InvalidCollection,
    /// The caller selected a nonpositive generation.
    InvalidGeneration,
    /// A generation selector requires its collection.
    GenerationNeedsCollection,
    /// Section retrieval waits for T032's authoritative reader.
    SectionUnavailable,
}

impl RequestError {
    /// Stable public error code.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::InvalidArguments => "invalid_arguments",
            Self::InvalidChunkId => "invalid_chunk_id",
            Self::InvalidCollection => "invalid_collection",
            Self::InvalidGeneration => "invalid_generation",
            Self::GenerationNeedsCollection => "generation_needs_collection",
            Self::SectionUnavailable => "section_id_unavailable",
        }
    }

    /// The privacy-safe explanation returned at a public boundary.
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::InvalidArguments => "arguments do not match the knowledge tool schema",
            Self::InvalidChunkId => "chunk_id must be nonblank and at most 256 UTF-8 bytes",
            Self::InvalidCollection => "collection is not a valid kernel name",
            Self::InvalidGeneration => "generation must be positive",
            Self::GenerationNeedsCollection => "generation requires collection",
            Self::SectionUnavailable => "section_id is not available yet",
        }
    }
}

impl GetRequest {
    /// A CLI request, validated by the same rules as MCP input.
    pub(crate) fn from_cli(
        chunk_id: String,
        collection: Option<String>,
        generation: Option<i64>,
    ) -> Result<Self, RequestError> {
        let request = Self {
            chunk_id,
            collection,
            generation,
        };
        request.validate()?;
        Ok(request)
    }

    /// Parses a strict MCP object, specially refusing deferred section IDs.
    pub(crate) fn parse(value: Value) -> Result<Self, RequestError> {
        let Some(fields) = value.as_object() else {
            return Err(RequestError::InvalidArguments);
        };
        if fields.keys().any(|field| {
            !matches!(
                field.as_str(),
                "chunk_id" | "collection" | "generation" | "section_id"
            )
        }) {
            return Err(RequestError::InvalidArguments);
        }
        if fields.contains_key("section_id") {
            return Err(RequestError::SectionUnavailable);
        }
        let request: Self =
            serde_json::from_value(value).map_err(|_| RequestError::InvalidArguments)?;
        request.validate()?;
        Ok(request)
    }

    /// Applies shared identifier and generation bounds before database work.
    fn validate(&self) -> Result<(), RequestError> {
        if self.chunk_id.trim().is_empty() || self.chunk_id.len() > 256 {
            return Err(RequestError::InvalidChunkId);
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
    use super::{GetRequest, RequestError};
    use serde_json::json;

    #[test]
    fn chunk_id_validation_uses_nonblank_utf8_byte_bounds() {
        assert_eq!(
            GetRequest::from_cli(" \t ".to_owned(), None, None).expect_err("blank ID"),
            RequestError::InvalidChunkId
        );
        assert!(GetRequest::from_cli("é".repeat(128), None, None).is_ok());
        assert_eq!(
            GetRequest::from_cli("x".repeat(257), None, None).expect_err("oversized ID"),
            RequestError::InvalidChunkId
        );
    }

    #[test]
    fn strict_get_parser_refuses_unknown_and_deferred_section_fields() {
        assert_eq!(
            GetRequest::parse(json!({"chunk_id": "chunk", "principal": "other"}))
                .expect_err("unknown identity field"),
            RequestError::InvalidArguments
        );
        assert_eq!(
            GetRequest::parse(json!({"section_id": "section"}))
                .expect_err("section reader is deferred"),
            RequestError::SectionUnavailable
        );
    }
}
