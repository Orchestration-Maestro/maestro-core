//! The command line's grammar, noun then verb (plan D12), as clap derives it
//! from these types, whose comments are the help it prints.

use clap::{Args, Parser, Subcommand};
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
    /// What to work on.
    #[command(subcommand)]
    pub(super) noun: Noun,
}

/// What a command works on, or the machine it sets up and checks.
#[derive(Debug, Subcommand)]
pub(super) enum Noun {
    /// Collections, their imports and their status.
    #[command(subcommand)]
    Knowledge(KnowledgeCommand),
    /// Jobs: long work run under a lease.
    #[command(subcommand)]
    Job(JobCommand),
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
    Mcp,
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

/// The explicit inputs to a publication.
#[derive(Debug, Args)]
pub(super) struct PublishArguments {
    /// The collection's ID, as its declaration names it.
    #[arg(long)]
    pub(super) collection: String,
    /// The recorded embedder model card's SHA-256 digest.
    #[arg(long)]
    pub(super) card: String,
    /// The complete chunk set to publish; defaults to the latest complete set.
    #[arg(long)]
    pub(super) chunk_set: Option<String>,
    /// Build a new generation instead of reusing an already published one.
    #[arg(long)]
    pub(super) again: bool,
}

/// What to do with the knowledge of a collection.
#[derive(Debug, Subcommand)]
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
    /// Retrieve an exact chunk from its visible published or retained generation.
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

/// What to do with a job.
#[derive(Debug, Subcommand)]
pub(super) enum JobCommand {
    /// Follow a job until it ends, and exit with its outcome.
    Wait {
        /// The job's ID.
        id: Ulid,
    },
}
