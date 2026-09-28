//! Bounded, resumable local drafting; only aggregate counters cross the CLI boundary.
use super::{
    super::{health, output::Output},
    draft_io,
    draft_journal::FileJournal,
    draft_manifest::{DraftInputs, DraftManifest},
    graph_output::{self, Code, Counts},
    private_run::CheckedRun,
};
use crate::failure::Failure;
use maestro_kernel::{
    artifact::Digest,
    gateway::{ModelPort, RouterClient},
};
use maestro_knowledge::eval::{
    draft::{DraftError, DraftRequest},
    draft_progress::{DraftJournal, DraftOutcome, DraftRun, resume_draft},
};
use std::{path::Path, process::ExitCode};
use tokio::runtime::Builder;

/// Admit explicit scratch/local bindings before any authority or inference is opened.
pub(in crate::cli) fn run(output: Output, path: &Path) -> Result<ExitCode, Failure> {
    let inputs = DraftManifest::read(path).map_err(Code::failure)?;
    let result = (|| {
        let url = health::router_url(Some(inputs.manifest.router.as_ref()))
            .map_err(|_| Code::Manifest)?;
        let port = RouterClient::new(url).map_err(|_| Code::Manifest)?;
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Code::Output)?;
        runtime.block_on(execute(&inputs, &port))
    })();
    match result {
        Ok(counts) => {
            graph_output::emit(output, &inputs.digest, &counts).map_err(Code::failure)?;
            Ok(if counts.failed == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Err(code) => {
            graph_output::diagnostic(&inputs.run.output, &code).map_err(Code::failure)?;
            Err(code.failure())
        }
    }
}

/// Shared real/fake inference path; all source reads and writes stay behind checked bindings.
pub(super) async fn execute<P: ModelPort + Sync>(
    inputs: &DraftInputs,
    port: &P,
) -> Result<Counts, Code> {
    let mut kernel = inputs.run.open().map_err(|_| Code::Labels)?;
    let settings = &inputs.manifest;
    let card = settings.model(&kernel)?;
    inputs.inventory.membership(&kernel)?;
    let mut journal = FileJournal::open(&inputs.run, settings.max_windows)?;
    let run = DraftRun {
        digest: inputs.digest.clone(),
        max_windows: settings.max_windows,
        max_tokens: settings.max_tokens,
    };
    let mut counts = Counts::default();
    for item in &inputs.inventory.windows {
        kernel.refresh_scopes().map_err(|_| Code::Labels)?;
        inputs.inventory.membership(&kernel)?;
        let window = item.load(&kernel, settings)?;
        let request = DraftRequest {
            card: &card,
            card_digest: &settings.card,
            prompt: &inputs.prompt,
            prompt_digest: &settings.prompt_digest,
            window: &window,
            budget: settings.budget,
        };
        match resume_draft(port, &request, &run, &mut journal).await {
            Ok(_) => {
                counts.items += 1;
                counts.unreviewed += 1;
            }
            Err(DraftError::Journal) => return Err(Code::Output),
            Err(_) => counts.failed += 1,
        }
    }
    export(inputs, &journal)?;
    Ok(counts)
}

/// Write interoperable suite/label JSONL and immutable provenance after all outcomes persist.
fn export(inputs: &DraftInputs, journal: &FileJournal<'_>) -> Result<(), Code> {
    let mut suite = String::new();
    let mut labels = String::new();
    for receipt in journal.receipts() {
        if let DraftOutcome::Draft(candidate) = &receipt.outcome {
            suite.push_str(candidate.suite.trim());
            suite.push('\n');
            labels.push_str(candidate.labels.trim());
            labels.push('\n');
        }
    }
    let provenance = serde_json::json!({
        "schema":"maestro-graph-draft-provenance/1", "reviewed":false,
        "manifest_digest":inputs.digest, "card_digest":inputs.manifest.card,
        "prompt_digest":inputs.manifest.prompt_digest, "drafter_family":inputs.manifest.family,
        "generation":inputs.inventory.generation,
        "suite_digest":Digest::of(suite.as_bytes()), "labels_digest":Digest::of(labels.as_bytes()),
    });
    for (name, bytes) in [
        ("draft-suite.jsonl", suite.into_bytes()),
        ("draft-labels.jsonl", labels.into_bytes()),
        ("draft-provenance.json", provenance.to_string().into_bytes()),
    ] {
        write_once(&inputs.run, name, &bytes)?;
    }
    Ok(())
}

/// Replay may reuse identical exported artifacts, but can never overwrite changed private data.
fn write_once(run: &CheckedRun, name: &str, bytes: &[u8]) -> Result<(), Code> {
    let path = run.output.join(name);
    if path.try_exists().map_err(|_| Code::Output)? {
        let old = draft_io::read(&run.input(&path)?, bytes.len()).map_err(|_| Code::Output)?;
        if old != bytes {
            return Err(Code::Output);
        }
    } else {
        run.write(&path, bytes)?;
    }
    Ok(())
}
