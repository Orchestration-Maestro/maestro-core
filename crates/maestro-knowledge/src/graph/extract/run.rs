//! Candidate extraction and source-only evidence construction.

use super::windows::{Window, WindowPolicy, locate_quote, windows};
use crate::graph::{
    rules::{Extraction, Extractor, Rejection},
    verify::{Source, check_source},
};
use maestro_kernel::{
    artifact::Digest,
    evidence::Span,
    facts::{Claim, Provenance, Support, Validity},
    gateway::{
        Candidate, Error as GatewayError, ExtractRequest, MAX_EXTRACT_OUTPUT_TOKENS, ModelCard,
        ModelPort, Room, extraction_prompt_digest, extraction_system_prompt,
    },
};
use std::{collections::BTreeMap, io, time::Duration};
use tokio::{
    runtime::{Builder, Runtime},
    time as tokio_time,
};

/// Maximum wait for one bounded extractor call.
const CALL_DEADLINE: Duration = Duration::from_secs(120);

/// A replaceable model-backed extractor whose profile freezes every behavior input.
#[derive(Debug)]
pub struct ModelExtractor<P> {
    /// Replaceable model port.
    port: P,
    /// Explicit Extractor card.
    card: ModelCard,
    /// Frozen bounded windows.
    policy: WindowPolicy,
    /// Frozen provenance carried by accepted claims.
    provenance: Provenance,
    /// Card identity recorded in durable job inputs.
    card_digest: Digest,
    /// Prompt identity recorded in durable job inputs.
    prompt_digest: Digest,
    /// Window policy identity recorded in durable job inputs.
    policy_digest: Digest,
    /// Cumulative input and reserved-output token ceiling.
    token_budget: usize,
    /// Current-thread runtime kept outside any caller runtime.
    runtime: Runtime,
}

impl<P: ModelPort> ModelExtractor<P> {
    /// Creates an extractor from its explicitly chosen card and bounded window policy.
    ///
    /// # Errors
    ///
    /// Refuses when the local async runtime cannot be created.
    pub fn new(
        port: P,
        card: ModelCard,
        policy: WindowPolicy,
        policy_digest: Digest,
        token_budget: usize,
    ) -> Result<Self, io::Error> {
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let card_digest = card.digest().clone();
        let prompt_digest = extraction_prompt_digest();
        let identity = serde_json::json!({
            "card": card_digest.as_str(),
            "prompt": prompt_digest.as_str(),
            "window_policy": policy_digest.as_str(),
            "token_budget": token_budget,
        });
        let provenance = Provenance {
            extractor: format!("model/{}", card.digest().as_str()),
            profile: Digest::of(identity.to_string().as_bytes()),
        };
        Ok(Self {
            port,
            card,
            policy,
            provenance,
            card_digest,
            prompt_digest,
            policy_digest,
            token_budget,
            runtime,
        })
    }

    /// Extracts one verified source window without exposing gateway error text.
    fn call(&self, text: &str) -> Result<Vec<Candidate>, GatewayError> {
        let request = ExtractRequest::new(text)?;
        self.runtime.block_on(async {
            tokio_time::timeout(CALL_DEADLINE, self.port.extract(&self.card, &request))
                .await
                .unwrap_or(Err(GatewayError::Unsupported))
        })
    }
}

impl<P: ModelPort> Extractor for ModelExtractor<P> {
    fn provenance(&self) -> Provenance {
        self.provenance.clone()
    }

    fn token_budget(&self) -> Option<usize> {
        Some(self.token_budget)
    }

    fn job_inputs(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "card_digest": self.card_digest.as_str(),
            "prompt_digest": self.prompt_digest.as_str(),
            "window_policy_digest": self.policy_digest.as_str(),
            "profile_digest": self.provenance.profile.as_str(),
            "token_budget": self.token_budget,
        }))
    }

    fn estimated_tokens(&self, source: &Source) -> Result<usize, &'static str> {
        let source_windows = windows(source, &self.policy)?;
        let system = extraction_system_prompt();
        let mut total = 0_usize;
        for window in source_windows {
            let text = format!("{system}\n{}", window.text);
            let tokens = self
                .runtime
                .block_on(async {
                    tokio_time::timeout(
                        CALL_DEADLINE,
                        self.port.tokenize(&self.card, Room::Free, &text),
                    )
                    .await
                })
                .map_err(|_| "token estimate timed out")?
                .map_err(|_| "token estimate refused")?;
            total = total
                .saturating_add(tokens.len())
                .saturating_add(MAX_EXTRACT_OUTPUT_TOKENS as usize);
            if total > self.token_budget {
                return Ok(total);
            }
        }
        Ok(total)
    }

    fn extract(&self, source: &Source) -> Extraction {
        let mut result = Extraction::default();
        let digest = Digest::of(source.markdown().as_bytes());
        if check_source(source, &digest).is_err() {
            result
                .rejections
                .push(rejection(source, None, "source verification failed"));
            return result;
        }
        let source_windows = match windows(source, &self.policy) {
            Ok(windows) => windows,
            Err(reason) => {
                result.rejections.push(rejection(source, None, reason));
                return result;
            }
        };
        for window in source_windows {
            let Ok(candidates) = self.call(&window.text) else {
                result.rejections.push(rejection(
                    source,
                    Some(&window.block_id),
                    "extractor call refused",
                ));
                continue;
            };
            append_candidates(&mut result, source, &window, &self.provenance, candidates);
        }
        result
    }
}

/// Turns a typed model candidate into a claim only after verifying its exact quote pointer.
pub(super) fn candidate_claim(
    candidate: Candidate,
    source: &Source,
    window: &Window,
    provenance: &Provenance,
) -> Result<Claim, &'static str> {
    let span = locate_quote(window, &candidate.quote)?;
    let quote = source
        .markdown()
        .get(span.start..span.end)
        .filter(|quote| *quote == candidate.quote)
        .ok_or("quote source mismatch")?;
    Ok(Claim {
        subject: candidate.subject,
        predicate: candidate.predicate,
        object: candidate.object,
        conditions: BTreeMap::new(),
        version: Validity::Unknown,
        world: Validity::Unknown,
        provenance: provenance.clone(),
        supports: vec![Support {
            revision_id: source.revision_id().to_owned(),
            block_id: window.block_id.clone(),
            span: Span {
                start: span.start,
                end: span.end,
            },
            quote_digest: Digest::of(quote.as_bytes()),
        }],
    })
}

/// Adds verified candidates and retains fixed refusals for the others.
fn append_candidates(
    result: &mut Extraction,
    source: &Source,
    window: &Window,
    provenance: &Provenance,
    candidates: Vec<Candidate>,
) {
    for candidate in candidates {
        match candidate_claim(candidate, source, window, provenance) {
            Ok(claim) if !result.claims.contains(&claim) => result.claims.push(claim),
            Ok(_) => {}
            Err(reason) => {
                result
                    .rejections
                    .push(rejection(source, Some(&window.block_id), reason));
            }
        }
    }
}

/// Retains only a fixed reason code and source identifiers, never model text.
fn rejection(source: &Source, block_id: Option<&str>, reason: &str) -> Rejection {
    Rejection {
        revision_id: source.revision_id().to_owned(),
        block_id: block_id.map(str::to_owned),
        reason: reason.to_owned(),
    }
}
