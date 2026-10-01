//! A validator of data against the committed event schemas, over the subset
//! of JSON Schema that schemars generates for the event types: what it
//! cannot check, it reports, never passes.

use super::subset::{
    BOUNDS, DEPTH, allowed_values, below, constants, json_type, located, members, names, number,
    resolved, types, unread,
};
use serde_json::{Map, Value, json};

/// Why `instance` does not follow `schema`, whose `$ref`s point into `root`,
/// one line each, naming the value at fault by its path: none when it
/// follows it.
pub(super) fn violations(instance: &Value, schema: &Value, root: &Value) -> Vec<String> {
    let mut validation = Validation {
        root,
        found: Vec::new(),
    };
    validation.check(instance, schema, "", 0);
    validation.found
}

/// One validation of a value against a schema.
struct Validation<'a> {
    /// The schema whole, which its `$ref`s point into.
    root: &'a Value,
    /// Why the value does not follow it.
    found: Vec<String>,
}

impl Validation<'_> {
    /// Notes why the value at `at` does not follow its schema.
    fn note(&mut self, at: &str, message: &str) {
        self.found.push(located(at, message));
    }

    /// Checks the value `instance` at `at` against `schema`, `depth` schemas
    /// below the root.
    fn check(&mut self, instance: &Value, schema: &Value, at: &str, depth: usize) {
        if depth > DEPTH {
            self.note(at, "nests too deep to check");
            return;
        }
        let (schema, _) = resolved(schema, self.root);
        let local = [
            unchecked(&schema),
            wrong_type(instance, &schema),
            unlisted(instance, &schema),
            out_of_range(instance, &schema),
        ];
        for line in local.concat() {
            self.note(at, &line);
        }
        if let Value::Object(fields) = instance {
            self.fields(fields, &schema, at, depth);
        }
        if let (Value::Array(elements), Some(items)) = (instance, schema.get("items")) {
            for (index, element) in elements.iter().enumerate() {
                self.check(element, items, &below(at, index), depth + 1);
            }
        }
    }

    /// Whether an object has every required property, and each of its
    /// properties follows the schema of its own.
    fn fields(
        &mut self,
        fields: &Map<String, Value>,
        schema: &Map<String, Value>,
        at: &str,
        depth: usize,
    ) {
        for name in names(schema, "required") {
            if !fields.contains_key(&name) {
                self.note(at, &format!("property {name} is missing"));
            }
        }
        let properties = members(schema, "properties");
        for (name, field) in fields {
            let place = below(at, name);
            match (properties.get(name), schema.get("additionalProperties")) {
                (Some(property), _) => self.check(field, property, &place, depth + 1),
                (None, Some(Value::Bool(false))) => {
                    self.note(at, &format!("property {name} is not allowed"));
                }
                (None, Some(other @ Value::Object(_))) => {
                    self.check(field, other, &place, depth + 1);
                }
                (None, _) => {}
            }
        }
    }
}

/// What the validator cannot check of `schema`: a keyword it does not read,
/// a `$ref` it cannot follow, a `oneOf` or an `anyOf` that does not list
/// constants, and a format other than `uint64`.
fn unchecked(schema: &Map<String, Value>) -> Vec<String> {
    let mut lines: Vec<String> = schema
        .keys()
        .filter(|key| unread(key))
        .map(|key| format!("cannot check {key}"))
        .collect();
    lines.extend(
        schema
            .get("$ref")
            .map(|reference| format!("cannot follow {reference}")),
    );
    let branches = ["oneOf", "anyOf"].into_iter().filter(|key| {
        let listed = schema.get(*key).map(constants);
        listed.is_some_and(|constants| constants.is_none())
    });
    lines.extend(branches.map(|key| format!("cannot check {key}")));
    let format = schema.get("format").and_then(Value::as_str);
    let other = format.filter(|format| *format != "uint64");
    lines.extend(other.map(|format| format!("cannot check format {format}")));
    lines
}

