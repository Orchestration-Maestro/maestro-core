//! Source classes: the port that names where a document comes from, and the
//! soft prior that ranks the configured classes after official pages.
//!
//! The classifier is a port: [`SourceClassTable`](super::SourceClassTable)
//! is its default adapter, and a search without one ranks as before. The
//! prior reorders candidates; it never admits or removes one.

use super::deadline;
use maestro_kernel::{chunk_set::Chunk, retrieval::ReadControl, scope::ScopeSet, store::Database};
use std::{
    collections::{BTreeSet, HashMap},
    fmt,
};

/// The class vocabulary: a class is its index here, and names only these.
const CLASSES: &[&str] = &[
    "official_docs",
    "official_kb",
    "official_code",
    "community",
    "third_party",
    "internal_code",
];

/// The classes the default prior ranks after official pages: repository,
/// community and third-party documents.
const DEMOTED_BY_DEFAULT: &[&str] = &["official_code", "community", "third_party", "internal_code"];

/// The default prior's weight, chosen by the official-first measurement.
const DEFAULT_WEIGHT: f32 = 0.6;

/// One class of the vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceClass(u8);

impl SourceClass {
    /// The class `name` stands for, if the vocabulary has it.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        CLASSES
            .iter()
            .position(|class| *class == name)
            .and_then(|index| u8::try_from(index).ok())
            .map(Self)
    }

    /// Its stable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        CLASSES
            .get(usize::from(self.0))
            .copied()
            .unwrap_or_default()
    }
}

/// A set of classes; unknown names cannot enter it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceClassSet(u8);

impl SourceClassSet {
    /// Adds a known class, returning false without modification for an
    /// unknown name.
    pub fn insert(&mut self, name: &str) -> bool {
        let Some(class) = SourceClass::named(name) else {
            return false;
        };
        self.0 |= 1 << class.0;
        true
    }

    /// Whether `class` is in the set.
    #[must_use]
    pub const fn contains(self, class: SourceClass) -> bool {
        self.0 & (1 << class.0) != 0
    }
}

/// What the kernel records of a document's origin.
#[derive(Clone, Copy, Debug)]
pub struct SourceMetadata<'a> {
    /// The revision's `source_kind` metadata, when it has one.
    pub source_kind: Option<&'a str>,
    /// The document's origin URL, or `corpus-path:` and its path.
    pub source_ref: &'a str,
}

/// A document's class and the label a person reads for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classification {
    /// Its class.
    pub class: SourceClass,
    /// Its readable source label, such as a product's documentation name.
    pub label: String,
}

/// Names a document's source class; `None` leaves it unclassified, which no
/// prior penalizes.
pub trait SourceClassifier: fmt::Debug + Send + Sync {
    /// The class and label of the document `source` describes.
    fn classify(&self, source: &SourceMetadata<'_>) -> Option<Classification>;
}

/// The classification of revision `revision_id`, read through `scopes`;
/// `None` when it is unclassified or its records cannot be read.
pub fn classify_revision(
    classifier: &dyn SourceClassifier,
    database: &Database,
    scopes: &ScopeSet,
    revision_id: &str,
) -> Option<Classification> {
    let revision = database.revision(scopes, revision_id).ok()??;
    let document = database.document(scopes, &revision.document_id).ok()??;
    classifier.classify(&SourceMetadata {
        source_kind: revision
            .metadata
            .get("source_kind")
            .and_then(|kind| kind.as_str()),
        source_ref: &document.source_ref,
    })
}

/// A soft preference for official sources; official-first by default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SourcePrior {
    /// Keep today's order exactly.
    Off,
    /// Down-weight documents of the configured classes.
    Soft {
        /// Fraction of reciprocal-rank score removed, in 0..=1.
        weight: f32,
        /// The classes ranked after official pages.
        classes: SourceClassSet,
    },
}

impl Default for SourcePrior {
    fn default() -> Self {
        let mut classes = SourceClassSet::default();
        for name in DEMOTED_BY_DEFAULT {
            classes.insert(name);
        }
        Self::Soft {
            weight: DEFAULT_WEIGHT,
            classes,
        }
    }
}

impl SourcePrior {
    /// Whether its weight is finite and within 0..=1.
    #[must_use]
    pub fn is_valid(self) -> bool {
        match self {
            Self::Off => true,
            Self::Soft { weight, .. } => weight.is_finite() && (0.0..=1.0).contains(&weight),
        }
    }

    /// Whether it can penalize anything, so sources need classifying.
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Soft { classes, .. } if classes.0 != 0)
    }

    /// Whether a document of `class` is ranked after official pages.
    #[must_use]
    pub const fn penalizes(self, class: SourceClass) -> bool {
        matches!(self, Self::Soft { classes, .. } if classes.contains(class))
    }

    /// The reciprocal-rank multiplier of a penalized candidate.
    #[must_use]
    pub fn multiplier(self) -> Option<f64> {
        match self {
            Self::Off => None,
            Self::Soft { weight, .. } => Some(1.0 - f64::from(weight)),
        }
    }
}

/// The chunks among `chunks` whose source class `prior` penalizes, each
/// revision classified once; none without a classifier or an active prior.
/// Classification is optional enrichment: it stops when `control` closes,
/// and every chunk not classified by then keeps its rank.
pub(super) fn penalized<'a>(
    (database, scopes, control): (&Database, &ScopeSet, &ReadControl),
    classifier: Option<&dyn SourceClassifier>,
    prior: SourcePrior,
    chunks: impl IntoIterator<Item = &'a Chunk>,
) -> BTreeSet<String> {
    let Some(classifier) = classifier.filter(|_| prior.is_active()) else {
        return BTreeSet::new();
    };
    let mut revisions = HashMap::new();
    chunks
        .into_iter()
        .take_while(|_| deadline::open(control))
        .filter(|chunk| {
            *revisions
                .entry(chunk.revision_id.as_str())
                .or_insert_with(|| {
                    classify_revision(classifier, database, scopes, &chunk.revision_id)
                        .is_some_and(|found| prior.penalizes(found.class))
                })
        })
        .map(|chunk| chunk.id.clone())
        .collect()
}
