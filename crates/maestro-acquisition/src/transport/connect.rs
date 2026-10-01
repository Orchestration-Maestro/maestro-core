//! One checked destination per connection, with no proxy, pool or second DNS.
use super::address::AddressTable;
use crate::{policy::identity::FetchIdentity, refusal::Refusal};
use reqwest::{Url, header::HeaderMap};
use rustls::{ClientConfig, RootCertStore, crypto::ring, pki_types::ServerName};
use std::{fmt, future::Future, net::SocketAddr, pin::Pin, sync::Arc};
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, client::TlsStream};

pub use super::dns::{Resolver, SystemResolver};
/// Immutable checked address and certificate hostname; callers cannot forge it.
#[derive(Debug)]
pub struct CheckedDestination {
    /// Exact socket address selected after checking every candidate.
    socket: SocketAddr,
    /// URL hostname used for certificate verification, never the numeric IP.
    hostname: String,
}
impl CheckedDestination {
    /// Resolve once and check every candidate before selecting the first.
    /// Re-run for every dispatch; old connections cannot be pooled or rebound.
    ///
    /// # Errors
    /// Empty, excessive, failed or partially denied DNS answers refuse entirely.
    pub fn resolve(
        identity: &FetchIdentity,
        table: &AddressTable,
        resolver: &dyn Resolver,
    ) -> Result<Self, Refusal> {
        let hostname = identity.url().domain().ok_or(Refusal::Invalid)?;
        let addresses = resolver.resolve(hostname)?;
        if addresses.len() > 256 {
            return Err(Refusal::Access);
        }
        let checked = addresses
            .iter()
            .map(|address| table.admit(address))
            .collect::<Result<Vec<_>, _>>()?;
        let ip = *checked.first().ok_or(Refusal::Access)?;
        Ok(Self {
            socket: SocketAddr::new(
                ip,
                identity
                    .url()
                    .port_or_known_default()
                    .ok_or(Refusal::Invalid)?,
            ),
            hostname: hostname.into(),
        })
    }
    /// Only this selected socket may receive a connection.
    #[must_use]
    pub fn socket(&self) -> SocketAddr {
        self.socket
    }
    /// Certificate verification and SNI use the original admitted DNS hostname.
    #[must_use]
    pub fn hostname(&self) -> &str {
        &self.hostname
    }
}
/// Origin-bound credential material; Debug never exposes header bytes.
pub struct OriginCredentials {
    /// Exact scheme/host/effective-port tuple.
    origin: String,
    /// Protected origin-specific headers; never a global default header map.
    headers: HeaderMap,
}
impl fmt::Debug for OriginCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OriginCredentials([redacted])")
    }
}
impl OriginCredentials {
    /// Bind all supplied sensitive headers to one origin.
    #[must_use]
    pub fn new(origin: &FetchIdentity, mut headers: HeaderMap) -> Self {
        for value in headers.values_mut() {
            value.set_sensitive(true);
        }
        Self {
            origin: origin.url().origin().ascii_serialization(),
            headers,
        }
    }
    /// A new origin receives no credentials, even for an approved auth hop.
    #[must_use]
    pub fn for_url(&self, url: &Url) -> HeaderMap {
        if self.origin == url.origin().ascii_serialization() {
            self.headers.clone()
        } else {
            HeaderMap::new()
        }
    }
}
/// Admit fresh DNS answers on every connection; never reuse a prior socket.
/// The caller supplies its currently checked policy and URL admission.
///
/// # Errors
/// Any denied candidate or failed connection refuses before subsequent work.
pub async fn connect<T: PinnedTransport>(
    identity: &FetchIdentity,
    table: &AddressTable,
    resolver: &dyn Resolver,
    transport: &T,
) -> Result<T::Connection, Refusal> {
    let destination = CheckedDestination::resolve(identity, table, resolver)?;
    transport.connect(destination).await
}

/// Native pinned TLS: socket-only dialing, platform roots and hostname validation.
/// Each call opens a new connection; there is no pool, proxy or redirect engine.
#[derive(Clone)]
pub struct NativeTlsTransport {
    /// Immutable TLS verifier with the explicit ring provider.
    connector: TlsConnector,
}
impl fmt::Debug for NativeTlsTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NativeTlsTransport")
    }
}
impl NativeTlsTransport {
    /// Load platform roots; an unavailable or empty root store refuses.
    ///
    /// # Errors
    /// Root loading, invalid certificates or TLS configuration failures refuse.
    pub fn from_native_roots() -> Result<Self, Refusal> {
        let native = rustls_native_certs::load_native_certs();
        if !native.errors.is_empty() || native.certs.is_empty() {
            return Err(Refusal::Unsupported);
        }
        let mut roots = RootCertStore::empty();
        for certificate in native.certs {
            roots.add(certificate).map_err(|_| Refusal::Unsupported)?;
        }
        Ok(Self {
            connector: TlsConnector::from(Arc::new(tls_config(roots)?)),
        })
    }
}
/// Explicit provider and roots, never dangerous certificate-verifier overrides.
fn tls_config(roots: RootCertStore) -> Result<ClientConfig, Refusal> {
    Ok(
        ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|_| Refusal::Unsupported)?
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}
impl PinnedTransport for NativeTlsTransport {
    type Connection = TlsStream<TcpStream>;
    fn connect(
        &self,
        destination: CheckedDestination,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Connection, Refusal>> + Send + '_>> {
        Box::pin(async move {
            let hostname =
                ServerName::try_from(destination.hostname).map_err(|_| Refusal::Invalid)?;
            let stream = TcpStream::connect(destination.socket)
                .await
                .map_err(|_| Refusal::Access)?;
            self.connector
                .connect(hostname, stream)
                .await
                .map_err(|_| Refusal::Access)
        })
    }
}

