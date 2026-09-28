//! Source-scoped kernel operations for the CLI and MCP read surfaces.

mod implementation;
mod search;
#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
use implementation::section_read_failure;
#[cfg(test)]
pub(crate) use implementation::{CollectionItem, GetExcerpt};
pub(crate) use implementation::{
    CollectionsData, GetData, KnowledgeError, collections_with, ensure_current_scopes, get_with,
};
pub(crate) use search::search_with;
