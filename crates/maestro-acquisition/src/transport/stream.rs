//! Shared cumulative wire/decode accounting, reusable by downstream decoders.
pub use super::accounting::Accounting;
pub use super::failure::{DecodeStage, Failure};
use super::wire_quota::Quota;
use flate2::{Decompress, FlushDecompress, Status};
use http_body_util::BodyExt as _;
use hyper::{Response, body::Incoming};
use reqwest::header::CONTENT_ENCODING;

/// Hyper 1.11.1 has no trailer-size setting: proto/h1/decode.rs:25,181
/// fixes this 16 KiB ceiling. Reserve it before connecting; discard its fields.
pub const TRAILER_MAX_BYTES: u64 = 16_384;
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
            self.output(produced as u64, accounting)?;
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
    fn output(&mut self, bytes: u64, accounting: &mut Accounting) -> Result<(), Failure> {
        let output = self
            .output_bytes
            .checked_add(bytes)
            .ok_or(Failure::ExpansionRatio)?;
        accounting.admit_step_ratio(output, self.input_bytes, DecodeStage::Http)?;
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
    accounting.nesting(names.len() as u64)?;
    let workspace = (names.len() as u64)
        .checked_mul(CODEC_WORKSPACE)
        .and_then(|bytes| bytes.checked_add(accounting.workspace_bytes()))
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
    quota: &Quota,
) -> Result<(Vec<u8>, Vec<u8>), Failure> {
    let mut layers = decoders(&response, accounting)?;
    let mut incoming = response.into_body();
    let mut body = Vec::new();
    let mut wire = Vec::new();
    loop {
        let frame = incoming.frame().await;
        charge_quota(quota, accounting)?;
        accounting.validate()?;
        let Some(frame) = frame else {
            break;
        };
        let frame = frame.map_err(|_| Failure::Partial)?;
        let Ok(bytes) = frame.into_data() else {
            continue;
        };
        accounting.retained(accounting.expanded_bytes())?;
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
    accounting.validate()?;
    Ok((body, wire))
}

/// Refuse trailing data or an incomplete stream instead of returning partial bytes.
fn completed(output: Vec<u8>, invalid: bool) -> Result<Vec<u8>, Failure> {
    if invalid {
        return Err(Failure::Content);
    }
    Ok(output)
}

/// Transfer every raw read to accounting before examining decoded frames.
pub(super) fn charge_quota(quota: &Quota, accounting: &mut Accounting) -> Result<(), Failure> {
    let (received, failure) = quota.take_received()?;
    accounting.encoded(received)?;
    if let Some(failure) = failure {
        return Err(failure);
    }
    Ok(())
}

#[cfg(test)]
mod mutation_tests {
    use super::{Accounting, Decoder, Failure};
    use crate::transport::mutation_tests::limits;
    use flate2::{Compression, Decompress, write::GzEncoder};
    use std::{io::Write as _, sync::mpsc, thread, time::Duration};

    /// Detached call allows an endless decode mutation to fail an assertion.
    fn checked(input: Vec<u8>, split: usize, expected: Vec<u8>, members: u64) {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut limits = limits();
            limits.wire_bytes = 100_000.try_into().unwrap();
            limits.decode.expanded_bytes = 100_000.try_into().unwrap();
            limits.decode.expansion_ratio = 10_000.try_into().unwrap();
            let mut accounting = Accounting::new(limits);
            accounting.encoded(input.len() as u64).unwrap();
            let mut decoder = Decoder {
                codec: Decompress::new_gzip(15),
                ended: false,
                gzip: true,
                started: false,
                input_bytes: 0,
                output_bytes: 0,
            };
            let mut output = decoder
                .push(input.get(..split).unwrap(), false, &mut accounting)
                .unwrap();
            output.extend(
                decoder
                    .push(input.get(split..).unwrap(), false, &mut accounting)
                    .unwrap(),
            );
            assert_eq!(output, expected, "complete input must drain pending output");
            assert!(decoder.push(&[], true, &mut accounting).unwrap().is_empty());
            sender.send(accounting.members()).unwrap();
        });
        assert_eq!(receiver.recv_timeout(Duration::from_secs(10)), Ok(members));
    }
    #[test]
    fn s6t_stream_split_and_exact_scratch_completion() {
        for size in [0, 40, 8192, 8193, 32768] {
            let expected: Vec<u8> = (0..=250).cycle().take(size).collect();
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&expected).unwrap();
            let compressed = encoder.finish().unwrap();
            for split in [0, 1, compressed.len() - 1, compressed.len()] {
                checked(compressed.clone(), split, expected.clone(), 1);
            }
        }
    }
    #[test]
    fn s6t_stream_concatenated_members_preserve_all_output() {
        let mut compressed = Vec::new();
        for text in [b"first".as_slice(), b"second".as_slice()] {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(text).unwrap();
            compressed.extend(encoder.finish().unwrap());
        }
        checked(compressed, 0, b"firstsecond".to_vec(), 2);
    }
    #[test]
    fn s6t_stream_unfinished_input_is_not_completion() {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut accounting = Accounting::new(limits());
            accounting.encoded(2).unwrap();
            let mut decoder = Decoder {
                codec: Decompress::new_gzip(15),
                ended: false,
                gzip: true,
                started: false,
                input_bytes: 0,
                output_bytes: 0,
            };
            assert_eq!(decoder.push(&[0x1f], false, &mut accounting), Ok(vec![]));
            sender
                .send(decoder.push(&[], true, &mut accounting))
                .unwrap();
        });
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(10)),
            Ok(Err(Failure::Content))
        );
    }
}

#[cfg(test)]
mod clock_mutation_tests {
    use super::{Accounting, Decoder};
    use crate::transport::mutation_tests::limits;
    use flate2::Decompress;
    use maestro_kernel::retrieval::Clock;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };
    #[derive(Debug)]
    struct Cutoff {
        start: Instant,
        calls: AtomicUsize,
    }
    impl Clock for Cutoff {
        fn now(&self) -> Instant {
            if self.calls.fetch_add(1, Ordering::SeqCst) >= 5 {
                self.start + Duration::from_secs(1)
            } else {
                self.start
            }
        }
    }
    #[test]
    fn s6t_stream_deadline_and_partial_frame_boundaries() {
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write as _;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&[b'x'; 8192]).unwrap();
        let mut compressed = encoder.finish().unwrap();
        compressed.pop().unwrap();
        for (input, finish, expected) in [
            (vec![0x1f], false, Ok(Vec::<u8>::new())),
            (vec![0x1f], true, Err(super::Failure::Timeout)),
            (compressed, false, Err(super::Failure::Timeout)),
        ] {
            let mut limits = limits();
            limits.elapsed_ms = 1000.try_into().unwrap();
            limits.decode.elapsed_ms = limits.elapsed_ms;
            limits.decode.expansion_ratio = 10_000.try_into().unwrap();
            let clock = Arc::new(Cutoff {
                start: Instant::now(),
                calls: AtomicUsize::new(0),
            });
            let mut accounting = Accounting::with_clock(limits, clock);
            accounting.encoded(input.len() as u64).unwrap();
            let mut decoder = Decoder {
                codec: Decompress::new_gzip(15),
                ended: false,
                gzip: true,
                started: false,
                input_bytes: 0,
                output_bytes: 0,
            };
            assert_eq!(decoder.push(&input, finish, &mut accounting), expected);
        }
    }
}
