//! The command line's grammar, noun then verb (plan D12), as clap derives it
//! from these types, whose comments are the help it prints.

use super::policy::PolicyCommand;
use clap::{Args, Parser, Subcommand};
use maestro_kernel::evidence::RequestBudget;
use std::path::PathBuf;
use ulid::Ulid;

/// Maestro's command line: collections, their imports, the jobs that run
/// them, and the machine's setup and checks.
#[derive(Debug, Parser)]
#[command(name = "maestro", version)]
pub(super) struct Arguments {
    /// Print one JSON document on stdout instead of text; diagnostics go to
    /// stderr.
    #[arg(long, global = true)]
    pub(super) json: bool,
    /// Set a setting for this run only, over the project and user files;
    /// repeatable. `maestro config list` names every setting.
    #[arg(long = "set", global = true, value_name = "KEY=VALUE")]
    pub(super) set: Vec<String>,
    /// Answer language for this session, as a supported BCP 47 tag.
    #[arg(long, global = true)]
    pub(super) language: Option<String>,
    /// Answer tone for this session: brief, normal or detailed.
    #[arg(long, global = true)]
    pub(super) tone: Option<String>,
    /// What to work on.
    #[command(subcommand)]
    pub(super) noun: Noun,
}

impl Arguments {
    /// Explicit flags only: no parser default can mask a stored preference.
    pub(super) fn settings(&self) -> Vec<String> {
        let mut flags = self.set.clone();
        for (key, value) in [("language", &self.language), ("tone", &self.tone)] {
            if let Some(value) = value {
                flags.push(format!("{key}={value}"));
            }
        }
        flags
    }
}

/// What a command works on, or the machine it sets up and checks.
#[derive(Debug, Subcommand)]
pub(super) enum Noun {
    /// Collections, their imports and their status.
    #[command(subcommand)]
    Knowledge(KnowledgeCommand),
    /// Register, qualify, select and list model cards.
    #[command(subcommand)]
    Model(ModelCommand),
    /// Jobs: long work run under a lease.
    #[command(subcommand)]
    Job(JobCommand),
    /// Evaluations of a collection's search and answers.
    #[command(subcommand)]
    Eval(EvalCommand),
    /// The catalog's authoring sources.
    #[command(subcommand)]
    Catalog(CatalogCommand),
    /// Effect-free authoring policy checks and neighbour tests.
    #[command(subcommand)]
    Policy(PolicyCommand),
    /// Preview a project bootstrap; only --apply writes files.
    /// Use --set language=en, --set tone=brief, --set updates=off or a
    /// documented `config list` key to draft preferences (preview only until C05j).
    Init {
        /// Reviewed authoring catalog directory.
        #[arg(long, value_name = "DIR")]
        catalog_dir: PathBuf,
        /// Explicit preset name; repeat to compose overlays.
        #[arg(long = "preset", required = true)]
        presets: Vec<String>,
        /// Apply the displayed digest-bound plan.
        #[arg(long)]
        apply: bool,
    },
    /// Preview the search service Maestro needs, or install it with --yes.
    Setup {
        /// Take the steps the preview lists, rather than only print them.
        #[arg(long)]
        yes: bool,
    },
    /// Summarize which services and collections are ready.
    Status,
    /// Check the kernel, the search service, the model router and each
    /// role's model card, naming the next action for every failure.
    Doctor,
    /// Serve the local knowledge tools over stdio MCP.
    Mcp {
        /// The workspace whose project file (`.maestro/config.toml`, found
        /// upward within home) the server reads; without it, only the user
        /// file. The server's working directory never selects one.
        #[arg(long, value_name = "DIR")]
        workspace: Option<PathBuf>,
    },
    /// Every configurable behaviour: the user file `preferences.toml`, the
    /// project file `.maestro/config.toml`, and `--set`.
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Back up the kernel to a new or empty directory.
    Backup {
        /// The directory to write.
        #[arg(long, value_name = "DIR")]
        to: PathBuf,
    },
    /// Restore a checked backup into a data directory without a kernel.
    Restore {
        /// The backup directory.
        #[arg(long, value_name = "DIR")]
        from: PathBuf,
    },
}

