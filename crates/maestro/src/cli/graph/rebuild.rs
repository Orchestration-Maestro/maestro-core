//! One frozen-input projection build; published-generation repair stays explicit.
use crate::{cli::output::Output, failure::Failure, kernel::Kernel, settings::Session};
use maestro_kernel::artifact::Digest;
use std::process::ExitCode;
use ulid::Ulid;

/// Literal messages kept together for the translation-key migration.
const GENERATION_REQUIRED: &str = "select the attached generation with --generation";
/// A resolution is never inferred from live review state.
const RESOLUTION_REQUIRED: &str = "no pinned resolution; supply --resolution SHA256";
/// The pilot does not reconstruct shared search generations.
const REPAIR_PENDING: &str = "model-free repair of a published generation is pending design";

/// Native-disabled diagnostic.
#[cfg(not(feature = "engine"))]
const ENGINE_REQUIRED: &str = "graph rebuild requires a build with the engine feature";
/// Disabled runtime selection.
#[cfg(feature = "engine")]
const ENGINE_OFF: &str = "the graph is off (graph.engine = none)";
/// Scope-safe authority diagnostic.
#[cfg(feature = "engine")]
const UNKNOWN_GENERATION: &str = "unknown or unauthorized graph generation";
/// No implicit membership choice.
#[cfg(feature = "engine")]
const UNATTACHED: &str = "generation has no attached claim set";
/// A completed job is not enough without authority readiness.
#[cfg(feature = "engine")]
const MISSING_RECEIPT: &str = "completed graph project has no readiness receipt";
/// Successful immutable readiness check.
#[cfg(feature = "engine")]
const READY: &str = "ready, nothing to rebuild";
/// Successful first publication.
#[cfg(feature = "engine")]
const BUILT: &str = "graph projection ready";

/// Explicit target and recovery selection; bare repair text parses and reports guidance.
#[derive(Debug, clap::Args)]
pub(in crate::cli) struct Arguments {
    /// Existing attached, verified generation; no search generation is created.
    #[arg(long)]
    pub(in crate::cli) generation: Option<i64>,
    /// Frozen resolution digest; defaults only to the existing receipt's pin.
    #[arg(long, value_name = "SHA256")]
    pub(in crate::cli) resolution: Option<String>,
    /// Reopen this interrupted loader-owned project job, never a terminal job.
    #[arg(long, value_name = "JOB")]
    pub(in crate::cli) resume: Option<Ulid>,
}

/// Dispatch without activating an absent native backend.
pub(in crate::cli) fn run(
    kernel: &Kernel,
    output: Output,
    session: &Session,
    arguments: &Arguments,
) -> Result<ExitCode, Failure> {
    arguments
        .generation
        .ok_or_else(|| Failure::refused(GENERATION_REQUIRED))?;
    #[cfg(feature = "engine")]
    {
        native(kernel, output, session, arguments)
    }
    #[cfg(not(feature = "engine"))]
    {
        let _ = (kernel, output, session);
        Err(Failure::refused(ENGINE_REQUIRED))
    }
}

/// Chosen identity and its provenance, also printed on a readiness no-op.
#[cfg_attr(
    all(not(feature = "engine"), not(test)),
    expect(
        dead_code,
        reason = "featureless tests verify resolution policy without activating a backend"
    )
)]
fn resolution(
    flag: Option<&str>,
    receipt: Option<&Digest>,
) -> Result<(Digest, &'static str), Failure> {
    if let Some(flag) = flag {
        return Digest::parse(flag)
            .map(|id| (id, "flag"))
            .map_err(|error| Failure::refused_by(&error));
    }
    receipt
        .cloned()
        .map(|id| (id, "receipt"))
        .ok_or_else(|| Failure::refused(RESOLUTION_REQUIRED))
}

/// The only temporary refusal policy replaced by the designed recovery slice.
#[cfg_attr(
    all(not(feature = "engine"), not(test)),
    expect(
        dead_code,
        reason = "featureless tests verify published refusal policy"
    )
)]
fn published_refusal(case: &str) -> Failure {
    Failure::refused(format!("graph rebuild refused ({case}); {REPAIR_PENDING}"))
}

