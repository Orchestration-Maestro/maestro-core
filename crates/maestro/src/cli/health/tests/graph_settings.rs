//! Graph health preferences never relax runtime adapter admission.
use super::support::Scratch;
use crate::{cli::session, settings::GraphEngine};
use maestro_catalog::{
    policy::workspace::{CheckedTrust, TrustBoundaries},
    settings::NoWorkspaceTrust,
};
use maestro_knowledge::graph::projection::health::{ProbeError, PublishedGraph, Receipt};

#[test]
fn health_preferences_ignore_unadmitted_authoring_locks_without_relaxing_runtime() {
    use std::fs;
    let scratch = Scratch::new();
    let boundaries = TrustBoundaries::new(&scratch.data(), &[]).unwrap();
    let trust = CheckedTrust::new(&NoWorkspaceTrust, &boundaries);
    let workspace = scratch.data().join("workspace");
    let directory = workspace.join(".maestro");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("authoring.lock.json"), b"invalid").unwrap();
    let health = session::health_at(
        &scratch.config(),
        Some(&workspace),
        Some(&scratch.data()),
        &[],
        &trust,
    )
    .unwrap();
    assert_eq!(
        GraphEngine::from_session(&health).unwrap(),
        GraphEngine::None
    );
    assert!(
        session::at(
            &scratch.config(),
            Some(&workspace),
            Some(&scratch.data()),
            &[]
        )
        .is_err()
    );
    assert_eq!(
        fs::read(directory.join("authoring.lock.json")).unwrap(),
        b"invalid"
    );
    assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
}

#[test]
fn the_graph_engine_is_none_unless_ladybug_is_selected() {
    let scratch = Scratch::new();
    let boundaries = TrustBoundaries::new(&scratch.data(), &[]).unwrap();
    let trust = CheckedTrust::new(&NoWorkspaceTrust, &boundaries);
    for (flags, expected) in [
        (vec![], GraphEngine::None),
        (
            vec!["graph.engine=ladybug".to_owned()],
            GraphEngine::Ladybug,
        ),
        (vec!["graph.engine=none".to_owned()], GraphEngine::None),
    ] {
        let session = session::health_at(&scratch.config(), None, None, &flags, &trust).unwrap();
        assert_eq!(GraphEngine::from_session(&session).unwrap(), expected);
    }
    #[cfg(not(feature = "engine"))]
    assert_eq!(
        session::at(
            &scratch.config(),
            None,
            None,
            &["graph.engine=ladybug".to_owned()]
        )
        .unwrap_err()
        .to_string(),
        "backend adapter is not compiled into this build",
    );
}

/// Health must never request a receipt when selection or admission forbids activation.
struct NoReceipt;
impl PublishedGraph for NoReceipt {
    fn receipt(&self) -> Result<Receipt<'_>, ProbeError> {
        panic!("receipt/native calls are forbidden");
    }
}

#[test]
fn retained_graph_refusal_and_explicit_none_read_no_receipt() {
    use super::super::{check::Outcome, graph::check_with};
    use crate::settings::GraphActivationError;
    use maestro_kernel::paths::Environment;
    let scratch = Scratch::new();
    let mut environment = Environment::default();
    environment.xdg_data_home = Some(scratch.data().into_os_string());
    let boundaries = TrustBoundaries::new(&scratch.data(), &[]).unwrap();
    let trust = CheckedTrust::new(&NoWorkspaceTrust, &boundaries);
    let mut session = session::health_at(
        &scratch.config(),
        None,
        None,
        &["graph.engine=none".into()],
        &trust,
    )
    .unwrap();
    for engine_built in [false, true] {
        session.graph_activation_error = Some(GraphActivationError::EngineMissing);
        let check = check_with(&environment, Ok(&session), engine_built, &NoReceipt);
        let Outcome::NotChecked(detail) = check.outcome else {
            panic!("{check:?}");
        };
        assert_eq!(detail, "the graph is off (graph.engine = none)");
        session.graph_activation_error = Some(GraphActivationError::Refused(
            "authoring.lock.json changed; run maestro init".into(),
        ));
        let check = check_with(&environment, Ok(&session), engine_built, &NoReceipt);
        let Outcome::Failed { problem, next } = check.outcome else {
            panic!("{check:?}");
        };
        assert!(problem.contains("authoring.lock.json"));
        assert!(next.contains("maestro init"));
    }
    assert!(!scratch.data().join("maestro/graph").exists());
}
