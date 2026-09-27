//! Filtered, point-ID-ordered pages for the fake's Qdrant scroll API.

use super::{
    filter::matches_filter,
    response::shown,
    state::{Fake, key},
};
use qdrant_client::qdrant::{ScrollPoints, ScrollResponse};
use tonic::{Request, Response, Status};

/// Applies the filter before offset and limit, as Qdrant's payload scroll does.
pub(super) fn run(
    fake: &Fake,
    request: Request<ScrollPoints>,
) -> Result<Response<ScrollResponse>, Status> {
    fake.admit("scroll")?;
    let scroll = request.into_inner();
    let payload = scroll
        .with_payload
        .and_then(|selector| selector.selector_options);
    let vectors = scroll
        .with_vectors
        .and_then(|selector| selector.selector_options);
    let offset = scroll.offset.as_ref().map(key);
    let limit = scroll
        .limit
        .and_then(|limit| usize::try_from(limit).ok())
        .unwrap_or(10);
    let mut state = fake.state();
    if let Some(filter) = &scroll.filter {
        state.scroll_filters.push(filter.clone());
    }
    let collection = state.collection(&scroll.collection_name)?;
    let mut matches = collection
        .points
        .iter()
        .filter(|(id, point)| {
            offset.as_ref().is_none_or(|offset| *id >= offset)
                && scroll
                    .filter
                    .as_ref()
                    .is_none_or(|filter| matches_filter(point, filter))
        })
        .map(|(_, point)| point)
        .collect::<Vec<_>>();
    let more = matches.len() > limit;
    let next_page_offset = more
        .then(|| matches.get(limit).and_then(|point| point.id.clone()))
        .flatten();
    matches.truncate(limit);
    let result = matches
        .into_iter()
        .map(|point| shown(point, payload.as_ref(), vectors.as_ref()))
        .collect();
    Ok(Response::new(ScrollResponse {
        next_page_offset,
        result,
        time: 0.0,
        usage: None,
    }))
}
