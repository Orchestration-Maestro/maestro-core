//! Effect-free type checking for the architecture 03 condition language.

use crate::limits::Limits;
use jsonschema::{Draft, Registry, Uri, uri};
use serde_json::{Number, Value};
use std::sync::Arc;

/// A lexical token. Literals retain their type, never executable text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token<'a> {
    /// Graph-local field or predicate variable.
    Name(&'a str),
    /// A string, number, boolean or null literal.
    Literal(&'static str),
    /// A language operator or delimiter.
    Mark(&'a str),
}

/// A JSON Schema type, retaining its nested shape for field/array access.
#[derive(Clone)]
struct Type<'a> {
    /// Integer and number share the numeric comparison domain.
    kind: &'static str,
    /// Object properties or array items, when this is a contract field.
    schema: Option<&'a Value>,
    /// Preserve the real resolver's resource scope through field/item selection.
    base: Option<Arc<Uri<String>>>,
}

/// A bounded recursive-descent checker; no AST evaluator or calls exist.
struct Parser<'a> {
    /// Already parsed source-node schema.
    root: &'a Value,
    /// The same prepared registry used for real contract validation.
    registry: &'a Registry<'a>,
    /// Tokens, bounded by the expression's bytes.
    tokens: Vec<Token<'a>>,
    /// Next token to consume.
    position: usize,
    /// Lexically scoped any/all variables.
    variables: Vec<(&'a str, Type<'a>)>,
    /// Parentheses, negations and predicates share a depth budget.
    depth: usize,
}

/// Check one condition against its source node's already parsed JSON Schema.
pub(super) fn check<'a>(
    text: &'a str,
    schema: &'a Value,
    registry: &'a Registry<'a>,
) -> Result<(), String> {
    if text.len() as u64 > Limits::PRODUCTION.source_file_bytes {
        return Err("condition exceeds source byte limit".to_owned());
    }
    let mut parser = Parser {
        root: schema,
        registry,
        tokens: lex(text)?,
        position: 0,
        variables: vec![],
        depth: 0,
    };
    let result = parser.expression()?;
    if parser.position != parser.tokens.len() || result.kind != "boolean" {
        return Err("condition must be one complete boolean expression".to_owned());
    }
    Ok(())
}

/// Tokenize only the named language; shell syntax, calls and arithmetic refuse.
fn lex(text: &str) -> Result<Vec<Token<'_>>, String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        if let Some(mark) = [
            "&&", "||", "==", "!=", "<=", ">=", "=>", "!", "<", ">", "(", ")", ",",
        ]
        .into_iter()
        .find(|mark| rest.starts_with(mark))
        {
            tokens.push(Token::Mark(mark));
            rest = rest.get(mark.len()..).unwrap_or_default();
        } else if rest.starts_with(['\'', '"']) {
            let size = string_length(rest)?;
            tokens.push(Token::Literal("string"));
            rest = rest.get(size..).unwrap_or_default();
        } else {
            let size = rest
                .bytes()
                .take_while(|byte| byte.is_ascii_alphanumeric() || b"_.-+".contains(byte))
                .count();
            let word = rest.get(..size).unwrap_or_default();
            let token = match word {
                "true" | "false" => Token::Literal("boolean"),
                "null" => Token::Literal("null"),
                _ if word.parse::<Number>().is_ok() => Token::Literal("number"),
                _ if identifier(word) => Token::Name(word),
                _ => return Err(format!("unsupported condition token {word:?}")),
            };
            tokens.push(token);
            rest = rest.get(size..).unwrap_or_default();
        }
    }
    Ok(tokens)
}