/// What to do with registered model cards.
#[derive(Debug, Subcommand)]
pub(super) enum ModelCommand {
    /// Register a strict v2 model card and import its pinned evidence for a collection.
    Register {
        /// The collection whose registry records the card.
        #[arg(long)]
        collection: String,
        /// The v2 card JSON file.
        #[arg(long)]
        card: PathBuf,
        /// The directory containing digest-named evidence files.
        #[arg(long, value_name = "DIR")]
        evidence: PathBuf,
        /// The GGUF file pinned by the card.
        #[arg(long, value_name = "FILE")]
        gguf: PathBuf,
    },
    /// Qualify a registered reranker card through the model router.
    Check {
        /// The collection whose card is checked.
        #[arg(long)]
        collection: String,
        /// The registered card's SHA-256 digest.
        #[arg(long)]
        digest: String,
    },
    /// Select a card for a role, only after a real eligible evaluation.
    Select {
        /// The collection whose role is selected.
        #[arg(long)]
        collection: String,
        /// The role to select.
        #[arg(long)]
        role: String,
        /// The registered card's SHA-256 digest.
        #[arg(long)]
        digest: String,
    },
    /// List cards, evaluations and current role selections.
    List {
        /// The collection to list.
        #[arg(long)]
        collection: String,
        /// Restrict all results to one role.
        #[arg(long)]
        role: Option<String>,
    },
}

/// The explicit inputs to a publication.
#[derive(Debug, Args)]
pub(super) struct PublishArguments {
    /// The collection's ID, as its declaration names it.
    #[arg(long)]
    pub(super) collection: String,
    /// The recorded embedder model card's SHA-256 digest.
    #[arg(long)]
    pub(super) card: String,
    /// The complete chunk set to publish; defaults to the latest complete set
    /// of the chunking profile.
    #[arg(long)]
    pub(super) chunk_set: Option<String>,
    /// The chunking profile whose latest complete set is published when no
    /// chunk set is named, by its chunker version: mapped-structural-chunks/3,
    /// the default, as for prepare. A set of another profile is published
    /// only when named.
    #[arg(long, value_name = "PROFILE", conflicts_with = "chunk_set")]
    pub(super) chunk_profile: Option<String>,
    /// Build a new generation instead of reusing an already published one.
    #[arg(long)]
    pub(super) again: bool,
}

// The `--evidence-bytes` help names this shared evidence ceiling.
const _: () = assert!(RequestBudget::MAX_EVIDENCE_BUDGET == 24_000);

