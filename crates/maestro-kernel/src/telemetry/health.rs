//! Health: how each component is doing, asked of its own check.

use std::{
    any::Any,
    collections::{BTreeMap, btree_map::Entry},
    error, fmt,
    panic::{self, AssertUnwindSafe},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

/// How a component is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// It works.
    Ready,
    /// It works in part.
    Degraded {
        /// What is missing, and why.
        reason: String,
    },
    /// It does not work.
    Down {
        /// Why.
        reason: String,
    },
}

/// How one component is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The component's name.
    pub component: String,
    /// Its status.
    pub status: Status,
}

/// Asks one component how it is doing.
type Check = Arc<dyn Fn() -> Status + Send + Sync>;

/// The components whose health is reported, each with its check.
#[derive(Default)]
pub struct Components {
    /// Each component's check, by the component's name.
    checks: BTreeMap<String, Check>,
}

impl fmt::Debug for Components {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Components")
            .field("names", &self.checks.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Components {
    /// Registers `component`, whose health `check` reports. A check bounds
    /// its own waits where it can; [`Components::health`] stops waiting for
    /// it after its patience anyway.
    ///
    /// # Errors
    ///
    /// [`DuplicateComponent`] when a component of the same name is registered
    /// already; its check stays as it was.
    pub fn register(
        &mut self,
        component: impl Into<String>,
        check: impl Fn() -> Status + Send + Sync + 'static,
    ) -> Result<(), DuplicateComponent> {
        match self.checks.entry(component.into()) {
            Entry::Occupied(taken) => Err(DuplicateComponent(taken.key().clone())),
            Entry::Vacant(slot) => {
                slot.insert(Arc::new(check));
                Ok(())
            }
        }
    }

    /// One report per registered component, in name order. Every check runs
    /// on a thread of its own, so a slow one delays no other. A component
    /// whose check cannot start, panics or gives no answer within `patience`
    /// is down with that reason: a component is never missing from the
    /// report. A check still running then is left to finish on its own.
    #[must_use]
    pub fn health(&self, patience: Duration) -> Vec<Report> {
        let started = Instant::now();
        let (sender, answers) = mpsc::channel();
        let mut statuses = Vec::with_capacity(self.checks.len());
        for (index, check) in self.checks.values().enumerate() {
            let sender = sender.clone();
            let check = Arc::clone(check);
            let asked = thread::Builder::new().spawn(move || {
                let status = panic::catch_unwind(AssertUnwindSafe(&*check))
                    .unwrap_or_else(|payload| panicked(&*payload));
                // Past the patience nobody listens any more, and that is fine.
                drop(sender.send((index, status)));
            });
            statuses.push(Status::Down {
                reason: match asked {
                    Ok(_running) => format!("no answer within {patience:?}"),
                    Err(error) => format!("its check could not start: {error}"),
                },
            });
        }
        // Once every check has answered, no sender is left and the wait ends.
        drop(sender);
        while let Ok((index, status)) =
            answers.recv_timeout(patience.saturating_sub(started.elapsed()))
        {
            if let Some(slot) = statuses.get_mut(index) {
                *slot = status;
            }
        }
        self.checks
            .keys()
            .cloned()
            .zip(statuses)
            .map(|(component, status)| Report { component, status })
            .collect()
    }
}

/// The status of a component whose check panicked with `payload`: down, with
/// the panic's message when it has one.
fn panicked(payload: &(dyn Any + Send)) -> Status {
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
    Status::Down {
        reason: message.map_or_else(
            || "its check panicked".to_owned(),
            |message| format!("its check panicked: {message}"),
        ),
    }
}

/// A component registered under a name already taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateComponent(String);

impl DuplicateComponent {
    /// The name already taken.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DuplicateComponent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a component named {:?} is already registered",
            self.0
        )
    }
}

impl error::Error for DuplicateComponent {}
