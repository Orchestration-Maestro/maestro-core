//! The compatibility check of the committed event schemas: what a schema
//! generated from an event's type changes of what its committed predecessor
//! held. Within a major version a schema only gains optional properties
//! (docs/architecture/07 §3.2), so any other change is a breaking change,
//! descriptions aside. A change it cannot read is refused whole, so the
//! check never lets a breaking change through.

use super::subset::{
    BOUNDS, DEPTH, allowed_values, below, constants, json_type, located, members, names, number,
    resolved, types, unread,
};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, fmt::Display};

/// The keyword whose schema describes the properties an object's schema
/// does not name.
const ADDITIONAL: &str = "additionalProperties";

/// What `fresh` changes of what `committed` held, one line each, naming the
/// schema it is found at by its path below the root: none when `fresh` only
/// adds optional properties or describes.
pub(super) fn breaking_changes(committed: &Value, fresh: &Value) -> Vec<String> {
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
    /// What the fresh schema breaks.
    found: Vec<String>,
}

impl Check<'_> {
    /// Notes what the fresh schema at `at` breaks.
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
        for line in changed_here(&committed, &fresh) {
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
    /// changed, or more or fewer additional properties accepted.
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
        let (before, after) = (committed.get(ADDITIONAL), fresh.get(ADDITIONAL));
        if let (Some(old @ Value::Object(_)), Some(new @ Value::Object(_))) = (before, after) {
            self.compare(old, new, &below(at, ADDITIONAL), depth + 1);
            return;
        }
        match (openness(before), openness(after)) {
            (Some(old), Some(new)) if new < old => {
                self.note(at, "accepts fewer additional properties");
            }
            (Some(old), Some(new)) if new > old => {
                self.note(at, "accepts more additional properties");
            }
            (Some(_), Some(_)) => {}
            _ => self.note(
                at,
                "additionalProperties changed, which the check cannot compare",
            ),
        }
    }

    /// The items of an array changed, constrained where they were not, or
    /// left free where they were constrained.
    fn items(
        &mut self,
        committed: &Map<String, Value>,
        fresh: &Map<String, Value>,
        at: &str,
        depth: usize,
    ) {
        match (committed.get("items"), fresh.get("items")) {
            (None, None) => {}
            (Some(before), Some(after)) => {
                self.compare(before, after, &below(at, "items"), depth + 1);
            }
            (None, Some(_)) => self.note(at, "accepts fewer items"),
            (Some(_), None) => self.note(at, "accepts any items now"),
        }
    }

    /// A committed definition the comparison never reached, which the fresh
    /// schema changed: it may hold a breaking change the check cannot see.
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

/// How many additional properties the value of `additionalProperties` lets
/// through, in order: none (`false`), those its schema accepts, or any (no
/// value, or `true`). `None` for a value the check cannot read.
fn openness(value: Option<&Value>) -> Option<u8> {
    match value {
        Some(Value::Bool(false)) => Some(0),
        Some(Value::Object(_)) => Some(1),
        None | Some(Value::Bool(true)) => Some(2),
        Some(_) => None,
    }
}

/// What the fresh schema changes of a restriction the committed schema held,
/// on the JSON types or on the values it accepts, `None` accepting any:
/// a member lost or gained, or the restriction imposed or lifted. `noun`
/// names a member.
fn changed<T: Display + PartialEq>(
    committed: Option<Vec<T>>,
    fresh: Option<Vec<T>>,
    noun: &str,
) -> Vec<String> {
    match (committed, fresh) {
        (None, None) => Vec::new(),
        (None, Some(fresh)) => {
            let listed: Vec<String> = fresh.iter().map(ToString::to_string).collect();
            vec![format!("accepts only {} now", listed.join(" or "))]
        }
        (Some(_), None) => vec![format!("accepts any {noun} now")],
        (Some(committed), Some(fresh)) => {
            let lost = committed.iter().filter(|member| !fresh.contains(member));
            let gained = fresh.iter().filter(|member| !committed.contains(member));
            let lost = lost.map(|member| format!("no longer accepts {member}"));
            lost.chain(gained.map(|member| format!("now accepts {member}")))
                .collect()
        }
    }
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

/// What the fresh schema changes in the keywords of the schema itself: a
/// JSON type or a value it accepts, a bound, its format, a keyword the check
/// does not read, and a `$ref` it cannot follow.
fn changed_here(committed: &Map<String, Value>, fresh: &Map<String, Value>) -> Vec<String> {
    let mut lines = changed(kinds(committed), kinds(fresh), "type");
    lines.extend(changed(
        allowed_values(committed),
        allowed_values(fresh),
        "value",
    ));
    for keyword in BOUNDS.map(|bound| bound.keyword) {
        match (number(committed, keyword), number(fresh, keyword)) {
            (None, Some(after)) => lines.push(format!("{keyword} {after} added")),
            (Some(before), None) => lines.push(format!("{keyword} {before} removed")),
            (Some(before), Some(after)) if after > before => {
                lines.push(format!("{keyword} raised from {before} to {after}"));
            }
            (Some(before), Some(after)) if after < before => {
                lines.push(format!("{keyword} lowered from {before} to {after}"));
            }
            _ => {}
        }
    }
    let (before, after) = (committed.get("format"), fresh.get("format"));
    if before != after {
        let [before, after] = [before, after]
            .map(|format| format.map_or_else(|| "none".to_owned(), Value::to_string));
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
