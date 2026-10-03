//! N09 response projection into the single kernel-owned envelope contract.
use crate::transport::http::Response;
use maestro_kernel::{
    acquisition::{
        CaptureEnvelope, ReceiptError, Representation, SafeIdentity, Transport, safe_headers,
        safe_media,
    },
    artifact::Digest,
};

/// Bind N09's decoded HTTP payload, never DOM bytes or encoded transfer bytes.
/// The caller supplies frozen run/access/profile identities, not another model.
/// # Errors
/// A mismatched transport/representation refuses, never silently relabels content.
pub fn http_envelope(
    envelope: &mut CaptureEnvelope,
    response: &Response,
) -> Result<(), ReceiptError> {
    if envelope.transport != Transport::Http
        || envelope.representation != Representation::WireBody
        || envelope.source != response.identity.source_id()
    {
        return Err(ReceiptError::Invalid);
    }
    envelope.final_identity = SafeIdentity::new(response.identity.as_str())?;
    envelope.redirects.clone_from(&response.redirects);
    envelope.status = response.status;
    envelope.headers = safe_headers(&response.headers)?;
    envelope.declared_media = response
        .headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .and_then(safe_media);
    // Artifact identity uses N09's bounded identity output. Transfer gzip/length
    // remains header evidence, not a different document or a fabricated DOM.
    envelope.artifact = Digest::of(&response.body);
    envelope.length = response.body.len() as u64;
    Ok(())
}
