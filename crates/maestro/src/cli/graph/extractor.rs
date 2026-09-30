//! Selects the explicit table-rule or registered model extractor and its sources.

use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{
    artifact::Digest,
    document::Revision,
    gateway::{Role, RouterClient},
};
use maestro_knowledge::{
    graph::{
        extract::{ModelExtractor, WindowPolicy},
        rules::{Extractor, TableRule},
    },
    quality,
};
use std::{env, fs, path::Path};

/// Inputs for choosing one build extractor.
#[derive(Clone, Copy)]
pub(super) struct Inputs<'a> {
    /// Optional table-rule path.
    pub(super) rule_path: Option<&'a Path>,
    /// Optional registered extractor card digest.
    pub(super) extractor_card: Option<&'a str>,
    /// Optional window-policy path.
    pub(super) window_policy_path: Option<&'a Path>,
    /// Optional whole-plan estimate limit.
    pub(super) token_budget: Option<usize>,
}

/// Selects exactly one extractor mode and the eligible source revisions.
pub(super) fn select(
    kernel: &Kernel,
    collection: &str,
    inputs: Inputs<'_>,
) -> Result<(Box<dyn Extractor>, Vec<Revision>), Failure> {
    let Inputs {
        rule_path,
        extractor_card,
        window_policy_path,
        token_budget,
    } = inputs;
    match (rule_path, extractor_card, window_policy_path, token_budget) {
        (Some(path), None, None, None) => {
            let text = fs::read_to_string(path)
                .map_err(|_| Failure::refused("the graph rule cannot be read"))?;
            let rule = TableRule::parse(&text).map_err(|error| Failure::refused_by(&error))?;
            let revisions = sources(kernel, collection, &rule)?;
            Ok((Box::new(rule), revisions))
        }
        (None, Some(card_digest), Some(policy_path), Some(token_budget)) => {
            let card_digest = Digest::parse(card_digest)
                .map_err(|_| Failure::refused("extractor card digest is invalid"))?;
            let card = kernel
                .database
                .model_card(&kernel.scopes, collection, &card_digest)
                .map_err(|error| Failure::failed_by(&error))?
                .ok_or_else(|| {
                    Failure::refused("extractor card is not registered for this collection")
                })?;
            if card.fields().role != Role::Extractor {
                return Err(Failure::refused("model card role is not Extractor"));
            }
            let metadata = fs::metadata(policy_path)
                .map_err(|_| Failure::refused("window policy cannot be read"))?;
            if metadata.len() > 65_536 {
                return Err(Failure::refused("window policy exceeds 65536 bytes"));
            }
            let policy_text = fs::read_to_string(policy_path)
                .map_err(|_| Failure::refused("window policy is not UTF-8"))?;
            let policy =
                WindowPolicy::parse(&policy_text).map_err(|error| Failure::refused_by(&error))?;
            let policy_digest = Digest::of(policy_text.as_bytes());
            let router_url = super::super::health::router_url(
                env::var_os(super::super::health::ROUTER_VARIABLE).as_deref(),
            )
            .map_err(|_| Failure::refused("model router URL is invalid"))?;
            let port = RouterClient::new(router_url)
                .map_err(|_| Failure::failed("model router client could not be created"))?;
            let extractor = ModelExtractor::new(port, card, policy, policy_digest, token_budget)
                .map_err(|_| Failure::failed("model extraction runtime could not be created"))?;
            let revisions = all_sources(kernel, collection)?;
            Ok((Box::new(extractor), revisions))
        }
        _ => Err(Failure::refused(
            "choose --rule, or provide --extractor-card, --window-policy and \
             --token-budget together",
        )),
    }
}

/// Every eligible revision in stable document order.
fn all_sources(kernel: &Kernel, collection: &str) -> Result<Vec<Revision>, Failure> {
    let mut revisions = quality::eligible(&kernel.database, &kernel.scopes, collection)
        .map_err(|error| Failure::failed_by(&error))?;
    if revisions.is_empty() {
        return Err(Failure::refused(format!(
            "the collection {collection} has no eligible revisions"
        )));
    }
    revisions.sort_by(|left, right| left.document_id.cmp(&right.document_id));
    Ok(revisions)
}

/// Eligible revisions whose original digest is bound to the rule, in document order.
fn sources(kernel: &Kernel, collection: &str, rule: &TableRule) -> Result<Vec<Revision>, Failure> {
    let mut eligible: Vec<Revision> =
        quality::eligible(&kernel.database, &kernel.scopes, collection)
            .map_err(|error| Failure::failed_by(&error))?
            .into_iter()
            .filter(|revision| revision.original_digest == *rule.source_sha256())
            .collect();
    if eligible.is_empty() {
        return Err(Failure::refused(format!(
            "no eligible revision of the collection {collection} has the rule's source digest \
             sha256:{}",
            rule.source_sha256().as_str()
        )));
    }
    eligible.sort_by(|left, right| left.document_id.cmp(&right.document_id));
    Ok(eligible)
}
