//! Private evaluation's single public error boundary; raw causes stay on disk.

use super::super::output::Output;
use crate::failure::Failure;
use maestro_kernel::artifact::Digest;
use serde::Serialize;
use std::{fmt::Debug, fs::OpenOptions, io::Write as _, path::Path};
use ulid::Ulid;

/// Closed refusal vocabulary shared by graph check and private ladder commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Code {
    /// Manifest syntax or a frozen input is invalid.
    Manifest,
    /// Scratch bindings or approval are absent, expired or overlap live state.
    Isolation,
    /// The scratch authority cannot validate labels.
    Labels,
    /// Reading the scratch authority failed operationally.
    Authority,
    /// An attempted private ladder failed.
    Ladder,
    /// Graph retrieval is not delivered by G13/G27 yet.
    GraphUnavailable,
    /// A private receipt cannot be written.
    Output,
}

impl Code {
    /// Only fixed public text, never an underlying parser, source or service error.
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Manifest => "graph_manifest_invalid",
            Self::Isolation => "graph_scratch_refused",
            Self::Labels => "graph_labels_invalid",
            Self::Authority => "graph_authority_failed",
            Self::Ladder => "graph_ladder_failed",
            Self::GraphUnavailable => "graph_route_not_available_until_g13_g27",
            Self::Output => "graph_private_output_failed",
        }
    }

    /// Crosses the ordinary CLI failure boundary without attaching private causes.
    pub(super) fn failure(self) -> Failure {
        match self {
            Self::Authority | Self::Ladder | Self::Output => Failure::failed(self.name()),
            _ => Failure::refused(self.name()),
        }
    }

    /// Sanitizes arbitrary text while preserving failure category and known safe codes.
    pub(super) fn sanitize(self, error: &Failure) -> Failure {
        let text = error.to_string();
        let safe = [
            Self::Manifest,
            Self::Isolation,
            Self::Labels,
            Self::Authority,
            Self::Ladder,
            Self::GraphUnavailable,
            Self::Output,
        ]
        .into_iter()
        .any(|code| code.name() == text);
        let message = if safe { text.as_str() } else { self.name() };
        match error {
            Failure::Refused(_) => Failure::refused(message),
            Failure::Failed(_) => Failure::failed(message),
        }
    }
}

/// Retains a raw diagnostic only inside an already checked private output root.
/// Failure to retain it is itself explicit, never replaced by printing the cause.
pub(super) fn diagnostic(root: &Path, error: &dyn Debug) -> Result<(), Code> {
    let path = root.join(format!("diagnostic-{}.txt", Ulid::generate()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| Code::Output)?;
    writeln!(file, "{error:?}").map_err(|_| Code::Output)
}

/// Safe aggregate contract reused by check, drafting and review.
#[derive(Debug, Default, Serialize)]
pub(super) struct Counts {
    /// Input items processed.
    pub(super) items: usize,
    /// Items still requiring review.
    pub(super) unreviewed: usize,
    /// Answerable items.
    pub(super) answerable: usize,
    /// Unanswerable items.
    pub(super) unanswerable: usize,
    /// Validated links.
    pub(super) links: usize,
    /// Validated anchors.
    pub(super) anchors: usize,
    /// Complete delivered proofs.
    pub(super) complete: usize,
    /// Retained failed attempts.
    pub(super) failed: usize,
}

/// Closed safe success document. Digests are validated types, not arbitrary strings.
#[derive(Serialize)]
struct Summary<'a> {
    /// Versioned public contract.
    schema: &'static str,
    /// No partial CLI run can claim M2 acceptance before its dependent adapters land.
    m2: &'static str,
    /// Frozen input identity.
    digest: &'a str,
    /// Aggregate values only.
    #[serde(flatten)]
    counts: &'a Counts,
}

/// Emits aggregate-only success; no generic serializer can accidentally expose text.
pub(super) fn emit(output: Output, digest: &Digest, counts: &Counts) -> Result<(), Code> {
    let summary = Summary {
        schema: "maestro-private-eval/1",
        m2: "blocked_g13_g27_g32",
        digest: digest.as_str(),
        counts,
    };
    output
        .result(
            &summary,
            &format!(
                "m2=blocked_g13_g27_g32 items={} failed={} digest={}",
                counts.items,
                counts.failed,
                digest.as_str()
            ),
        )
        .map_err(|_| Code::Output)
}
