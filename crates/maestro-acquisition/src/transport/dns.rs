//! Read-only native DNS waiting is bounded independently of the HTTP future.
use crate::refusal::Refusal;
use maestro_kernel::retrieval::{Clock, SystemClock};
use std::{
    fmt::Debug,
    net::ToSocketAddrs as _,
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

/// Replaceable resolver; all returned candidates must pass core classification.
pub trait Resolver: Debug {
    /// Return address strings only; zone IDs and noncanonical forms refuse.
    ///
    /// # Errors
    /// Failed or unavailable resolution refuses, with no fallback.
    fn resolve(&self, hostname: &str) -> Result<Vec<String>, Refusal>;
}
/// Native DNS is read-only, bounded, and never runs at connect time.
#[derive(Debug)]
pub struct SystemResolver {
    /// Explicit policy/settings resolution bound, not a hidden time constant.
    timeout: Duration,
    /// At most one abandoned lookup per adapter; a stalled one refuses new work.
    active: Arc<AtomicBool>,
}
impl SystemResolver {
    /// Bind a finite settings-derived deadline in milliseconds.
    #[must_use]
    pub fn new(timeout_ms: NonZeroU64) -> Self {
        Self {
            timeout: Duration::from_millis(timeout_ms.get()),
            active: Arc::new(AtomicBool::new(false)),
        }
    }
    /// Wait only until the trusted deadline; an abandoned result is never consumed.
    fn lookup(
        &self,
        clock: &dyn Clock,
        lookup: impl FnOnce() -> Result<Vec<String>, Refusal> + Send + 'static,
    ) -> Result<Vec<String>, Refusal> {
        let deadline = clock
            .now()
            .checked_add(self.timeout)
            .ok_or(Refusal::Invalid)?;
        if self.active.swap(true, Ordering::SeqCst) {
            return Err(Refusal::Access);
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let active = self.active.clone();
        let spawned = thread::Builder::new()
            .name("maestro-dns".into())
            .spawn(move || {
                let result = lookup();
                drop(sender.send(result));
                active.store(false, Ordering::SeqCst);
            });
        if spawned.is_err() {
            self.active.store(false, Ordering::SeqCst);
            return Err(Refusal::Access);
        }
        let left = remaining(deadline, clock)?;
        let result = receiver.recv_timeout(left).map_err(|error| match error {
            RecvTimeoutError::Timeout => Refusal::Deadline,
            RecvTimeoutError::Disconnected => Refusal::Access,
        })?;
        remaining(deadline, clock)?;
        result
    }
}
impl Resolver for SystemResolver {
    fn resolve(&self, hostname: &str) -> Result<Vec<String>, Refusal> {
        let hostname = hostname.to_owned();
        self.lookup(&SystemClock, move || {
            (hostname.as_str(), 443)
                .to_socket_addrs()
                .map_err(|_| Refusal::Access)
                .map(|addresses| addresses.map(|address| address.ip().to_string()).collect())
        })
    }
}
/// No zero timeout or post-deadline result can become address evidence.
fn remaining(deadline: Instant, clock: &dyn Clock) -> Result<Duration, Refusal> {
    let now = clock.now();
    if now >= deadline {
        return Err(Refusal::Deadline);
    }
    Ok(deadline.duration_since(now))
}

#[cfg(test)]
mod tests {
    use super::{Clock, Duration, Refusal, SystemResolver};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        thread,
        time::Instant,
    };
    /// Advance to the exact cutoff without sleeps or relying on scheduling.
    #[derive(Debug)]
    struct Cutoff {
        start: Instant,
        calls: AtomicUsize,
        after: usize,
    }
    impl Clock for Cutoff {
        fn now(&self) -> Instant {
            if self.calls.fetch_add(1, Ordering::SeqCst) >= self.after {
                self.start + Duration::from_secs(1)
            } else {
                self.start
            }
        }
    }
    #[test]
    fn n09_native_dns_never_returning_lookup_is_bounded() {
        let resolver = Arc::new(SystemResolver::new(20.try_into().unwrap()));
        // Both pre-wait clock reads are strictly before the cutoff. The lookup
        // cannot complete until this test releases it, after observing timeout.
        let clock = Cutoff {
            start: Instant::now(),
            calls: AtomicUsize::new(0),
            after: usize::MAX,
        };
        let (started, running) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let (complete, result) = mpsc::channel();
        let owned = resolver.clone();
        let waiter = thread::spawn(move || {
            let result = owned.lookup(&clock, move || {
                started.send(()).unwrap();
                blocked.recv().unwrap();
                Ok(vec!["8.8.8.8".into()])
            });
            complete.send(result).unwrap();
        });
        let entered = running.recv_timeout(Duration::from_secs(2));
        let observed = result.recv_timeout(Duration::from_secs(2));
        let second = resolver.lookup(&super::SystemClock, || Ok(vec![]));
        // Clean up before asserting, so an overlong wait mutation fails on the
        // watchdog assertion instead of hanging the test process.
        release.send(()).unwrap();
        waiter.join().unwrap();
        assert_eq!(entered, Ok(()));
        assert_eq!(observed, Ok(Err(Refusal::Deadline)));
        assert_eq!(second, Err(Refusal::Access));
    }
    #[test]
    fn n09_native_dns_result_at_deadline_is_refused() {
        let resolver = SystemResolver::new(1000.try_into().unwrap());
        let clock = Cutoff {
            start: Instant::now(),
            calls: AtomicUsize::new(0),
            after: 2,
        };
        assert_eq!(
            resolver.lookup(&clock, || Ok(vec!["8.8.8.8".into()])),
            Err(Refusal::Deadline)
        );
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::{Clock, Refusal, Resolver, SystemResolver, remaining};
    use std::time::{Duration, Instant};

    #[derive(Debug)]
    struct Stopped(Instant);
    impl Clock for Stopped {
        fn now(&self) -> Instant {
            self.0
        }
    }
    #[test]
    fn s6t_dns_remaining_exact_duration() {
        let start = Instant::now();
        assert_eq!(
            remaining(start + Duration::from_secs(1), &Stopped(start)),
            Ok(Duration::from_secs(1))
        );
        assert_eq!(remaining(start, &Stopped(start)), Err(Refusal::Deadline));
    }
    #[test]
    fn s6t_dns_numeric_loopback_resolution() {
        let resolver = SystemResolver::new(1000.try_into().unwrap());
        assert_eq!(resolver.resolve("127.0.0.1"), Ok(vec!["127.0.0.1".into()]));
    }
}
