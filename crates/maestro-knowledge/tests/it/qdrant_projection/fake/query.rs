//! Qdrant's filtered nearest-vector query, as the projection tests' fake serves it.

use super::state::Fake;
use qdrant_client::qdrant::{
    Filter, QueryPoints, QueryResponse, RetrievedPoint, ScoredPoint, condition::ConditionOneOf,
    r#match::MatchValue, query, value::Kind, vector_input::Variant as VectorInputVariant,
    vector_output, vectors_output, with_payload_selector,
};
use std::collections::HashMap;
use tonic::{Request, Response, Status};

#[derive(Debug)]
enum SearchVector {
    Dense(Vec<f32>),
    Sparse(Vec<u32>, Vec<f32>),
}

/// Serves the request against the named collection and applies its scope filter before ranking.
pub(super) fn run(
    fake: &Fake,
    request: Request<QueryPoints>,
) -> Result<Response<QueryResponse>, Status> {
    fake.admit("query")?;
    let request = request.into_inner();
    let name = request.collection_name;
    let vector_name = request.using.unwrap_or_else(|| "dense".to_owned());
    let query = request
        .query
        .and_then(|query| query.variant)
        .and_then(|variant| match variant {
            query::Variant::Nearest(input) => input.variant,
            _ => None,
        })
        .ok_or_else(|| Status::invalid_argument("the fake needs a nearest-vector query"))?;
    let query = match query {
        VectorInputVariant::Dense(vector) => SearchVector::Dense(vector.data),
        VectorInputVariant::Sparse(vector) => SearchVector::Sparse(vector.indices, vector.values),
        _ => {
            return Err(Status::invalid_argument(
                "the fake needs a dense or sparse vector",
            ));
        }
    };
    let limit = request
        .limit
        .and_then(|limit| usize::try_from(limit).ok())
        .unwrap_or(10);
    let payload = matches!(
        request
            .with_payload
            .and_then(|selector| selector.selector_options),
        Some(with_payload_selector::SelectorOptions::Enable(true))
    );
    let filter = request.filter;
    let offset = request
        .offset
        .and_then(|offset| usize::try_from(offset).ok())
        .unwrap_or(0);
    let mut state = fake.state();
    let collection = state.collection(&name)?;
    let mut ranked: Vec<_> = collection
        .points
        .values()
        .filter(|point| {
            filter
                .as_ref()
                .is_none_or(|filter| matches_filter(point, filter))
        })
        .filter_map(|point| score(point, &vector_name, &query).map(|score| (point, score)))
        .collect();
    ranked.sort_by(|(_, left), (_, right)| right.total_cmp(left));
    let result = ranked
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(point, score)| ScoredPoint {
            id: point.id.clone(),
            payload: if payload {
                point.payload.clone()
            } else {
                HashMap::new()
            },
            score,
            ..ScoredPoint::default()
        })
        .collect();
    Ok(Response::new(QueryResponse {
        result,
        time: 0.0,
        usage: None,
    }))
}

fn matches_filter(point: &RetrievedPoint, filter: &Filter) -> bool {
    filter.must.iter().all(|condition| {
        let Some(ConditionOneOf::Field(field)) = condition.condition_one_of.as_ref() else {
            return false;
        };
        if field.key != "scope_tags" {
            return false;
        }
        let Some(value) = field
            .r#match
            .as_ref()
            .and_then(|value| value.match_value.as_ref())
        else {
            return false;
        };
        let granted: Vec<&str> = match value {
            MatchValue::Keyword(scope) => vec![scope],
            MatchValue::Keywords(scopes) => scopes.strings.iter().map(String::as_str).collect(),
            _ => return false,
        };
        let Some(Kind::ListValue(tags)) = point
            .payload
            .get("scope_tags")
            .and_then(|value| value.kind.as_ref())
        else {
            return false;
        };
        tags.values.iter().any(|tag| {
            matches!(
                tag.kind.as_ref(),
                Some(Kind::StringValue(tag)) if granted.contains(&tag.as_str())
            )
        })
    })
}

fn score(point: &RetrievedPoint, name: &str, query: &SearchVector) -> Option<f32> {
    let Some(vectors_output::VectorsOptions::Vectors(named)) = point
        .vectors
        .as_ref()
        .and_then(|vectors| vectors.vectors_options.as_ref())
    else {
        return None;
    };
    let vector = named.vectors.get(name)?.vector.as_ref()?;
    match (vector, query) {
        (vector_output::Vector::Dense(point), SearchVector::Dense(query))
            if point.data.len() == query.len() =>
        {
            let dot = point
                .data
                .iter()
                .zip(query)
                .map(|(left, right)| left * right)
                .sum::<f32>();
            let norm = query.iter().map(|value| value * value).sum::<f32>().sqrt();
            (norm > 0.0).then_some(dot / norm)
        }
        (vector_output::Vector::Sparse(point), SearchVector::Sparse(indices, values)) => Some(
            indices
                .iter()
                .zip(values)
                .filter_map(|(index, value)| {
                    point
                        .indices
                        .binary_search(index)
                        .ok()
                        .and_then(|at| point.values.get(at))
                        .map(|stored| stored * value)
                })
                .sum(),
        ),
        _ => None,
    }
}
