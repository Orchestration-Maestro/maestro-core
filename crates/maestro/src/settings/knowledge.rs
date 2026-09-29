//! The knowledge operations' settings, read from a session's resolved
//! values: what CLI search, CLI ask and the MCP server's tools run with.
//! The settings port of the knowledge operations: they take this value and
//! never read a file, a flag or the registry themselves. Every default is
//! today's behaviour ([`KnowledgeSettings::default`], pinned by a test).
//! Evaluation runs never take it.

use maestro_kernel::evidence::RequestBudget;
use maestro_knowledge::{
    answer::{AnswerPrompt, AskBudget, DEFAULT_MODEL, Presentation, PromptVersion, Tone},
    prepare::ChunkProfile,
    search::{
        CandidateContext, IntentExpansion, IntentTrigger, SearchConfiguration, SectionClassSet,
        SectionPrior, SourceClassSet, SourcePrior, StageWindow,
        evidence::{CounterMode, EvidenceSettings, ExpansionMode, ParentChainOrder},
    },
};
use maestro_settings::{AUTO, Resolved};
use serde::de::DeserializeOwned;
use std::{
    cell::RefCell,
    collections::BTreeSet,
    num::{NonZeroU32, NonZeroUsize},
    time::Duration,
};

/// Where models run (`models.compute`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Compute {
    /// Code only: no model call. Search runs its code routes without
    /// reranking; ask, prepare and publish refuse.
    Off,
    /// The machine's GPU backend, as today.
    #[default]
    Gpu,
}

/// The settings the knowledge operations run with.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct KnowledgeSettings {
    /// How searches, and the searches of asks, run.
    pub(crate) search: SearchConfiguration,
    /// How their evidence is assembled.
    pub(crate) evidence: EvidenceSettings,
    /// A search's bounds when its request gives none.
    pub(crate) search_budget: RequestBudget,
    /// An ask's bounds when its request gives none.
    pub(crate) ask_budget: AskBudget,
    /// The answerer's router entry when an ask names none.
    pub(crate) model: String,
    /// The answer prompt, in the session's language and tone.
    pub(crate) prompt: AnswerPrompt,
    /// Where models run.
    pub(crate) compute: Compute,
    /// The chunking profile of prepare, and of publish without a chunk set.
    pub(crate) chunk_profile: ChunkProfile,
}

impl Default for KnowledgeSettings {
    /// Today's behaviour, before settings existed.
    fn default() -> Self {
        Self {
            search: SearchConfiguration::default(),
            evidence: EvidenceSettings::default(),
            search_budget: RequestBudget {
                evidence_bytes: RequestBudget::DEFAULT_SEARCH_EVIDENCE_BYTES,
                ..RequestBudget::default()
            },
            ask_budget: AskBudget::default(),
            model: DEFAULT_MODEL.to_owned(),
            prompt: AnswerPrompt::Presented {
                version: PromptVersion::default(),
                presentation: Presentation::default(),
            },
            compute: Compute::Gpu,
            chunk_profile: ChunkProfile::CompleteIdeas,
        }
    }
}

impl KnowledgeSettings {
    /// The settings `resolved` gives.
    ///
    /// # Errors
    ///
    /// The key of a setting that is missing or of another kind than read.
    pub(crate) fn from_resolved(resolved: &Resolved<'_>) -> Result<Self, String> {
        Self::read(resolved).map(|(settings, _)| settings)
    }

