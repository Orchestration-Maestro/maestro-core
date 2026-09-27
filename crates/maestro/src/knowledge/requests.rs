//! Strict arguments shared by the CLI and MCP tools.

use maestro_kernel::scope::check_name;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

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
        }
    }

    /// The privacy-safe explanation returned at a public boundary.
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::InvalidArguments => "arguments do not match the knowledge tool schema",
            Self::InvalidChunkId => "chunk_id must be nonblank and at most 256 UTF-8 bytes",
            Self::InvalidSectionId => "section_id must be nonblank and at most 256 UTF-8 bytes",
            Self::InvalidCollection => "collection is not a valid kernel name",
            Self::InvalidGeneration => "generation must be positive",
            Self::GenerationNeedsCollection => "generation requires collection",
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
    use super::{GetRequest, RequestError};
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
}
