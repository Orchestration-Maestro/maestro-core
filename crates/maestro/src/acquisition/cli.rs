//! Only working public manual operations are registered, without a synthetic bypass.
use super::{bindings, inspect::inspect, output::Report, resources, timer};
#[cfg(target_os = "linux")]
use super::{
    command::{preview, sync},
    controls::{Controls, Runtime},
};
use crate::failure::Failure;
#[cfg(target_os = "linux")]
use crate::kernel::Kernel;
use clap::{Args, Subcommand};
#[cfg(target_os = "linux")]
use maestro_acquisition::Principal;
#[cfg(target_os = "linux")]
use maestro_acquisition::lifecycle::full::Mode;
use maestro_kernel::{
    acquisition::{Handle, Receipts},
    paths::{Environment, data_dir},
    scope::LOCAL,
    store::Database,
};
#[cfg(target_os = "linux")]
use maestro_kernel::{paths::config_dir, scope::Config};
use maestro_knowledge::collection::Declaration;
#[cfg(target_os = "linux")]
use rustix::process::geteuid;
#[cfg(target_os = "linux")]
use std::time::SystemTime;
use std::{env, path::PathBuf};
use ulid::Ulid;
#[cfg(target_os = "linux")]
use {
    maestro_acquisition::{
        lifecycle::resources::Resources,
        policy::authority_socket::AuthoritySocket,
        transport::{connect::NativeTlsTransport, dns::SystemResolver, pacing::OriginLedger},
    },
    std::{sync::Arc, time::Instant},
    tokio::runtime::Builder,
};

/// Caller-selected portable manifest and separate local bindings.
#[derive(Debug, Args)]
pub(crate) struct Inputs {
    /// Portable strict maestro-collection/2 declaration.
    #[arg(long)]
    manifest: PathBuf,
    /// Strict machine-local maestro-acquisition-bindings/1 file.
    #[arg(long)]
    bindings: PathBuf,
    /// N05's separately authenticated read-only authority socket.
    #[arg(long)]
    authority_socket: PathBuf,
    /// Actual platform owner/authority UID, not a manifest approval.
    #[arg(long)]
    authority_uid: u32,
}
/// Manual public capture with full or incremental verification windows.
#[derive(Debug, Subcommand)]
pub(crate) enum Acquire {
    /// Show every seed decision without network or credential calls.
    Preview {
        /// Checked manifest and local resource/authority bindings.
        #[command(flatten)]
        inputs: Inputs,
    },
    /// Capture current public pending work through the built-in HTTP adapter.
    Sync {
        /// Full revalidation or conservative local incremental windows.
        #[arg(long, default_value = "incremental", value_parser = ["full", "incremental"])]
        mode: String,
        /// Checked manifest and local resource/authority bindings.
        #[command(flatten)]
        inputs: Inputs,
    },
    /// Recurring live activation refuses until an exact active-source grant exists.
    Timer,
    /// Durably disable an owned local timer and wait for cancellation acknowledgement.
    Stop {
        /// Exact acquisition.schedule job; never a process ID.
        #[arg(long)]
        schedule: Ulid,
        /// Pi fast-tool default and timer maximum; OA3 declares no shutdown timeout.
        #[arg(
            long,
            default_value = "300000",
            value_parser = clap::value_parser!(u64).range(1..=2147483647)
        )]
        deadline_ms: u64,
    },
    /// Read a durable authorized receipt; no source, grant or network is consulted.
    Inspect {
        /// Exact receipt attempt to inspect.
        #[arg(long, group = "inspection", required = true)]
        receipt: Option<Handle>,
        /// Inspect the latest authorized attempt of a logical run.
        #[arg(long, group = "inspection")]
        run: Option<Handle>,
    },
}
/// Invalid input is checked before the kernel is opened for any write.
pub(crate) fn run(command: &Acquire) -> Result<Report, Failure> {
    match command {
        Acquire::Timer => return timer::activate(),
        Acquire::Stop {
            schedule,
            deadline_ms,
        } => return timer::stop(*schedule, *deadline_ms),
        _ => {}
    }
    if let Acquire::Inspect { receipt, run } = command {
        let data = data_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))?;
        if !data
            .join("kernel.sqlite3")
            .try_exists()
            .map_err(|error| Failure::failed_by(&error))?
        {
            return Err(Failure::refused("acquisition receipt unavailable"));
        }
        let database =
            Database::open_read_only(&data).map_err(|error| Failure::failed_by(&error))?;
        let attempt = match receipt {
            Some(attempt) => *attempt,
            None => latest(
                &database,
                run.ok_or_else(|| Failure::refused("acquisition run missing"))?,
            )?,
        };
        return inspect(&database, LOCAL, attempt);
    }
    let inputs = match command {
        Acquire::Preview { inputs } | Acquire::Sync { inputs, .. } => inputs,
        Acquire::Inspect { .. } | Acquire::Timer | Acquire::Stop { .. } => {
            return Err(Failure::refused("acquisition operation invalid"));
        }
    };
    // Decode both strict documents before any kernel/resource/authority start.
    drop(bindings::read::<Declaration>(&inputs.manifest)?);
    // Full immutable resolution below remains the authority for schema and digest checks.
    configured(command, inputs)
}
/// Linux-only live composition; synthetic port contracts still run on every host.
#[cfg(target_os = "linux")]
fn configured(command: &Acquire, inputs: &Inputs) -> Result<Report, Failure> {
    let config = config_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))?;
    let scopes = Config::load(&config)
        .map_err(|error| Failure::refused_by(&error))?
        .read_scopes();
    let principal_id = geteuid().as_raw().to_string();
    let principal = Principal {
        id: &principal_id,
        platform: env::consts::OS,
        scopes: &scopes,
    };
    let (policy, collection) = bindings::load(&inputs.manifest, &inputs.bindings, &principal)?;
    let scope = format!(
        "workspace/default/collection/{}",
        policy.policy().resource.collection_id
    );
    let authority = AuthoritySocket {
        socket: inputs.authority_socket.clone(),
        authority_uid: inputs.authority_uid,
    };
    let controls = Controls::new(&policy, &authority, &principal_id, &scope);
    if matches!(command, Acquire::Preview { .. }) {
        return preview(&policy, &controls, SystemTime::now());
    }
    resources::supported(env::consts::OS)?;
    let kernel = Kernel::open()?;
    // Writing open refreshes grants: a concurrent config revocation must win.
    let principal = Principal {
        id: &principal_id,
        platform: env::consts::OS,
        scopes: &kernel.scopes,
    };
    let root = data_dir(&Environment::current())
        .map_err(|_| Failure::refused("acquisition storage root unavailable"))?;
    let resources = Resources::new(Arc::new(resources::Host {
        root,
        policy: policy.policy().aggregate_limits.clone(),
    }));
    let transport = NativeTlsTransport::from_native_roots()
        .map_err(|_| Failure::refused("acquisition transport unavailable"))?;
    let resolver = SystemResolver::new(policy.policy().aggregate_limits.elapsed_ms);
    let pacing = OriginLedger::new(0);
    let runtime = Runtime {
        authority: &authority,
        resolver: &resolver,
        transport: &transport,
        pacing: &pacing,
        controls: &controls,
        epoch: Instant::now(),
        run_now: SystemTime::now(),
        clock: &SystemTime::now,
        mode: lifecycle_mode(command),
        collection,
        kernel_principal: LOCAL,
        frontier_page_size: 1000,
    };
    Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Failure::failed("acquisition runtime unavailable"))?
        .block_on(sync(
            &*kernel.database,
            &policy,
            &principal,
            &runtime,
            &resources,
        ))
}
/// Freeze the explicit CLI lifecycle selection without changing adapter composition.
#[cfg(target_os = "linux")]
fn lifecycle_mode(command: &Acquire) -> Mode {
    match command {
        Acquire::Sync { mode, .. } if mode == "full" => Mode::Full,
        _ => Mode::Incremental,
    }
}
/// There is no unqualified or same-user authority fallback on another host.
#[cfg(not(target_os = "linux"))]
fn configured(_: &Acquire, _: &Inputs) -> Result<Report, Failure> {
    resources::supported(env::consts::OS)?;
    Err(Failure::refused(
        "acquisition authority unsupported on this host",
    ))
}
/// Read only current authorized attempt records; never dispatch a resumed source.
fn latest(store: &dyn Receipts, run: Handle) -> Result<Handle, Failure> {
    let mut after = None;
    loop {
        let page = store
            .page(LOCAL, run, after, 1000)
            .map_err(|_| Failure::failed("acquisition storage failed"))?;
        if page.is_empty() {
            return after.ok_or_else(|| Failure::refused("acquisition receipt unavailable"));
        }
        after = page.last().map(|progress| progress.receipt);
    }
}

