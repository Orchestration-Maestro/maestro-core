//! Frozen acceptance inventory and local drafting settings, admitted before private source reads.
use super::{
    draft_io,
    graph_output::Code,
    private_run::{CheckedRun, PrivateRun, endpoint},
};
use crate::kernel::Kernel;
use maestro_kernel::{
    artifact::Digest,
    document::Outcome,
    gateway::{ModelCard, Role},
};
use maestro_knowledge::eval::draft::{DraftBudget, DraftWindow};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

/// Manifest metadata cannot require more than one MiB before admission.
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
/// Inventory decoding is bounded independently of its declared window count.
const MAX_INVENTORY_BYTES: usize = 16 * 1024 * 1024;

/// The complete private drafting contract, not a ladder or a reviewed suite.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DraftManifest {
    /// Versioned manifest schema.
    schema: String,
    /// Collection covered by the approval.
    pub(super) collection: String,
    /// Digest-pinned inventory under PRIVATE.
    inventory: PathBuf,
    /// Exact inventory bytes approved before source reads.
    inventory_digest: Digest,
    /// Digest-pinned private prompt file.
    prompt: PathBuf,
    /// Exact prompt identity.
    pub(super) prompt_digest: Digest,
    /// Pinned immutable answerer card in scratch artifacts.
    pub(super) card: Digest,
    /// Drafter family retained for later independent-review separation.
    pub(super) family: String,
    /// Explicit local-only router, never an environment/default fallback.
    pub(super) router: String,
    /// Per-window inference budget.
    pub(super) budget: DraftBudget,
    /// Maximum cumulative reserved input and output tokens.
    pub(super) max_tokens: u64,
    /// Maximum inventory windows, including failed attempts.
    pub(super) max_windows: usize,
    /// Maximum original source bytes admitted for one window.
    pub(super) max_source_bytes: usize,
    /// Shared approval, scratch and private destination admission.
    private_run: PrivateRun,
}

/// Approved whole-generation inventory; only its bounded windows may be read.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Inventory {
    /// Versioned receipt schema.
    schema: String,
    /// Acceptance permission, distinct from the pilot.
    scope: String,
    /// Approved collection.
    collection: String,
    /// Frozen published generation.
    pub(super) generation: i64,
    /// Frozen development-selected policy; never tuned from drafts.
    window_policy: Digest,
    /// Explicit document/version/source windows.
    pub(super) windows: Vec<Window>,
}

/// One host-selected window of one exact published source revision.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Window {
    /// Stable question/window ID, independent of model text.
    id: String,
    /// Original source reference.
    source_ref: String,
    /// Document identity from the frozen generation.
    document_id: String,
    /// Exact revision identity.
    revision_id: String,
    /// Source version, not inferred from record time.
    version: String,
    /// Digest of original bytes.
    original: Digest,
    /// Approved half-open original-byte window.
    span: [usize; 2],
}

/// Admitted private inputs; Debug deliberately absent to avoid source logging.
pub(super) struct DraftInputs {
    /// Strict settings.
    pub(super) manifest: DraftManifest,
    /// Digest of the entire manifest, binding approvals and every setting.
    pub(super) digest: Digest,
    /// Frozen inventory.
    pub(super) inventory: Inventory,
    /// Private prompt text.
    pub(super) prompt: String,
    /// Shared checked bindings.
    pub(super) run: CheckedRun,
}

impl DraftManifest {
    /// Refuses invalid bindings, remote inference, and changed pins before opening authority.
    pub(super) fn read(path: &Path) -> Result<DraftInputs, Code> {
        let text = draft_io::read(path, MAX_MANIFEST_BYTES).map_err(|_| Code::Manifest)?;
        let manifest: Self = serde_json::from_slice(&text).map_err(|_| Code::Manifest)?;
        if manifest.schema != "maestro-graph-draft/1"
            || manifest.family.trim().is_empty()
            || manifest.max_windows == 0
            || manifest.max_tokens == 0
            || manifest.max_source_bytes == 0
        {
            return Err(Code::Manifest);
        }
        endpoint(&manifest.router)?;
        let run = manifest.private_run.check(&manifest.collection)?;
        run.input(path)?;
        let base = path.parent().ok_or(Code::Manifest)?;
        let inventory: Inventory = serde_json::from_str(&frozen(
            &run,
            &base.join(&manifest.inventory),
            &manifest.inventory_digest,
            MAX_INVENTORY_BYTES,
        )?)
        .map_err(|_| Code::Manifest)?;
        inventory.check(&manifest)?;
        let prompt = frozen(
            &run,
            &base.join(&manifest.prompt),
            &manifest.prompt_digest,
            manifest.budget.input_bytes,
        )?;
        Ok(DraftInputs {
            manifest,
            digest: Digest::of(&text),
            inventory,
            prompt,
            run,
        })
    }

