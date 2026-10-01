//! Machine-path checks for v2 provenance fields.

use super::{
    v2::{card_identity, digest, scratch_store},
    v2_validation::refuses,
};
use crate::gateway::card_v2::{CardIdentity, Observation};
use std::{
    fs,
    num::{NonZeroU32, NonZeroU64},
};

fn observation<T>(value: T, text: &str, unavailable: bool) -> Observation<T> {
    if unavailable {
        Observation::Unavailable {
            reason: text.to_owned(),
        }
    } else {
        Observation::Measured {
            value,
            provenance: text.to_owned(),
        }
    }
}

fn set_observation(identity: &mut CardIdentity, field: &str, text: &str, unavailable: bool) {
    match field {
        "host_memory_bytes" => {
            identity.resources.hardware.host_memory_bytes =
                observation(NonZeroU64::new(1).unwrap(), text, unavailable);
        }
        "qualified_limits.context_tokens" => {
            identity.resources.qualified_limits.context_tokens =
                observation(NonZeroU32::new(1).unwrap(), text, unavailable);
        }
        "qualified_limits.output_tokens" => {
            identity.resources.qualified_limits.output_tokens =
                observation(NonZeroU32::new(1).unwrap(), text, unavailable);
        }
        "qualified_limits.concurrency" => {
            identity.resources.qualified_limits.concurrency =
                observation(NonZeroU32::new(1).unwrap(), text, unavailable);
        }
        "qualified_limits.peak_memory_bytes" => {
            identity.resources.qualified_limits.peak_memory_bytes =
                observation(NonZeroU64::new(1).unwrap(), text, unavailable);
        }
        _ => panic!("unknown observation field {field}"),
    }
}

#[test]
fn provenance_artifact_names_refuse_local_paths() {
    let (path, store) = scratch_store();
    for local_path in [
        "/var/lib/maestro/run/qualification.json",
        "file:///opt/maestro/bin/llama-bench",
        r"C:\bin\llama-bench",
        r"\\server\share\bin\llama-bench",
    ] {
        let mut invalid = card_identity();
        invalid
            .provenance
            .artifacts
            .insert(local_path.to_owned(), digest('e'));
        refuses(
            &invalid,
            &store,
            "provenance.artifacts key asset name is a path",
        );
    }
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn memory_estimate_source_refuses_local_paths() {
    let (path, store) = scratch_store();
    for local_path in [
        "measured by /var/lib/maestro/run/qualification.json",
        "measured by file:///opt/maestro/bin/llama-bench",
        r"measured by C:\bin\llama-bench",
        r"measured by \\server\share\bin\llama-bench",
    ] {
        let mut invalid = card_identity();
        invalid.resources.memory_estimate.source = local_path.to_owned();
        refuses(
            &invalid,
            &store,
            "memory_estimate.source contains a machine path",
        );
    }
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn memory_estimate_source_refuses_standalone_path_roots() {
    let (path, store) = scratch_store();
    for local_path in ["/var/lib", r"C:\bin", r"\\server", "//server"] {
        let mut invalid = card_identity();
        invalid.resources.memory_estimate.source = local_path.to_owned();
        refuses(
            &invalid,
            &store,
            "memory_estimate.source contains a machine path",
        );
    }
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn measurement_provenance_and_reason_refuse_paths_in_every_field() {
    let (path, store) = scratch_store();
    let fields = [
        "host_memory_bytes",
        "qualified_limits.context_tokens",
        "qualified_limits.output_tokens",
        "qualified_limits.concurrency",
        "qualified_limits.peak_memory_bytes",
    ];
    let local_paths = [
        "measured by /opt/maestro/bin/llama-bench",
        r"measured by C:\bin\llama-bench",
        r"measured by \\server\share\bin\llama-bench",
    ];
    for field in fields {
        for local_path in local_paths {
            let mut invalid = card_identity();
            set_observation(&mut invalid, field, local_path, false);
            refuses(
                &invalid,
                &store,
                &format!("{field} contains a machine path"),
            );
            let mut invalid = card_identity();
            set_observation(&mut invalid, field, local_path, true);
            refuses(
                &invalid,
                &store,
                &format!("{field} contains a machine path"),
            );
        }
    }
    fs::remove_dir_all(path).unwrap();
}