#[cfg(feature = "engine")]
use super::rebuild_work::{Selection, Work};
#[cfg(feature = "engine")]
use maestro_filesystem::{OwnedRoot, SystemFileLock};
#[cfg(feature = "engine")]
use maestro_kernel::{
    facts::{Error as FactError, InputMismatchKind, ProjectionReceipt},
    generation::GenerationState,
    paths::{self, Environment},
};
#[cfg(feature = "engine")]
use maestro_knowledge::graph::projection::{
    EngineSettings, ProjectionEngine, ProjectionFactory, ProjectionScope,
};
#[cfg(feature = "engine")]
use serde_json::{Value, json};
#[cfg(feature = "engine")]
use std::{io::ErrorKind, path::Path};

/// Select authority and pins before any writer reservation or job replay.
#[cfg(feature = "engine")]
fn native(
    kernel: &Kernel,
    output: Output,
    session: &Session,
    arguments: &Arguments,
) -> Result<ExitCode, Failure> {
    let generation = arguments
        .generation
        .ok_or_else(|| Failure::refused(GENERATION_REQUIRED))?;
    let settings = session
        .graph_settings()?
        .ok_or_else(|| Failure::refused(ENGINE_OFF))?;
    let record = kernel
        .database
        .generation(&kernel.scopes, generation)
        .map_err(|error| Failure::failed_by(&error))?
        .ok_or_else(|| Failure::refused(UNKNOWN_GENERATION))?;
    let attachment = kernel
        .database
        .graph_attachment(&kernel.scopes, generation)
        .map_err(|error| Failure::refused_by(&error))?
        .ok_or_else(|| Failure::refused(UNATTACHED))?;
    let receipt = match kernel.database.projection_ready(&kernel.scopes, generation) {
        Ok(receipt) => receipt,
        Err(FactError::ProjectionInputMismatch(InputMismatchKind::Format)) => {
            resolution(arguments.resolution.as_deref(), None)?;
            return Err(published_refusal("legacy receipt"));
        }
        Err(error) => return Err(Failure::failed_by(&error)),
    };
    let (resolution, origin) = resolution(
        arguments.resolution.as_deref(),
        receipt.as_ref().map(|receipt| &receipt.resolution_id),
    )?;
    let selection = Selection {
        scope: ProjectionScope {
            collection_id: record.collection_id,
            generation_id: generation,
        },
        claim_set: attachment.claim_set_id,
        resolution,
    };
    let data =
        paths::data_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))?;
    let path = data.join("graph");
    let factory = ProjectionFactory::new(
        &path,
        ProjectionEngine::Ladybug,
        settings.clone(),
        &SystemFileLock,
    );
    let chosen = json!({"schema":"maestro-cli/knowledge-graph-rebuild/1", "generation":generation,
        "resolution":selection.resolution, "resolution_source":origin});
    if let Some(receipt) = receipt {
        published_ready(kernel, &factory, &path, &selection, (&settings, &receipt))?;
        output.step(&chosen)?;
        output.result(&json!({
            "schema":"maestro-cli/knowledge-graph-rebuild/1", "action":"ready", "selection":chosen
        }), READY)?;
        return Ok(ExitCode::SUCCESS);
    }
    if record.state != GenerationState::Verified {
        return Err(Failure::refused(
            "graph rebuild requires a verified generation",
        ));
    }
    let snapshot = selection.read(kernel)?;
    let fresh = Work {
        selection: &selection,
        snapshot: &snapshot,
        factory: &factory,
        settings: &settings,
        descriptors: None,
    }
    .run(kernel, output, arguments.resume)?;
    let receipt = kernel
        .database
        .projection_ready(&kernel.scopes, generation)
        .map_err(|error| Failure::failed_by(&error))?
        .ok_or_else(|| Failure::failed(MISSING_RECEIPT))?;
    let action = completion_action(&selection, &settings, &receipt, fresh)?;
    output.step(&chosen)?;
    output.result(
        &json!({
            "schema":"maestro-cli/knowledge-graph-rebuild/1",
            "action":action, "selection":chosen,
            "receipt":receipt_document(&receipt)
        }),
        if fresh { BUILT } else { READY },
    )?;
    Ok(ExitCode::SUCCESS)
}