/// What to do with the knowledge of a collection.
#[derive(Debug, Subcommand)]
#[expect(
    clippy::min_ident_chars,
    reason = "the public search-budget flag uses the shared maestro-evidence name k"
)]
pub(super) enum KnowledgeCommand {
    /// Collections and their declarations.
    #[command(subcommand)]
    Collection(CollectionCommand),
    /// Import a collection's corpus manifests as a job, printing its ID first.
    Import {
        /// The collection's ID, as its declaration names it.
        #[arg(long)]
        collection: String,
        /// Import again, as a job of its own, though the same declaration and
        /// manifests were imported: after files restored or fixed, it records
        /// what the last import refused.
        #[arg(long)]
        again: bool,
    },
    /// Give each revision of a collection its quality disposition, as a job
    /// printing its ID first.
    Quality {
        /// The collection's ID, as its declaration names it.
        #[arg(long)]
        collection: String,
    },
    /// Prepare a collection's eligible revisions as a chunk set, as a job.
    Prepare {
        /// The collection's ID, as its declaration names it.
        #[arg(long)]
        collection: String,
        /// The recorded embedder model card's SHA-256 digest.
        #[arg(long)]
        card: String,
        /// The chunking profile, by its chunker version: mapped-structural-chunks/3, the
        /// default, or mapped-structural-chunks/2, which indexes page chrome and packs compatible
        /// blocks within one section and container. Another profile makes another chunk set; the
        /// published one stays as it is.
        #[arg(long, value_name = "PROFILE")]
        chunk_profile: Option<String>,
    },
    /// Publish a complete chunk set as a verified Qdrant generation, as a job.
    Publish {
        /// The collection, card and explicit recovery request.
        #[command(flatten)]
        arguments: PublishArguments,
    },
    /// Verify the collection's published Qdrant generation, as a job.
    Verify {
        /// The collection's ID, as its declaration names it.
        #[arg(long)]
        collection: String,
    },
    /// Report a collection's documents, revisions and generations.
    Status {
        /// The collection's ID, as its declaration names it.
        #[arg(long)]
        collection: String,
    },
    /// List collection metadata visible to the local principal.
    Collections,
    /// Search a published generation and return bounded source-backed evidence.
    Search {
        /// The collection whose current published generation is searched.
        #[arg(long)]
        collection: String,
        /// The original question to search for.
        #[arg(long)]
        query: String,
        /// Restrict results to this exact documented version.
        #[arg(long)]
        version: Option<String>,
        /// Maximum final passage count (1..=50, default 10).
        #[arg(long = "k")]
        max_passages: Option<u32>,
        /// Maximum evidence size in UTF-8 bytes (1..=24000, default 12000).
        #[arg(long)]
        evidence_bytes: Option<u32>,
        /// Search deadline in milliseconds (1..=30000, default 30000).
        #[arg(long)]
        deadline_ms: Option<u32>,
    },
    /// Retrieve an exact chunk or section from its visible published or retained generation.
    Get {
        /// The chunk's stable ID.
        #[arg(long, group = "get_selector", required = true)]
        chunk_id: Option<String>,
        /// The canonical section ID.
        #[arg(long, group = "get_selector")]
        section_id: Option<String>,
        /// Restrict lookup to this collection.
        #[arg(long)]
        collection: Option<String>,
        /// Pin a published or retained generation; requires --collection.
        #[arg(long, requires = "collection")]
        generation: Option<i64>,
    },
    /// Answer a question from verified passages, or refuse when they do not suffice.
    Ask {
        /// The collection to search.
        #[arg(long)]
        collection: String,
        /// The question to answer.
        #[arg(long)]
        question: String,
        /// A registered answerer router entry; defaults to qwen3-4b.
        #[arg(long)]
        model: Option<String>,
        /// Restrict search to this exact version.
        #[arg(long)]
        version: Option<String>,
        /// Maximum number of passages to assemble.
        #[arg(long)]
        k: Option<u32>,
        /// Maximum evidence size in UTF-8 bytes (1..=24000, default 6000), about
        /// 4 bytes per English or French token.
        #[arg(long)]
        evidence_bytes: Option<u32>,
        /// Search and evidence deadline in milliseconds (1..=30000, default
        /// 30000).
        #[arg(long)]
        search_deadline_ms: Option<u32>,
        /// Maximum generated tokens per chat call (1..=2048; default: the
        /// answerer card's output limit, or 1024 when it declares none).
        #[arg(long)]
        output_tokens: Option<u32>,
        /// Print on stderr each search route's status, such as a reranker
        /// that did not run, then the check each rejected answer failed and
        /// the offending tokens. An ask that ends in an error, such as a
        /// timeout or an unavailable answerer, prints no explanation.
        #[arg(long)]
        explain: bool,
    },
}

/// What to do with the settings.
#[derive(Debug, Subcommand)]
pub(super) enum ConfigCommand {
    /// Print a setting's effective value.
    Get {
        /// The setting's dotted key, such as `tone`.
        key: String,
    },
    /// Write a setting in the user file, or with --project the project
    /// file, keeping the file's comments and order; the change is journaled.
    Set {
        /// The setting's dotted key.
        key: String,
        /// Its value, as `config get` prints it: `brief`, `20`, `0.5`, `off`,
        /// `changelog,conversion`.
        value: String,
        /// The file to write.
        #[command(flatten)]
        target: Target,
    },
    /// Remove a setting from the user file, or with --project the project
    /// file; the change is journaled.
    Unset {
        /// The setting's dotted key.
        key: String,
        /// The file to write.
        #[command(flatten)]
        target: Target,
    },
    /// List every setting with its effective value and the layer that set it.
    List,
    /// Explain a setting, or every one: its value, the layer that set it,
    /// the layers it overrode, what it accepts, and the files read.
    Explain {
        /// The setting's dotted key; every setting without one.
        key: Option<String>,
    },
    /// List the journaled changes of settings, oldest first.
    History,
}