    /// The settings `resolved` gives, and the key of every setting read to
    /// make them.
    ///
    /// # Errors
    ///
    /// As [`KnowledgeSettings::from_resolved`].
    pub(crate) fn read(resolved: &Resolved<'_>) -> Result<(Self, BTreeSet<String>), String> {
        let read = Reader {
            resolved,
            keys: RefCell::default(),
        };
        let compute = if read.text("models.compute")? == "off" {
            Compute::Off
        } else {
            Compute::Gpu
        };
        let models = compute == Compute::Gpu;
        let mut search = read.search()?;
        search.dense_enabled &= models;
        search.rerank_enabled &= models;
        if !models {
            search.intent_expansion = IntentExpansion::Off;
        }
        let expansion = read.named::<ExpansionMode>("evidence.expansion")?;
        let parent_chain_order = match read.text("evidence.parent_chain_order")? {
            "off" => None,
            "minimum_complete_first" if expansion != ExpansionMode::ParentChain => None,
            _ => Some(read.named::<ParentChainOrder>("evidence.parent_chain_order")?),
        };
        let settings = Self {
            search,
            evidence: EvidenceSettings {
                parent_chain_order,
                expansion,
                evidence_counter: read.named::<CounterMode>("evidence.counter")?,
            },
            search_budget: RequestBudget {
                k: read.whole("search.k")?,
                evidence_bytes: read.whole("search.evidence_bytes")?,
                deadline_ms: read.whole("search.deadline_ms")?,
            },
            ask_budget: AskBudget {
                k: read.whole("ask.k")?,
                evidence_bytes: read.whole("ask.evidence_bytes")?,
                search_deadline_ms: read.whole("ask.search_deadline_ms")?,
                output_tokens: read.optional_whole("ask.output_tokens")?,
            },
            model: read.text("ask.model")?.to_owned(),
            prompt: AnswerPrompt::Presented {
                version: read.named::<PromptVersion>("ask.prompt")?,
                presentation: read.presentation()?,
            },
            compute,
            chunk_profile: ChunkProfile::named(read.text("chunking.profile")?)
                .ok_or_else(|| "chunking.profile".to_owned())?,
        };
        Ok((settings, read.keys.into_inner()))
    }
}

/// Typed reads of resolved values, each failing with its key.
struct Reader<'resolved, 'registry> {
    /// The values read.
    resolved: &'resolved Resolved<'registry>,
    /// The key of each value read so far.
    keys: RefCell<BTreeSet<String>>,
}

impl Reader<'_, '_> {
    /// The search configuration.
    fn search(&self) -> Result<SearchConfiguration, String> {
        let rerank_depth: usize = self.whole("search.rerank.depth")?;
        let fusion_pool: usize = self.whole("search.fusion_pool")?;
        let identifier_limit: usize = self.whole("search.routes.identifier_limit")?;
        if rerank_depth > fusion_pool {
            return Err(format!(
                "search.rerank.depth ({rerank_depth}) must not exceed \
                 search.fusion_pool ({fusion_pool})"
            ));
        }
        Ok(SearchConfiguration {
            routes_limit: self.whole("search.routes.limit")?,
            identifier_limit,
            fusion_pool,
            dense_enabled: self.flag("search.routes.dense")?,
            lexical_enabled: self.flag("search.routes.lexical")?,
            identifier_enabled: self.flag("search.routes.identifier")?,
            structured_enabled: self.flag("search.routes.structured")?,
            identifier_noise_guard: self.flag("search.identifier.noise_guard")?,
            rrf_k: NonZeroU32::new(self.whole("search.rrf_k")?)
                .ok_or_else(|| "search.rrf_k".to_owned())?,
            dense_weight: self.number("search.weights.dense")?,
            lexical_weight: self.number("search.weights.lexical")?,
            identifier_weight: self.number("search.weights.identifier")?,
            structured_weight: self.number("search.weights.structured")?,
            rerank_enabled: self.flag("search.rerank.enabled")?,
            rerank_depth: NonZeroUsize::new(rerank_depth)
                .ok_or_else(|| "search.rerank.depth".to_owned())?,
            min_rerank_score: self.optional_number("ask.min_rerank_score")?.map(narrow),
            rerank_blend: self.optional_number("search.rerank.blend")?.map(narrow),
            rerank_demotion_cap: self.optional_whole("search.rerank.demotion_cap")?,
            candidate_context: self.candidate_context()?,
            section_prior: self.section_prior()?,
            stage_window: self
                .optional_whole("search.stage_window_ms")?
                .map_or(StageWindow::Derived, |window| {
                    StageWindow::Fixed(Duration::from_millis(window))
                }),
            intent_expansion: self.named::<IntentExpansion>("search.intent.expansion")?,
            intent_trigger: self
                .optional_number("search.intent.min_top_rerank")?
                .map_or(IntentTrigger::Always, |min_top_rerank| {
                    IntentTrigger::LowConfidence { min_top_rerank }
                }),
            intent_deadline_ms: self.whole("search.intent.deadline_ms")?,
            intent_weight: self.number("search.intent.weight")?,
            intent_rerank_additions: self.whole("search.intent.rerank_additions")?,
            source_prior: self.source_prior()?,
        })
    }

