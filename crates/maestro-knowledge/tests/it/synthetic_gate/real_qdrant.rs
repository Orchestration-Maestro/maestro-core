//! Exercises the mandatory synthetic pipeline against the pinned Qdrant service.

use super::{pipeline, qdrant::REAL_URL, support::TestDirectory};
use qdrant_client::{
    Qdrant,
    qdrant::{
        CreateCollectionBuilder, DeleteCollection, Distance, VectorParamsBuilder,
        VectorsConfigBuilder,
    },
};
use std::{
    env,
    error::Error,
    io,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
};
use tokio::runtime::Builder;

static NEXT_REFUSAL_COLLECTION: AtomicU64 = AtomicU64::new(0);

#[test]
fn real_qdrant_runner_refuses_missing_and_foreign_urls() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new()?;
    let missing =
        pipeline::run_real_qdrant_baseline(&directory.path.join("missing"), None).unwrap_err();
    assert_eq!(missing.stage, "qdrant-url");

    let foreign = pipeline::run_real_qdrant_baseline(
        &directory.path.join("foreign"),
        Some("https://qdrant.example.org:6334"),
    )
    .unwrap_err();
    assert_eq!(foreign.stage, "qdrant-url-ownership");
    Ok(())
}

#[test]
#[ignore = "requires a test-owned pinned Qdrant scratch service at 127.0.0.1:16634"]
fn real_qdrant_runs_synthetic_gate() -> Result<(), Box<dyn Error>> {
    let report_dir = PathBuf::from(
        env::var_os("MAESTRO_SYNTHETIC_REPORT_DIR")
            .expect("MAESTRO_SYNTHETIC_REPORT_DIR is required"),
    );
    let url = env::var("MAESTRO_QDRANT_URL").ok();
    let url_ref = url.as_deref();
    assert_nonempty_service_is_refused(url_ref)?;
    let (first, second) = match pipeline::run_real_qdrant_baseline(&report_dir, url_ref) {
        Ok(outputs) => outputs,
        Err(failure) => {
            pipeline::append_summary(&failure.to_string());
            panic!("{failure}");
        }
    };
    pipeline::append_summary(&pipeline::summary(&second));
    pipeline::assert_repeated_run_identity(&first, &second);
    eprintln!(
        "real Qdrant repeated-run identity matched; second-run metrics: recall@5={:?}, \
         recall@10={:?}, mrr@10={:?}, ndcg@10={:?}, no-answer={:?}, false-abstentions={:?}",
        second.report.metrics.recall_at_5.map(|metric| metric.value),
        second
            .report
            .metrics
            .recall_at_10
            .map(|metric| metric.value),
        second.report.metrics.mrr_at_10.map(|metric| metric.value),
        second.report.metrics.ndcg_at_10.map(|metric| metric.value),
        second
            .report
            .metrics
            .no_answer_accuracy
            .map(|metric| metric.value),
        second
            .report
            .metrics
            .false_abstentions
            .map(|metric| metric.value),
    );
    Ok(())
}

fn assert_nonempty_service_is_refused(url: Option<&str>) -> Result<(), Box<dyn Error>> {
    let Some(url) = url.filter(|url| *url == REAL_URL) else {
        return Err(io::Error::other("expected the test-owned Qdrant URL").into());
    };
    let runtime = Builder::new_current_thread().enable_all().build()?;
    let client = Qdrant::from_url(url).skip_compatibility_check().build()?;
    let collection = loop {
        let name = format!(
            "maestro-synthetic-refusal-{}-{}",
            process::id(),
            NEXT_REFUSAL_COLLECTION.fetch_add(1, Ordering::Relaxed)
        );
        if !runtime.block_on(client.collection_exists(&name))? {
            break name;
        }
    };
    let mut vectors = VectorsConfigBuilder::default();
    vectors.add_named_vector_params("dense", VectorParamsBuilder::new(1, Distance::Dot));
    runtime.block_on(
        client.create_collection(CreateCollectionBuilder::new(&collection).vectors_config(vectors)),
    )?;

    let refusal_dir = TestDirectory::new()?;
    let refusal = pipeline::run_real_qdrant_baseline(&refusal_dir.path.join("report"), Some(url));
    runtime.block_on(client.delete_collection(DeleteCollection {
        collection_name: collection,
        timeout: None,
    }))?;
    let Err(refusal) = refusal else {
        return Err(io::Error::other("non-empty Qdrant service was accepted").into());
    };
    if refusal.stage != "qdrant-not-empty" {
        return Err(io::Error::other(format!(
            "unexpected non-empty refusal stage: {}",
            refusal.stage
        ))
        .into());
    }
    Ok(())
}
