//! The first table rule (FR-S2-004, plan A0): `maestro-graph-table-rule/1`,
//! a closed, data-only object bound to one source digest, that turns each
//! body row of the one table under an exact heading path into a
//! `DEFAULTS_TO` claim. Its subject, type and lexeme come from the columns it
//! names, never from their order; its object is a typed literal with the
//! source's lexeme unchanged, never an entity. It runs no script and holds no
//! expression, and a rule file with any key it does not name, or one key
//! twice, is refused, so the test envelope and its `expected` oracle are
//! never a rule.
//!
//! A rule is one [`Extractor`]: what it cannot turn into a claim it rejects
//! with its reason, and the claims it extracts go to the kernel's one write
//! path, which verifies them again.

use super::{
    structure::{Row, Table, tables},
    verify::{Source, check_source, locate},
};
use crate::shape;

pub use super::resolve::{Resolution, resolve};
use maestro_kernel::{
    artifact::Digest,
    facts::{
        Claim, EntityKind, EntityName, Literal, LiteralKind, Object, Predicate, Provenance,
        Support, Validity,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    error, fmt,
};

/// A strict table rule, checked: every name nonempty, the columns distinct,
/// and each role bound to a distinct declared column. [`TableRule::parse`]
/// alone makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRule(RuleFile);

/// A rule file's closed fields, as read before they are checked.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleFile {
    /// The contract it follows.
    #[serde(deserialize_with = "shape::name")]
    schema: Schema,
    /// Its identity, the extractor its claims name.
    id: String,
    /// The SHA-256 of the one original Markdown it reads.
    #[serde(deserialize_with = "shape::digest")]
    source_sha256: Digest,
    /// The exact heading path of its table.
    heading_path: Vec<String>,
    /// The table's header cells, in order.
    columns: Vec<String>,
    /// The column holding the subject's spelling.
    subject_column: String,
    /// The column holding the literal's type.
    type_column: String,
    /// The column holding the literal's lexeme.
    lexeme_column: String,
    /// The kind of every subject.
    subject_kind: String,
    /// What each claim says.
    #[serde(deserialize_with = "shape::name")]
    predicate: PredicateName,
}

/// The one contract a table rule follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Schema {
    /// `maestro-graph-table-rule/1`.
    #[serde(rename = "maestro-graph-table-rule/1")]
    V1,
}

/// The predicates a table rule may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum PredicateName {
    /// `DEFAULTS_TO`.
    #[serde(rename = "DEFAULTS_TO")]
    DefaultsTo,
}

/// Why a rule file is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleError(String);

/// What extracts claims from a source: a rule, or later a model.
pub trait Extractor {
    /// Its name and the digest of its rule or profile, which every claim it
    /// extracts carries.
    fn provenance(&self) -> Provenance;

    /// The claims `source` gives, and what it rejected.
    fn extract(&self, source: &Source) -> Extraction;
}

/// The claims an extractor found in a source, in source order, and what it
/// rejected there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extraction {
    /// The candidate claims, each with its located support.
    pub claims: Vec<Claim>,
    /// What gave no claim, and why.
    pub rejections: Vec<Rejection>,
}

/// A candidate refused, kept with its reason for review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rejection {
    /// The revision it came from.
    pub revision_id: String,
    /// The canonical block it came from, when it came from one.
    pub block_id: Option<String>,
    /// Why, for people.
    pub reason: String,
}

/// A body row's block, the subject its canonical cell names, if it names
/// one, and its claim or why it has none.
type Candidate = (String, Option<String>, Result<Claim, String>);

impl RuleFile {
    /// Checks the rules its shape alone cannot.
    fn check(&self) -> Result<(), &'static str> {
        if self.id.is_empty() {
            return Err("the rule's id is empty");
        }
        if EntityKind::parse(&self.subject_kind).is_none() {
            return Err("the subject kind is outside the closed vocabulary");
        }
        if self.heading_path.is_empty() || self.heading_path.iter().any(String::is_empty) {
            return Err("the heading path is empty or holds an empty heading");
        }
        if self.columns.is_empty() {
            return Err("the rule declares no column");
        }
        if self.columns.iter().any(String::is_empty) {
            return Err("the rule declares an empty column");
        }
        if self.columns.iter().collect::<BTreeSet<_>>().len() < self.columns.len() {
            return Err("the rule declares a column twice");
        }
        let roles = [&self.subject_column, &self.type_column, &self.lexeme_column];
        if roles.iter().any(|role| !self.columns.contains(role)) {
            return Err("a role names what is not a declared column");
        }
        if roles.iter().collect::<BTreeSet<_>>().len() < roles.len() {
            return Err("the subject, type and lexeme columns are not distinct");
        }
        Ok(())
    }
}

