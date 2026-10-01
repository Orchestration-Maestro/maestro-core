//! Shared cumulative wire/decode accounting, reusable by downstream decoders.
use super::budget::{Limits, tighten};
use crate::{Refusal, policy::authority::AuthorityRefusal};
use flate2::{Decompress, FlushDecompress, Status};
use http_body_util::BodyExt as _;
use hyper::{Response, body::Incoming};
use maestro_kernel::retrieval::Clock;
use reqwest::header::CONTENT_ENCODING;
use std::{
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, Instant as StdInstant},
};
use tokio::time::Instant;
/// Content-free failures; no source URL, credentials or partial bytes escape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Current policy/URL/address controls refused this hop.
    Admission(Refusal),
    /// Current N05 grant was refused or expired.
    Authority(AuthorityRefusal),
    /// Invalid effective settings, including fewer than five robots redirects.
    Configuration,
    /// The same canonical identity was visited twice.
    RedirectLoop,
    /// The composed redirect ceiling was reached.
    RedirectLimit,
    /// The cumulative request ceiling was reached.
    Requests,
    /// Connection/HTTP work failed; eligible for a bounded freshly admitted retry.
    Transport,
    /// Checked acquisition profile selected a different content transport.
    TransportMismatch,
    /// Owned connection and driver were dropped on deadline.
    Timeout,
    /// Authentication is required; no automatic credential escalation.
    Authentication,
    /// Forbidden/challenge response; no challenge-solving fallback.
    Challenge,
    /// Truncated or unsolicited range response; no partial promotion.
    Partial,
    /// Unknown, malformed or incomplete content encoding.
    Content,
    /// Encoded wire-byte ceiling, distinct from expanded bytes.
    EncodedBytes,
    /// Cumulative bytes produced by all decode stages exceeded the ceiling.
    ExpandedBytes,
    /// Cumulative expanded/wire ratio exceeded the ceiling.
    ExpansionRatio,
    /// Decoder workspace or retained bytes exceed the memory ceiling.
    Memory,
    /// Content encoding layers exceed the nested decode ceiling.
    Nesting,
    /// Cumulative gzip/deflate/container members exceed the decode ceiling.
    Members,
}

