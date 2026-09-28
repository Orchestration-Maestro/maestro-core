//! The ladder's score as a Markdown table.

use super::ladder::{Floor, FloorResult, FloorStatus, LadderScore, LanguageCounts, Measure};
use std::fmt::Write as _;

impl LadderScore {
    /// The score as a Markdown table, one row per floor, then the supported
    /// answers, the false refusals, the failed searches and asks, the rows
    /// missing or rejected, and a table of counts per language.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut table = String::from("| Floor | Measured | Target | Status |\n");
        table.push_str("| --- | --- | --- | --- |\n");
        for result in &self.floors {
            let (measured, target) = describe(result);
            let _ = writeln!(
                table,
                "| {} | {measured} | {target} | {} |",
                title(result.floor),
                status(result.status)
            );
        }
        let questions = self.floors.iter().find_map(|result| match result.measure {
            Measure::Latency { of, .. } => Some(of),
            Measure::Share { .. } | Measure::Literals { .. } | Measure::NotRun { .. } => None,
        });
        let questions = questions.unwrap_or_default();
        let rejected = if self.rejected_ids.is_empty() {
            "none".to_owned()
        } else {
            self.rejected_ids.join(", ")
        };
        let failed_asks = if self.asked {
            format!("{}/{questions}", self.failed_asks)
        } else {
            "not run".to_owned()
        };
        let _ = write!(
            table,
            "\nSupported answers: {}/{}\nFalse refusals: {}/{}\n\
             Failed searches: {}/{questions}\nFailed asks: {failed_asks}\n\
             Missing rows: {}\nRejected rows: {rejected}\nAll floors: {}\n",
            self.supported_answers,
            self.answerable,
            self.false_refusals,
            self.answerable,
            self.failed_searches,
            self.missing,
            if self.passed { "PASS" } else { "FAIL" }
        );
        if !self.languages.is_empty() {
            table.push_str(
                "\n| Language | Top-10 | Refused | False refusals | Supported answers |\n\
                 | --- | --- | --- | --- | --- |\n",
            );
            for language in &self.languages {
                table.push_str(&language_row(language));
            }
        }
        table
    }
}

/// `counts` as a row of the table per language.
fn language_row(counts: &LanguageCounts) -> String {
    let of = |count: Option<usize>, of: usize| {
        count.map_or_else(|| "not run".to_owned(), |count| format!("{count}/{of}"))
    };
    format!(
        "| {} | {}/{} | {} | {} | {} |\n",
        counts.language,
        counts.top_10,
        counts.answerable,
        of(counts.refused, counts.unanswerable),
        of(counts.false_refusals, counts.answerable),
        of(counts.supported_answers, counts.answerable)
    )
}

/// The title of `floor` in the table.
const fn title(floor: Floor) -> &'static str {
    match floor {
        Floor::Top10 => "Right document top-10",
        Floor::Top1 => "Right document first",
        Floor::Refused => "Unanswerable refused",
        Floor::Citation => "Right-section citation",
        Floor::Answered => "Answerable answered right",
        Floor::Literals => "Invented literals",
        Floor::SearchP95 => "Search p95",
        Floor::AskP95 => "Ask p95",
    }
}

/// `status` in the table.
const fn status(status: FloorStatus) -> &'static str {
    match status {
        FloorStatus::Pass => "PASS",
        FloorStatus::Fail => "FAIL",
        FloorStatus::Unavailable => "UNAVAILABLE",
    }
}

/// What `result` measured and its target, as the table shows them.
fn describe(result: &FloorResult) -> (String, String) {
    match result.measure {
        Measure::Share {
            count,
            of,
            percent,
            required,
        } => {
            let measured = if of == 0 {
                "0/0".to_owned()
            } else {
                format!("{count}/{of} ({})", tenths_of_percent(count, of))
            };
            (measured, format!(">= {percent}% ({required}/{of})"))
        }
        Measure::Literals { invented, .. } => (invented.to_string(), "0".to_owned()),
        Measure::NotRun { .. } => ("not run".to_owned(), "not run".to_owned()),
        Measure::Latency {
            p95_us, limit_us, ..
        } => (
            p95_us.map_or_else(|| "not ended".to_owned(), milliseconds),
            format!("<= {}", milliseconds(limit_us)),
        ),
    }
}

/// `count` over `of`, a positive number, in percent rounded half up to one
/// decimal.
fn tenths_of_percent(count: usize, of: usize) -> String {
    let tenths = (count * 1000 + of / 2) / of;
    format!("{}.{}%", tenths / 10, tenths % 10)
}

/// `micros` microseconds in milliseconds, to the microsecond.
fn milliseconds(micros: u64) -> String {
    format!("{}.{:03} ms", micros / 1000, micros % 1000)
}
