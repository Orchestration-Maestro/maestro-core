//! Only working public manual operations are registered, without a synthetic bypass.
use super::{bindings, inspect::inspect, output::Report, resources};
#[cfg(target_os = "linux")]
use super::{
    command::{preview, sync},
    controls::{Controls, Runtime},
};
use crate::{failure::Failure, kernel::Kernel};
use clap::{Args, Subcommand};
#[cfg(target_os = "linux")]
use maestro_acquisition::Principal;
use maestro_kernel::{
    acquisition::{Handle, Receipts},
    scope::LOCAL,
};
use maestro_knowledge::collection::Declaration;
#[cfg(target_os = "linux")]
use std::time::SystemTime;
use std::{env, path::PathBuf};
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
#[cfg(target_os = "linux")]
use {
    maestro_kernel::paths::{Environment, data_dir},
    rustix::process::geteuid,
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
/// Manual capture MVP only; lifecycle modes and schedules are not registered.
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
        /// Checked manifest and local resource/authority bindings.
        #[command(flatten)]
        inputs: Inputs,
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
    if let Acquire::Inspect { receipt, run } = command {
        let kernel = Kernel::open()?;
        let attempt = match receipt {
            Some(attempt) => *attempt,
            None => latest(
                &*kernel.database,
                run.ok_or_else(|| Failure::refused("acquisition run missing"))?,
            )?,
        };
        return inspect(&*kernel.database, LOCAL, attempt);
    }
    let inputs = match command {
        Acquire::Preview { inputs } | Acquire::Sync { inputs } => inputs,
        Acquire::Inspect { .. } => return Err(Failure::refused("acquisition operation invalid")),
    };
    // Decode both strict documents before any kernel/resource/authority start.
    drop(bindings::read::<Declaration>(&inputs.manifest)?);
    // Full immutable resolution below remains the authority for schema and digest checks.
    configured(command, inputs)
}
/// Linux-only live composition; synthetic port contracts still run on every host.
#[cfg(target_os = "linux")]
fn configured(command: &Acquire, inputs: &Inputs) -> Result<Report, Failure> {
    let kernel = Kernel::open()?;
    let principal_id = geteuid().as_raw().to_string();
    let principal = Principal {
        id: &principal_id,
        platform: env::consts::OS,
        scopes: &kernel.scopes,
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
