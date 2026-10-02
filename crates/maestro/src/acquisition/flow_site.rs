//! Synthetic admitted HTTP site with explicit source revision cases.
use maestro_acquisition::{
    Refusal,
    transport::connect::{CheckedDestination, PinnedTransport},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _, DuplexStream, duplex};
/// Declared empty leaves differ from leaves with out-of-scope references.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Leaf {
    /// No candidate reference exists.
    #[default]
    Empty,
    /// A candidate exists beyond the leaf's depth.
    Linked,
    /// Definitive exclusions and an unresolved identity on the same leaf.
    Mixed,
}
/// Synthetic source revisions independent of a visible-text shortcut.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Revision {
    /// Original replies and link inventory.
    #[default]
    Original,
    /// Edited/deleted replies plus link-only discovery.
    Changed,
    /// Validator-only changes with identical bytes.
    Metadata,
}
/// In-memory HTTP site, including redirect, denied neighbours and an attachment.
#[derive(Clone, Debug, Default)]
pub(super) struct Site {
    /// Observed public paths, not a network route.
    pub(super) requests: Arc<Mutex<Vec<String>>>,
    /// Allowed-only fixture for completion and reuse tests.
    pub(super) clean: bool,
    /// Leaf references expose declared-scope and run-ceiling neighbours.
    pub(super) leaf: Leaf,
    /// A non-success content response is a pending transport disposition.
    pub(super) failure: bool,
    /// No declared media must not masquerade as a leaf attachment.
    pub(super) unknown_media: bool,
    /// Reply edits/deletions and a new link with unchanged visible anchor text.
    pub(super) revision: Revision,
}
impl PinnedTransport for Site {
    type Connection = DuplexStream;
    fn connect(
        &self,
        destination: CheckedDestination,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Connection, Refusal>> + Send + '_>> {
        assert_eq!(destination.socket().ip().to_string(), "8.8.8.8");
        let site = self.clone();
        Box::pin(async move {
            let (client, server) = duplex(4096);
            tokio::spawn(serve(server, site));
            Ok(client)
        })
    }
}

/// Complete one synthetic exchange; no socket or credential provider exists.
async fn serve(mut server: DuplexStream, site: Site) {
    let mut bytes = Vec::new();
    let mut byte = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        server.read_exact(&mut byte).await.unwrap();
        bytes.push(byte[0]);
    }
    let request = String::from_utf8(bytes).unwrap();
    let path = request.split_whitespace().nth(1).unwrap().to_owned();
    assert!(!request.to_ascii_lowercase().contains("authorization:"));
    site.requests.lock().unwrap().push(path.clone());
    let revision = revised(&path, site.revision);
    let (status, mut headers, body) = revision.unwrap_or_else(|| match path.as_str() {
        "/robots.txt" => (
            200,
            "Content-Type: text/plain\r\n",
            "User-agent: *\nDisallow: /docs/robot\n",
        ),
        _ if site.failure => (
            502,
            "Content-Type: text/html\r\n",
            "failed synthetic response",
        ),
        "/docs/start" if site.clean => (
            200,
            "Content-Type: text/html\r\n",
            "<a href='/docs/allowed'>a</a><a href='/docs/final'>b</a>",
        ),
        "/docs/allowed" if site.clean && site.leaf == Leaf::Empty => {
            (200, "Content-Type: text/html\r\n", "child one")
        }
        "/docs/final" if site.clean && site.leaf == Leaf::Empty => {
            (200, "Content-Type: text/html\r\n", "different child two")
        }
        "/docs/allowed" | "/docs/final" if site.leaf == Leaf::Mixed => (
            200,
            "Content-Type: text/html\r\n",
            concat!(
                "<a href='/docs/private'>denied</a>",
                "<a href='mailto:test@example.invalid'>non-fetch</a>",
                "<a href='/docs/unknown?secret=x'>unresolved</a>",
                "<a href='/docs/beyond'>eligible</a>"
            ),
        ),
        "/docs/start" => (
            200,
            "Content-Type: text/html\r\n",
            concat!(
                "<a href='/docs/allowed'>a</a><a href='/docs/private'>deny</a>",
                "<a href='/docs/robot'>robot</a><a href='/docs/redirect'>redirect</a>",
                "<a href='/docs/file.pdf'>file</a><a href='mailto:test@example.invalid'>mail</a>",
                "<a href='/docs/unknown?secret=x'>unknown</a>"
            ),
        ),
        "/docs/redirect" => (302, "Location: /docs/final\r\n", ""),
        "/docs/file.pdf" => (200, "Content-Type: application/pdf\r\n", "%PDF synthetic"),
        "/docs/allowed" | "/docs/final" => (
            200,
            "Content-Type: text/html\r\n",
            "<a href='/docs/beyond'>beyond</a>",
        ),
        _ if path.starts_with("/docs/legacy-") => (
            200,
            "Content-Type: application/pdf\r\n",
            "%PDF-1.4 synthetic legacy attachment",
        ),
        _ => panic!("unexpected fixture request {path}"),
    });
    if site.unknown_media && path != "/robots.txt" {
        headers = "";
    }
    let validators = if site.revision == Revision::Metadata {
        "ETag: synthetic-v2\r\n"
    } else {
        ""
    };
    let response = format!(
        "HTTP/1.1 {status} Synthetic\r\nContent-Length: {}\r\n{headers}{validators}\r\n{body}",
        body.len()
    );
    server.write_all(response.as_bytes()).await.unwrap();
}
/// Source-only fixture changes do not add complexity to the HTTP adapter.
fn revised(path: &str, revision: Revision) -> Option<(u16, &'static str, &'static str)> {
    let body = match (path, revision) {
        ("/docs/start", Revision::Changed) => {
            "<a href='/docs/new'>a</a><a href='/docs/final'>b</a>"
        }
        ("/docs/allowed", Revision::Changed) => "edited reply; deleted second reply",
        ("/docs/new", _) => "newly discovered reply",
        _ => return None,
    };
    Some((200, "Content-Type: text/html\r\n", body))
}