impl TableRule {
    /// The rule `text` holds: one JSON object, and nothing after it.
    ///
    /// # Errors
    ///
    /// [`RuleError`] when it is not a strict table rule.
    pub fn parse(text: &str) -> Result<Self, RuleError> {
        let rule: RuleFile = shape::parse(text).map_err(|error| RuleError(error.to_string()))?;
        rule.check()
            .map_err(|reason| RuleError(reason.to_owned()))?;
        Ok(Self(rule))
    }

    /// Its id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.0.id
    }

    /// The digest of the one original it reads.
    #[must_use]
    pub fn source_sha256(&self) -> &Digest {
        &self.0.source_sha256
    }

    /// The SHA-256 of its content in a canonical form: a change to any of
    /// its fields changes it, and its file's layout does not.
    #[must_use]
    pub fn profile(&self) -> Digest {
        let schema = match self.0.schema {
            Schema::V1 => "maestro-graph-table-rule/1",
        };
        let predicate = self.predicate().as_str();
        let form = json!([
            schema,
            self.0.id,
            self.0.source_sha256.as_str(),
            self.0.heading_path,
            self.0.columns,
            self.0.subject_column,
            self.0.type_column,
            self.0.lexeme_column,
            self.0.subject_kind,
            predicate,
        ]);
        Digest::of(form.to_string().as_bytes())
    }

    /// The predicate it names.
    fn predicate(&self) -> Predicate {
        match self.0.predicate {
            PredicateName::DefaultsTo => Predicate::DefaultsTo,
        }
    }

    /// The claim of the body row `cells` locate, or why it has none.
    fn claim(&self, cells: &[&str], support: Support) -> Result<Claim, String> {
        let cell = |column: &String| {
            self.0
                .columns
                .iter()
                .position(|name| name == column)
                .and_then(|index| cells.get(index).copied())
                .unwrap_or_default()
        };
        let (subject, type_name, lexeme) = (
            cell(&self.0.subject_column),
            cell(&self.0.type_column),
            cell(&self.0.lexeme_column),
        );
        if subject.is_empty() {
            return Err("the subject is empty".to_owned());
        }
        if lexeme.is_empty() {
            return Err(format!("the default of {subject:?} is empty"));
        }
        let kind = LiteralKind::parse(type_name)
            .ok_or_else(|| format!("the type {type_name:?} is not a literal type"))?;
        if !kind.admits(lexeme) {
            return Err(format!(
                "the default {lexeme:?} does not read as {type_name}"
            ));
        }
        Ok(Claim {
            subject: EntityName {
                kind: EntityKind::parse(&self.0.subject_kind)
                    .ok_or("the subject kind is outside the closed vocabulary")?,
                name: subject.to_owned(),
            },
            predicate: self.predicate(),
            object: Object::Literal(Literal {
                kind,
                lexeme: lexeme.to_owned(),
            }),
            conditions: BTreeMap::new(),
            version: Validity::Unknown,
            world: Validity::Unknown,
            provenance: self.provenance(),
            supports: vec![support],
        })
    }

    /// The candidates of the body rows of `table`, once its header holds
    /// this rule's columns.
    fn rows(&self, source: &Source, table: &Table<'_>) -> Result<Vec<Candidate>, String> {
        let Some((header, body)) = table.rows.split_first() else {
            return Err("the table has no header".to_owned());
        };
        let located = locate(source, table.block, header.block, &header.cells)?;
        if located.cells != self.0.columns {
            return Err(format!(
                "the table's columns are {:?}, not the rule's {:?}",
                located.cells, self.0.columns
            ));
        }
        Ok(body
            .iter()
            .map(|row| {
                let claim = self.row_claim(source, table, row);
                (row.block.block_id.clone(), self.subject_of(row), claim)
            })
            .collect())
    }

    /// The subject the canonical cell of `row` in the subject column names,
    /// whether or not the row gives a claim; none when that cell is missing
    /// or empty.
    fn subject_of(&self, row: &Row<'_>) -> Option<String> {
        let column = self
            .0
            .columns
            .iter()
            .position(|name| *name == self.0.subject_column)?;
        let text = &row.cells.get(column)?.retrieval_text;
        (!text.is_empty()).then(|| text.clone())
    }

    /// The claim of the body row `row` of `table`, or why it has none.
    fn row_claim(
        &self,
        source: &Source,
        table: &Table<'_>,
        row: &Row<'_>,
    ) -> Result<Claim, String> {
        let located = locate(source, table.block, row.block, &row.cells)?;
        if located.cells.len() != self.0.columns.len() {
            return Err(format!(
                "the row has {} cells, not the rule's {} columns",
                located.cells.len(),
                self.0.columns.len()
            ));
        }
        self.claim(&located.cells, located.support)
    }
}

