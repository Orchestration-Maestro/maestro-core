//! Shared bounded output collection for native PTY and `ConPTY` receipts.
use std::{
    fs::File,
    io::Read,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

/// Drain concurrently: both native terminal APIs can block if their output pipe fills.
pub(super) fn output(reader: File) -> Receiver<Vec<u8>> {
    let (send, receive) = mpsc::channel();
    thread::spawn(move || read(reader, &send));
    receive
}
/// Worker owns the descriptor and ends when its terminal closes or parent disappears.
fn read(mut reader: File, send: &Sender<Vec<u8>>) {
    let mut buffer = [0; 8192];
    while let Ok(count) = reader.read(&mut buffer) {
        if count == 0 || send.send(buffer.get(..count).unwrap().to_vec()).is_err() {
            break;
        }
    }
}
/// Match the existing CLI test deadline; this is not a product timeout.
pub(super) fn until(output: &Receiver<Vec<u8>>, bytes: &mut Vec<u8>, marker: &str) {
    let deadline = Instant::now() + Duration::from_secs(60);
    let visible_marker = text(marker.as_bytes());
    while !text(bytes).contains(&visible_marker) {
        assert!(
            Instant::now() < deadline,
            "missing {marker}: {}",
            String::from_utf8_lossy(bytes)
        );
        if let Ok(chunk) = output.recv_timeout(Duration::from_millis(100)) {
            bytes.extend(chunk);
        }
    }
}
/// Collect the last cleanup commands after the child has exited.
pub(super) fn drain(output: &Receiver<Vec<u8>>, bytes: &mut Vec<u8>) {
    while let Ok(chunk) = output.recv_timeout(Duration::from_millis(100)) {
        bytes.extend(chunk);
    }
}

/// Match labels across styling and cursor-positioned whitespace, not exact frame bytes.
pub(super) fn text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut chars = text.chars();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            if !ch.is_whitespace() {
                result.push(ch);
            }
            continue;
        }
        match chars.next() {
            Some('[') => {
                chars.by_ref().find(|code| ('@'..='~').contains(code));
            }
            Some(']') => {
                chars.by_ref().find(|code| *code == '\u{7}');
            }
            _ => {}
        }
    }
    result
}