/// A competing completion must not be reported under this command's inputs.
#[cfg(feature = "engine")]
fn completion_action(
    selection: &Selection,
    settings: &EngineSettings,
    receipt: &ProjectionReceipt,
    fresh: bool,
) -> Result<&'static str, Failure> {
    if receipt.identity.claim_set_id != selection.claim_set
        || receipt.resolution_id != selection.resolution
        || receipt.settings_identity != settings.identity()
        || receipt.frozen_lock != *settings.frozen_lock()
    {
        return Err(Failure::refused("another build published different inputs"));
    }
    Ok(if fresh { "built" } else { "ready" })
}

/// Refuse in-place repair without touching an immutable receipt or file.
#[cfg(feature = "engine")]
fn published_ready(
    kernel: &Kernel,
    factory: &ProjectionFactory<'_>,
    path: &Path,
    selection: &Selection,
    pins: (&EngineSettings, &ProjectionReceipt),
) -> Result<(), Failure> {
    let (settings, receipt) = pins;
    if receipt.resolution_id != selection.resolution {
        return Err(published_refusal("changed resolution"));
    }
    if receipt.settings_identity != settings.identity() {
        return Err(published_refusal("changed settings"));
    }
    if receipt.frozen_lock != *settings.frozen_lock() {
        return Err(published_refusal("changed lock"));
    }
    let root = OwnedRoot::open(path, false).map_err(|error| Failure::refused_by(&error))?;
    match root.check_regular(&receipt.identity.file_name) {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(published_refusal("missing file"));
        }
        Err(_) => return Err(published_refusal("unsafe file")),
        Ok(()) => {}
    }
    factory
        .reader(&kernel.database, &kernel.scopes, selection.scope.clone())
        .map_err(|_| published_refusal("corrupt or unavailable file"))?;
    Ok(())
}