/// Accounting follows one document across HTTP hops and every decode stage.
#[derive(Debug)]
pub struct Accounting {
    /// Retained tighter effective limits, never widened on reuse.
    limits: Limits,
    /// Bytes received before content decoding, across all hops.
    wire: u64,
    /// Bytes emitted by every decoder, not only final output.
    expanded: u64,
    /// All dispatch attempts, including failed connects.
    requests: u64,
    /// First deadline retained across all stages and retries.
    deadline: Option<Instant>,
    /// Decoder members across every hop and stage, including empty members.
    members: u64,
    /// Current fixed parser/codec workspaces; never hidden in body buffers.
    workspace: u64,
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
            limits,
            wire: 0,
            expanded: 0,
            requests: 0,
            deadline: None,
            members: 0,
            workspace: 0,
            clock,
        }
    }
    /// Effective bounds for downstream adapters; immutable to callers.
    #[must_use]
    pub fn limits(&self) -> &Limits {
        &self.limits
    }
    /// Cumulative received encoded bytes, including redirect bodies read.
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
        let next = Instant::from_std(self.clock.now())
            .checked_add(term)
            .ok_or(Failure::Configuration)?;
        let deadline = self.deadline.map_or(next, |old| old.min(next));
        self.deadline = Some(deadline);
        self.check_time()?;
        Ok(deadline)
    }
    /// Cumulative number of decode members, including empty gzip members.
    #[must_use]
    pub fn members(&self) -> u64 {
        self.members
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
        Ok(())
    }
    /// Reserve bounded parser and decoder workspaces before allocation.
    pub(super) fn workspace(&mut self, bytes: u64) -> Result<(), Failure> {
        self.workspace = bytes;
        self.retained(self.expanded)
    }
    /// Conservative realloc/copy peak: two copies of cumulative wire/output.
    fn retained(&self, expanded: u64) -> Result<(), Failure> {
        let bytes = self
            .wire
            .checked_add(expanded)
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(self.workspace))
            .ok_or(Failure::Memory)?;
        self.memory(bytes)
    }
    /// Check a downstream decoder's deadline even during synchronous expansion.
    ///
    /// # Errors
    /// Expired document deadlines cancel work, rather than promoting partial output.
    pub fn check_time(&self) -> Result<(), Failure> {
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
        Ok(())
    }
    /// Admit every decode-stage output before growing its intermediate buffer.
    ///
    /// # Errors
    /// Cumulative expanded, ratio, memory or time ceilings refuse.
    pub fn decoded(&mut self, bytes: u64) -> Result<(), Failure> {
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
        Ok(())
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
/// Codec workspaces are allocation costs, not configurable resource ceilings.
/// zlib-rs's bounded inflate window/state plus one scratch block fit this reserve.
const CODEC_WORKSPACE: u64 = 262_144;
/// One bounded incremental gzip/zlib layer, retaining its end-of-stream state.
struct Decoder {
    /// Bounded incremental inflate engine.
    codec: Decompress,
    /// A complete header, stream and checked trailer have been received.
    ended: bool,
    /// Only gzip permits concatenated independently checked members.
    gzip: bool,
    /// Count each new member exactly once even across split input frames.
    started: bool,
    /// Encoded input to this layer, cumulative across its gzip members.
    input_bytes: u64,
    /// Expanded output from this layer, independent of other hop padding.
    output_bytes: u64,
}
impl Decoder {
    /// Expand only into a fixed stack scratch; admit output before retention.
    fn push(
        &mut self,
        mut input: &[u8],
        finish: bool,
        accounting: &mut Accounting,
    ) -> Result<Vec<u8>, Failure> {
        let mut output = Vec::new();
        self.input_bytes = self
            .input_bytes
            .checked_add(input.len() as u64)
            .ok_or(Failure::ExpansionRatio)?;
        loop {
            if self.ended && input.is_empty() {
                return Ok(output);
            }
            if self.ended {
                self.next_member()?;
            }
            if !self.started && !input.is_empty() {
                accounting.member()?;
                self.started = true;
            }
            accounting.check_time()?;
            let mut scratch = [0; 8192];
            let before_in = self.codec.total_in();
            let before_out = self.codec.total_out();
            let status = self
                .codec
                .decompress(
                    input,
                    &mut scratch,
                    if finish {
                        FlushDecompress::Finish
                    } else {
                        FlushDecompress::None
                    },
                )
                .map_err(|_| Failure::Content)?;
            let consumed =
                usize::try_from(self.codec.total_in() - before_in).map_err(|_| Failure::Content)?;
            let produced = usize::try_from(self.codec.total_out() - before_out)
                .map_err(|_| Failure::Content)?;
            input = input.get(consumed..).ok_or(Failure::Content)?;
            self.output(
                produced as u64,
                accounting.limits.decode.expansion_ratio.get(),
            )?;
            accounting.decoded(produced as u64)?;
            append(
                &mut output,
                scratch.get(..produced).ok_or(Failure::Content)?,
            )?;
            if status == Status::StreamEnd {
                self.ended = true;
                continue;
            }
            if consumed == 0 && produced == 0 {
                return completed(output, finish || !input.is_empty());
            }
            if input.is_empty() && produced < scratch.len() && !finish {
                return Ok(output);
            }
        }
    }
    /// A decode step cannot borrow ratio headroom from another hop or layer.
    fn output(&mut self, bytes: u64, ratio: u64) -> Result<(), Failure> {
        let output = self
            .output_bytes
            .checked_add(bytes)
            .ok_or(Failure::ExpansionRatio)?;
        if u128::from(output) > u128::from(self.input_bytes) * u128::from(ratio) {
            return Err(Failure::ExpansionRatio);
        }
        self.output_bytes = output;
        Ok(())
    }
    /// zlib validates each new gzip header, CRC32 and ISIZE independently.
    fn next_member(&mut self) -> Result<(), Failure> {
        if !self.gzip {
            return Err(Failure::Content);
        }
        self.codec = Decompress::new_gzip(15);
        self.ended = false;
        self.started = false;
        Ok(())
    }
}
/// Parse all encoding header values, decoding known layers in reverse wire order.
fn decoders(
    response: &Response<Incoming>,
    accounting: &mut Accounting,
) -> Result<Vec<Decoder>, Failure> {
    let mut names = Vec::new();
    for value in response.headers().get_all(CONTENT_ENCODING) {
        for name in value.to_str().map_err(|_| Failure::Content)?.split(',') {
            let name = name.trim();
            if !["identity", "gzip", "deflate"]
                .iter()
                .any(|known| name.eq_ignore_ascii_case(known))
            {
                return Err(Failure::Content);
            }
            if !name.eq_ignore_ascii_case("identity") {
                names.push(name);
            }
        }
    }
    if names.len() as u64 > accounting.limits.decode.nested_levels.get() {
        return Err(Failure::Nesting);
    }
    let workspace = (names.len() as u64)
        .checked_mul(CODEC_WORKSPACE)
        .and_then(|bytes| bytes.checked_add(accounting.workspace))
        .ok_or(Failure::Memory)?;
    accounting.workspace(workspace)?;
    Ok(names
        .into_iter()
        .rev()
        .map(|name| Decoder {
            codec: if name.eq_ignore_ascii_case("gzip") {
                Decompress::new_gzip(15)
            } else {
                Decompress::new(true)
            },
            ended: false,
            gzip: name.eq_ignore_ascii_case("gzip"),
            started: false,
            input_bytes: 0,
            output_bytes: 0,
        })
        .collect())
}
/// Retain exact bytes without Vec's unchecked geometric capacity growth.
fn append(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), Failure> {
    output
        .try_reserve_exact(bytes.len())
        .map_err(|_| Failure::Memory)?;
    output.extend_from_slice(bytes);
    Ok(())
}
/// Read incrementally; hyper's fixed parser read buffer is never body collection.
pub(super) async fn read_body(
    response: Response<Incoming>,
    accounting: &mut Accounting,
) -> Result<(Vec<u8>, Vec<u8>), Failure> {
    let mut layers = decoders(&response, accounting)?;
    let mut incoming = response.into_body();
    let mut body = Vec::new();
    let mut wire = Vec::new();
    while let Some(frame) = incoming.frame().await {
        let frame = frame.map_err(|_| Failure::Partial)?;
        let Ok(bytes) = frame.into_data() else {
            continue;
        };
        accounting.encoded(bytes.len() as u64)?;
        accounting.retained(accounting.expanded)?;
        append(&mut wire, &bytes)?;
        if layers.is_empty() {
            accounting.decoded(bytes.len() as u64)?;
            append(&mut body, &bytes)?;
        } else {
            let (first, rest) = layers.split_first_mut().ok_or(Failure::Content)?;
            let mut decoded = first.push(&bytes, false, accounting)?;
            for layer in rest {
                decoded = layer.push(&decoded, false, accounting)?;
            }
            append(&mut body, &decoded)?;
        }
    }
    let mut final_bytes = Vec::new();
    for layer in &mut layers {
        final_bytes = layer.push(&final_bytes, true, accounting)?;
    }
    append(&mut body, &final_bytes)?;
    Ok((body, wire))
}

/// Refuse trailing data or an incomplete stream instead of returning partial bytes.
fn completed(output: Vec<u8>, invalid: bool) -> Result<Vec<u8>, Failure> {
    if invalid {
        return Err(Failure::Content);
    }
    Ok(output)
}

/// Tokio's monotonic clock honors paused time while satisfying the kernel port.
#[derive(Debug)]
struct TokioClock;
impl Clock for TokioClock {
    fn now(&self) -> StdInstant {
        Instant::now().into_std()
    }
}