#[cfg(test)]
mod tests {
    use super::Acquire;
    use clap::Parser;
    use ulid::Ulid;

    /// Parse the real acquisition grammar, without opening a kernel.
    #[derive(Parser)]
    struct Commands {
        /// Production command and its defaults.
        #[command(subcommand)]
        command: Acquire,
    }

    #[test]
    fn n42_stop_deadline_range_endpoints_and_exact_pi_default() {
        let id = Ulid::generate().to_string();
        for (argument, expected) in [
            (Some("1"), 1),
            (Some("2147483647"), 2_147_483_647),
            (None, 300_000),
        ] {
            let mut args = vec!["maestro", "stop", "--schedule", &id];
            if let Some(value) = argument {
                args.extend(["--deadline-ms", value]);
            }
            let parsed = Commands::try_parse_from(args).unwrap();
            let Acquire::Stop { deadline_ms, .. } = parsed.command else {
                panic!("not stop")
            };
            assert_eq!(deadline_ms, expected);
        }
    }
    #[test]
    fn debt_cli_latest_returns_exact_authorized_receipt() {
        use crate::acquisition::flow_fixture::{Fixture, clean};
        use maestro_kernel::scope::LOCAL;
        let fixture = Fixture::mapped(clean, "reader", LOCAL);
        let report = fixture.sync();
        assert_eq!(
            super::latest(&fixture.db, report.run.unwrap()).unwrap(),
            report.receipt.unwrap()
        );
        fixture.finish();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn debt_cli_lifecycle_mode_preserves_full_and_incremental() {
        use super::{Inputs, Mode, lifecycle_mode};
        use std::path::PathBuf;
        for (mode, expected) in [("full", Mode::Full), ("incremental", Mode::Incremental)] {
            let inputs = Inputs {
                manifest: PathBuf::new(),
                bindings: PathBuf::new(),
                authority_socket: PathBuf::new(),
                authority_uid: 0,
            };
            assert_eq!(
                lifecycle_mode(&Acquire::Sync {
                    mode: mode.into(),
                    inputs
                }),
                expected
            );
        }
    }
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn debt_cli_unqualified_hosts_refuse_instead_of_empty_report() {
        use super::Inputs;
        use std::path::PathBuf;
        let inputs = Inputs {
            manifest: PathBuf::new(),
            bindings: PathBuf::new(),
            authority_socket: PathBuf::new(),
            authority_uid: 0,
        };
        assert!(super::configured(&Acquire::Timer, &inputs).is_err());
    }
}