/// Public CLI summary; authority receipt types deliberately have no wire serialization.
#[cfg(feature = "engine")]
fn receipt_document(receipt: &ProjectionReceipt) -> Value {
    json!({"identity": {
        "file_name":receipt.identity.file_name, "claim_set":receipt.identity.claim_set_id,
        "knowledge_edge_count":receipt.identity.knowledge_edge_count,
        "entity_fact_count":receipt.identity.entity_fact_count,
        "content_digest":receipt.identity.content_digest
    }, "resolution":receipt.resolution_id,
        "settings_identity":receipt.settings_identity, "frozen_lock":receipt.frozen_lock})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graph_rebuild_resolution_selection_never_uses_latest() {
        let pin = Digest::of(b"receipt");
        let flag = Digest::of(b"flag");
        assert_eq!(
            resolution(None, Some(&pin)).unwrap(),
            (pin.clone(), "receipt")
        );
        assert_eq!(
            resolution(Some(flag.as_str()), Some(&pin)).unwrap(),
            (flag, "flag")
        );
        assert!(resolution(None, None).is_err());
        assert!(resolution(Some("bad"), Some(&pin)).is_err());
    }
    #[test]
    fn graph_rebuild_published_refusal_names_case_and_design_gap() {
        for case in [
            "legacy receipt",
            "missing file",
            "changed settings",
            "changed lock",
        ] {
            assert_eq!(
                published_refusal(case).to_string(),
                format!("graph rebuild refused ({case}); {REPAIR_PENDING}")
            );
        }
    }
    #[cfg(all(feature = "engine", not(windows)))]
    #[test]
    fn graph_rebuild_each_published_input_pin_refuses_before_native_open() {
        use crate::cli::graph::tests::cleanup_support::cleanup_fixture;
        use std::fs;
        let (fixture, generation, name) = cleanup_fixture();
        let kernel = &fixture.kernel;
        let settings =
            EngineSettings::new(16 * 1024 * 1024, 64 * 1024 * 1024, 1, Digest::of(b"lock"))
                .unwrap();
        let mut receipt = kernel
            .database
            .projection_ready(&kernel.scopes, generation)
            .unwrap()
            .unwrap();
        receipt.settings_identity = settings.identity();
        receipt.frozen_lock = settings.frozen_lock().clone();
        let selection = Selection {
            scope: ProjectionScope {
                collection_id: receipt.identity.collection_id.clone(),
                generation_id: generation,
            },
            claim_set: receipt.identity.claim_set_id.clone(),
            resolution: receipt.resolution_id.clone(),
        };
        let path = fixture.root.join("graph");
        let factory = ProjectionFactory::new(
            &path,
            ProjectionEngine::Ladybug,
            settings.clone(),
            &SystemFileLock,
        );
        let before = fs::read(path.join(&name)).unwrap();
        for (case, change) in [
            ("changed resolution", 0),
            ("changed settings", 1),
            ("changed lock", 2),
        ] {
            let mut changed = receipt.clone();
            match change {
                0 => changed.resolution_id = Digest::of(b"other"),
                1 => changed.settings_identity = Digest::of(b"other"),
                _ => changed.frozen_lock = Digest::of(b"other"),
            }
            let error = published_ready(kernel, &factory, &path, &selection, (&settings, &changed))
                .unwrap_err();
            assert_eq!(error.to_string(), published_refusal(case).to_string());
            assert_eq!(fs::read(path.join(&name)).unwrap(), before);
        }
    }
    #[cfg(all(feature = "engine", not(windows)))]
    #[test]
    fn graph_rebuild_concurrent_completion_rechecks_each_pin_and_identical_is_ready() {
        use crate::cli::graph::tests::rebuild_support::authority;
        use maestro_filesystem::ControlFile;
        let (fixture, selection) = authority();
        let kernel = &fixture.kernel;
        let path = fixture.root.join("graph");
        let root = OwnedRoot::open(&path, true).unwrap();
        for control in [ControlFile::Access, ControlFile::Writer] {
            root.ensure_control(control).unwrap();
        }
        let settings =
            EngineSettings::new(16 * 1024 * 1024, 64 * 1024 * 1024, 1, Digest::of(b"lock"))
                .unwrap();
        let factory = ProjectionFactory::new(
            &path,
            ProjectionEngine::Ladybug,
            settings.clone(),
            &SystemFileLock,
        );
        // Freeze this request before a competing command finishes.
        let snapshot = selection.read(kernel).unwrap();
        let work = Work {
            selection: &selection,
            snapshot: &snapshot,
            factory: &factory,
            settings: &settings,
            descriptors: None,
        };
        assert!(work.run(kernel, Output::new(true), None).unwrap());
        let receipt = kernel
            .database
            .projection_ready(&kernel.scopes, selection.scope.generation_id)
            .unwrap()
            .unwrap();
        for pin in 0..4 {
            let changed = Selection {
                scope: selection.scope.clone(),
                claim_set: if pin == 0 {
                    Digest::of(b"other")
                } else {
                    selection.claim_set.clone()
                },
                resolution: if pin == 1 {
                    Digest::of(b"other")
                } else {
                    selection.resolution.clone()
                },
            };
            let changed_settings = if pin == 2 {
                EngineSettings::new(16 * 1024 * 1024, 32 * 1024 * 1024, 1, Digest::of(b"lock"))
                    .unwrap()
            } else {
                settings.clone()
            };
            // Hold the already validated snapshot: the race is after preflight.
            let pending = Work {
                selection: &changed,
                snapshot: &snapshot,
                factory: &factory,
                settings: &changed_settings,
                descriptors: None,
            };
            assert!(
                !pending.run(kernel, Output::new(true), None).unwrap(),
                "existing completion is ready, never built"
            );
            let mut returned = receipt.clone();
            if pin == 3 {
                returned.frozen_lock = Digest::of(b"other");
            }
            let error =
                completion_action(&changed, &changed_settings, &returned, false).unwrap_err();
            assert!(matches!(error, Failure::Refused(_)));
            assert_eq!(
                error.to_string(),
                "another build published different inputs"
            );
        }
        assert!(!work.run(kernel, Output::new(true), None).unwrap());
        assert_eq!(
            completion_action(&selection, &settings, &receipt, false).unwrap(),
            "ready"
        );
        assert_eq!(
            completion_action(&selection, &settings, &receipt, true).unwrap(),
            "built"
        );
    }
}