/// Whether `types` accept a value of the JSON type `kind`: a `number`
/// accepts an `integer`.
fn accepts(types: &[String], kind: &str) -> bool {
    let named = |name: &str| types.iter().any(|kind| kind == name);
    named(kind) || (kind == "integer" && named("number"))
}

/// Why `instance` is of no JSON type `schema` accepts, when it names them.
fn wrong_type(instance: &Value, schema: &Map<String, Value>) -> Vec<String> {
    let kind = json_type(instance);
    match types(schema) {
        Some(kinds) if !accepts(&kinds, kind) => {
            vec![format!(
                "is {kind} where the schema accepts {}",
                kinds.join(" or ")
            )]
        }
        _ => Vec::new(),
    }
}

/// Why `instance` is not a value `schema` lists, when it lists them.
fn unlisted(instance: &Value, schema: &Map<String, Value>) -> Vec<String> {
    let listed = allowed_values(schema);
    let missing = listed.filter(|values| !values.contains(instance));
    missing
        .map(|_| format!("{instance} is not a value the schema allows"))
        .into_iter()
        .collect()
}

/// Why the number `instance` is not a `uint64` its schema wants, or is past
/// one of its bounds.
fn out_of_range(instance: &Value, schema: &Map<String, Value>) -> Vec<String> {
    let Value::Number(number_value) = instance else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    let unsigned = schema
        .get("format")
        .is_some_and(|format| format == "uint64");
    if unsigned && !number_value.is_u64() {
        lines.push(format!("{instance} is not a uint64"));
    }
    let value = number_value.as_f64().unwrap_or(f64::NAN);
    for bound in BOUNDS {
        if number(schema, bound.keyword).is_some_and(|limit| (bound.outside)(value, limit)) {
            let (side, keyword) = (bound.side, bound.keyword);
            lines.push(format!(
                "{instance} is {side} the {keyword} {}",
                schema[keyword]
            ));
        }
    }
    lines
}

#[test]
fn the_validator_refuses_what_a_schema_does_not_allow() {
    let schema = json!({
        "type": "object",
        "properties": {
            "count": {"type": "integer", "format": "uint64", "minimum": 0, "maximum": 10},
            "state": {"$ref": "#/$defs/State"},
            "tags": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["count", "state"],
        "additionalProperties": false,
        "$defs": {"State": {"oneOf": [
            {"type": "string", "const": "a"},
            {"type": "string", "const": "b"}
        ]}}
    });
    let check = |instance: Value| violations(&instance, &schema, &schema);
    assert_eq!(
        check(json!({"count": 3, "state": "a", "tags": ["x"]})),
        Vec::<String>::new()
    );
    assert_eq!(
        check(json!({"count": -1, "state": "c", "tags": [1], "other": true})),
        [
            "/count: -1 is not a uint64",
            "/count: -1 is below the minimum 0",
            "property other is not allowed",
            "/state: \"c\" is not a value the schema allows",
            "/tags/0: is integer where the schema accepts string",
        ]
    );
    assert_eq!(
        check(json!({"count": 11.5})),
        [
            "property state is missing",
            "/count: is number where the schema accepts integer",
            "/count: 11.5 is not a uint64",
            "/count: 11.5 is above the maximum 10",
        ]
    );
    assert_eq!(
        check(json!("text")),
        ["is string where the schema accepts object"]
    );
}

#[test]
fn the_validator_reports_what_it_cannot_check() {
    let root = json!({});
    for (schema, found) in [
        (json!({"pattern": "^a"}), "cannot check pattern"),
        (
            json!({"format": "date-time"}),
            "cannot check format date-time",
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            "cannot check anyOf",
        ),
        (
            json!({"$ref": "#/$defs/Missing"}),
            "cannot follow \"#/$defs/Missing\"",
        ),
    ] {
        assert_eq!(violations(&json!("a"), &schema, &root), [found], "{schema}");
    }
}
