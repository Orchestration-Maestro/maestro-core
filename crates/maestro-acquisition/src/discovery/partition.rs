//! Captured-page discovery acknowledges only after a durable bounded checkpoint.
use super::links::LinkExtractor;
use crate::{
    CheckedPolicy, Refusal,
    policy::{decision::check_target, identity::FetchIdentity},
    transport::budget::compose,
};
use maestro_kernel::{
    acquisition::{
        Batch, CaptureContext, CaptureEnvelope, Captures, ChangeKeys, DiscoveredItem, Enumeration,
        Handle, NewItem, Partition, Partitions, ReceiptError,
    },
    artifact::Digest,
};
use std::collections::BTreeMap;

/// Enumerate a current leased capture, checkpoint every eligible item, then acknowledge.
/// No transport starts here: N09 fetched the bytes, N12 captured them, and N09
/// must re-admit every later frontier dispatch under current authority/policy.
/// Completeness is relative to the recorded extractor selector contract only.
/// # Errors
/// Lost ownership, unsupported media, invalid policy or failed durable enqueue refuses.
pub async fn discover(
    store: &(impl Partitions + Captures),
    extractor: &dyn LinkExtractor,
    capture: (&CaptureContext, Handle),
    policy: &CheckedPolicy,
    mut partition: Partition,
) -> Result<Batch, ReceiptError> {
    let (context, handle) = capture;
    let source = policy
        .policy()
        .sources
        .iter()
        .find(|source| source.id == context.writer.source)
        .ok_or(ReceiptError::Invalid)?;
    let limits = compose(&[
        source.limits.clone(),
        policy.policy().aggregate_limits.clone(),
    ])
    .map_err(|_| ReceiptError::Invalid)?;
    partition.max_items = partition
        .max_items
        .min(u16::try_from(limits.pages.get().min(1000)).map_err(|_| ReceiptError::Invalid)?);
    partition.max_batches = partition
        .max_batches
        .min(u16::try_from(limits.partitions.get().min(1000)).map_err(|_| ReceiptError::Invalid)?);
    let max_bytes = limits
        .decode
        .expanded_bytes
        .get()
        .min(limits.dom_bytes.get())
        .min(limits.memory_bytes.get());
    let (envelope, bytes) = store.read_capture(context, handle, max_bytes)?;
    if envelope.profile != source.acquisition_profile.digest
        || envelope.declared_media.as_deref() != Some("text/html")
        || !(200..300).contains(&envelope.status)
    {
        return Err(ReceiptError::Invalid);
    }
    partition.run = envelope.run;
    partition.kind = Enumeration::Links;
    let mut batch = Batch {
        partition,
        cursor: None,
        next: None,
        terminal: true,
        stable: true,
        truncated: bytes.is_none(),
        expected: None,
        items: vec![],
        capture: Some(handle),
        extractor: Some(extractor.contract().to_owned()),
        denied: vec![],
    };
    if let Some(bytes) = bytes {
        let output = extractor
            .extract(envelope.final_identity.as_str(), &bytes)
            .await?;
        if output.contract != extractor.contract() {
            return Err(ReceiptError::Invalid);
        }
        let mut links = output.links;
        links.sort_unstable();
        links.dedup();
        let keys = keys(&envelope, &links)?;
        let mut eligible = BTreeMap::new();
        for url in links {
            let identity = FetchIdentity::parse(source, &url).and_then(|identity| {
                check_target(source, &identity, &policy.decisions)?;
                Ok(identity)
            });
            match identity {
                Ok(identity) => {
                    eligible.insert(identity.as_str().to_owned(), keys.clone());
                }
                Err(refusal) => {
                    batch.stable &= refusal == Refusal::Access;
                    batch.denied.push(Digest::of(url.as_bytes()));
                }
            }
        }
        batch.expected = u16::try_from(eligible.len()).ok();
        batch.truncated = output.limit_hit
            || eligible.len() > usize::from(batch.partition.max_items)
            || batch.denied.len() > 1000;
        batch.denied.truncate(1000);
        batch.items = eligible
            .into_iter()
            .take(usize::from(batch.partition.max_items))
            .map(|(fetch_identity, keys)| DiscoveredItem {
                request: NewItem {
                    fetch_identity,
                    authorization_context: envelope.authorization_context.clone(),
                    representation_profile: envelope.profile.clone(),
                },
                keys,
            })
            .collect();
    }
    store.checkpoint(&context.writer, &batch, context.now)?;
    if batch.stable && !batch.truncated {
        store.acknowledge_capture(context, handle)?;
    }
    Ok(batch)
}
/// Independent representation, metadata, permission, validator and canonical link keys.
fn keys(envelope: &CaptureEnvelope, links: &[String]) -> Result<ChangeKeys, ReceiptError> {
    Ok(ChangeKeys {
        revision: None,
        validator: Some(Digest::of(&serde_json::to_vec(&envelope.headers)?)),
        metadata: Some(Digest::of(&serde_json::to_vec(&(
            &envelope.declared_media,
            &envelope.detected_media,
        ))?)),
        permissions: envelope.authorization_context.clone(),
        links: Digest::of(&serde_json::to_vec(links)?),
        representation: envelope.artifact.clone(),
    })
}
