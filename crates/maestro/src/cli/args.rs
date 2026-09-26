//! The command line's grammar, noun then verb (plan D12), as clap derives it
//! from these types, whose comments are the help it prints.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use ulid::Ulid;

/// Maestro's command line: collections, their imports and the jobs that run
/// them.
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

/// What a command works on.
#[derive(Debug, Subcommand)]
pub(super) enum Noun {
    /// Collections, their imports and their status.
    #[command(subcommand)]
    Knowledge(KnowledgeCommand),
    /// Jobs: long work run under a lease.
    #[command(subcommand)]
    Job(JobCommand),
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
    },
    /// Report a collection's documents, revisions and generations.
    Status {
        /// The collection's ID, as its declaration names it.
        #[arg(long)]
        collection: String,
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
