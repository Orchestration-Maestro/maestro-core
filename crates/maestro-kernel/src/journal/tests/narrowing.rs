//! The compatibility check of the committed event schemas: what a schema
//! generated from an event's type removes or narrows of what its committed
//! predecessor held. A change it cannot read is refused whole, so the check
//! never lets a narrowing through.

use super::subset::{
    BOUNDS, DEPTH, accepts, allowed_values, below, constants, json_type, located, members, names,
    number, resolved, types, unread,
};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, fmt::Display};

/// What `fresh` removes or narrows of what `committed` held, one line each,
/// naming the schema it is found at by its path below the root: none when
/// `fresh` only adds or describes.
pub(super) fn narrowings(committed: &Value, fresh: &Value) -> Vec<String> {
    let mut check = Check {
        committed,
        fresh,
        reached: BTreeSet::new(),
        found: Vec::new(),
    };
    check.compare(committed, fresh, "", 0);
    check.unreached();
    check.found
}

/// One comparison of a committed schema with a fresh one.
struct Check<'a> {
    /// The committed schema whole, which its `$ref`s point into.
    committed: &'a Value,
    /// The fresh schema whole, which its `$ref`s point into.
    fresh: &'a Value,
    /// The committed definitions the comparison reached through a `$ref`.
    reached: BTreeSet<String>,
    /// What the fresh schema removes or narrows.
    found: Vec<String>,
}

impl Check<'_> {
    /// Notes what the fresh schema at `at` removes or narrows.
    fn note(&mut self, at: &str, message: &str) {
        self.found.push(located(at, message));
    }

    /// Compares the committed schema `committed` with the fresh schema
    /// `fresh` at `at`, `depth` schemas below the root.
    fn compare(&mut self, committed: &Value, fresh: &Value, at: &str, depth: usize) {
        if depth > DEPTH {
            self.note(at, "nests too deep to compare");
            return;
        }
        let (committed, followed) = resolved(committed, self.committed);
        self.reached.extend(followed);
        let (fresh, _) = resolved(fresh, self.fresh);
        for line in narrowed_here(&committed, &fresh) {
            self.note(at, &line);
        }
        self.branches(&committed, &fresh, at, depth);
        self.properties(&committed, &fresh, at, depth);
        self.items(&committed, &fresh, at, depth);
    }

    /// The branches of a `oneOf` or an `anyOf`, when they are not the
    /// constants [`allowed_values`] reads: compared one by one when both
    /// schemas have as many, refused otherwise.
    fn branches(
        &mut self,
        committed: &Map<String, Value>,
        fresh: &Map<String, Value>,
        at: &str,
        depth: usize,
    ) {
        for key in ["oneOf", "anyOf"] {
            let (before, after) = (committed.get(key), fresh.get(key));
            let mut both = before.into_iter().chain(after);
            if both.all(|branches| constants(branches).is_some()) {
                continue;
            }
            let pairs = before
                .and_then(Value::as_array)
                .zip(after.and_then(Value::as_array))
                .filter(|(before, after)| before.len() == after.len());
            let Some((before, after)) = pairs else {
                let message = format!("{key} changed its branches, which the check cannot compare");
                self.note(at, &message);
                continue;
            };
            for (index, (old, new)) in before.iter().zip(after).enumerate() {
                self.compare(old, new, &below(at, format!("{key}/{index}")), depth + 1);
            }
        }
    }

    /// A property gone, newly required or no longer required, a property
    /// narrowed, or fewer additional properties accepted.
    fn properties(
        &mut self,
        committed: &Map<String, Value>,
        fresh: &Map<String, Value>,
        at: &str,
        depth: usize,
    ) {
        let fresh_properties = members(fresh, "properties");
        for (name, before) in members(committed, "properties") {
            match fresh_properties.get(&name) {
                Some(after) => self.compare(&before, after, &below(at, &name), depth + 1),
                None => self.note(at, &format!("property {name} is gone")),
            }
        }
        let (before, after) = (names(committed, "required"), names(fresh, "required"));
        for name in after.difference(&before) {
            self.note(at, &format!("property {name} is newly required"));
        }
        for name in before.difference(&after) {
            if fresh_properties.contains_key(name) {
                self.note(at, &format!("property {name} is no longer required"));
            }
        }
        match (
            committed.get("additionalProperties"),
            fresh.get("additionalProperties"),
        ) {
            (Some(Value::Bool(false)), _) | (_, None | Some(Value::Bool(true))) => {}
            (Some(before @ Value::Object(_)), Some(after @ Value::Object(_))) => {
                self.compare(before, after, &below(at, "additionalProperties"), depth + 1);
            }
            _ => self.note(at, "accepts fewer additional properties"),
        }
    }

    /// The items of an array narrowed, or constrained where they were not.
    fn items(
        &mut self,
        committed: &Map<String, Value>,
        fresh: &Map<String, Value>,
        at: &str,
        depth: usize,
    ) {
        match (committed.get("items"), fresh.get("items")) {
            (_, None) => {}
            (Some(before), Some(after)) => {
                self.compare(before, after, &below(at, "items"), depth + 1);
            }
            (None, Some(_)) => self.note(at, "accepts fewer items"),
        }
    }

    /// A committed definition the comparison never reached, which the fresh
    /// schema changed: it may hold a narrowing the check cannot see.
    fn unreached(&mut self) {
        let fresh = members(self.fresh.as_object().unwrap_or(&Map::new()), "$defs");
        let committed = members(self.committed.as_object().unwrap_or(&Map::new()), "$defs");
        for (name, definition) in committed {
            let reached = self.reached.contains(&format!("#/$defs/{name}"));
            if !reached && fresh.get(&name) != Some(&definition) {
                self.found.push(format!(
                    "definition {name} changed where the check does not follow it"
                ));
            }
        }
    }
}

