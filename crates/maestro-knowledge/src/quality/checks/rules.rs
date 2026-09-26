//! The automatic checks, run in the order of the quality module's table on a
//! revision's canonical document.

use super::{
    body::{self, Body},
    flag::Flag,
    page, record, secret, text,
};
use maestro_canonicalization::CanonicalDocument;

/// What every automatic check flags in `document`, canonicalized from the
/// original Markdown `markdown`, in the order of the quality module's list.
pub(in crate::quality) fn run(document: &CanonicalDocument, markdown: &str) -> Vec<Flag> {
    let body = Body::of(document);
    [
        record::canonicalization_failed(document),
        body::near_empty(&body),
        body::navigation_heavy(&body),
        text::replacement_characters(document),
        text::extraction_artifacts(document),
        secret::suspected_secret(markdown),
        record::table_incomplete(document),
        page::application_error(document),
        page::sign_in(document, &body),
        record::metadata_missing(document),
        record::assets_missing(document),
    ]
    .into_iter()
    .flatten()
    .collect()
}
