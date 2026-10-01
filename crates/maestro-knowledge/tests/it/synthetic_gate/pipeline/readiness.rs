//! Waits for an exact, green Qdrant collection before the real retrieval pass.

use maestro_kernel::generation::Generation;
use qdrant_client::{
    Qdrant,
    qdrant::{CollectionStatus, CountPointsBuilder},
};
use std::time::{Duration, Instant};
use tokio::{runtime::Runtime, task::yield_now};

pub(super) fn wait_for_green(
    runtime: &Runtime,
    generation: &Generation,
    expected_points: u64,
    url: &str,
) -> Result<(), String> {
    let client = Qdrant::from_url(url)
        .timeout(Duration::from_secs(10))
        .skip_compatibility_check()
        .build()
        .map_err(|error| error.to_string())?;
    let collection = format!("maestro-{}-g{}", generation.collection_id, generation.id);
    runtime.block_on(async {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let response = client
                .collection_info(collection.as_str())
                .await
                .map_err(|error| error.to_string())?;
            let info = response
                .result
                .ok_or_else(|| format!("Qdrant returned no info for {collection}"))?;
            let optimizer_ok = info
                .optimizer_status
                .as_ref()
                .is_some_and(|status| status.ok && status.error.is_empty());
            let expected_count_ready = info.status == CollectionStatus::Green as i32
                && optimizer_ok
                && info.points_count == Some(expected_points);
            let count = if expected_count_ready {
                client
                    .count(CountPointsBuilder::new(collection.as_str()).exact(true))
                    .await
                    .map_err(|error| error.to_string())?
                    .result
                    .map(|result| result.count)
            } else {
                None
            };
            if count == Some(expected_points) {
                eprintln!(
                    "Qdrant ready before search: {collection}, Green, optimizer ok, \
                     {expected_points} points"
                );
                return Ok(());
            }
            let status = format!(
                "status={:?}, optimizer_ok={optimizer_ok}, points={:?}, exact_points={count:?}",
                CollectionStatus::try_from(info.status).ok(),
                info.points_count
            );
            if Instant::now() >= deadline {
                return Err(format!("{collection} not ready within 120 s: {status}"));
            }
            yield_now().await;
        }
    })
}
