//! V2 card resource, provenance, role, sampling, and canonical-load validation tests.

use super::{
    v2::{answerer_identity, card_identity, digest, scratch_store},
    v2_golden::CANONICAL_ANSWERER_CARD,
    v2_validation::{accepts, configured_sampling, refuses, reranker_identity},
};
use crate::gateway::{
    ModelCard,
    card_types::CardError,
    card_v2::{
        Backend, Capability, Dimensions, EmbeddingFormat, FlagValue, Normalization, Observation,
        OffloadMode, Pooling, Sampling, TextFormat,
    },
};
use std::{
    fs,
    num::{NonZeroU32, NonZeroU64},
    str,
};

#[test]
fn v2_answerer_golden_pins_float_wire_bytes() {
    let (path, store) = scratch_store();
    let card = ModelCard::record_v2(&store, &answerer_identity()).unwrap();
    let bytes = store.get(card.digest()).unwrap();
    assert_eq!(str::from_utf8(&bytes).unwrap(), CANONICAL_ANSWERER_CARD);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_resources_validate_measurements_offload_and_batch_boundaries() {
    let (path, store) = scratch_store();
    let mut invalid = card_identity();
    invalid.resources.memory_estimate.source.clear();
    refuses(&invalid, &store, "memory_estimate.source is blank");
    for (field, value) in [
        ("hardware.device", ""),
        ("hardware.driver_version", " "),
        ("hardware.host_cpu", "\t"),
    ] {
        let mut invalid = card_identity();
        match field {
            "hardware.device" => invalid.resources.hardware.device = value.to_owned(),
            "hardware.driver_version" => {
                invalid.resources.hardware.driver_version = value.to_owned();
            }
            "hardware.host_cpu" => invalid.resources.hardware.host_cpu = value.to_owned(),
            _ => panic!("unknown hardware field {field}"),
        }
        refuses(&invalid, &store, &format!("{field} is blank"));
    }

    let mut invalid = card_identity();
    invalid.resources.micro_batch_size = NonZeroU32::new(513).unwrap();
    refuses(&invalid, &store, "micro_batch_size exceeds batch_size");
    let mut equal_batches = card_identity();
    equal_batches.resources.micro_batch_size = equal_batches.resources.batch_size;
    accepts(&equal_batches, &store);

    for mode in [OffloadMode::Automatic, OffloadMode::CpuOnly] {
        let mut invalid = card_identity();
        invalid.resources.offload.mode = mode;
        invalid.resources.offload.devices = vec!["gpu0".to_owned()];
        refuses(
            &invalid,
            &store,
            "automatic and cpu_only offload have no device names",
        );
        let mut valid = card_identity();
        valid.resources.offload.mode = mode;
        valid.resources.offload.devices.clear();
        accepts(&valid, &store);
    }
    let mut invalid = card_identity();
    invalid.resources.offload.devices.clear();
    refuses(&invalid, &store, "explicit_devices offload has no devices");
    let mut invalid = card_identity();
    invalid.resources.offload.devices = vec![" ".to_owned()];
    refuses(&invalid, &store, "offload.device is blank");
    let mut invalid = card_identity();
    invalid.invocation.backend = Backend::Cpu;
    refuses(&invalid, &store, "CPU backend cannot use explicit devices");

    let mut invalid = card_identity();
    invalid.resources.hardware.host_memory_bytes = Observation::Measured {
        value: NonZeroU64::new(1).unwrap(),
        provenance: " ".to_owned(),
    };
    refuses(&invalid, &store, "host_memory_bytes is blank");
    let mut invalid = card_identity();
    invalid.resources.qualified_limits.output_tokens = Observation::Unavailable {
        reason: " ".to_owned(),
    };
    refuses(&invalid, &store, "qualified_limits.output_tokens is blank");
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_provenance_requires_calendar_dates_and_nonblank_maps() {
    let (path, store) = scratch_store();
    for date in [
        "2026-02-29",
        "2026/02-01",
        "2026-02/01",
        "2026-02-aa",
        "2026-02-01 ",
    ] {
        let mut invalid = card_identity();
        invalid.provenance.identity_created = date.to_owned();
        let expected = if date == "2026-02-29" {
            "identity_created is not a calendar date"
        } else {
            "identity_created must be YYYY-MM-DD"
        };
        refuses(&invalid, &store, expected);
    }
    for date in ["2024-02-29", "2028-02-29", "2000-02-29", "2026-04-30"] {
        let mut valid = card_identity();
        valid.provenance.identity_created = date.to_owned();
        accepts(&valid, &store);
    }
    for date in [
        "1900-02-29",
        "2100-02-29",
        "2026-04-31",
        "2026-01-00",
        "2026-13-01",
    ] {
        let mut invalid = card_identity();
        invalid.provenance.identity_created = date.to_owned();
        refuses(&invalid, &store, "identity_created is not a calendar date");
    }
    let mut invalid = card_identity();
    invalid.provenance.qualification_created = "2026-02-30".to_owned();
    refuses(
        &invalid,
        &store,
        "qualification_created is not a calendar date",
    );

    let mut invalid = card_identity();
    invalid
        .provenance
        .tool_versions
        .insert(" ".to_owned(), "1".to_owned());
    refuses(&invalid, &store, "tool_versions is blank");
    let mut invalid = card_identity();
    invalid
        .provenance
        .tool_versions
        .insert("runtime".to_owned(), " ".to_owned());
    refuses(&invalid, &store, "tool_versions is blank");
    let mut invalid = card_identity();
    invalid
        .provenance
        .artifacts
        .insert(" ".to_owned(), digest('e'));
    refuses(&invalid, &store, "provenance.artifacts key is blank");

    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_provenance_allows_portable_text_but_refuses_paths() {
    let (path, store) = scratch_store();
    let mut valid = card_identity();
    valid.formats.system_format = "/no_think".to_owned();
    valid.resources.memory_estimate.source = "estimate from /props".to_owned();
    valid.resources.hardware.host_memory_bytes = Observation::Measured {
        value: NonZeroU64::new(1024).unwrap(),
        provenance: "reported via /no_think".to_owned(),
    };
    valid.resources.qualified_limits.output_tokens = Observation::Unavailable {
        reason: "not measured for /v1".to_owned(),
    };
    valid
        .provenance
        .tool_versions
        .insert("tokenizer".to_owned(), "router /props".to_owned());
    valid
        .invocation
        .server_flags
        .insert("--api-prefix".to_owned(), FlagValue::Text("/v1".to_owned()));
    valid.invocation.server_flags.insert(
        "--alias".to_owned(),
        FlagValue::Text("BAAI/bge-m3".to_owned()),
    );
    accepts(&valid, &store);

    for path_value in [
        "/srv/model.gguf",
        r"\server\share\model.gguf",
        r"C:\models\model.gguf",
        "file:///srv/model.gguf",
        "a/b",
        r"a\b",
    ] {
        let mut invalid = card_identity();
        invalid
            .invocation
            .server_flags
            .insert("--foo".to_owned(), FlagValue::Text(path_value.to_owned()));
        refuses(
            &invalid,
            &store,
            "file path must be a named digest-bound asset",
        );
    }
    for device in ["/dev/gpu0", r"C:\devices\gpu0", r"\\server\share\gpu0"] {
        let mut invalid = card_identity();
        invalid.resources.offload.devices = vec![device.to_owned()];
        refuses(&invalid, &store, "offload.device is a machine path");
    }
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_rejects_windows_drive_paths_in_upstream_model_ids() {
    let (path, store) = scratch_store();
    let mut invalid = card_identity();
    invalid.weights.upstream_model_id = r"C:\models\model".to_owned();
    refuses(&invalid, &store, "upstream_model_id is a machine path");
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_roles_require_their_exact_format_dimension_and_generation_profiles() {
    let (path, store) = scratch_store();
    for change in 0..7 {
        let mut invalid = card_identity();
        match change {
            0 => invalid.invocation.dimensions = Dimensions::NotApplicable,
            1 => invalid.formats.embedding = EmbeddingFormat::NotApplicable,
            2 => invalid.formats.document = Capability::Unsupported,
            3 => invalid.formats.query = Capability::Unsupported,
            4 => invalid.invocation.sampling = Sampling::Configured(configured_sampling()),
            5 => invalid.invocation.reasoning = Capability::Unsupported,
            6 => invalid.invocation.limits.output_tokens = NonZeroU32::new(128),
            _ => panic!("unknown embedder profile field {change}"),
        }
        refuses(&invalid, &store, "disagree with embedder role");
    }

    let valid_reranker = reranker_identity();
    accepts(&valid_reranker, &store);
    for change in 0..6 {
        let mut invalid = reranker_identity();
        match change {
            0 => {
                invalid.invocation.dimensions =
                    Dimensions::Measured(NonZeroU32::new(3).unwrap().try_into().unwrap());
            }
            1 => {
                invalid.formats.embedding = EmbeddingFormat::Supported {
                    pooling: Pooling::Mean,
                    normalization: Normalization::L2,
                }
            }
            2 => {
                invalid.formats.document = Capability::Supported(TextFormat {
                    prefix: String::new(),
                    suffix: String::new(),
                });
            }
            3 => {
                invalid.formats.query = Capability::Supported(TextFormat {
                    prefix: String::new(),
                    suffix: String::new(),
                });
            }
            4 => invalid.invocation.sampling = Sampling::Configured(configured_sampling()),
            5 => invalid.invocation.reasoning = Capability::Unsupported,
            _ => panic!("unknown reranker profile field {change}"),
        }
        refuses(&invalid, &store, "disagree with reranker role");
    }
    let mut invalid = reranker_identity();
    invalid.invocation.limits.output_tokens = NonZeroU32::new(128);
    refuses(&invalid, &store, "disagree with reranker role");

    accepts(&answerer_identity(), &store);
    let mut invalid = answerer_identity();
    invalid.invocation.sampling = Sampling::NotApplicable;
    refuses(&invalid, &store, "disagree with answerer role");
    let mut invalid = answerer_identity();
    invalid.invocation.limits.output_tokens = None;
    refuses(&invalid, &store, "disagree with answerer role");
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn configured_sampling_checks_probability_and_penalty_boundaries() {
    let (path, store) = scratch_store();
    let valid = answerer_identity();
    accepts(&valid, &store);
    let mut zero_temperature = answerer_identity();
    zero_temperature.invocation.sampling = Sampling::Configured(configured_sampling());
    accepts(&zero_temperature, &store);
    for (field, value) in [
        ("top_p", -0.1),
        ("top_p", 1.1),
        ("min_p", -0.1),
        ("min_p", 1.1),
        ("typical_p", -0.1),
        ("typical_p", 1.1),
        ("temperature", -0.1),
        ("repeat_penalty", 0.0),
    ] {
        let mut invalid = answerer_identity();
        let Sampling::Configured(sampling) = &mut invalid.invocation.sampling else {
            panic!("answerer sampling should be configured");
        };
        match field {
            "top_p" => sampling.top_p = value,
            "min_p" => sampling.min_p = value,
            "typical_p" => sampling.typical_p = value,
            "temperature" => sampling.temperature = value,
            "repeat_penalty" => sampling.repeat_penalty = value,
            _ => panic!("unknown sampling field {field}"),
        }
        refuses(
            &invalid,
            &store,
            "sampling probability or penalty is out of range",
        );
    }
    let mut invalid = answerer_identity();
    let Sampling::Configured(sampling) = &mut invalid.invocation.sampling else {
        panic!("answerer sampling should be configured");
    };
    sampling.frequency_penalty = f64::NAN;
    refuses(&invalid, &store, "frequency_penalty must be finite");
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_loading_refuses_noncanonical_json_bytes() {
    let (path, store) = scratch_store();
    let card = ModelCard::record_v2(&store, &card_identity()).unwrap();
    let bytes = store.get(card.digest()).unwrap();
    let noncanonical = format!(" {}", str::from_utf8(&bytes).unwrap()).into_bytes();
    assert!(matches!(
        ModelCard::from_json_bytes(&noncanonical),
        Err(CardError::Invalid(_))
    ));
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn v2_schema_and_unique_map_errors_name_the_refused_shape() {
    let (path, store) = scratch_store();
    let card = ModelCard::record_v2(&store, &card_identity()).unwrap();
    let bytes = store.get(card.digest()).unwrap();
    let canonical = str::from_utf8(&bytes).unwrap();
    let unknown_v2 = canonical.replacen(
        r#""schema":"maestro-model-card/2""#,
        r#""schema":"maestro-model-card/9""#,
        1,
    );
    assert!(matches!(
        ModelCard::from_json_bytes(unknown_v2.as_bytes()),
        Err(CardError::Invalid(reason)) if reason.contains("schema")
    ));

    let v1 = concat!(
        r#"{"schema":"maestro-model-card/1","role":"embedder","router_entry":"embed","#,
        r#""file_digest":"ca56847038f3f329524caec5a86e14865f49918d68094c90"#,
        r#"dd021a5e67b927f6","template_digest":null,"server_build":"b6500-3f2c9a1b","#,
        r#""dimensions":1024,"limits":{"context_tokens":8192,"#,
        r#""output_tokens":null},"suite_results":[]}"#,
    );
    let mut unknown_v1: serde_json::Value = serde_json::from_str(v1).unwrap();
    unknown_v1["schema"] = serde_json::json!("maestro-model-card/999");
    assert!(matches!(
        ModelCard::from_json_bytes(&serde_json::to_vec(&unknown_v1).unwrap()),
        Err(CardError::Invalid(_))
    ));

    let mut malformed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    malformed["identity"]["provenance"]["tool_versions"] = serde_json::json!("not-a-map");
    let error = ModelCard::from_json_bytes(&serde_json::to_vec(&malformed).unwrap()).unwrap_err();
    assert!(matches!(
        error,
        CardError::Invalid(reason) if reason.contains("a map with unique keys")
    ));
    fs::remove_dir_all(path).unwrap();
}
