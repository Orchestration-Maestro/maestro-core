//! Owner authority commands; no pipeline writer or same-user store fallback.
use crate::failure::Failure;
use clap::Subcommand;
use serde_json::Value;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::time::Duration;

/// Separately authenticated local authority operations.
#[derive(Debug, Subcommand)]
pub(crate) enum AuthorityCommand {
    /// Verify actual pipeline/connector store denials through an authorized launcher.
    Qualify {
        /// Admin-provisioned host configuration, never a source manifest.
        #[arg(long)]
        config: PathBuf,
        /// Cumulative qualification probe hang guard (1–300 seconds).
        #[arg(long, default_value_t = 15, value_parser = clap::value_parser!(u64).range(1..=300))]
        probe_timeout_seconds: u64,
    },
    /// Serve grants only after matching real identity-separation qualification.
    Serve {
        /// The same qualified host configuration.
        #[arg(long)]
        config: PathBuf,
    },
    /// Submit a bounded exact request; peer credentials, not this JSON, authenticate.
    Request {
        /// Qualified authority socket.
        #[arg(long)]
        socket: PathBuf,
        /// Expected authority peer UID, from trusted local configuration.
        #[arg(long)]
        authority_uid: u32,
        /// Exact grant/confirmation, revocation or read-only decision JSON.
        #[arg(long)]
        file: PathBuf,
    },
    /// Internal setup probe; success means all three filesystem mutations were denied.
    #[command(hide = true)]
    ProbeStore {
        /// Protected store directory whose actual permissions are probed.
        #[arg(long)]
        store: PathBuf,
        /// One-shot endpoint authenticated by the qualifying process.
        #[arg(long)]
        qualification_socket: PathBuf,
    },
}
/// Execute a separately scoped authority command without opening the kernel.
pub(crate) fn run(
    command: &AuthorityCommand,
    ready: impl FnOnce() -> Result<(), Failure>,
) -> Result<Value, Failure> {
    #[cfg(target_os = "linux")]
    {
        use super::{authority_host, authority_service};
        match command {
            AuthorityCommand::Qualify {
                config,
                probe_timeout_seconds,
            } => authority_host::qualify(config, Duration::from_secs(*probe_timeout_seconds)),
            AuthorityCommand::Serve { config } => authority_service::serve(config, ready),
            AuthorityCommand::Request {
                socket,
                authority_uid,
                file,
            } => authority_service::request(socket, *authority_uid, file),
            AuthorityCommand::ProbeStore {
                store,
                qualification_socket,
            } => authority_host::probe(store, qualification_socket),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        // No qualified peer-credential/store adapter exists on these hosts yet.
        let _ = command;
        let _ready = ready;
        Err(Failure::refused("authority unqualified"))
    }
}
