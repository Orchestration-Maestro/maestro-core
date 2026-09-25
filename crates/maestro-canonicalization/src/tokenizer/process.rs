//! The counter subprocess: bounded pipes, a timeout and a child that is always reaped.
use crate::Error;
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// The most output the counter may print; more is refused, not truncated.
const MAX_STDOUT_BYTES: usize = 16 * 1024 * 1024;
/// The most diagnostics the counter may print on its error stream.
const MAX_STDERR_BYTES: usize = 1024 * 1024;

/// Read at most `limit` bytes; one byte more is an error.
pub(super) fn read_bounded(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, Error> {
    let bound = u64::try_from(limit)
        .ok()
        .and_then(|length| length.checked_add(1))
        .ok_or_else(|| Error("invalid tokenizer output limit".into()))?;
    let mut bytes = Vec::new();
    Read::take(&mut reader, bound)
        .read_to_end(&mut bytes)
        .map_err(|_| Error("tokenizer pipe read failed".into()))?;
    if bytes.len() > limit {
        return Err(Error("tokenizer output exceeds resource limit".into()));
    }
    Ok(bytes)
}

/// A child process that is killed and waited for when dropped, on every path out.
struct Reap(Child);
impl Drop for Reap {
    fn drop(&mut self) {
        // Normal exit may already be reaped; cleanup must also run on early errors.
        drop(self.0.kill());
        drop(self.0.wait());
    }
}

/// Run the counter with the input on its standard input and both output streams drained, within the
/// timeout; a failure, a timeout or oversized output returns nothing.
pub(super) fn run_native(
    command: &mut Command,
    input: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, Error> {
    let started = Instant::now();
    let child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| Error("tokenizer process could not start".into()))?;
    thread::scope(|scope| {
        // Drop the child inside the scope, before any automatic pipe-thread joins.
        let mut child = Reap(child);
        let mut stdin = child
            .0
            .stdin
            .take()
            .ok_or_else(|| Error("tokenizer stdin unavailable".into()))?;
        let stdout = child
            .0
            .stdout
            .take()
            .ok_or_else(|| Error("tokenizer stdout unavailable".into()))?;
        let stderr = child
            .0
            .stderr
            .take()
            .ok_or_else(|| Error("tokenizer stderr unavailable".into()))?;
        let writer = scope.spawn(move || stdin.write_all(input));
        let output = scope.spawn(move || read_bounded(stdout, MAX_STDOUT_BYTES));
        let diagnostic = scope.spawn(move || read_bounded(stderr, MAX_STDERR_BYTES));
        let status = loop {
            match child.0.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {}
                Err(_) => break Err(Error("tokenizer process status failed".into())),
            }
            if started.elapsed() >= timeout {
                break Err(Error("tokenizer process timed out".into()));
            }
            thread::sleep(Duration::from_millis(10));
        };
        drop(child);
        let written = writer.join();
        let output = output.join();
        let diagnostic = diagnostic.join();
        let status = status?;
        written
            .map_err(|_| Error("tokenizer input worker failed".into()))?
            .map_err(|_| Error("tokenizer input write failed".into()))?;
        let output = output.map_err(|_| Error("tokenizer output worker failed".into()))??;
        diagnostic.map_err(|_| Error("tokenizer diagnostic worker failed".into()))??;
        if !status.success() {
            return Err(Error("tokenizer process failed".into()));
        }
        Ok(output)
    })
}
