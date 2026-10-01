//! Bounded HTTP/1 over one freshly admitted pinned connection per hop.
use super::{
    budget::compose,
    connect::{CheckedDestination, OriginCredentials, PinnedTransport},
    dns::Resolver,
    robots::{RobotsBinding, RobotsCache},
    stream::{Accounting, read_body},
};
use crate::{
    CheckedPolicy, Refusal,
    policy::{
        acquisition::Transport,
        authority::{Authority, Operation, Target},
        decision::{AdmissionControls, Request, RequestKind, admit},
        identity::FetchIdentity,
    },
};
use http_body_util::Empty;
use hyper::{
    Request as WireRequest,
    body::{Bytes, Incoming},
    client::conn::http1,
};
use hyper_util::rt::TokioIo;
use reqwest::header::{
    ACCEPT_ENCODING, CONTENT_TYPE, HOST, HeaderMap, HeaderName, HeaderValue, LOCATION,
};
use std::{collections::BTreeSet, fmt, time::SystemTime};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::{Instant, timeout_at},
};

pub use super::stream::Failure;
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
/// Authorized transient response, not a capture envelope or publish permission.
pub struct Response {
    /// Complete decoded body; every intermediate stage shares accounting.
    pub body: Vec<u8>,
    /// Complete encoded wire body, never mislabeled as decoded content.
    pub wire_body: Vec<u8>,
    /// Final HTTP status.
    pub status: u16,
    /// Selected noncredential metadata only.
    pub headers: HeaderMap,
    /// Admitted final fetch identity, protected from diagnostic output.
    pub identity: FetchIdentity,
    /// Redirect-only protected target; never part of safe capture metadata.
    location: Option<HeaderValue>,
}
impl fmt::Debug for Response {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Response")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}
/// Replaceable dependencies; no pool, proxy, implicit resolver or client defaults.
#[derive(Debug)]
pub struct Http<'a, T> {
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
}
impl<T: PinnedTransport> Http<'_, T>
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
        let deadline = accounting.deadline()?;
        let started = Instant::now();
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
        started: Instant,
    ) -> Result<Response, Failure> {
        let mut url = fetch.request.url.to_owned();
        let mut visited = BTreeSet::new();
        let mut redirects = 0;
        let mut retries = 0;
        let mut kind = fetch.request.kind;
        loop {
            let request = Request {
                url: &url,
                kind,
                ..fetch.request
            };
            let identity = if fetch.robots {
                let source = self
                    .policy
                    .policy()
                    .sources
                    .iter()
                    .find(|source| source.id == request.source_id)
                    .ok_or(Failure::Admission(Refusal::Access))?;
                self.controls
                    .caller(source, &request)
                    .map_err(Failure::Admission)?;
                let identity = self
                    .policy
                    .admit_robots(request.source_id, request.url)
                    .map_err(Failure::Admission)?;
                self.controls
                    .network(&identity)
                    .map_err(Failure::Admission)?;
                identity
            } else {
                admit(self.policy, &request, self.controls)
                    .map_err(Failure::Admission)?
                    .identity()
                    .clone()
            };
            if !visited.insert(identity.as_str().to_owned()) && kind != RequestKind::Retry {
                return Err(Failure::RedirectLoop);
            }
            accounting.request()?;
            let destination =
                CheckedDestination::resolve(&identity, self.policy.address_table(), self.resolver);
            accounting.check_time()?;
            let destination = destination.map_err(Failure::Admission)?;
            let authority = self.authorize(fetch, &identity, started);
            accounting.check_time()?;
            authority?;
            let result = self.hop(fetch, identity, destination, accounting).await;
            let response = match result {
                Err(Failure::Transport) if retries < accounting.limits().retries => {
                    retries += 1;
                    kind = RequestKind::Retry;
                    continue;
                }
                other => other?,
            };
            if !redirect(response.status) {
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
            // Preserve raw dot segments/escapes for N07 before URL normalization.
            url = redirect_url(response.identity.url(), location);
            kind = RequestKind::Redirect;
        }
    }
    /// N05 grant identity excludes query/fragment, but keeps this hop's exact path.
    /// Queries remain part of N07 fetch identity, never authority prefix widening.
    fn authorize(
        &self,
        fetch: &Fetch<'_>,
        identity: &FetchIdentity,
        started: Instant,
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
            .checked_add(started.elapsed())
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
        let builder = parser(accounting)?;
        let connection = self
            .transport
            .connect(destination)
            .await
            .map_err(|_| Failure::Transport)?;
        let (mut sender, driver) = builder
            .handshake(TokioIo::new(connection))
            .await
            .map_err(|_| Failure::Transport)?;
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
        request.headers_mut().insert(
            ACCEPT_ENCODING,
            "gzip, deflate, identity"
                .parse()
                .map_err(|_| Failure::Content)?,
        );
        let work = async {
            let response = sender
                .send_request(request)
                .await
                .map_err(|error| protocol_error(&error))?;
            read_response(response, identity, fetch.robots, accounting).await
        };
        tokio::pin!(work);
        tokio::select! { biased;
            result = &mut work => result,
            result = driver => { result.map_err(|error| protocol_error(&error))?; work.await }
        }
    }
}
/// Only standard HTTP redirect statuses trigger another admitted request.
fn redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// Typed status handling and body ownership live outside the driver future.
async fn read_response(
    response: hyper::Response<Incoming>,
    identity: FetchIdentity,
    robots: bool,
    accounting: &mut Accounting,
) -> Result<Response, Failure> {
    let status = response.status().as_u16();
    if !robots {
        check_status(status)?;
    }
    let location = response.headers().get(LOCATION).cloned();
    let mut headers = HeaderMap::new();
    for name in [CONTENT_TYPE] {
        if let Some(value) = response.headers().get(&name) {
            headers.insert(name, value.clone());
        }
    }
    let (body, wire_body) = if robots && (status == 404 || status == 410) {
        (Vec::new(), Vec::new())
    } else {
        read_body(response, accounting).await?
    };
    Ok(Response {
        body,
        wire_body,
        status,
        headers,
        identity,
        location,
    })
}
/// No authentication escalation, challenge solving or partial promotion.
fn check_status(status: u16) -> Result<(), Failure> {
    match status {
        401 => Err(Failure::Authentication),
        403 => Err(Failure::Challenge),
        206 => Err(Failure::Partial),
        _ => Ok(()),
    }
}