    /// What the reranker reads.
    fn candidate_context(&self) -> Result<CandidateContext, String> {
        Ok(match self.text("search.rerank.context")? {
            "bounded_section" => CandidateContext::BoundedSection {
                max_bytes: self.whole("search.rerank.context_max_bytes")?,
            },
            _ => CandidateContext::Chunk,
        })
    }

    /// The section prior: off, or soft over its classes.
    fn section_prior(&self) -> Result<SectionPrior, String> {
        let Some(weight) = self.optional_number("search.section_prior.weight")? else {
            return Ok(SectionPrior::Off);
        };
        let mut classes = SectionClassSet::default();
        self.classes("search.section_prior.classes", |class| {
            classes.insert(class)
        })?;
        Ok(SectionPrior::Soft {
            weight: narrow(weight),
            classes,
        })
    }

    /// The source prior: off, or soft over its classes.
    fn source_prior(&self) -> Result<SourcePrior, String> {
        let Some(weight) = self.optional_number("search.source_prior.weight")? else {
            return Ok(SourcePrior::Off);
        };
        let mut classes = SourceClassSet::default();
        self.classes("search.source_prior.classes", |class| classes.insert(class))?;
        Ok(SourcePrior::Soft {
            weight: narrow(weight),
            classes,
        })
    }

    /// Each class of the list `key`, given to `insert`, which refuses a
    /// class it cannot take.
    fn classes(&self, key: &str, mut insert: impl FnMut(&str) -> bool) -> Result<(), String> {
        self.keys.borrow_mut().insert(key.to_owned());
        for class in self.resolved.list(key).ok_or_else(|| key.to_owned())? {
            if !insert(class) {
                return Err(key.to_owned());
            }
        }
        Ok(())
    }

    /// The answer's language and tone.
    fn presentation(&self) -> Result<Presentation, String> {
        let language = self.text("language")?;
        let tone = self.text("tone")?;
        Ok(Presentation {
            language: (language != AUTO).then(|| language.to_owned()),
            tone: [Tone::Brief, Tone::Normal, Tone::Detailed]
                .into_iter()
                .find(|each| each.name() == tone)
                .ok_or_else(|| "tone".to_owned())?,
        })
    }

    /// The flag of `key`.
    fn flag(&self, key: &str) -> Result<bool, String> {
        self.keys.borrow_mut().insert(key.to_owned());
        self.resolved.flag(key).ok_or_else(|| key.to_owned())
    }

    /// The number of `key`.
    fn number(&self, key: &str) -> Result<f64, String> {
        self.keys.borrow_mut().insert(key.to_owned());
        self.resolved.number(key).ok_or_else(|| key.to_owned())
    }

    /// The number of `key`, `None` when it is off.
    fn optional_number(&self, key: &str) -> Result<Option<f64>, String> {
        if self.resolved.is_off(key) {
            self.keys.borrow_mut().insert(key.to_owned());
            return Ok(None);
        }
        self.number(key).map(Some)
    }

    /// The whole number of `key`, in the type its consumer takes.
    fn whole<T: TryFrom<i64>>(&self, key: &str) -> Result<T, String> {
        self.keys.borrow_mut().insert(key.to_owned());
        self.resolved
            .integer(key)
            .and_then(|integer| T::try_from(integer).ok())
            .ok_or_else(|| key.to_owned())
    }

    /// The whole number of `key`, `None` when it is off.
    fn optional_whole<T: TryFrom<i64>>(&self, key: &str) -> Result<Option<T>, String> {
        if self.resolved.is_off(key) {
            self.keys.borrow_mut().insert(key.to_owned());
            return Ok(None);
        }
        self.whole(key).map(Some)
    }

    /// The text of `key`.
    fn text(&self, key: &str) -> Result<&str, String> {
        self.keys.borrow_mut().insert(key.to_owned());
        self.resolved.text(key).ok_or_else(|| key.to_owned())
    }

    /// The value its serde name `key` holds names, as the knowledge types
    /// spell their variants.
    fn named<T: DeserializeOwned>(&self, key: &str) -> Result<T, String> {
        serde_json::from_value(self.text(key)?.into()).map_err(|_| key.to_owned())
    }
}

/// `number` as the single precision the search configuration keeps: every
/// such setting's range is small.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the settings read as f32 range within -100 and 100"
)]
const fn narrow(number: f64) -> f32 {
    number as f32
}