impl Extractor for TableRule {
    fn provenance(&self) -> Provenance {
        Provenance {
            extractor: self.0.id.clone(),
            profile: self.profile(),
        }
    }

    fn extract(&self, source: &Source) -> Extraction {
        let mut extraction = Extraction::default();
        let reject = |block: Option<&str>, reason: String| Rejection {
            revision_id: source.revision_id().to_owned(),
            block_id: block.map(str::to_owned),
            reason,
        };
        if let Err(reason) = check_source(source, &self.0.source_sha256) {
            extraction.rejections.push(reject(None, reason));
            return extraction;
        }
        let selected: Vec<Table<'_>> = tables(source.canonical())
            .into_iter()
            .filter(|table| table.block.heading_path == self.0.heading_path)
            .collect();
        let [table] = selected.as_slice() else {
            let reason = |count: usize| {
                format!(
                    "{count} tables lie under the heading path {:?}",
                    self.0.heading_path
                )
            };
            if selected.is_empty() {
                let reason = format!("no table under the heading path {:?}", self.0.heading_path);
                extraction.rejections.push(reject(None, reason));
            }
            for table in &selected {
                let block = Some(table.block.block_id.as_str());
                extraction
                    .rejections
                    .push(reject(block, reason(selected.len())));
            }
            return extraction;
        };
        let candidates = match self.rows(source, table) {
            Ok(candidates) => candidates,
            Err(reason) => {
                let block = Some(table.block.block_id.as_str());
                extraction.rejections.push(reject(block, reason));
                return extraction;
            }
        };
        admit_rows(source, candidates, &mut extraction);
        extraction
    }
}

/// Adds to `extraction` the claims of `candidates`, rows of one table of
/// `source`, and rejects the others with their reason. A subject named by
/// more than one row, good or bad, is refused whole: each of its rows is
/// rejected, a bad one with its own reason, a good one as ambiguous.
fn admit_rows(source: &Source, candidates: Vec<Candidate>, extraction: &mut Extraction) {
    let mut rows = BTreeMap::<String, usize>::new();
    for subject in candidates
        .iter()
        .filter_map(|(_, subject, _)| subject.clone())
    {
        *rows.entry(subject).or_default() += 1;
    }
    for (block_id, subject, candidate) in candidates {
        let count = subject
            .as_ref()
            .and_then(|subject| rows.get(subject))
            .copied()
            .unwrap_or_default();
        let reason = match candidate {
            Ok(claim) if count <= 1 => {
                extraction.claims.push(claim);
                continue;
            }
            Ok(claim) => format!(
                "the subject {:?} is ambiguous: it has {count} rows",
                claim.subject.name
            ),
            Err(reason) => reason,
        };
        extraction.rejections.push(Rejection {
            revision_id: source.revision_id().to_owned(),
            block_id: Some(block_id),
            reason,
        });
    }
}

impl fmt::Display for RuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "the table rule is refused: {}", self.0)
    }
}

impl error::Error for RuleError {}
