//! Optional G35 orchestration uses real frozen documents and replaceable projection doubles.
#![cfg(all(feature = "engine", not(windows)))]
use super::rebuild_support::authority;
use crate::cli::{
    graph::{
        rebuild_descriptors::{Descriptors, project},
        rebuild_work::{Selection, Work},
    },
    output::Output,
};
use crate::kernel::Kernel;
use maestro_filesystem::{ControlFile, OwnedRoot, SystemFileLock};
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{CardFields, FakeModels, Limits, ModelCard, Role, RouterEntry},
    scope::LOCAL,
};
use maestro_knowledge::{
    graph::descriptors::{
        self, DescriptorEmbedder, DescriptorError, DescriptorInput, DescriptorPin,
        DescriptorProjection, DescriptorQuery, DescriptorReceipt, EmbeddedDescriptors,
    },
    graph::projection::{EngineSettings, ProjectionEngine, ProjectionFactory},
    index::PointHit,
};
use std::{
    cell::RefCell,
    future::{Future, ready},
    num::{NonZeroU32, NonZeroUsize},
    time::Duration,
};

use tokio::runtime::Builder;

#[derive(Default)]
struct Projection {
    calls: RefCell<Vec<&'static str>>,
    fail_verify: bool,
    fail_rebuild: bool,
}
impl DescriptorProjection for Projection {
    fn rebuild(
        &self,
        _: &EmbeddedDescriptors,
    ) -> impl Future<Output = Result<(), DescriptorError>> {
        self.calls.borrow_mut().push("rebuild");
        ready(if self.fail_rebuild {
            Err(DescriptorError::Refused("rebuild unavailable".into()))
        } else {
            Ok(())
        })
    }
    fn verify(&self, _: &EmbeddedDescriptors) -> impl Future<Output = Result<(), DescriptorError>> {
        self.calls.borrow_mut().push("verify");
        ready(if self.fail_verify {
            Err(DescriptorError::Refused("unavailable".into()))
        } else {
            Ok(())
        })
    }
    fn delete(&self, _: &DescriptorReceipt) -> impl Future<Output = Result<(), DescriptorError>> {
        self.calls.borrow_mut().push("delete");
        ready(Err(DescriptorError::Refused("unexpected delete".into())))
    }
    fn lookup(
        &self,
        _: &DescriptorReceipt,
        _: DescriptorQuery,
    ) -> impl Future<Output = Result<Vec<PointHit>, DescriptorError>> {
        self.calls.borrow_mut().push("lookup");
        ready(Err(DescriptorError::Refused("unexpected lookup".into())))
    }
}

#[test]
fn graph_rebuild_optional_descriptors_share_snapshot_profile_and_verify_readiness() {
    let (fixture, selection) = authority();
    let kernel = &fixture.kernel;
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let output = embedded(kernel, &selection);
    let profile = output.receipt().profile.clone();
    let port = Projection::default();
    let apply = |output: &EmbeddedDescriptors| runtime.block_on(project(&port, output));
    let descriptors = Descriptors {
        output: &output,
        profile: &profile,
        project: &apply,
    };
    let root = OwnedRoot::open(&fixture.root.join("graph"), true).unwrap();
    for control in [ControlFile::Access, ControlFile::Writer] {
        root.ensure_control(control).unwrap();
    }
    let settings =
        EngineSettings::new(16 * 1024 * 1024, 64 * 1024 * 1024, 1, Digest::of(b"lock")).unwrap();
    let factory = ProjectionFactory::new(
        &fixture.root.join("graph"),
        ProjectionEngine::Ladybug,
        settings.clone(),
        &SystemFileLock,
    );
    let snapshot = selection.read(kernel).unwrap();
    Work {
        selection: &selection,
        snapshot: &snapshot,
        factory: &factory,
        settings: &settings,
        descriptors: Some(descriptors),
    }
    .run(kernel, Output::new(true), None)
    .unwrap();
    assert_eq!(*port.calls.borrow(), ["rebuild", "verify"]);
    assert!(
        kernel
            .database
            .projection_ready(&kernel.scopes, selection.scope.generation_id)
            .unwrap()
            .is_some()
    );
    let mut changed = profile.clone();
    changed.linking = Digest::of(b"changed");
    assert!(
        Descriptors {
            output: &output,
            profile: &changed,
            project: &apply
        }
        .run(
            kernel,
            (
                &selection.scope,
                &selection.claim_set,
                &selection.resolution
            )
        )
        .is_err()
    );
    assert_eq!(
        port.calls.borrow().len(),
        2,
        "changed profile cannot replay descriptors"
    );
    let failing = Projection {
        fail_verify: true,
        ..Projection::default()
    };
    assert!(runtime.block_on(project(&failing, &output)).is_err());
    assert_eq!(*failing.calls.borrow(), ["rebuild", "verify"]);
}