/// Which preferences file `config set` and `config unset` write.
#[derive(Debug, Args)]
pub(super) struct Target {
    /// The user file, `preferences.toml` in the configuration directory: the
    /// default.
    #[arg(long, conflicts_with = "project")]
    pub(super) user: bool,
    /// The project file: the nearest `.maestro/config.toml` upward from the
    /// working directory, within home, or a new one in the working directory.
    #[arg(long)]
    pub(super) project: bool,
}

/// What to do with a collection's declaration.
#[derive(Debug, Subcommand)]
pub(super) enum CollectionCommand {
    /// Add a collection from its strict declaration, or take its new version.
    Add {
        /// The declaration: a `maestro-collection/1` file.
        declaration: PathBuf,
    },
}

/// What to do with a catalog's authoring sources.
#[derive(Debug, Subcommand)]
pub(super) enum CatalogCommand {
    /// Check a catalog's sources strictly, running none of its content.
    Check {
        /// The catalog's directory.
        #[arg(long, value_name = "DIR")]
        catalog_dir: PathBuf,
    },
}

/// Which evaluation to run.
#[derive(Debug, Subcommand)]
pub(super) enum EvalCommand {
    /// Run the M1 ladder a private manifest describes: each rung searches and
    /// asks every question of the suite, and its floors are scored.
    Ladder {
        /// The `maestro-ladder-manifest/1` file.
        #[arg(long, value_name = "PATH")]
        manifest: PathBuf,
    },
}

/// What to do with a job.
#[derive(Debug, Subcommand)]
pub(super) enum JobCommand {
    /// Follow a job until it ends, and exit with its outcome.
    Wait {
        /// The job's ID.
        id: Ulid,
    },
}

#[cfg(test)]
mod tests {
    use super::{Arguments, KnowledgeCommand, Noun};
    use clap::Parser as _;

    #[test]
    fn knowledge_ask_parses_the_required_question_and_optional_bounds() {
        let parsed = Arguments::try_parse_from([
            "maestro",
            "knowledge",
            "ask",
            "--collection",
            "docs",
            "--question",
            "How is the service configured?",
        ]);
        assert!(parsed.is_ok(), "{parsed:?}");
        if let Ok(arguments) = parsed {
            assert!(matches!(
                arguments.noun,
                Noun::Knowledge(KnowledgeCommand::Ask {
                    collection,
                    question,
                    model: None,
                    version: None,
                    k: None,
                    evidence_bytes: None,
                    search_deadline_ms: None,
                    output_tokens: None,
                    explain: false,
                }) if collection == "docs" && question == "How is the service configured?"
            ));
        }
    }

    #[test]
    fn knowledge_ask_accepts_explain() {
        let parsed = Arguments::try_parse_from([
            "maestro",
            "knowledge",
            "ask",
            "--collection",
            "docs",
            "--question",
            "How is the service configured?",
            "--explain",
        ]);
        assert!(matches!(
            parsed.map(|arguments| arguments.noun),
            Ok(Noun::Knowledge(KnowledgeCommand::Ask { explain: true, .. }))
        ));
    }

    #[test]
    fn knowledge_ask_accepts_the_explicit_global_language_option() {
        let arguments = Arguments::try_parse_from([
            "maestro",
            "knowledge",
            "ask",
            "--collection",
            "docs",
            "--question",
            "How is the service configured?",
            "--language",
            "fr",
        ])
        .unwrap();
        assert_eq!(arguments.settings(), ["language=fr"]);
    }
}