    /// Load precisely the pinned model, never select the latest or a default.
    pub(super) fn model(&self, kernel: &Kernel) -> Result<ModelCard, Code> {
        let card = ModelCard::load(&kernel.artifacts, &self.card).map_err(|_| Code::Manifest)?;
        if card.fields().role != Role::Answerer {
            return Err(Code::Manifest);
        }
        Ok(card)
    }
}

impl Inventory {
    /// Reject pilot-only scope, repeated IDs, and unbounded windows before any source read.
    fn check(&self, manifest: &DraftManifest) -> Result<(), Code> {
        let mut ids = BTreeSet::new();
        if self.schema != "maestro-graph-draft-inputs/1"
            || self.scope != "acceptance"
            || self.collection != manifest.collection
            || self.generation <= 0
            || self.windows.is_empty()
            || self.windows.len() > manifest.max_windows
        {
            return Err(Code::Manifest);
        }
        for window in &self.windows {
            if !ids.insert(&window.id)
                || window.id.is_empty()
                || window.id.len() > 128
                || !window
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-.:".contains(&byte))
                || window.span[0] >= window.span[1]
                || window.span[1] - window.span[0] > manifest.budget.input_bytes
                || window.version.is_empty()
            {
                return Err(Code::Manifest);
            }
        }
        // The typed digest is part of the inventory identity even when no model runs.
        let _ = &self.window_policy;
        Ok(())
    }

    /// Verify publication and each window's membership without reading source bytes.
    pub(super) fn membership(&self, kernel: &Kernel) -> Result<(), Code> {
        let generation = kernel
            .database
            .published_generation(&kernel.scopes, &self.collection)
            .map_err(|_| Code::Labels)?
            .ok_or(Code::Labels)?;
        if generation.id != self.generation {
            return Err(Code::Labels);
        }
        let chunks = kernel
            .database
            .chunks(&kernel.scopes, &generation.chunk_set_id)
            .map_err(|_| Code::Labels)?;
        let revisions: BTreeSet<_> = chunks
            .iter()
            .map(|chunk| chunk.revision_id.as_str())
            .collect();
        if self
            .windows
            .iter()
            .any(|window| !revisions.contains(window.revision_id.as_str()))
        {
            return Err(Code::Labels);
        }
        Ok(())
    }
}

impl Window {
    /// Recheck current scope, eligibility, document version and source digest before every call.
    pub(super) fn load(
        &self,
        kernel: &Kernel,
        settings: &DraftManifest,
    ) -> Result<DraftWindow, Code> {
        let revision = kernel
            .database
            .eligible_revisions(&kernel.scopes, &settings.collection)
            .map_err(|_| Code::Labels)?
            .into_iter()
            .find(|revision| revision.id == self.revision_id)
            .ok_or(Code::Labels)?;
        if revision.document_id != self.document_id
            || revision.original_digest != self.original
            || revision
                .metadata
                .get("version")
                .and_then(serde_json::Value::as_str)
                != Some(self.version.as_str())
        {
            return Err(Code::Labels);
        }
        let document = kernel
            .database
            .document(&kernel.scopes, &self.document_id)
            .map_err(|_| Code::Labels)?
            .ok_or(Code::Labels)?;
        if document.source_ref != self.source_ref {
            return Err(Code::Labels);
        }
        let disposition = kernel
            .database
            .disposition(&kernel.scopes, &self.revision_id)
            .map_err(|_| Code::Labels)?;
        if !disposition.is_some_and(|value| {
            matches!(
                value.outcome,
                Outcome::Accepted | Outcome::AcceptedWithWarnings
            )
        }) {
            return Err(Code::Labels);
        }
        let artifact = kernel
            .database
            .artifact(&self.original)
            .map_err(|_| Code::Labels)?
            .ok_or(Code::Labels)?;
        if artifact.bytes > settings.max_source_bytes as u64 {
            return Err(Code::Labels);
        }
        let bytes = kernel
            .database
            .get(&self.original)
            .map_err(|_| Code::Labels)?;
        if bytes.len() > settings.max_source_bytes {
            return Err(Code::Labels);
        }
        let source = String::from_utf8(bytes).map_err(|_| Code::Labels)?;
        Ok(DraftWindow {
            id: self.id.clone(),
            source_ref: self.source_ref.clone(),
            revision_id: self.revision_id.clone(),
            original: self.original.clone(),
            span: self.span,
            source,
        })
    }
}

/// Private digest-pinned bytes; no raw parse or filesystem errors leave this boundary.
fn frozen(run: &CheckedRun, path: &Path, expected: &Digest, limit: usize) -> Result<String, Code> {
    let bytes = draft_io::read(&run.input(path)?, limit).map_err(|_| Code::Manifest)?;
    let text = String::from_utf8(bytes).map_err(|_| Code::Manifest)?;
    if Digest::of(text.as_bytes()) != *expected {
        return Err(Code::Manifest);
    }
    Ok(text)
}
