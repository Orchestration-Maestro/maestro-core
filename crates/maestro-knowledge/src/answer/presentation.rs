//! How an answer speaks: the session's language and tone (the owner's
//! settings of 2026-09-28; S3 plan D6). Normal tone in the question's
//! language is today's prompt byte for byte. A tone adds one instruction to
//! the system text and changes prose only, never the citation, evidence or
//! refusal rules; an explicit language replaces "the language of the
//! question" with its canonical tag, quoted as data. The instructions are
//! versioned by [`PRESENTATION_VERSION`]: a change to their text bumps it,
//! and an evaluation never runs a presented prompt.

use serde_json::Value;

/// The version of the presentation instructions' text.
pub const PRESENTATION_VERSION: &str = "presentation/1";

/// The phrase of every version's system text that an explicit language
/// replaces.
const QUESTION_LANGUAGE: &str = "in the language of the question";

/// The instruction of the brief tone.
const BRIEF: &str = "Keep the answer brief: the fewest full sentences that answer the question \
                     completely, each still ending with its marker.";

/// The instruction of the detailed tone, displayed as "Very detailed".
const DETAILED: &str = "Give a detailed answer: explain each step, condition and prerequisite \
                        the passages state, each sentence still ending with its marker.";

/// How much an answer explains.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tone {
    /// The shortest complete answer.
    Brief,
    /// Today's answers.
    #[default]
    Normal,
    /// More explanation; the owner's "super explainer".
    Detailed,
}

impl Tone {
    /// Its name, as the `tone` setting writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::Normal => "normal",
            Self::Detailed => "detailed",
        }
    }

    /// The instruction it adds to the system text, none for normal.
    const fn instruction(self) -> Option<&'static str> {
        match self {
            Self::Brief => Some(BRIEF),
            Self::Normal => None,
            Self::Detailed => Some(DETAILED),
        }
    }
}

/// The language and tone of an answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presentation {
    /// The canonical language tag the answer is written in, validated by the
    /// caller; `None` answers in the question's language, as S1 does.
    pub language: Option<String>,
    /// How much it explains.
    pub tone: Tone,
}

impl Presentation {
    /// Its identity, for an explanation: the instructions' version, the
    /// language and the tone.
    #[must_use]
    pub fn identity(&self) -> String {
        format!(
            "{PRESENTATION_VERSION} language={} tone={}",
            self.language.as_deref().unwrap_or("auto"),
            self.tone.name()
        )
    }

    /// The version's system text `system`, presented.
    pub(super) fn system(&self, system: &str) -> String {
        let mut presented = match &self.language {
            // A string serialized to JSON is a quoted, escaped literal.
            Some(tag) => system.replacen(
                QUESTION_LANGUAGE,
                &format!(
                    "in the language whose BCP 47 tag is {}",
                    Value::from(tag.as_str())
                ),
                1,
            ),
            None => system.to_owned(),
        };
        if let Some(instruction) = self.tone.instruction() {
            presented.push(' ');
            presented.push_str(instruction);
        }
        presented
    }
}
