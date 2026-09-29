//! The settings Maestro ships, one descriptor each. Every default is
//! today's behaviour; the `maestro` crate's tests pin each against the value
//! its consumer used before settings existed. A later knob is one more entry.

use crate::descriptor::{ReservedValue, SettingClass, SettingDescriptor, SettingKind, Texts};
use std::borrow::Cow;

/// The choice values `$value`, as a descriptor's const texts.
macro_rules! texts {
    ($($value:literal),+ $(,)?) => {
        Cow::Borrowed(&[$(Cow::Borrowed($value)),+])
    };
}

/// A free setting of `key`, `kind` and `default`, described by
/// `description`.
const fn free(
    key: &'static str,
    kind: SettingKind,
    default: &'static str,
    description: &'static str,
) -> SettingDescriptor {
    SettingDescriptor {
        key: Cow::Borrowed(key),
        kind,
        default: Cow::Borrowed(default),
        description: Cow::Borrowed(description),
        class: SettingClass::Free,
    }
}

/// A whole number from `min` to `max`.
const fn integer(min: i64, max: i64) -> SettingKind {
    SettingKind::Integer {
        min,
        max,
        off: false,
    }
}

/// A whole number from `min` to `max`, or `off`.
const fn optional_integer(min: i64, max: i64) -> SettingKind {
    SettingKind::Integer {
        min,
        max,
        off: true,
    }
}

/// A number from `min` to `max`, `off` too when `off`.
const fn number(min: f64, max: f64, off: bool) -> SettingKind {
    SettingKind::Number { min, max, off }
}

/// One of `values`.
const fn choice(values: Texts) -> SettingKind {
    SettingKind::Choice {
        values,
        reserved: Cow::Borrowed(&[]),
    }
}

/// Each route's weight in reciprocal rank fusion: finite and nonnegative,
/// as search requires, bounded for a hand-edited file.
const WEIGHT: SettingKind = number(0.0, 100.0, false);

