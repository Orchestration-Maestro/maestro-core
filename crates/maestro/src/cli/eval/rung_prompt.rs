//! A rung's answer prompt: a prompt version, or a private prompt file of
//! `system` and `user` texts, read and checked with the manifest, whose
//! SHA-256 the rung's provenance records.

use crate::failure::Failure;
use maestro_kernel::artifact::Digest;
use maestro_knowledge::answer::{AnswerPrompt, PromptText, PromptVersion};
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A rung's answer prompt, as its manifest writes it: `"v1"`, `"v2"` or
/// `{"file": "<path>"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(super) enum RungPrompt {
    /// The constant texts of a prompt version.
    Version(PromptVersion),
    /// The texts of a private prompt file.
    File(PromptFile),
}

impl Default for RungPrompt {
    fn default() -> Self {
        Self::Version(PromptVersion::default())
    }
}

impl<'de> Deserialize<'de> for RungPrompt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            name @ Value::String(_) => PromptVersion::deserialize(name).map(Self::Version),
            file => PromptFile::deserialize(file).map(Self::File),
        }
        .map_err(de::Error::custom)
    }
}

/// A private prompt file, holding the JSON object of `system` and `user`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PromptFile {
    /// Its path; relative, it resolves from the manifest's directory.
    pub(super) file: PathBuf,
    /// Its texts and digest, once the manifest read them.
    #[serde(skip)]
    loaded: Option<LoadedPrompt>,
}

/// A prompt file's checked texts and the SHA-256 of its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedPrompt {
    /// Its texts.
    text: PromptText,
    /// The digest of its bytes.
    digest: Digest,
}

/// A prompt file's content.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PromptTexts {
    /// The system instruction.
    system: String,
    /// The user instruction, holding the `{data}` slot once.
    user: String,
}

impl RungPrompt {
    /// Its name in a report: the version's, or `file`.
    pub(super) const fn name(&self) -> &'static str {
        match self {
            Self::Version(version) => version.name(),
            Self::File(_) => "file",
        }
    }

    /// The SHA-256 of its file once read, in hexadecimal; none for a
    /// version.
    pub(super) fn digest(&self) -> Option<String> {
        match self {
            Self::Version(_) => None,
            Self::File(file) => file
                .loaded
                .as_ref()
                .map(|loaded| loaded.digest.as_str().to_owned()),
        }
    }

    /// The prompt the answerer is given; none for a file not read yet.
    pub(super) fn answer_prompt(&self) -> Option<AnswerPrompt> {
        match self {
            Self::Version(version) => Some((*version).into()),
            Self::File(file) => file
                .loaded
                .as_ref()
                .map(|loaded| AnswerPrompt::Text(loaded.text.clone())),
        }
    }
}

impl PromptFile {
    /// Reads and checks the file at `base` joined with its path, for the
    /// rung `rung`.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] for a file that cannot be read, is not a JSON
    /// object of `system` and `user`, or whose user text does not hold the
    /// `{data}` slot exactly once.
    pub(super) fn read(&mut self, base: &Path, rung: &str) -> Result<(), Failure> {
        self.file = base.join(&self.file);
        let bytes = fs::read(&self.file).map_err(|error| {
            Failure::refused(format!(
                "the rung `{rung}`'s prompt file cannot be read: {error}"
            ))
        })?;
        let refused = |reason: String| {
            Failure::refused(format!(
                "the rung `{rung}`'s prompt file is refused: {reason}"
            ))
        };
        let texts: PromptTexts =
            serde_json::from_slice(&bytes).map_err(|error| refused(error.to_string()))?;
        let text = PromptText::new(texts.system, texts.user)
            .map_err(|error| refused(error.to_string()))?;
        self.loaded = Some(LoadedPrompt {
            text,
            digest: Digest::of(&bytes),
        });
        Ok(())
    }
}