/// What the fresh schema's restriction `fresh` takes away from the committed
/// schema's, `committed`: the JSON types or the values each accepts, `None`
/// accepting any, and `keeps` telling whether a restriction keeps a member.
fn lost<T: Display>(
    committed: Option<Vec<T>>,
    fresh: Option<Vec<T>>,
    keeps: impl Fn(&[T], &T) -> bool,
) -> Vec<String> {
    let Some(fresh) = fresh else {
        return Vec::new();
    };
    let Some(committed) = committed else {
        let listed: Vec<String> = fresh.iter().map(ToString::to_string).collect();
        return vec![format!("accepts only {} now", listed.join(" or "))];
    };
    committed
        .iter()
        .filter(|member| !keeps(&fresh, member))
        .map(|member| format!("no longer accepts {member}"))
        .collect()
}

/// The JSON types `schema` accepts, when it restricts them: those it names
/// in `type`, or those of the values it lists.
fn kinds(schema: &Map<String, Value>) -> Option<Vec<String>> {
    types(schema).or_else(|| {
        let values = allowed_values(schema)?;
        let kinds: BTreeSet<&str> = values.iter().map(json_type).collect();
        Some(kinds.into_iter().map(str::to_owned).collect())
    })
}

/// What the fresh schema narrows in the keywords of the schema itself: a
/// JSON type or a value it no longer accepts, a bound it tightens, a format
/// it gives anew, a keyword the check does not read that changed, and a
/// `$ref` it cannot follow.
fn narrowed_here(committed: &Map<String, Value>, fresh: &Map<String, Value>) -> Vec<String> {
    let keeps_type = |kinds: &[String], kind: &String| accepts(kinds, kind);
    let keeps_value = |values: &[Value], value: &Value| values.contains(value);
    let mut lines = lost(kinds(committed), kinds(fresh), keeps_type);
    lines.extend(lost(
        allowed_values(committed),
        allowed_values(fresh),
        keeps_value,
    ));
    for bound in BOUNDS {
        let Some(after) = number(fresh, bound.keyword) else {
            continue;
        };
        match number(committed, bound.keyword) {
            None => lines.push(format!("{} {after} added", bound.keyword)),
            Some(before) if (bound.outside)(before, after) => {
                let moved = bound.tightened;
                lines.push(format!(
                    "{} {moved} from {before} to {after}",
                    bound.keyword
                ));
            }
            Some(_) => {}
        }
    }
    let (before, after) = (committed.get("format"), fresh.get("format"));
    if let Some(after) = after.filter(|after| before != Some(after)) {
        let before = before.map_or_else(|| "none".to_owned(), Value::to_string);
        lines.push(format!("format {after} where it was {before}"));
    }
    let keys: BTreeSet<&String> = committed.keys().chain(fresh.keys()).collect();
    for key in keys.into_iter().filter(|key| unread(key)) {
        if committed.get(key) != fresh.get(key) {
            lines.push(format!("{key} changed, which the check does not read"));
        }
    }
    if let Some(reference) = committed.get("$ref").or_else(|| fresh.get("$ref")) {
        lines.push(format!("{reference} cannot be followed"));
    }
    lines
}