/// A field path consists solely of ASCII identifier segments.
fn identifier(text: &str) -> bool {
    text.split('.').all(|part| {
        part.bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}

/// Find and validate a quoted literal, including escapes, without executing it.
fn string_length(text: &str) -> Result<usize, String> {
    let quote = text.chars().next().ok_or("missing quote")?;
    let mut escaped = false;
    let mut unicode = 0;
    for (position, character) in text.char_indices().skip(1) {
        if unicode > 0 {
            if !character.is_ascii_hexdigit() {
                return Err("invalid unicode escape".to_owned());
            }
            unicode -= 1;
        } else if escaped {
            escaped = false;
            if character == 'u' {
                unicode = 4;
            } else if !"\\\"'/bnrtf".contains(character) {
                return Err("invalid string escape".to_owned());
            }
        } else if character == '\\' {
            escaped = true;
        } else if character == quote {
            return Ok(position + 1);
        } else if character.is_control() {
            return Err("control character in condition string".to_owned());
        }
    }
    Err("unterminated condition string".to_owned())
}

impl<'a> Parser<'a> {
    /// Consume an exact operator if present.
    fn take(&mut self, mark: &str) -> bool {
        if self.tokens.get(self.position) == Some(&Token::Mark(mark)) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    /// Require punctuation; never ignore a malformed suffix.
    fn require(&mut self, mark: &str) -> Result<(), String> {
        if self.take(mark) {
            Ok(())
        } else {
            Err(format!("expected {mark:?}"))
        }
    }

    /// Boolean disjunction, below conjunction in precedence.
    fn expression(&mut self) -> Result<Type<'a>, String> {
        let mut left = self.conjunction()?;
        while self.take("||") {
            let right = self.conjunction()?;
            boolean(left.kind)?;
            boolean(right.kind)?;
            left = literal("boolean");
        }
        Ok(left)
    }

    /// Boolean conjunction, below comparisons in precedence.
    fn conjunction(&mut self) -> Result<Type<'a>, String> {
        let mut left = self.comparison()?;
        while self.take("&&") {
            let right = self.comparison()?;
            boolean(left.kind)?;
            boolean(right.kind)?;
            left = literal("boolean");
        }
        Ok(left)
    }

    /// A single comparison, with matching primitive types.
    fn comparison(&mut self) -> Result<Type<'a>, String> {
        let left = self.primary()?;
        let Some(Token::Mark(mark @ ("==" | "!=" | "<" | ">" | "<=" | ">="))) =
            self.tokens.get(self.position).cloned()
        else {
            return Ok(left);
        };
        self.position += 1;
        let right = self.primary()?;
        if left.kind != right.kind || matches!(left.kind, "object" | "array") {
            return Err("comparison needs matching scalar types".to_owned());
        }
        if !matches!(mark, "==" | "!=") && left.kind != "number" {
            return Err("ordered comparison needs numbers".to_owned());
        }
        Ok(literal("boolean"))
    }

    /// Parentheses, ! and any/all are the only recursive constructs.
    fn primary(&mut self) -> Result<Type<'a>, String> {
        self.depth += 1;
        if self.depth > Limits::PRODUCTION.source_depth {
            return Err("condition nesting exceeds 32 levels".to_owned());
        }
        let result = self.atom();
        self.depth -= 1;
        result
    }

    /// Read a literal, field, grouped expression, negation or array predicate.
    fn atom(&mut self) -> Result<Type<'a>, String> {
        if self.take("!") {
            boolean(self.primary()?.kind)?;
            return Ok(literal("boolean"));
        }
        if self.take("(") {
            let inner = self.expression()?;
            self.require(")")?;
            return Ok(inner);
        }
        let token = self
            .tokens
            .get(self.position)
            .cloned()
            .ok_or("missing operand")?;
        self.position += 1;
        match token {
            Token::Literal(kind) => Ok(literal(kind)),
            Token::Name("any" | "all") if self.take("(") => self.predicate(),
            Token::Name(name) => self.field(name),
            Token::Mark(mark) => Err(format!("unexpected {mark:?}")),
        }
    }

    /// Type-check the predicate's lambda with its array item in lexical scope.
    fn predicate(&mut self) -> Result<Type<'a>, String> {
        let array = self.expression()?;
        if array.kind != "array" {
            return Err("any/all needs an array".to_owned());
        }
        self.require(",")?;
        let Some(Token::Name(variable)) = self.tokens.get(self.position).cloned() else {
            return Err("any/all needs a predicate variable".to_owned());
        };
        if variable.contains('.') {
            return Err("predicate variable cannot be a field path".to_owned());
        }
        self.position += 1;
        self.require("=>")?;
        let items = array
            .schema
            .and_then(|schema| schema.get("items"))
            .ok_or("array needs a typed items schema")?;
        let item = self.typed(items, array.base)?;
        self.variables.push((variable, item));
        let result = self.expression();
        self.variables.pop();
        boolean(result?.kind)?;
        self.require(")")?;
        Ok(literal("boolean"))
    }

    /// Resolve a dotted path against the source schema or a scoped array item.
    fn field(&self, name: &str) -> Result<Type<'a>, String> {
        let mut parts = name.split('.');
        let first = parts.next().ok_or("empty field")?;
        let variable = self.variables.iter().rev().find(|(name, _)| *name == first);
        let mut current = if let Some((_, value)) = variable {
            value.clone()
        } else {
            self.property(self.typed(self.root, None)?, first)?
        };
        for part in parts {
            current = self.property(current, part)?;
        }
        Ok(current)
    }

    /// Require a declared property, not additionalProperties or a guessed type.
    fn property(&self, parent: Type<'a>, name: &str) -> Result<Type<'a>, String> {
        if parent.kind != "object" {
            return Err(format!("{name} needs an object"));
        }
        let required = parent
            .schema
            .and_then(|schema| schema.get("required"))
            .and_then(Value::as_array)
            .is_some_and(|required| required.iter().any(|value| value.as_str() == Some(name)));
        if !required {
            return Err(format!("contract field {name:?} may be absent"));
        }
        let schema = parent
            .schema
            .and_then(|schema| schema.get("properties"))
            .and_then(|properties| properties.get(name))
            .ok_or_else(|| format!("undeclared contract field {name:?}"))?;
        self.typed(schema, parent.base)
    }

    /// Reuse the real registry resolver with a recursion bound; ambiguous types refuse.
    fn typed(
        &self,
        mut schema: &'a Value,
        base: Option<Arc<Uri<String>>>,
    ) -> Result<Type<'a>, String> {
        let uri = if let Some(uri) = base {
            (*uri).clone()
        } else {
            let id = self
                .root
                .get("$id")
                .and_then(Value::as_str)
                .ok_or("condition schema needs its checked identity")?;
            uri::from_str(id).map_err(|error| error.to_string())?
        };
        let mut resolver = self
            .registry
            .resolver(uri)
            .in_subresource(Draft::Draft202012.create_resource_ref(schema))
            .map_err(|error| error.to_string())?;
        for _ in 0..Limits::PRODUCTION.source_depth {
            let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
                return Ok(Type {
                    kind: declared_type(schema)?,
                    schema: Some(schema),
                    base: Some(resolver.base_uri()),
                });
            };
            let resolved = resolver
                .lookup(reference)
                .map_err(|error| error.to_string())?;
            schema = resolved.contents();
            resolver = resolved.resolver().clone();
        }
        Err("condition schema reference depth exceeds 32".to_owned())
    }
}

/// Condition fields need one explicit type, not an ambiguous schema union.
fn declared_type(schema: &Value) -> Result<&'static str, String> {
    match schema.get("type").and_then(Value::as_str) {
        Some("integer" | "number") => Ok("number"),
        Some("string") => Ok("string"),
        Some("boolean") => Ok("boolean"),
        Some("null") => Ok("null"),
        Some("object") => Ok("object"),
        Some("array") => Ok("array"),
        _ => Err("condition field needs an unambiguous contract type".to_owned()),
    }
}

/// Primitive literal type.
fn literal(kind: &'static str) -> Type<'static> {
    Type {
        kind,
        schema: None,
        base: None,
    }
}

/// Boolean operators and lambda bodies cannot coerce scalars to truth values.
fn boolean(kind: &str) -> Result<(), String> {
    if kind == "boolean" {
        Ok(())
    } else {
        Err("expected boolean operand".to_owned())
    }
}
