//! Private receipt files with an OS-held lock; crashes retain consumed reservations.
use super::{draft_io, graph_output::Code, private_run::CheckedRun};
use maestro_knowledge::eval::{
    draft::DraftError,
    draft_progress::{DraftJournal, DraftReceipt},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
};

/// Bound the complete private receipt before JSON decoding or replay.
const MAX_RECEIPT_BYTES: usize = 16 * 1024 * 1024;

/// Explicit on-disk protocol; unknown versions are refused by deserialization.
#[derive(Serialize, Deserialize)]
enum ReceiptSchema {
    /// Initial bounded draft receipt format.
    #[serde(rename = "maestro-graph-draft-receipt/1")]
    V1,
}

/// Disk envelope, separate from the replaceable journal's in-memory receipt.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptEnvelope {
    /// Required protocol discriminator.
    schema: ReceiptSchema,
    /// Reserved or terminal draft attempt.
    receipt: DraftReceipt,
}

/// One exclusive run's checked private receipt adapter.
pub(super) struct FileJournal<'a> {
    /// Reusable approved output-path boundary.
    run: &'a CheckedRun,
    /// Lock lives until this adapter is dropped, including inference awaits.
    _lock: File,
    /// Loaded and successfully persisted records only.
    receipts: Vec<DraftReceipt>,
}

impl<'a> FileJournal<'a> {
    /// Open only checked private paths and refuse concurrent writers or partial records.
    pub(super) fn open(run: &'a CheckedRun, max_windows: usize) -> Result<Self, Code> {
        let path = run.output.join("draft.lock");
        if !path.try_exists().map_err(|_| Code::Output)? {
            run.write(&path, b"")?;
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(run.input(&path)?)
            .map_err(|_| Code::Output)?;
        lock.try_lock().map_err(|_| Code::Output)?;
        let mut paths = fs::read_dir(&run.output)
            .map_err(|_| Code::Output)?
            .map(|entry| entry.map(|entry| entry.path()).map_err(|_| Code::Output))
            .collect::<Result<Vec<_>, _>>()?;
        paths.retain(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("draft-receipt-"))
        });
        paths.sort();
        if paths.len() > max_windows.checked_mul(2).ok_or(Code::Manifest)? {
            return Err(Code::Manifest);
        }
        let mut receipts = Vec::new();
        for (index, path) in paths.into_iter().enumerate() {
            if path != receipt_path(run, index) {
                return Err(Code::Output);
            }
            let bytes =
                draft_io::read(&run.input(&path)?, MAX_RECEIPT_BYTES).map_err(|_| Code::Output)?;
            let envelope: ReceiptEnvelope =
                serde_json::from_slice(&bytes).map_err(|_| Code::Output)?;
            receipts.push(envelope.receipt);
        }
        Ok(Self {
            run,
            _lock: lock,
            receipts,
        })
    }
}

impl DraftJournal for FileJournal<'_> {
    fn receipts(&self) -> &[DraftReceipt] {
        &self.receipts
    }

    fn append(&mut self, receipt: DraftReceipt) -> Result<(), DraftError> {
        let envelope = ReceiptEnvelope {
            schema: ReceiptSchema::V1,
            receipt,
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| DraftError::Journal)?;
        if bytes.len() > MAX_RECEIPT_BYTES {
            return Err(DraftError::Journal);
        }
        self.run
            .write(&receipt_path(self.run, self.receipts.len()), &bytes)
            .map_err(|_| DraftError::Journal)?;
        self.receipts.push(envelope.receipt);
        Ok(())
    }
}

/// Host-owned stable name; no model ID or private text is interpolated into paths.
fn receipt_path(run: &CheckedRun, index: usize) -> PathBuf {
    run.output.join(format!("draft-receipt-{index:020}.json"))
}