#[test]
fn graph_rebuild_descriptor_rebuild_failure_never_verifies() {
    let (fixture, selection) = authority();
    let output = embedded(&fixture.kernel, &selection);
    let port = Projection {
        fail_rebuild: true,
        ..Projection::default()
    };
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    assert!(runtime.block_on(project(&port, &output)).is_err());
    assert_eq!(*port.calls.borrow(), ["rebuild"]);
}

#[test]
fn graph_rebuild_valid_resolution_with_different_canonical_documents_refuses() {
    use maestro_kernel::facts::{EXACT_RESOLVER_VERSION, ResolutionInput};
    let (fixture, selection) = authority();
    let kernel = &fixture.kernel;
    let output = embedded(kernel, &selection);
    let changed = kernel
        .database
        .record_resolution(
            &kernel.scopes,
            LOCAL,
            &ResolutionInput {
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                sets: vec![selection.claim_set.clone()],
                previous: Some(selection.resolution.clone()),
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap()
        .id;
    let pin = &output.receipt().pin;
    let input =
        DescriptorInput::read(&kernel.database, &kernel.scopes, LOCAL, (pin, &changed)).unwrap();
    let documents = descriptors::build(&input).unwrap();
    assert_ne!(
        documents,
        output.descriptors(),
        "both valid snapshots yield different canonical documents"
    );
    let called = RefCell::new(false);
    let apply = |_: &EmbeddedDescriptors| {
        *called.borrow_mut() = true;
        Ok(())
    };
    let error = Descriptors {
        output: &output,
        profile: &output.receipt().profile,
        project: &apply,
    }
    .run(kernel, (&selection.scope, &selection.claim_set, &changed))
    .unwrap_err();
    assert!(error.to_string().contains("descriptor content differs"));
    assert!(!*called.borrow());
}

fn embedded(kernel: &Kernel, selection: &Selection) -> EmbeddedDescriptors {
    let pin = DescriptorPin {
        collection_id: selection.scope.collection_id.clone(),
        generation_id: selection.scope.generation_id,
        version: None,
    };
    let input = DescriptorInput::read(
        &kernel.database,
        &kernel.scopes,
        LOCAL,
        (&pin, &selection.resolution),
    )
    .unwrap();
    let documents = descriptors::build(&input).unwrap();
    let card = card(&kernel.artifacts);
    let embedder = DescriptorEmbedder {
        models: &FakeModels,
        card: &card,
        linking: Digest::of(b"linking"),
        deadline: Duration::from_secs(1),
    };
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    runtime
        .block_on(embedder.prepare(&pin, &documents, None))
        .unwrap()
}

#[test]
fn graph_rebuild_optional_descriptors_refuse_each_changed_scope_membership_and_resolution() {
    let (fixture, selection) = authority();
    let kernel = &fixture.kernel;
    let output = embedded(kernel, &selection);
    let calls = RefCell::new(0);
    let apply = |_: &EmbeddedDescriptors| {
        *calls.borrow_mut() += 1;
        Ok(())
    };
    for change in 0..4 {
        let mut scope = selection.scope.clone();
        let mut claim_set = selection.claim_set.clone();
        let mut resolution = selection.resolution.clone();
        match change {
            0 => scope.collection_id = "other".into(),
            1 => scope.generation_id += 1,
            2 => claim_set = Digest::of(b"other"),
            _ => resolution = Digest::of(b"unknown"),
        }
        let changed = Selection {
            scope,
            claim_set,
            resolution,
        };
        assert!(
            Descriptors {
                output: &output,
                profile: &output.receipt().profile,
                project: &apply
            }
            .run(
                kernel,
                (&changed.scope, &changed.claim_set, &changed.resolution)
            )
            .is_err()
        );
    }
    assert_eq!(*calls.borrow(), 0);
}

fn card(store: &Store) -> ModelCard {
    ModelCard::record(
        store,
        &CardFields {
            role: Role::Embedder,
            router_entry: RouterEntry::parse("embed").unwrap(),
            file_digest: Digest::of(b"synthetic weights"),
            template_digest: None,
            server_build: "synthetic/1".into(),
            dimensions: NonZeroUsize::new(4),
            limits: Limits {
                context_tokens: NonZeroU32::new(512).unwrap(),
                output_tokens: None,
            },
            suite_results: vec![],
        },
    )
    .unwrap()
}
