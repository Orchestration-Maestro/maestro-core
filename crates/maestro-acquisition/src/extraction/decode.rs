//! Typed parser-IPC preflight boundary over N09's one core-owned ledger.
pub use crate::transport::accounting::Accounting;
use crate::transport::accounting::DecodeCharge;
pub use crate::transport::stream::DecodeStage;
pub use crate::transport::stream::Failure;
use serde::{Deserialize, Serialize};
use std::mem::size_of;

/// Incremental pre-allocation proposal, not cumulative counters from a parser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodeRequest {
    /// Exact decode operation; retained on success and refusal.
    pub stage: DecodeStage,
    /// Encoded input to this individual decode operation, for per-step ratio.
    pub input_bytes: u64,
    /// Additional expanded output bytes.
    pub expanded_bytes: u64,
    /// Additional nested layers, including parents across parser handoffs.
    pub levels: u64,
    /// Additional archive members.
    pub members: u64,
    /// Requested XML entities; protected policy permits only zero.
    pub entities: u64,
    /// Additional decoded pixels.
    pub pixels: u64,
    /// Additional reserved parser memory, not an unchecked observed peak.
    pub memory_bytes: u64,
}
/// Precise content-free held receipt; no partial data becomes eligible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeRefusal {
    /// Operation that first refused or crashed.
    /// `None`: no decode operation was charged before the refusal.
    pub stage: Option<DecodeStage>,
    /// Exact reason, including encoded versus expanded byte limits.
    pub reason: Failure,
}
/// One acknowledged proposal receipt. Adapters may proceed only after admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeReceipt {
    /// Exact admitted costs and operation.
    pub request: DecodeRequest,
    /// Shared expanded total after admission, including prior HTTP stages.
    pub cumulative_expanded_bytes: u64,
}
/// Borrowing core-side IPC hook; counters never leave the core or restart.
/// N17–N19 isolation adapters must call `admit` before every allocation or
/// expansion, enforce acknowledged reservations in the child process, cancel
/// owned work on refusal, and call `crashed` on parser death. They must call
/// `finish` before releasing output to fidelity or canonical preparation.
/// This contract does not implement OS IPC or containment.
#[derive(Debug)]
pub struct ParserDecode<'a> {
    /// The existing document ledger, including HTTP costs and first refusal.
    accounting: &'a mut Accounting,
    /// All admitted operation receipts, also inspectable after a hold.
    receipts: Vec<DecodeReceipt>,
}
impl<'a> ParserDecode<'a> {
    /// Attach to an existing document's ledger; never manufacture a new budget.
    #[must_use]
    pub fn new(accounting: &'a mut Accounting) -> Self {
        Self {
            accounting,
            receipts: Vec::new(),
        }
    }
    /// Charge before allocating. Failure latches the first stage/reason.
    /// # Errors
    /// Any shared or per-step limit, expired time or earlier parser hold refuses.
    pub fn admit(&mut self, request: DecodeRequest) -> Result<(), DecodeRefusal> {
        if let Some((stage, reason)) = self.accounting.decode_hold() {
            return Err(DecodeRefusal { stage, reason });
        }
        let Some(memory_bytes) = request
            .memory_bytes
            .checked_add(size_of::<DecodeReceipt>() as u64)
        else {
            self.accounting
                .hold_decode(Some(request.stage), Failure::Memory);
            return Err(DecodeRefusal {
                stage: Some(request.stage),
                reason: Failure::Memory,
            });
        };
        let charge = DecodeCharge {
            stage: request.stage,
            input_bytes: request.input_bytes,
            expanded_bytes: request.expanded_bytes,
            levels: request.levels,
            members: request.members,
            entities: request.entities,
            pixels: request.pixels,
            memory_bytes,
        };
        if let Err(reason) = self.accounting.parser_charge(&charge) {
            self.accounting
                .hold_decode(Some(request.stage), reason.clone());
            return Err(DecodeRefusal {
                stage: Some(request.stage),
                reason,
            });
        }
        if self.receipts.try_reserve_exact(1).is_err() {
            self.accounting
                .hold_decode(Some(request.stage), Failure::Memory);
            return Err(DecodeRefusal {
                stage: Some(request.stage),
                reason: Failure::Memory,
            });
        }
        self.receipts.push(DecodeReceipt {
            request,
            cumulative_expanded_bytes: self.accounting.expanded_bytes(),
        });
        Ok(())
    }
    /// Mark an owned parser crash without discarding any admitted receipts.
    pub fn crashed(&mut self, stage: DecodeStage) {
        self.accounting
            .hold_decode(Some(stage), Failure::ParserCrash);
    }
    /// All admitted proposals remain available even when completion holds.
    #[must_use]
    pub fn receipts(&self) -> &[DecodeReceipt] {
        &self.receipts
    }
    /// Complete only if every preflight passed and the deadline remains live.
    /// # Errors
    /// First refusal/crash or an expired deadline prevents partial promotion.
    pub fn finish(&mut self) -> Result<&[DecodeReceipt], DecodeRefusal> {
        if let Some((stage, reason)) = self.accounting.decode_hold() {
            return Err(DecodeRefusal { stage, reason });
        }
        if let Err(reason) = self.accounting.validate() {
            let stage = self.accounting.refusal_stage(&reason);
            self.accounting.hold_decode(stage, reason.clone());
            return Err(DecodeRefusal { stage, reason });
        }
        Ok(&self.receipts)
    }
}
