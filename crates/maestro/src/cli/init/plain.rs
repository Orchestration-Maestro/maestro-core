//! Sequential labelled prompts; no raw mode, repaint, color or terminal state.
use super::flow::{Answer, FlowPort};
use crate::failure::Failure;
use std::io::{self, BufRead, Write};

/// IO is injectable; terminal status is checked only by the trust adapter.
pub(in crate::cli) struct Plain<'a> {
    /// Buffered prompt input.
    pub(in crate::cli) input: &'a mut dyn BufRead,
    /// Information and labelled prompts, normally stderr.
    pub(in crate::cli) output: &'a mut dyn Write,
}

impl FlowPort for Plain<'_> {
    fn show(&mut self, text: &str) -> Result<(), Failure> {
        writeln!(self.output, "{text}")
            .and_then(|()| self.output.flush())
            .map_err(|error| Failure::failed_by(&error))
    }

    fn ask(&mut self, label: &str) -> Result<Answer, Failure> {
        write!(self.output, "{label}")
            .and_then(|()| self.output.flush())
            .map_err(|error| Failure::failed_by(&error))?;
        let mut line = String::new();
        match self.input.read_line(&mut line) {
            Ok(0) => return Ok(Answer::Cancel),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return Ok(Answer::Cancel),
            Err(error) => return Err(Failure::failed_by(&error)),
            Ok(_) => {}
        }
        let text = line.trim();
        Ok(match text.to_ascii_lowercase().as_str() {
            "cancel" | "exit" | "\u{3}" => Answer::Cancel,
            "back" | "\u{1b}" => Answer::Back,
            _ => Answer::Text(text.to_owned()),
        })
    }
}