/// Every setting Maestro knows, grouped as `config list` shows them.
pub const BUILT_IN: &[SettingDescriptor] = &[
    free(
        "language",
        SettingKind::Language,
        "auto",
        "The conversation language: auto answers in the question's language; a tag such as \
         fr or es-419 answers in that one. Code and documentation stay in English.",
    ),
    free(
        "tone",
        choice(texts!["brief", "normal", "detailed"]),
        "normal",
        "How much an answer explains: brief, normal, or detailed (shown as Very detailed). \
         Prose only; code and documents are unchanged.",
    ),
    free(
        "search.k",
        integer(1, 50),
        "10",
        "The most passages a search returns when --k (the MCP k) is not given.",
    ),
    free(
        "search.evidence_bytes",
        integer(1, 24_000),
        "12000",
        "A search's evidence budget, in UTF-8 bytes, when --evidence-bytes is not given.",
    ),
    free(
        "search.deadline_ms",
        integer(1, 30_000),
        "30000",
        "A search's deadline in milliseconds when --deadline-ms is not given.",
    ),
    free(
        "search.routes.dense",
        SettingKind::Flag,
        "true",
        "Whether the dense (embedding) route runs, in searches and asks.",
    ),
    free(
        "search.routes.lexical",
        SettingKind::Flag,
        "true",
        "Whether the lexical route runs, in searches and asks.",
    ),
    free(
        "search.routes.identifier",
        SettingKind::Flag,
        "true",
        "Whether the exact-identifier route runs, in searches and asks.",
    ),
    free(
        "search.routes.structured",
        SettingKind::Flag,
        "true",
        "Whether the structured route may run for global questions, in searches and asks.",
    ),
    free(
        "search.routes.limit",
        integer(1, 120),
        "100",
        "The maximum dense, lexical and intent candidates; identifier retrieval also uses \
         its identifier limit.",
    ),
    free(
        "search.routes.identifier_limit",
        integer(1, 120),
        "20",
        "The maximum identifier candidates, additionally bounded by \
         search.routes.limit.",
    ),
    free(
        "search.identifier.noise_guard",
        SettingKind::Flag,
        "false",
        "Whether the identifier route drops identifiers too common to rank, and fusion \
         takes no hits from an unavailable route, in searches and asks.",
    ),
    free(
        "search.rrf_k",
        integer(1, 1000),
        "60",
        "The reciprocal rank fusion constant K.",
    ),
    free(
        "search.weights.dense",
        WEIGHT,
        "1.0",
        "The dense route's weight in fusion.",
    ),
    free(
        "search.weights.lexical",
        WEIGHT,
        "1.0",
        "The lexical route's weight in fusion.",
    ),
    free(
        "search.weights.identifier",
        WEIGHT,
        "1.0",
        "The exact-identifier route's weight in fusion.",
    ),
    free(
        "search.weights.structured",
        WEIGHT,
        "1.0",
        "The structured route's weight in fusion.",
    ),
    free(
        "search.rerank.enabled",
        SettingKind::Flag,
        "true",
        "Whether the reranker reorders the fused candidates.",
    ),
    free(
        "search.fusion_pool",
        integer(1, 120),
        "120",
        "The maximum candidates retained by fusion; rerank depth cannot exceed this value.",
    ),
    free(
        "search.rerank.depth",
        integer(1, 120),
        "30",
        "How many fused candidates the reranker reads.",
    ),
    free(
        "search.rerank.blend",
        number(0.0, 1.0, true),
        "off",
        "The fused position's weight in a fusion with the rerank order; off keeps the \
         rerank order.",
    ),
    free(
        "search.rerank.demotion_cap",
        optional_integer(0, 120),
        "off",
        "The most positions reranking may drop a fused top-10 candidate; off sets no cap.",
    ),
    free(
        "search.rerank.context",
        choice(texts!["chunk", "bounded_section"]),
        "chunk",
        "What the reranker reads: the indexed chunk, or its whole section when that fits \
         search.rerank.context_max_bytes.",
    ),
    free(
        "search.rerank.context_max_bytes",
        integer(1, 1500),
        "1500",
        "The most bytes of section context the reranker reads under bounded_section.",
    ),
    free(
        "search.section_prior.weight",
        number(0.0, 1.0, true),
        "off",
        "How much of their fusion score the sections of search.section_prior.classes lose, \
         unless the question names them; off leaves the order as it is.",
    ),
    free(
        "search.section_prior.classes",
        SettingKind::ChoiceList {
            values: texts!["changelog", "release_notes", "conversion"],
        },
        "changelog,release_notes,conversion",
        "The section classes search.section_prior.weight applies to.",
    ),
    free(
        "search.stage_window_ms",
        optional_integer(1, 30_000),
        "off",
        "A fixed route window in milliseconds, for experiments; off derives it from the \
         deadline.",
    ),
    free(
        "search.intent.expansion",
        choice(texts!["off", "hyde"]),
        "off",
        "Whether a search adds a guarded hypothetical passage, written by the collection's \
         answerer, as extra retrieval routes: off, or hyde.",
    ),
    free(
        "search.intent.min_top_rerank",
        number(-100.0, 100.0, true),
        "off",
        "Under hyde, expand only when the best rerank score is below this one; off expands \
         every search.",
    ),
    free(
        "search.intent.deadline_ms",
        integer(1, 5000),
        "4000",
        "The most milliseconds the intent expansion may take, within the search deadline.",
    ),
    free(
        "search.intent.weight",
        WEIGHT,
        "1.0",
        "Each intent route's weight in fusion.",
    ),
    free(
        "search.intent.rerank_additions",
        integer(0, 120),
        "10",
        concat!(
            "The most candidates the intent routes may add beyond rerank depth; the combined ",
            "candidates can exceed search.fusion_pool."
        ),
    ),
    free(
        "search.source_prior.weight",
        number(0.0, 1.0, true),
        "0.6",
        "How much of their fusion score documents of search.source_prior.classes lose, so \
         official pages rank first; off leaves the order as it is.",
    ),
    free(
        "search.source_prior.classes",
        SettingKind::ChoiceList {
            values: texts![
                "official_docs",
                "official_kb",
                "official_code",
                "community",
                "third_party",
                "internal_code"
            ],
        },
        "official_code,community,third_party,internal_code",
        "The source classes search.source_prior.weight applies to.",
    ),
    free(
        "evidence.expansion",
        choice(texts!["full_section", "relevant_blocks", "parent_chain"]),
        "parent_chain",
        "How evidence grows around a match: its full section first, or its matched blocks \
         first, or complete parent-chain ranges first.",
    ),
    free(
        "evidence.parent_chain_order",
        choice(texts![
            "off",
            "minimum_complete_first",
            "largest_fitting_parent"
        ]),
        "minimum_complete_first",
        "Admission order for parent_chain expansion only; minimum_complete_first reserves the \
         smallest complete unit first.",
    ),
    free(
        "evidence.counter",
        choice(texts!["utf8", "utf8_answer_bound"]),
        "utf8",
        "What the evidence budget counts: every passage byte, or only the bytes the \
         answerer reads.",
    ),
    free(
        "ask.model",
        SettingKind::Name,
        "qwen3-4b",
        "The answerer's router entry when --model (the MCP model) is not given.",
    ),
    free(
        "ask.prompt",
        choice(texts!["v1", "v2", "procedure_first"]),
        "v2",
        "The answer prompt's version.",
    ),
    free(
        "ask.k",
        integer(1, 50),
        "5",
        "The most passages an answer reads when --k is not given.",
    ),
    free(
        "ask.evidence_bytes",
        integer(1, 24_000),
        "6000",
        "An answer's evidence budget, in UTF-8 bytes, when --evidence-bytes is not given.",
    ),
    free(
        "ask.search_deadline_ms",
        integer(1, 30_000),
        "30000",
        "An answer's search deadline in milliseconds when --search-deadline-ms is not given.",
    ),
    free(
        "ask.output_tokens",
        optional_integer(1, 2048),
        "off",
        "The most tokens each answerer reply generates when --output-tokens is not given; \
         off takes the answerer card's output limit, or 1024 when it declares none.",
    ),
    free(
        "ask.min_rerank_score",
        number(-100.0, 100.0, true),
        "off",
        "The least top reranker score an answer is given passages from; off answers \
         whatever the score. Search results are never filtered by it.",
    ),
    free(
        "models.compute",
        SettingKind::Choice {
            values: texts!["off", "gpu"],
            reserved: Cow::Borrowed(&[ReservedValue {
                value: Cow::Borrowed("cpu"),
                reason: Cow::Borrowed("cpu mode comes after M1"),
            }]),
        },
        "gpu",
        "Where models run: gpu, the machine's GPU backend (CUDA or Metal), or off for code \
         only: search keeps its keyword, exact-name and structured routes without reranking, \
         and ask refuses. cpu comes after M1.",
    ),
    free(
        "chunking.profile",
        choice(texts![
            "mapped-structural-chunks/2",
            "mapped-structural-chunks/3"
        ]),
        "mapped-structural-chunks/3",
        "Default for new collections; published profiles persist unless --chunk-profile overrides.",
    ),
];
