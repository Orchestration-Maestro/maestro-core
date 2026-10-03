//! Mutation-debt contracts for conjunctive policy checks and inclusive ceilings.
use super::{
    acquisition::{
        AcquisitionProfile, Attribute, DocumentState, DomStep, Readiness, ReadyCondition, Transport,
    },
    checks,
    schema::SourcePolicy,
};
use crate::Refusal;

/// Existing public synthetic policy, decoded without a second fixture definition.
fn policy() -> SourcePolicy {
    serde_json::from_str(include_str!("../../tests/fixtures/policy.json")).unwrap()
}

#[test]
fn policy_independent_required_source_lists() {
    let baseline = policy();
    assert_eq!(checks::policy(&baseline), Ok(()));
    for dimension in 0..5 {
        let mut changed = baseline.clone();
        let source = changed.sources.first_mut().unwrap();
        match dimension {
            0 => source.origins.clear(),
            1 => source.seeds.clear(),
            2 => source.selectors.clear(),
            3 => source.decisions.clear(),
            _ => source.selected_profiles.clear(),
        }
        assert_eq!(
            checks::policy(&changed),
            Err(Refusal::Invalid),
            "dimension {dimension}"
        );
    }
}

#[test]
fn policy_counts_accept_ceiling_and_refuse_overflow() {
    for count in [1000, 1001] {
        let mut value = policy();
        let source = value.sources.first().unwrap().clone();
        value.sources = (0..count)
            .map(|index| {
                let mut source = source.clone();
                source.id = format!("source-{index}");
                for selector in &mut source.selectors {
                    selector.source_id = source.id.clone();
                }
                source
            })
            .collect();
        assert_eq!(
            checks::policy(&value).is_ok(),
            count == 1000,
            "sources {count}"
        );
        let mut value = policy();
        let reference = value.acquisition_profiles.first().unwrap().clone();
        value.acquisition_profiles = (0..count)
            .map(|index| {
                let mut reference = reference.clone();
                reference.id = format!("profile-{index}");
                reference
            })
            .collect();
        assert_eq!(
            checks::policy(&value).is_ok(),
            count == 1000,
            "profiles {count}"
        );
    }
    assert_eq!(
        checks::unique(["same", "same"].into_iter()),
        Err(Refusal::Invalid)
    );
}

#[test]
fn policy_selector_each_list_is_a_sufficient_dimension() {
    let value = policy();
    let source = value.sources.first().unwrap();
    let mut empty = source.selectors.first().unwrap().clone();
    empty.origin = None;
    empty.path_prefix = None;
    assert_eq!(checks::selector(&empty, source), Err(Refusal::Invalid));
    for dimension in 0..4 {
        let mut selector = empty.clone();
        match dimension {
            0 => selector.object_ids.push("object".into()),
            1 => selector.versions.push("version".into()),
            2 => selector.channels.push("channel".into()),
            _ => selector.media_types.push("text/plain".into()),
        }
        assert_eq!(
            checks::selector(&selector, source),
            Ok(()),
            "dimension {dimension}"
        );
    }
}

#[test]
fn policy_overlap_absent_origin_and_prefix_directions() {
    let value = policy();
    let mut left = value
        .sources
        .first()
        .unwrap()
        .selectors
        .first()
        .unwrap()
        .clone();
    let mut right = left.clone();
    left.path_prefix = Some("/docs/chapter".into());
    assert!(checks::overlap(&left, &right));
    assert!(checks::overlap(&right, &left));
    right.path_prefix = Some("/other".into());
    assert!(!checks::overlap(&left, &right));
    right.path_prefix = left.path_prefix.clone();
    left.origin = None;
    assert!(checks::overlap(&left, &right));
    assert!(checks::overlap(&right, &left));
}

/// One rendered profile with independently adjustable semantic bounds.
fn rendered() -> AcquisitionProfile {
    let mut profile: AcquisitionProfile =
        serde_json::from_str(include_str!("../../tests/fixtures/http.json")).unwrap();
    profile.transport = Transport::BrowserRender;
    profile.readiness = Some(Readiness {
        document_state: DocumentState::Load,
        all_of: vec![ReadyCondition::ElementPresent {
            selector: vec![DomStep {
                tag: "article".into(),
                attributes: vec![],
            }],
        }],
        timeout_ms: 100.try_into().unwrap(),
        poll_interval_ms: 100.try_into().unwrap(),
        stable_for_ms: 100.try_into().unwrap(),
    });
    profile
}

#[test]
fn policy_readiness_inclusive_bounds() {
    let baseline = rendered();
    assert_eq!(checks::profile(&baseline, 100), Ok(()));
    assert_eq!(checks::profile(&baseline, 99), Err(Refusal::Invalid));
    for count in [32, 33, 34] {
        let mut profile = baseline.clone();
        let ready = profile.readiness.as_mut().unwrap();
        let condition = ready.all_of.first().unwrap().clone();
        ready.all_of = vec![condition; count];
        assert_eq!(
            checks::profile(&profile, 100).is_ok(),
            count == 32,
            "conditions {count}"
        );
    }
    for dimension in 0..2 {
        let mut profile = baseline.clone();
        let ready = profile.readiness.as_mut().unwrap();
        if dimension == 0 {
            ready.poll_interval_ms = 101.try_into().unwrap();
        } else {
            ready.stable_for_ms = 101.try_into().unwrap();
        }
        assert_eq!(checks::profile(&profile, 100), Err(Refusal::Invalid));
    }
}

#[test]
fn policy_dom_path_and_attribute_inclusive_bounds() {
    for count in [32, 33, 34] {
        let mut profile = rendered();
        profile.readiness.as_mut().unwrap().all_of = vec![ReadyCondition::ElementAbsent {
            selector: vec![
                DomStep {
                    tag: "article".into(),
                    attributes: vec![]
                };
                count
            ],
        }];
        assert_eq!(
            checks::profile(&profile, 100).is_ok(),
            count == 32,
            "path {count}"
        );
    }
    for count in [8, 9] {
        let mut profile = rendered();
        profile.readiness.as_mut().unwrap().all_of = vec![ReadyCondition::ElementAbsent {
            selector: vec![DomStep {
                tag: "article".into(),
                attributes: (0..count)
                    .map(|index| Attribute {
                        name: format!("attr-{index}"),
                        value: "value".into(),
                    })
                    .collect(),
            }],
        }];
        assert_eq!(
            checks::profile(&profile, 100).is_ok(),
            count == 8,
            "attributes {count}"
        );
    }
}
