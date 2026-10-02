//! One core-owned cumulative accounting ledger shared by HTTP and parser IPC.
use super::{
    budget::{Limits, tighten},
    failure::{DecodeStage, Failure},
};
use maestro_kernel::retrieval::Clock;
use std::{
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, Instant as StdInstant},
};
use tokio::time::Instant;
/// Accounting follows one document across HTTP hops and every decode stage.
#[derive(Debug)]
pub struct Accounting {
    /// Retained tighter effective limits, never widened on reuse.
    limits: Limits,
    /// Raw bytes after each final header CRLFCRLF: DATA, chunk framing and trailers.
    wire: u64,
    /// Bytes emitted by every decoder, not only final output.
    expanded: u64,
    /// All dispatch attempts, including failed connects.
    requests: u64,
    /// Original document start in the trusted clock domain; never restarted.
    started: StdInstant,
    /// Anchored deadline retained across all stages and retries.
    deadline: Option<Instant>,
    /// Independent logical-run cutoff; later contexts cannot extend it.
    run_deadline: Option<Instant>,
    /// Decoder members across every hop and stage, including empty members.
    members: u64,
    /// Decode layers charged across HTTP and parser IPC, never reset at handoff.
    levels: u64,
    /// Cumulative decoded image pixels across all owned stages.
    pixels: u64,
    /// First parser refusal/crash prevents a fresh session promoting partial data.
    held: Option<(Option<DecodeStage>, Failure)>,
    /// Current HTTP-hop parser/codec workspace, replaced on the next hop.
    workspace: u64,
    /// Cumulative parser IPC reservations, never replaced by HTTP setup.
    reservations: u64,
    /// Worst admitted per-step ratio and its charging stage, retained exactly.
    step_ratio: Option<(u64, u64, DecodeStage)>,
    /// Charging provenance survives recreation of the borrowing parser hook.
    stages: ChargeStages,
    /// Owned redacted metadata retained across hops.
    metadata: u64,
    /// Trusted clock port; production Tokio time remains pausable in tests.
    clock: Arc<dyn Clock>,
}
impl Accounting {
    /// Start with a complete explicit envelope, not an unbounded/default budget.
    #[must_use]
    pub fn new(limits: Limits) -> Self {
        Self::with_clock(limits, Arc::new(TokioClock))
    }
    /// Bind the kernel clock port; adapters cannot substitute remote source time.
    #[must_use]
    pub fn with_clock(limits: Limits, clock: Arc<dyn Clock>) -> Self {
        Self {
            started: clock.now(),
            limits,
            wire: 0,
            expanded: 0,
            requests: 0,
            deadline: None,
            run_deadline: None,
            members: 0,
            levels: 0,
            pixels: 0,
            held: None,
            workspace: 0,
            reservations: 0,
            step_ratio: None,
            stages: ChargeStages::default(),
            metadata: 0,
            clock,
        }
    }
    /// Effective bounds for downstream adapters; immutable to callers.
    #[must_use]
    pub fn limits(&self) -> &Limits {
        &self.limits
    }
    /// Number of attempted HTTP operations, including failures and robots.
    #[must_use]
    pub fn requests(&self) -> u64 {
        self.requests
    }
    /// All raw post-final-header bytes, including redirect DATA, framing and trailers.
    #[must_use]
    pub fn wire_bytes(&self) -> u64 {
        self.wire
    }
    /// Cumulative output of all HTTP and downstream decode stages.
    #[must_use]
    pub fn expanded_bytes(&self) -> u64 {
        self.expanded
    }
    /// Retain stricter policy/settings bounds across hops and downstream stages.
    pub fn tighten(&mut self, limits: &Limits) {
        tighten(&mut self.limits, limits);
    }
    /// Robots use the RFC ceiling and stricter reviewed policy on both byte kinds.
    pub(super) fn robots(&mut self, cap: u64) {
        let cap = cap.min(512_000);
        if let Some(bound) = NonZeroU64::new(cap) {
            self.limits.wire_bytes = self.limits.wire_bytes.min(bound);
            self.limits.decode.expanded_bytes = self.limits.decode.expanded_bytes.min(bound);
        }
    }
    /// Bound each owned dispatch, with a retained monotonic document deadline.
    pub(super) fn request(&mut self) -> Result<(), Failure> {
        self.requests = self.requests.checked_add(1).ok_or(Failure::Requests)?;
        self.stages.latest = Some(DecodeStage::Http);
        if self.requests > self.limits.requests.get() {
            return Err(Failure::Requests);
        }
        self.deadline()?;
        self.check_time()
    }
    /// Start or tighten a document deadline; reuse cannot restart its time budget.
    pub(super) fn deadline(&mut self) -> Result<Instant, Failure> {
        let term = Duration::from_millis(
            self.limits
                .elapsed_ms
                .min(self.limits.decode.elapsed_ms)
                .get(),
        );
        let deadline = Instant::from_std(self.started)
            .checked_add(term)
            .ok_or(Failure::Configuration)?;
        let deadline = self.run_deadline.map_or(deadline, |run| deadline.min(run));
        self.deadline = Some(deadline);
        self.check_time()?;
        Ok(deadline)
    }
    /// Retain the independent owning cutoff; only a tighter context can change it.
    pub(super) fn bind_run_deadline(&mut self, deadline: StdInstant) {
        let deadline = Instant::from_std(deadline);
        self.run_deadline = Some(self.run_deadline.map_or(deadline, |old| old.min(deadline)));
    }
    /// Snapshot the effective owned cutoff and clock for the read boundary.
    pub(super) fn read_timing(&self) -> Result<(Arc<dyn Clock>, StdInstant), Failure> {
        let deadline = self.deadline.ok_or(Failure::Configuration)?;
        Ok((Arc::clone(&self.clock), deadline.into_std()))
    }
    /// Current time from the document's trusted clock, also used for pacing.
    pub(super) fn now(&self) -> StdInstant {
        self.clock.now()
    }
    /// Check every retained counter against the currently effective envelope.
    pub(crate) fn validate(&self) -> Result<(), Failure> {
        self.check_time()?;
        if self.wire > self.limits.wire_bytes.get() {
            return Err(Failure::EncodedBytes);
        }
        if self.expanded > self.limits.decode.expanded_bytes.get() {
            return Err(Failure::ExpandedBytes);
        }
        if self.step_ratio_exceeded()
            || u128::from(self.expanded)
                > u128::from(self.wire) * u128::from(self.limits.decode.expansion_ratio.get())
        {
            return Err(Failure::ExpansionRatio);
        }
        if self.members > self.limits.decode.members.get() {
            return Err(Failure::Members);
        }
        if self.levels > self.limits.decode.nested_levels.get() {
            return Err(Failure::Nesting);
        }
        if self.pixels > self.limits.decode.decoded_pixels.get() {
            return Err(Failure::Pixels);
        }
        self.retained(self.expanded)
    }
    /// Cumulative number of decode members, including empty gzip members.
    #[must_use]
    pub fn members(&self) -> u64 {
        self.members
    }
    /// Reserve hop metadata before the retained history allocation grows.
    pub(super) fn metadata(&mut self, bytes: u64) -> Result<(), Failure> {
        let old = self.metadata;
        self.metadata = old.checked_add(bytes).ok_or(Failure::Memory)?;
        if let Err(error) = self.retained(self.expanded) {
            self.metadata = old;
            return Err(error);
        }
        self.stages.memory = Some(DecodeStage::Http);
        self.stages.latest = Some(DecodeStage::Http);
        Ok(())
    }
    /// Admit one member before allocating or starting its decoder.
    ///
    /// # Errors
    /// The shared member ceiling and document deadline are never reset per layer.
    pub fn member(&mut self) -> Result<(), Failure> {
        self.check_time()?;
        let members = self.members.checked_add(1).ok_or(Failure::Members)?;
        if members > self.limits.decode.members.get() {
            return Err(Failure::Members);
        }
        self.members = members;
        self.stages.members = Some(DecodeStage::Http);
        self.stages.latest = Some(DecodeStage::Http);
        Ok(())
    }
    /// Reserve bounded parser and decoder workspaces before allocation.
    pub(super) fn workspace(&mut self, bytes: u64) -> Result<(), Failure> {
        self.workspace = bytes;
        self.retained(self.expanded)?;
        self.stages.memory = Some(DecodeStage::Http);
        self.stages.latest = Some(DecodeStage::Http);
        Ok(())
    }
    /// Add a cumulative parser reservation before allocation.
    fn reserve(&mut self, bytes: u64, stage: DecodeStage) -> Result<(), Failure> {
        let old = self.reservations;
        self.reservations = old.checked_add(bytes).ok_or(Failure::Memory)?;
        if let Err(error) = self.retained(self.expanded) {
            self.reservations = old;
            return Err(error);
        }
        self.stages.memory = Some(stage);
        self.stages.latest = Some(stage);
        Ok(())
    }
    /// Conservative realloc/copy peak: two copies of cumulative wire/output.
    pub(super) fn retained(&self, expanded: u64) -> Result<(), Failure> {
        let bytes = self
            .wire
            .checked_add(expanded)
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(self.workspace))
            .and_then(|bytes| bytes.checked_add(self.reservations))
            .and_then(|bytes| bytes.checked_add(self.metadata))
            .ok_or(Failure::Memory)?;
        self.memory(bytes)
    }
    /// Check a downstream decoder's deadline even during synchronous expansion.
    ///
    /// # Errors
    /// Expired document deadlines cancel work, rather than promoting partial output.
    pub fn check_time(&self) -> Result<(), Failure> {
        if self.clock.now().saturating_duration_since(self.started)
            >= Duration::from_millis(
                self.limits
                    .elapsed_ms
                    .min(self.limits.decode.elapsed_ms)
                    .get(),
            )
        {
            return Err(Failure::Timeout);
        }
        if self
            .deadline
            .is_some_and(|deadline| self.clock.now() >= deadline.into_std())
        {
            return Err(Failure::Timeout);
        }
        Ok(())
    }
    /// Admit received bytes before retaining them in any body buffer.
    ///
    /// # Errors
    /// Overflow or the cumulative encoded ceiling refuses without promotion.
    pub fn encoded(&mut self, bytes: u64) -> Result<(), Failure> {
        self.check_time()?;
        let total = self.wire.checked_add(bytes).ok_or(Failure::EncodedBytes)?;
        if total > self.limits.wire_bytes.get() {
            return Err(Failure::EncodedBytes);
        }
        self.wire = total;
        if bytes > 0 {
            self.stages.wire = Some(DecodeStage::Http);
            self.stages.memory = Some(DecodeStage::Http);
            self.stages.latest = Some(DecodeStage::Http);
        }
        Ok(())
    }
    /// Admit every decode-stage output before growing its intermediate buffer.
    ///
    /// # Errors
    /// Cumulative expanded, ratio, memory or time ceilings refuse.
    pub fn decoded(&mut self, bytes: u64) -> Result<(), Failure> {
        self.decoded_at(bytes, DecodeStage::Http)
    }
    /// Charge output with the operation that actually produced it.
    fn decoded_at(&mut self, bytes: u64, stage: DecodeStage) -> Result<(), Failure> {
        self.check_time()?;
        let total = self
            .expanded
            .checked_add(bytes)
            .ok_or(Failure::ExpandedBytes)?;
        if total > self.limits.decode.expanded_bytes.get() {
            return Err(Failure::ExpandedBytes);
        }
        if u128::from(total)
            > u128::from(self.wire) * u128::from(self.limits.decode.expansion_ratio.get())
        {
            return Err(Failure::ExpansionRatio);
        }
        self.retained(total)?;
        self.expanded = total;
        if bytes > 0 {
            self.stages.expanded = Some(stage);
            self.stages.memory = Some(stage);
            self.stages.latest = Some(stage);
        }
        Ok(())
    }
    /// Current HTTP-hop workspace, excluding cumulative parser reservations.
    pub(super) fn workspace_bytes(&self) -> u64 {
        self.workspace
    }
    /// Admit layers before constructing decoders; handoffs cannot reset depth.
    pub(crate) fn nesting(&mut self, levels: u64) -> Result<(), Failure> {
        self.nesting_at(levels, DecodeStage::Http)
    }
    /// Charge depth with its originating operation, not the latest hook.
    fn nesting_at(&mut self, levels: u64, stage: DecodeStage) -> Result<(), Failure> {
        let total = self.levels.checked_add(levels).ok_or(Failure::Nesting)?;
        if total > self.limits.decode.nested_levels.get() {
            return Err(Failure::Nesting);
        }
        self.levels = total;
        if levels > 0 {
            self.stages.levels = Some(stage);
            self.stages.latest = Some(stage);
        }
        Ok(())
    }
    /// First parser refusal survives re-creating the borrowing IPC hook.
    pub(crate) fn decode_hold(&self) -> Option<(Option<DecodeStage>, Failure)> {
        self.held.clone()
    }
    /// Preserve the first precise stage/reason, including parser crashes.
    pub(crate) fn hold_decode(&mut self, stage: Option<DecodeStage>, reason: Failure) {
        if self.held.is_none() {
            self.held = Some((stage, reason));
        }
    }
    /// Charge one parser's pre-allocation proposal against the shared ledger.
    pub(crate) fn parser_charge(&mut self, charge: &DecodeCharge) -> Result<(), Failure> {
        self.validate()?;
        if u128::from(charge.expanded_bytes)
            > u128::from(charge.input_bytes) * u128::from(self.limits.decode.expansion_ratio.get())
        {
            return Err(Failure::ExpansionRatio);
        }
        // The only admitted XML policy disables all entity expansion.
        if charge.entities > 0 {
            return Err(Failure::Entities);
        }
        self.nesting_at(charge.levels, charge.stage)?;
        let members = self
            .members
            .checked_add(charge.members)
            .ok_or(Failure::Members)?;
        if members > self.limits.decode.members.get() {
            return Err(Failure::Members);
        }
        let pixels = self
            .pixels
            .checked_add(charge.pixels)
            .ok_or(Failure::Pixels)?;
        if pixels > self.limits.decode.decoded_pixels.get() {
            return Err(Failure::Pixels);
        }
        self.reserve(charge.memory_bytes, charge.stage)?;
        self.decoded_at(charge.expanded_bytes, charge.stage)?;
        self.members = members;
        self.pixels = pixels;
        if charge.members > 0 {
            self.stages.members = Some(charge.stage);
        }
        if charge.pixels > 0 {
            self.stages.pixels = Some(charge.stage);
        }
        self.admit_step_ratio(charge.expanded_bytes, charge.input_bytes, charge.stage)?;
        self.stages.latest = Some(charge.stage);
        Ok(())
    }
    /// Retain the worst admitted operation ratio without rounding or floats.
    pub(crate) fn admit_step_ratio(
        &mut self,
        output: u64,
        input: u64,
        stage: DecodeStage,
    ) -> Result<(), Failure> {
        if u128::from(output)
            > u128::from(input) * u128::from(self.limits.decode.expansion_ratio.get())
        {
            return Err(Failure::ExpansionRatio);
        }
        if output > 0
            && self.step_ratio.is_none_or(|(old_output, old_input, _)| {
                u128::from(output) * u128::from(old_input)
                    > u128::from(old_output) * u128::from(input)
            })
        {
            self.step_ratio = Some((output, input, stage));
        }
        Ok(())
    }
    /// Recheck the retained worst ratio after effective policy tightening.
    fn step_ratio_exceeded(&self) -> bool {
        self.step_ratio.is_some_and(|(output, input, _)| {
            u128::from(output)
                > u128::from(input) * u128::from(self.limits.decode.expansion_ratio.get())
        })
    }
    /// Completion reports the operation that charged the failing counter.
    pub(crate) fn refusal_stage(&self, reason: &Failure) -> Option<DecodeStage> {
        match reason {
            Failure::EncodedBytes => self.stages.wire,
            Failure::ExpansionRatio if self.step_ratio_exceeded() => {
                self.step_ratio.map(|(_, _, stage)| stage)
            }
            Failure::ExpandedBytes | Failure::ExpansionRatio => self.stages.expanded,
            Failure::Members => self.stages.members,
            Failure::Nesting => self.stages.levels,
            Failure::Pixels => self.stages.pixels,
            Failure::Memory => self.stages.memory,
            _ => self.stages.latest,
        }
    }
    /// Bound decoder workspaces and retained/intermediate storage before growth.
    ///
    /// # Errors
    /// Unknown/overflowing or excessive memory refuses.
    pub fn memory(&self, bytes: u64) -> Result<(), Failure> {
        if bytes
            > self
                .limits
                .memory_bytes
                .min(self.limits.decode.memory_bytes)
                .get()
        {
            return Err(Failure::Memory);
        }
        Ok(())
    }
}
/// Per-counter charging stages; zero-cost work does not replace provenance.
#[derive(Debug, Default)]
struct ChargeStages {
    /// Raw bytes supplied by HTTP.
    wire: Option<DecodeStage>,
    /// Latest operation producing expanded bytes.
    expanded: Option<DecodeStage>,
    /// Latest operation opening members.
    members: Option<DecodeStage>,
    /// Latest operation adding decode depth.
    levels: Option<DecodeStage>,
    /// Latest operation producing pixels.
    pixels: Option<DecodeStage>,
    /// Latest operation charging retained storage.
    memory: Option<DecodeStage>,
    /// Latest charged operation, used for time refusals.
    latest: Option<DecodeStage>,
}
/// Tokio's monotonic clock honors paused time while satisfying the kernel port.
#[derive(Debug)]
struct TokioClock;
impl Clock for TokioClock {
    fn now(&self) -> StdInstant {
        Instant::now().into_std()
    }
}

/// Proposed incremental costs, never a serialized replacement ledger.
#[derive(Debug)]
pub(crate) struct DecodeCharge {
    /// Operation that charged these costs.
    pub stage: DecodeStage,
    /// Encoded bytes supplied to this individual decode operation.
    pub input_bytes: u64,
    /// New expanded bytes, admitted before growing the output buffer.
    pub expanded_bytes: u64,
    /// Additional nested layers.
    pub levels: u64,
    /// Additional archive members.
    pub members: u64,
    /// Requested XML entity expansions; only zero is admissible.
    pub entities: u64,
    /// Additional decoded pixels.
    pub pixels: u64,
    /// Additional owned parser workspaces; retained conservatively across IPC.
    pub memory_bytes: u64,
}
