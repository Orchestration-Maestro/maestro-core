//! Strict baseline loading and deterministic candidate comparison.

use super::super::{
    comparison::{self, Refusal},
    failure::Failure,
};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::eval::{Report, Schema};
use std::{fs, path::Path, str};

#[derive(Debug)]
pub(in crate::synthetic_gate) struct Baseline {
    pub(in crate::synthetic_gate) report: Report,
    pub(in crate::synthetic_gate) digest: String,
}

pub(in crate::synthetic_gate) fn load(path: &Path) -> Result<Baseline, Failure> {
    let bytes = fs::read(path).map_err(|error| Failure::from_error("baseline-read", error))?;
    let text =
        str::from_utf8(&bytes).map_err(|error| Failure::from_error("baseline-format", error))?;
    let report: Report = text
        .parse()
        .map_err(|error| Failure::from_error("baseline-format", error))?;
    if report.schema != Schema::V1 {
        return Err(Failure::new("baseline-schema"));
    }
    Ok(Baseline {
        report,
        digest: Digest::of(&bytes).as_str().to_owned(),
    })
}

pub(in crate::synthetic_gate) fn compare(
    baseline: &Baseline,
    candidate: &Report,
) -> Result<(), Failure> {
    comparison::require_no_regression(&baseline.report, candidate).map_err(|refusal| {
        let stage = match refusal {
            Refusal::Identity => "baseline-identity",
            Refusal::Regression(_) => "baseline-regression",
            _ => "baseline-invalid",
        };
        Failure::from_error(stage, format!("{refusal:?}"))
    })
}