/// Resolve relative references without erasing raw segments before N07 checks.
fn redirect_url(base: &reqwest::Url, location: &str) -> String {
    if reqwest::Url::parse(location).is_ok() || location.contains("://") {
        return location.to_owned();
    }
    let origin = base.origin().ascii_serialization();
    if location.starts_with("//") {
        return format!("https:{location}");
    }
    if location.starts_with('/') {
        return format!("{origin}{location}");
    }
    if location.starts_with('?') {
        return format!("{origin}{}{location}", base.path());
    }
    if location.starts_with('#') || location.is_empty() {
        return format!("{}{location}", base.as_str());
    }
    let (parent, _) = base.path().rsplit_once('/').unwrap_or(("", ""));
    format!("{origin}{parent}/{location}")
}

/// Hyper's minimum bounded read buffer; the rest reserves header/index copies.
const PARSER_WORKSPACE: u64 = 32_768;
/// Byte and header-count ceilings are derived from the effective memory envelope.
fn parser(accounting: &mut Accounting) -> Result<http1::Builder, Failure> {
    accounting.workspace(PARSER_WORKSPACE)?;
    let memory = accounting
        .limits()
        .memory_bytes
        .min(accounting.limits().decode.memory_bytes)
        .get();
    let header_bytes = memory.min(PARSER_WORKSPACE) / 4;
    let header_count = usize::try_from(header_bytes).map_err(|_| Failure::Memory)?
        / size_of::<(HeaderName, HeaderValue)>();
    let mut builder = http1::Builder::new();
    let read_bytes = usize::try_from(header_bytes).map_err(|_| Failure::Memory)?;
    builder.max_buf_size(read_bytes).max_headers(header_count);
    Ok(builder)
}
/// Malformed/oversized protocol input is content failure, not a retryable socket.
fn protocol_error(error: &hyper::Error) -> Failure {
    if error.is_parse() {
        return Failure::Content;
    }
    if error.is_incomplete_message() {
        return Failure::Partial;
    }
    Failure::Transport
}
