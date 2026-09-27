//! Stages, items and completed-question counts for a failed synthetic run.

use std::{env, fmt};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Failure {
    pub(super) stage: &'static str,
    pub(super) item: Option<String>,
    pub(super) completed: usize,
}

impl Failure {
    pub(super) fn new(stage: &'static str) -> Self {
        Self {
            stage,
            item: None,
            completed: 0,
        }
    }

    pub(super) fn question(stage: &'static str, item: &str, completed: usize) -> Self {
        Self {
            stage,
            item: Some(item.to_owned()),
            completed,
        }
    }

    pub(super) fn from_error(stage: &'static str, error: impl fmt::Display) -> Self {
        log_cause(stage, error);
        Self::new(stage)
    }

    pub(super) fn question_from_error(
        stage: &'static str,
        item: &str,
        completed: usize,
        error: impl fmt::Display,
    ) -> Self {
        log_cause(stage, error);
        Self::question(stage, item, completed)
    }
}

fn log_cause(stage: &str, error: impl fmt::Display) {
    let mut cause = error.to_string();
    let mut private_paths = vec![env!("CARGO_MANIFEST_DIR").to_owned()];
    private_paths.extend(
        ["HOME", "USERPROFILE", "TMPDIR"]
            .into_iter()
            .filter_map(env::var_os)
            .map(|path| path.to_string_lossy().into_owned()),
    );
    private_paths.push(env::temp_dir().to_string_lossy().into_owned());
    for path in private_paths {
        if !path.is_empty() {
            cause = cause.replace(&path, "<path>");
        }
    }
    eprintln!("synthetic gate stage {stage} failed: {cause}");
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "synthetic gate failed at {}", self.stage)?;
        if let Some(item) = &self.item {
            write!(formatter, " for item {item}")?;
        }
        write!(formatter, " after {} completed items", self.completed)
    }
}