/// Replaceable pinned TLS transport. The adapter never receives a URL to resolve.
pub trait PinnedTransport: fmt::Debug {
    /// Established connection type, consumed by the bounded HTTP layer in N09.
    type Connection;
    /// Connect only to the checked socket, validating the checked hostname.
    ///
    /// # Errors
    /// Socket, TLS or certificate failures refuse; no fallback is allowed.
    fn connect(
        &self,
        destination: CheckedDestination,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Connection, Refusal>> + Send + '_>>;
}

#[cfg(test)]
mod tests {
    // Qualified synthetic boundary: private destination construction is test-only.
    use super::{CheckedDestination, NativeTlsTransport, PinnedTransport, tls_config};
    use maestro_test_scratch::scratch_directory;
    use rustls::{
        RootCertStore, ServerConfig,
        crypto::ring,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, pem::PemObject},
    };
    use std::{env, fs, process::Command, sync::Arc, time::Duration};
    use tokio::{net::TcpListener, runtime::Builder, time::timeout};
    use tokio_rustls::{TlsAcceptor, TlsConnector};

    /// Synthetic bytes are public test certificates, never a production trust anchor.
    fn certificate() -> CertificateDer<'static> {
        CertificateDer::from_pem_slice(include_bytes!("../../tests/fixtures/tls/certificate.pem"))
            .unwrap()
    }
    /// Real TLS over a qualified loopback fixture, without an admission bypass API.
    fn handshake(hostname: &str, trusted: bool) -> bool {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let socket = listener.local_addr().unwrap();
                let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                    serde_json::from_str::<Vec<u8>>(include_str!(
                        "../../tests/fixtures/tls/synthetic-key.json"
                    ))
                    .unwrap(),
                ));
                let server =
                    ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
                        .with_safe_default_protocol_versions()
                        .unwrap()
                        .with_no_client_auth()
                        .with_single_cert(vec![certificate()], key)
                        .unwrap();
                let task = tokio::spawn(async move {
                    let (stream, _) = listener.accept().await.unwrap();
                    TlsAcceptor::from(Arc::new(server)).accept(stream).await
                });
                let mut roots = RootCertStore::empty();
                if trusted {
                    roots.add(certificate()).unwrap();
                }
                let transport = NativeTlsTransport {
                    connector: TlsConnector::from(Arc::new(tls_config(roots).unwrap())),
                };
                let destination = CheckedDestination {
                    socket,
                    hostname: hostname.into(),
                };
                let accepted = timeout(Duration::from_secs(3), transport.connect(destination))
                    .await
                    .unwrap()
                    .is_ok();
                task.abort();
                accepted
            })
    }
    /// Certificate verification must use the hostname, not the selected IP address.
    #[test]
    fn n08_tls_verifies_hostname_and_roots() {
        assert!(handshake("garden.example", true));
        assert!(!handshake("other.example", true));
        assert!(!handshake("garden.example", false));
    }
    /// Root-loading failure cannot fall back to an unverified TLS connection.
    #[test]
    fn n08_native_roots_fail_closed() {
        if let Ok(expected) = env::var("N08_ROOT_CHILD") {
            assert_eq!(
                NativeTlsTransport::from_native_roots().is_ok(),
                expected == "valid"
            );
            return;
        }
        let scratch = scratch_directory().unwrap();
        let cert = scratch.join("root.pem");
        let empty = scratch.join("empty.pem");
        let directory = scratch.join("empty-roots");
        fs::create_dir(&directory).unwrap();
        fs::write(
            &cert,
            include_bytes!("../../tests/fixtures/tls/certificate.pem"),
        )
        .unwrap();
        fs::write(&empty, b"").unwrap();
        let malformed = scratch.join("malformed.pem");
        let mixed = scratch.join("mixed.pem");
        let invalid_der = b"-----BEGIN CERTIFICATE-----\nbm90IGNlcnQ=\n-----END CERTIFICATE-----\n";
        fs::write(&malformed, invalid_der).unwrap();
        let mut mixed_bytes = include_bytes!("../../tests/fixtures/tls/certificate.pem").to_vec();
        mixed_bytes
            .extend_from_slice(b"-----BEGIN CERTIFICATE-----\n!\n-----END CERTIFICATE-----\n");
        fs::write(&mixed, mixed_bytes).unwrap();
        for (path, expected) in [
            (cert, "valid"),
            (empty, "empty"),
            (malformed, "malformed"),
            (mixed, "mixed"),
            (scratch.join("missing.pem"), "missing"),
        ] {
            let output = Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "transport::connect::tests::n08_native_roots_fail_closed",
                    "--nocapture",
                ])
                .env("N08_ROOT_CHILD", expected)
                .env("SSL_CERT_FILE", path)
                .env("SSL_CERT_DIR", &directory)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
        fs::remove_dir_all(scratch).unwrap();
    }
    /// Run with a poisoned ambient proxy in a child, never mutate shared test env.
    #[test]
    fn n08_tls_ignores_ambient_proxy() {
        if env::var_os("N08_PROXY_CHILD").is_some() {
            assert!(handshake("garden.example", true));
            return;
        }
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "transport::connect::tests::n08_tls_ignores_ambient_proxy",
                "--nocapture",
            ])
            .env("N08_PROXY_CHILD", "1")
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("ALL_PROXY", "http://127.0.0.1:1")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }
}
