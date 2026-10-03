//! Adapter from delivered evidence to the graph scorer's source coordinates.

use crate::knowledge::output::{SearchOutputError, truncate_search_bundle};
use maestro_kernel::evidence::Bundle;
use maestro_knowledge::eval::Located;

/// Source spans surviving shared context assembly and transport packing.
pub(super) fn anchors(bundle: Bundle) -> Result<Vec<Located>, SearchOutputError> {
    let bounded = truncate_search_bundle(bundle)?;
    Ok(bounded
        .bundle
        .passages
        .iter()
        .map(|passage| Located {
            revision_id: passage.revision_id.clone(),
            span: [passage.span.start, passage.span.end],
        })
        .collect())
}
