//! The fake's exact keyword-condition evaluation shared by query and scroll.

use qdrant_client::qdrant::{
    Filter, RetrievedPoint, condition::ConditionOneOf, r#match::MatchValue, value::Kind,
};

/// Applies every required field condition before a result is ranked or paged.
pub(super) fn matches_filter(point: &RetrievedPoint, filter: &Filter) -> bool {
    filter.must.iter().all(|condition| {
        let Some(ConditionOneOf::Field(field)) = condition.condition_one_of.as_ref() else {
            return false;
        };
        let Some(matched) = field
            .r#match
            .as_ref()
            .and_then(|matched| matched.match_value.as_ref())
        else {
            return false;
        };
        match point
            .payload
            .get(&field.key)
            .and_then(|value| value.kind.as_ref())
        {
            Some(Kind::StringValue(value)) => matches_keyword(matched, value),
            Some(Kind::ListValue(values)) => values.values.iter().any(|value| {
                let Some(Kind::StringValue(value)) = value.kind.as_ref() else {
                    return false;
                };
                matches_keyword(matched, value)
            }),
            _ => false,
        }
    })
}

/// Matches one exact string against a keyword or any keyword in a set.
fn matches_keyword(matched: &MatchValue, value: &str) -> bool {
    match matched {
        MatchValue::Keyword(keyword) => keyword == value,
        MatchValue::Keywords(keywords) => keywords.strings.iter().any(|keyword| keyword == value),
        _ => false,
    }
}
