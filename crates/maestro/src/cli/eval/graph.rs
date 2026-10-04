//! Inference-free graph checks using only the scoped scratch authority.

use super::{
    super::output::Output,
    graph_manifest::{GraphManifest, Inputs},
    graph_output::{self, Code},
};
use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::eval::{CheckedLabels, Original, check_labels};
use std::{path::Path, process::ExitCode};

/// Checks a private manifest with no default-kernel fallback or inference.
pub(in crate::cli) fn run(output: Output, path: &Path) -> Result<ExitCode, Failure> {
    let inputs = GraphManifest::read(path).map_err(Code::failure)?;
    let labels = checked_labels(&inputs).map_err(Code::failure)?;
    let counts = &labels.summary;
    let summary = graph_output::Counts {
        items: counts.items,
        unreviewed: counts.unreviewed,
        answerable: counts.answerable,
        unanswerable: counts.unanswerable,
        links: counts.links,
        anchors: counts.anchors,
        ..graph_output::Counts::default()
    };
    graph_output::emit(output, &inputs.digest, &summary).map_err(Code::failure)?;
    Ok(ExitCode::SUCCESS)
}

/// Shared validation boundary for check and the existing private ladder.
pub(super) fn checked_labels(inputs: &Inputs) -> Result<CheckedLabels, Code> {
    let result = (|| {
        let kernel = inputs.run.open()?;
        check_labels(
            &inputs.suite,
            &inputs.labels,
            &inputs.digest,
            inputs.stage,
            |source, digest| original(&kernel, &inputs.collection, source, digest),
        )
        .map_err(|error| Failure::refused(format!("{error:?}")))
    })();
    result.map_err(|error| {
        graph_output::diagnostic(&inputs.run.output, &error)
            .err()
            .unwrap_or(match error {
                Failure::Refused(_) => Code::Labels,
                Failure::Failed(_) => Code::Authority,
            })
    })
}

/// Resolves a source/digest pair through scoped authoritative records only.
fn original(
    kernel: &Kernel,
    collection: &str,
    source: &str,
    digest: &Digest,
) -> Result<Option<Original>, Failure> {
    let revisions = kernel
        .database
        .eligible_revisions(&kernel.scopes, collection)
        .map_err(|error| Failure::failed_by(&error))?;
    let mut found = None;
    for revision in revisions {
        if revision.original_digest != *digest {
            continue;
        }
        let document = kernel
            .database
            .document(&kernel.scopes, &revision.document_id)
            .map_err(|error| Failure::failed_by(&error))?;
        if document.is_none_or(|document| document.source_ref != source) {
            continue;
        }
        if found.is_some() {
            return Err(Failure::refused("ambiguous_source"));
        }
        let bytes = kernel
            .database
            .get(digest)
            .map_err(|error| Failure::failed_by(&error))?;
        found = Some(Original {
            revision_id: revision.id,
            bytes,
        });
    }
    Ok(found)
}
