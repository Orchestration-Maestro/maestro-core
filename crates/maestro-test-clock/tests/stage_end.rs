//! Stage completion must wait for the watched span's outcome.

use maestro_test_clock::StageEnd;
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};
use tracing::field::Empty;

#[tokio::test]
async fn ended_waits_for_the_watched_stage_outcome() {
    let stage = StageEnd::watch("test.stage");
    let span = tracing::info_span!("test.stage", outcome = Empty);
    let mut ended = pin!(stage.ended());
    let mut context = Context::from_waker(Waker::noop());

    assert_eq!(ended.as_mut().poll(&mut context), Poll::Pending);
    span.record("outcome", "ok");
    assert_eq!(ended.as_mut().poll(&mut context), Poll::Ready(()));
}

#[tokio::test]
async fn ended_waits_for_every_watched_stage_outcome() {
    let stages = StageEnd::watch_all(&["test.first", "test.second"]);
    let unrelated = tracing::info_span!("test.unrelated", outcome = Empty);
    let first = tracing::info_span!("test.first", outcome = Empty, progress = Empty);
    let second = tracing::info_span!("test.second", outcome = Empty);
    let mut ended = pin!(stages.ended());
    let mut context = Context::from_waker(Waker::noop());

    unrelated.record("outcome", "ok");
    first.record("progress", 1);
    assert_eq!(ended.as_mut().poll(&mut context), Poll::Pending);
    first.record("outcome", "ok");
    assert_eq!(ended.as_mut().poll(&mut context), Poll::Pending);
    first.record("outcome", "ok");
    assert_eq!(ended.as_mut().poll(&mut context), Poll::Pending);
    second.record("outcome", "ok");
    assert_eq!(ended.as_mut().poll(&mut context), Poll::Ready(()));
}
