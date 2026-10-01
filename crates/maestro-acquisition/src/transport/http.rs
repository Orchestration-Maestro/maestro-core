//! Bounded HTTP/1 over one freshly admitted pinned connection per hop.
use super::{
    budget::compose,
    connect::{CheckedDestination, OriginCredentials, PinnedTransport},
    dns::Resolver,
    http_protocol::{parser, protocol_error, read_response, redirect, redirect_url},
    pacing::{Demand, OriginLedger, OriginPacing, PacingContext, PacingLimits, Pending},
    robots::{RobotsBinding, RobotsCache},
    stream::{Accounting, charge_quota},
    wire_quota::{BoundedIo, Quota},
};
use crate::{
    CheckedPolicy, Refusal,
    policy::{
        acquisition::Transport,
        authority::{Authority, Operation, Target},
        decision::{AdmissionControls, Request, RequestKind, admit},
        identity::FetchIdentity,
        utc,
    },
};
use http_body_util::Empty;
use hyper::{Request as WireRequest, body::Bytes};
use hyper_util::rt::TokioIo;
use reqwest::header::{ACCEPT_ENCODING, CONNECTION, HOST, HeaderValue};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant as StdInstant, SystemTime},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::{sleep, timeout_at},
};

pub use super::{http_protocol::Response, stream::Failure, wire_quota::MAX_INTERIM_RESPONSES};
/// Current authority context and explicit operation; creates no entitlement.
#[derive(Debug)]
pub struct Fetch<'a> {
    /// Initial candidate; every redirect/retry creates a new admission request.
    pub request: Request<'a>,
    /// Authenticated logical principal, supplied by the host.
    pub principal: &'a str,
    /// Exact collection scope, supplied by the host.
    pub scope: &'a str,
    /// Exact isolated account role, including an explicit public role.
    pub account: &'a str,
    /// Trusted host wall time at dispatch; monotonic elapsed advances rechecks.
    pub authority_time: SystemTime,
    /// Credentials are selected independently for each destination origin.
    pub credentials: Option<&'a OriginCredentials>,
    /// Narrow robots operation; cannot admit a content request to another path.
    pub robots: bool,
}
/// Replaceable dependencies; no pool, proxy, implicit resolver or client defaults.
#[derive(Debug)]
pub struct Http<'a, T, L = OriginLedger> {
    /// Immutable checked policy.
    pub policy: &'a CheckedPolicy,
    /// Current caller/network/robots controls; no effectful admission adapter.
    pub controls: &'a dyn AdmissionControls,
    /// Separate N05 authority, rechecked before every owned connection.
    pub authority: &'a dyn Authority,
    /// Fresh DNS, checked by N08 before dialing.
    pub resolver: &'a dyn Resolver,
    /// Socket-only, certificate-validating connection adapter.
    pub transport: &'a T,
    /// Shared origin reservation port; HTTP owns every hidden hop's permit.
    pub pacing: &'a L,
    /// Trusted run/monotonic domain shared with the ledger.
    pub pacing_context: PacingContext<'a>,
}
impl<T: PinnedTransport, L: OriginPacing> Http<'_, T, L>
where
    T::Connection: AsyncRead + AsyncWrite + Unpin + Send,
{
    /// Fetch with cumulative budgets retained across redirects, retries and decoders.
    /// The caller may retain accounting for downstream document decode stages.
    ///
    /// # Errors
    /// Every refusal cancels owned work; no partial body is returned.
    pub async fn fetch(
        &self,
        fetch: &Fetch<'_>,
        accounting: &mut Accounting,
    ) -> Result<Response, Failure> {
        let source = self
            .policy
            .policy()
            .sources
            .iter()
            .find(|source| source.id == fetch.request.source_id)
            .ok_or(Failure::Admission(Refusal::Access))?;
        let profile = self
            .policy
            .acquisition_profiles()
            .get(&source.acquisition_profile.id)
            .ok_or(Failure::TransportMismatch)?;
        if !fetch.robots && profile.transport != Transport::Http {
            return Err(Failure::TransportMismatch);
        }
        accounting.tighten(
            &compose(&[
                source.limits.clone(),
                self.policy.policy().aggregate_limits.clone(),
            ])
            .map_err(|_| Failure::Configuration)?,
        );
        if fetch.robots {
            if accounting.limits().redirects.get() < 5 {
                return Err(Failure::Configuration);
            }
            accounting.robots(source.robots.rules_max_bytes.get());
        }
        let run_until_ms = self
            .pacing_context
            .started_ms
            .checked_add(accounting.limits().elapsed_ms.get())
            .ok_or(Failure::Configuration)?
            .min(self.pacing_context.deadline_ms);
        let run_deadline = self
            .pacing_context
            .epoch
            .checked_add(Duration::from_millis(run_until_ms))
            .ok_or(Failure::Configuration)?;
        accounting.bind_run_deadline(run_deadline);
        let deadline = accounting.deadline()?;
        accounting.validate()?;
        let started = accounting.now();
        timeout_at(deadline, self.hops(fetch, accounting, started))
            .await
            .map_err(|_| Failure::Timeout)?
    }
    /// Feed every robots transport outcome to N10; initial origin binds the rules.
    ///
    /// # Errors
    /// Invalid source/origin binding refuses before construction of a cache entry.
    pub async fn fetch_robots(
        &self,
        fetch: &Fetch<'_>,
        accounting: &mut Accounting,
        now_ms: u64,
    ) -> Result<RobotsCache, Failure> {
        let identity = self
            .policy
            .admit_robots(fetch.request.source_id, fetch.request.url)
            .map_err(Failure::Admission)?;
        let binding =
            RobotsBinding::new(self.policy, fetch.request.source_id).map_err(Failure::Admission)?;
        let robots = Fetch {
            robots: true,
            request: Request { ..fetch.request },
            principal: fetch.principal,
            scope: fetch.scope,
            account: fetch.account,
            authority_time: fetch.authority_time,
            credentials: fetch.credentials,
        };
        Ok(match self.fetch(&robots, accounting).await {
            Ok(response) => {
                RobotsCache::response(&identity, binding, response.status, &response.body, now_ms)
            }
            Err(_) => RobotsCache::unreadable(&identity, binding, now_ms),
        })
    }
    /// One redirect chain; retries never reuse a prior socket or admission.
    async fn hops(
        &self,
        fetch: &Fetch<'_>,
        accounting: &mut Accounting,
        started: StdInstant,
    ) -> Result<Response, Failure> {
        let mut url = fetch.request.url.to_owned();
        let mut visited = BTreeSet::new();
        let mut redirects = 0;
        let mut retries = 0;
        let mut retry_until = 0;
        let mut kind = fetch.request.kind;
        loop {
            accounting.validate()?;
            let now = utc::advance(fetch.request.now, elapsed(accounting, started)?)
                .map_err(Failure::Admission)?;
            let request = Request {
                url: &url,
                kind,
                now: &now,
                ..fetch.request
            };
            let identity = self.admission(fetch.robots, &request)?;
            let limits =
                PacingLimits::compose([accounting.limits()]).map_err(Failure::Admission)?;
            let demand = self.demand(accounting, retries, retry_until)?;
            let permit = match self.pacing.acquire(&identity, limits, demand) {
                Ok(permit) => permit,
                Err(Pending::Delay { until_ms }) if until_ms > demand.now_ms => {
                    sleep(Duration::from_millis(until_ms - demand.now_ms)).await;
                    accounting.check_time()?;
                    continue; // Fresh admission after a wait, before any dial.
                }
                Err(pending) => return Err(Failure::Pacing(pending)),
            };
            if !visited.insert(identity.as_str().to_owned()) && kind != RequestKind::Retry {
                return Err(Failure::RedirectLoop);
            }
            accounting.request()?;
            let destination =
                CheckedDestination::resolve(&identity, self.policy.address_table(), self.resolver);
            accounting.check_time()?;
            let destination = destination.map_err(Failure::Admission)?;
            let authority = self.authorize(fetch, &identity, started, accounting);
            accounting.check_time()?;
            authority?;
            let result = self.hop(fetch, identity, destination, accounting).await;
            drop(permit); // Owned hop stopped; release before retry/redirect acquire.
            if retries < accounting.limits().retries
                && (result
                    .as_ref()
                    .is_err_and(|error| *error == Failure::Transport)
                    || result
                        .as_ref()
                        .is_ok_and(|response| matches!(response.status, 429 | 503)))
            {
                retries += 1;
                let server = result
                    .as_ref()
                    .ok()
                    .and_then(|response| response.retry_after.as_ref());
                retry_until = self.retry_until(
                    accounting,
                    fetch
                        .authority_time
                        .checked_add(elapsed(accounting, started)?)
                        .ok_or(Failure::Configuration)?,
                    retries,
                    server,
                )?;
                kind = RequestKind::Retry;
                continue;
            }
            let response = result?;
            if !redirect(response.status) {
                accounting.validate()?;
                return Ok(response);
            }
            if redirects >= accounting.limits().redirects.get() {
                return Err(Failure::RedirectLimit);
            }
            redirects += 1;
            let location = response
                .location
                .as_ref()
                .and_then(|value| value.to_str().ok())
                .ok_or(Failure::Content)?;
            url = redirect_url(response.identity.url(), location);
            kind = RequestKind::Redirect;
        }
    }
    /// Every admission uses the same trusted elapsed time as authority rechecks.
    fn admission(&self, robots: bool, request: &Request<'_>) -> Result<FetchIdentity, Failure> {
        if !robots {
            return Ok(admit(self.policy, request, self.controls)
                .map_err(Failure::Admission)?
                .identity()
                .clone());
        }
        let source = self
            .policy
            .policy()
            .sources
            .iter()
            .find(|source| source.id == request.source_id)
            .ok_or(Failure::Admission(Refusal::Access))?;
        self.controls
            .caller(source, request)
            .map_err(Failure::Admission)?;
        let identity = self
            .policy
            .admit_robots(request.source_id, request.url)
            .map_err(Failure::Admission)?;
        self.controls
            .network(&identity)
            .map_err(Failure::Admission)?;
        Ok(identity)
    }
    /// Use one trusted clock domain for ledger now and the owning deadline.
    fn demand<'a>(
        &'a self,
        accounting: &mut Accounting,
        retry: u64,
        retry_until: u64,
    ) -> Result<Demand<'a>, Failure> {
        let now_ms = millis(accounting.now(), self.pacing_context.epoch)?;
        let deadline_ms = millis(accounting.deadline()?.into_std(), self.pacing_context.epoch)?
            .min(self.pacing_context.deadline_ms);
        Ok(Demand {
            run_id: self.pacing_context.run_id,
            started_ms: self.pacing_context.started_ms,
            now_ms,
            deadline_ms,
            retry,
            server_delay_ms: retry_until.saturating_sub(now_ms),
        })
    }
    /// Exponential failure floor and server floor compose with max, never clamp.
    fn retry_until(
        &self,
        accounting: &Accounting,
        now: SystemTime,
        retry: u64,
        server: Option<&HeaderValue>,
    ) -> Result<u64, Failure> {
        let multiplier = 1_u64
            .checked_shl(u32::try_from(retry - 1).map_err(|_| Failure::Pacing(Pending::Budget))?)
            .ok_or(Failure::Pacing(Pending::Budget))?;
        let backoff = accounting
            .limits()
            .origin_interval_ms
            .get()
            .checked_mul(multiplier)
            .ok_or(Failure::Pacing(Pending::Budget))?;
        let server = server
            .map(|value| utc::retry_after(value.to_str().map_err(|_| Refusal::Invalid)?, now))
            .transpose()
            .map_err(Failure::Admission)?
            .unwrap_or(0);
        millis(accounting.now(), self.pacing_context.epoch)?
            .checked_add(backoff.max(server))
            .ok_or(Failure::Pacing(Pending::Budget))
    }
    /// N05 grant identity excludes query/fragment, but keeps this hop's exact path.
    /// Queries remain part of N07 fetch identity, never authority prefix widening.
    fn authorize(
        &self,
        fetch: &Fetch<'_>,
        identity: &FetchIdentity,
        started: StdInstant,
        accounting: &Accounting,
    ) -> Result<(), Failure> {
        let mut url = identity.url().clone();
        url.set_query(None);
        url.set_fragment(None);
        let target = Target {
            scope: fetch.scope.into(),
            source: identity.source_id().into(),
            account: fetch.account.into(),
            resource: url.to_string(),
        };
        target.validate().map_err(Failure::Admission)?;
        let now = fetch
            .authority_time
            .checked_add(elapsed(accounting, started)?)
            .ok_or(Failure::Configuration)?;
        self.authority
            .decide(fetch.principal, Operation::Fetch, &target, now)
            .map_err(Failure::Authority)?;
        Ok(())
    }
    /// Drive HTTP alongside response consumption in the same owned future.
    /// Dropping this future drops the connection; no detached driver remains.
    async fn hop(
        &self,
        fetch: &Fetch<'_>,
        identity: FetchIdentity,
        destination: CheckedDestination,
        accounting: &mut Accounting,
    ) -> Result<Response, Failure> {
        let (builder, headers) = parser(accounting)?;
        let connection = self
            .transport
            .connect(destination)
            .await
            .map_err(|_| Failure::Transport)?;
        accounting.check_time()?;
        let quota = Quota::default();
        let (mut sender, driver) = builder
            .handshake(TokioIo::new(BoundedIo::new(
                connection,
                quota.clone(),
                headers,
                accounting.read_timing()?,
            )))
            .await
            .map_err(|_| Failure::Transport)?;
        accounting.check_time()?;
        let path = identity.url().query().map_or_else(
            || identity.url().path().to_owned(),
            |query| format!("{}?{query}", identity.url().path()),
        );
        let mut request = WireRequest::builder()
            .method("GET")
            .uri(path)
            .body(Empty::<Bytes>::new())
            .map_err(|_| Failure::Content)?;
        if let Some(credentials) = fetch.credentials {
            *request.headers_mut() = credentials.for_url(identity.url());
        }
        let host = identity.url().host_str().ok_or(Failure::Content)?;
        let authority = identity
            .url()
            .port()
            .map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"));
        request
            .headers_mut()
            .insert(HOST, authority.parse().map_err(|_| Failure::Content)?);
        // Each hop owns a fresh connection; never read an idle next response.
        request
            .headers_mut()
            .insert(CONNECTION, HeaderValue::from_static("close"));
        request.headers_mut().insert(
            ACCEPT_ENCODING,
            "gzip, deflate, identity"
                .parse()
                .map_err(|_| Failure::Content)?,
        );
        let result = {
            let work = async {
                accounting.check_time()?;
                let response = sender
                    .send_request(request)
                    .await
                    .map_err(|error| protocol_error(&error))?;
                accounting.check_time()?;
                read_response(response, identity, fetch.robots, accounting, &quota).await
            };
            tokio::pin!(work);
            tokio::select! { biased;
                result = &mut work => result,
                result = driver => match result {
                    Ok(()) => work.await,
                    Err(error) => Err(protocol_error(&error)),
                }
            }
        };
        charge_quota(&quota, accounting)?;
        accounting.validate()?;
        result
    }
}
/// Reject invalid/backward clocks instead of treating elapsed time as zero.
fn elapsed(accounting: &Accounting, started: StdInstant) -> Result<Duration, Failure> {
    accounting
        .now()
        .checked_duration_since(started)
        .ok_or(Failure::Configuration)
}
/// Checked translation to the injected ledger's trusted monotonic domain.
fn millis(now: StdInstant, epoch: StdInstant) -> Result<u64, Failure> {
    u64::try_from(
        now.checked_duration_since(epoch)
            .ok_or(Failure::Configuration)?
            .as_millis(),
    )
    .map_err(|_| Failure::Configuration)
}
