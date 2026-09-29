//! A rung's question parts: its manifest's `question_parts`, the glossary
//! it pins by digest, which this machine binds for the collection as
//! `glossary_<collection>`, and what its reports record of each question's
//! parts, without their words. With nothing pinned, parts are ranked with
//! their own words.

use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{artifact::Digest, binding::Bindings, evidence::RouteStatus};
use maestro_knowledge::search::{
    Glossary, MAX_GLOSSARY_BYTES, PartsRecord, QuestionParts, Unsplit,
};
use serde::Serialize;
use std::{collections::BTreeMap, fmt::Display, fs, path::Path, sync::Arc};

/// Whether `parts` is off, which the manifest leaves unwritten.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
pub(super) fn is_off(parts: &QuestionParts) -> bool {
    *parts == QuestionParts::Off
}

/// The digest of the glossary `pin` names for a rung whose parts are
/// `parts`, if any.
///
/// # Errors
///
/// [`Failure::Refused`] for a pin that is not a SHA-256 digest, or a pin
/// on a rung whose parts are off.
pub(super) fn glossary_pin(
    parts: QuestionParts,
    pin: Option<&str>,
) -> Result<Option<Digest>, Failure> {
    let Some(pin) = pin else {
        return Ok(None);
    };
    if is_off(&parts) {
        return Err(Failure::refused(
            "a rung pins a glossary but its question parts are off",
        ));
    }
    Digest::parse(pin)
        .map(Some)
        .map_err(|_| Failure::refused("a rung's glossary is not a SHA-256 digest"))
}

/// The glossary `pin` names for a rung whose parts are `parts`: the one
/// `kernel` binds for `collection`, when its digest is the pin.
///
/// # Errors
///
/// As [`glossary_pin`] and [`load_glossary`], and [`Failure::Refused`] when
/// no glossary is bound or its digest differs from the pin.
pub(super) fn rung_glossary(
    kernel: &Kernel,
    collection: &str,
    parts: QuestionParts,
    pin: Option<&str>,
) -> Result<Option<Arc<Glossary>>, Failure> {
    let Some(pin) = glossary_pin(parts, pin)? else {
        return Ok(None);
    };
    let bound = load_glossary(&kernel.config_dir, collection)?
        .ok_or_else(|| Failure::refused("a rung pins a glossary, but none is bound"))?;
    if bound.digest() != &pin {
        return Err(Failure::refused(format!(
            "the bound glossary's digest is {}, not the rung's pin",
            bound.digest().as_str()
        )));
    }
    Ok(Some(bound))
}

/// The binding that names the glossary of `collection`.
pub(super) fn glossary_binding(collection: &str) -> String {
    format!("glossary_{collection}")
}

/// The glossary the bindings of `config_dir` name for `collection`; none
/// when nothing binds it.
///
/// # Errors
///
/// [`Failure::Refused`] when the bindings file is invalid, or the bound
/// glossary cannot be read, is over [`MAX_GLOSSARY_BYTES`] or is not a
/// `maestro-glossary/1` table.
pub(super) fn load_glossary(
    config_dir: &Path,
    collection: &str,
) -> Result<Option<Arc<Glossary>>, Failure> {
    let bindings = Bindings::load(config_dir).map_err(|error| Failure::refused_by(&error))?;
    let Ok(path) = bindings.path(&glossary_binding(collection)) else {
        return Ok(None);
    };
    let unreadable = |error: &dyn Display| {
        Failure::refused(format!(
            "cannot read the glossary {}: {error}",
            path.display()
        ))
    };
    let length = fs::metadata(path)
        .map_err(|error| unreadable(&error))?
        .len();
    if usize::try_from(length).map_or(true, |length| length > MAX_GLOSSARY_BYTES) {
        return Err(Failure::refused(format!(
            "the glossary {} is over {MAX_GLOSSARY_BYTES} bytes",
            path.display()
        )));
    }
    let bytes = fs::read(path).map_err(|error| unreadable(&error))?;
    Glossary::parse(&bytes)
        .map(|glossary| Some(Arc::new(glossary)))
        .map_err(|error| Failure::refused_by(&error))
}

/// What a private row records of a question's parts: no words.
#[derive(Debug, Serialize)]
pub(super) struct PartsRow<'run> {
    /// Why the question has no more parts, if a reason applies.
    unsplit: Option<&'static str>,
    /// The whole question's best passage's chunk, reserved before each
    /// part's.
    whole: Option<&'run str>,
    /// Each part, in reading order; empty when the question was not split.
    parts: Vec<PartRow<'run>>,
}

/// What a private row records of one part.
#[derive(Debug, Serialize)]
struct PartRow<'run> {
    /// Whether its ranking ran, or why it fell back to the whole question.
    status: &'run RouteStatus,
    /// How its bridge ended; absent when the rung has no glossary.
    bridge: Option<&'run RouteStatus>,
    /// Its best passage's chunk, which the bundle reserves.
    best: Option<&'run str>,
}

impl<'run> PartsRow<'run> {
    /// The row of `record`.
    pub(super) fn new(record: &'run PartsRecord) -> Self {
        Self {
            unsplit: record.unsplit.map(Unsplit::code),
            whole: record.whole.as_deref(),
            parts: record
                .parts
                .iter()
                .map(|part| PartRow {
                    status: &part.status,
                    bridge: part.bridge_status.as_ref(),
                    best: part.best.as_deref(),
                })
                .collect(),
        }
    }
}

/// How many questions were split, and how many stayed whole for each
/// reason, over `records`; empty when parts are off.
pub(super) fn part_counts<'run>(
    records: impl Iterator<Item = &'run PartsRecord>,
) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for record in records {
        let outcome = if record.parts.is_empty() {
            record.unsplit.map_or("whole", Unsplit::code)
        } else {
            "split"
        };
        *counts.entry(outcome).or_default() += 1;
    }
    counts
}
