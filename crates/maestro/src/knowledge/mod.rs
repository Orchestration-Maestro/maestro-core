//! Scoped knowledge operations shared by the CLI and stdio MCP server.

mod operations;
mod output;
mod requests;

#[cfg(test)]
pub(crate) use operations::tests::Scratch as RefreshScratch;
#[cfg(test)]
pub(crate) use operations::{CollectionItem, GetExcerpt};
pub(crate) use operations::{
    CollectionsData, GetData, KnowledgeError, collections_with, ensure_current_scopes, get_with,
};
pub(crate) use output::RESPONSE_LIMIT_BYTES;
pub(crate) use requests::{GetRequest, RequestError};
