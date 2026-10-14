//! Argument validation against a command's `args_schema`.
//!
//! We implement the small JSON Schema subset the catalog uses instead of pulling in a
//! general validator (the `jsonschema` crate brings dozens of transitive dependencies,
//! regex engines and optional HTTP for remote `$ref`s; the catalog needs none of that).
//!
//! Supported keywords:
//!
//! | Keyword                    | Applies to | Notes                                        |
//! | -------------------------- | ---------- | -------------------------------------------- |
//! | `type`                     | any        | one name or an array of names                |
//! | `enum`                     | any        | JSON equality                                |
//! | `properties`               | object     | each value is a sub-schema                   |
//! | `required`                 | object     | array of property names                      |
//! | `additionalProperties`     | object     | `true`, `false`, or a sub-schema             |
//! | `items`                    | array      | one sub-schema for every element             |
//! | `minItems` / `maxItems`    | array      |                                              |
//! | `minLength` / `maxLength`  | string     | counted in Unicode scalar values             |
//! | `minimum` / `maximum`      | number     | inclusive                                    |
//! | `default`, `description`, `title`, `examples`, `$schema` | any | annotations, ignored  |
//!
//! Any other keyword is a [`EngineError::BadSchema`]: a catalog schema that uses a
//! keyword we do not check would silently accept bad input, so it is refused instead.
//! A unit test runs every catalog schema through [`check_schema`].
//!
//! Recursion follows the schema, not the input, so its depth is bounded by the
//! catalog's (static) schemas; input depth beyond what the schema describes is never
//! visited.

use serde_json::{Map, Value};

use crate::EngineError;

/// Keywords that only annotate; they never reject a value.
const ANNOTATIONS: &[&str] = &["default", "description", "title", "examples", "$schema"];

/// Keywords we check.
const ASSERTIONS: &[&str] = &[
    "type",
    "enum",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "minItems",
    "maxItems",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
];

/// Schema nesting deeper than this is refused (the catalog uses three levels at most).
const MAX_SCHEMA_DEPTH: usize = 32;

/// One rule an argument value broke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// RFC 6901 JSON pointer into the arguments object (`""` is the object itself).
    pub pointer: String,
    /// The schema keyword that rejected the value (`required`, `type`, ...).
    pub rule: &'static str,
    /// Human-readable detail.
    pub message: String,
}

/// Parses a command's `args_schema` text.
pub(crate) fn parse_schema(command: &str, text: &str) -> Result<Value, EngineError> {
    serde_json::from_str(text).map_err(|e| EngineError::BadSchema {
        command: command.to_string(),
        message: format!("not JSON: {e}"),
    })
}

/// Checks that a schema uses only keywords this module understands, with the right
/// value shapes. Catalog schemas are static, so a failure here is a bug in the catalog.
pub fn check_schema(command: &str, schema: &Value) -> Result<(), EngineError> {
    check_schema_at(command, schema, "", 0)
}

fn bad(command: &str, at: &str, message: impl Into<String>) -> EngineError {
    let message = message.into();
    EngineError::BadSchema {
        command: command.to_string(),
        message: if at.is_empty() {
            message
        } else {
            format!("at {at}: {message}")
        },
    }
}

fn check_schema_at(
    command: &str,
    schema: &Value,
    at: &str,
    depth: usize,
) -> Result<(), EngineError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(bad(command, at, "schema nested too deeply"));
    }
    let Some(obj) = schema.as_object() else {
        return Err(bad(command, at, "a schema must be a JSON object"));
    };
    for (key, value) in obj {
        let here = format!("{at}/{}", escape(key));
        if ANNOTATIONS.contains(&key.as_str()) {
            continue;
        }
        if !ASSERTIONS.contains(&key.as_str()) {
            return Err(bad(command, at, format!("unsupported keyword `{key}`")));
        }
        match key.as_str() {
            "type" => {
                let names: Vec<&Value> = match value {
                    Value::Array(items) => items.iter().collect(),
                    other => vec![other],
                };
                for name in names {
                    match name.as_str() {
                        Some(n) if TYPE_NAMES.contains(&n) => {}
                        _ => return Err(bad(command, &here, format!("unknown type {name}"))),
                    }
                }
            }
            "enum" => {
                if !value.is_array() {
                    return Err(bad(command, &here, "`enum` must be an array"));
                }
            }
            "properties" => {
                let Some(props) = value.as_object() else {
                    return Err(bad(command, &here, "`properties` must be an object"));
                };
                for (name, sub) in props {
                    check_schema_at(
                        command,
                        sub,
                        &format!("{here}/{}", escape(name)),
                        depth.saturating_add(1),
                    )?;
                }
            }
            "required" => {
                let ok = value
                    .as_array()
                    .is_some_and(|names| names.iter().all(Value::is_string));
                if !ok {
                    return Err(bad(
                        command,
                        &here,
                        "`required` must be an array of strings",
                    ));
                }
            }
            "additionalProperties" => {
                if !value.is_boolean() {
                    check_schema_at(command, value, &here, depth.saturating_add(1))?;
                }
            }
            "items" => check_schema_at(command, value, &here, depth.saturating_add(1))?,
            "minItems" | "maxItems" | "minLength" | "maxLength" => {
                if value.as_u64().is_none() {
                    return Err(bad(
                        command,
                        &here,
                        format!("`{key}` must be a non-negative integer"),
                    ));
                }
            }
            "minimum" | "maximum" => {
                if !value.is_number() {
                    return Err(bad(command, &here, format!("`{key}` must be a number")));
                }
            }
            _ => return Err(bad(command, at, format!("unsupported keyword `{key}`"))),
        }
    }
    Ok(())
}

const TYPE_NAMES: &[&str] = &[
    "object", "array", "string", "integer", "number", "boolean", "null",
];

/// Validates `value` against `schema` and returns every violation found (empty when the
/// value is valid). The schema must already have passed [`check_schema`].
pub fn validate(schema: &Value, value: &Value) -> Vec<Violation> {
    let mut out = Vec::new();
    validate_at(schema, value, "", 0, &mut out);
    out
}

fn validate_at(schema: &Value, value: &Value, at: &str, depth: usize, out: &mut Vec<Violation>) {
    if depth > MAX_SCHEMA_DEPTH {
        return;
    }
    let Some(schema) = schema.as_object() else {
        return;
    };
    if let Some(ty) = schema.get("type") {
        if !type_matches(ty, value) {
            out.push(Violation {
                pointer: at.to_string(),
                rule: "type",
                message: format!("expected {}, got {}", type_label(ty), type_of(value)),
            });
            // Every other keyword assumes the type matched.
            return;
        }
    }
    if let Some(Value::Array(allowed)) = schema.get("enum") {
        if !allowed.contains(value) {
            let list: Vec<String> = allowed.iter().map(Value::to_string).collect();
            out.push(Violation {
                pointer: at.to_string(),
                rule: "enum",
                message: format!("must be one of {}", list.join(", ")),
            });
        }
    }
    match value {
        Value::Object(map) => validate_object(schema, map, at, depth, out),
        Value::Array(items) => {
            check_count(schema, "minItems", items.len(), at, out);
            check_count(schema, "maxItems", items.len(), at, out);
            if let Some(item_schema) = schema.get("items") {
                for (i, item) in items.iter().enumerate() {
                    validate_at(
                        item_schema,
                        item,
                        &format!("{at}/{i}"),
                        depth.saturating_add(1),
                        out,
                    );
                }
            }
        }
        Value::String(s) => {
            let len = s.chars().count();
            check_count(schema, "minLength", len, at, out);
            check_count(schema, "maxLength", len, at, out);
        }
        Value::Number(n) => {
            if let Some(x) = n.as_f64() {
                if let Some(min) = schema.get("minimum").and_then(Value::as_f64) {
                    if x < min {
                        out.push(Violation {
                            pointer: at.to_string(),
                            rule: "minimum",
                            message: format!("must be >= {min}"),
                        });
                    }
                }
                if let Some(max) = schema.get("maximum").and_then(Value::as_f64) {
                    if x > max {
                        out.push(Violation {
                            pointer: at.to_string(),
                            rule: "maximum",
                            message: format!("must be <= {max}"),
                        });
                    }
                }
            }
        }
        Value::Bool(_) | Value::Null => {}
    }
}

/// Checks one of `minItems`, `maxItems`, `minLength`, `maxLength`.
fn check_count(
    schema: &Map<String, Value>,
    keyword: &'static str,
    actual: usize,
    at: &str,
    out: &mut Vec<Violation>,
) {
    let Some(limit) = schema.get(keyword).and_then(Value::as_u64) else {
        return;
    };
    let actual = u64::try_from(actual).unwrap_or(u64::MAX);
    let is_min = keyword.starts_with("min");
    let fails = if is_min {
        actual < limit
    } else {
        actual > limit
    };
    if fails {
        let bound = if is_min { "at least" } else { "at most" };
        let unit = if keyword.ends_with("Items") {
            "items"
        } else {
            "characters"
        };
        out.push(Violation {
            pointer: at.to_string(),
            rule: keyword,
            message: format!("must have {bound} {limit} {unit}, has {actual}"),
        });
    }
}

fn validate_object(
    schema: &Map<String, Value>,
    map: &Map<String, Value>,
    at: &str,
    depth: usize,
    out: &mut Vec<Violation>,
) {
    let props = schema.get("properties").and_then(Value::as_object);
    if let Some(Value::Array(required)) = schema.get("required") {
        for name in required.iter().filter_map(Value::as_str) {
            if !map.contains_key(name) {
                out.push(Violation {
                    pointer: format!("{at}/{}", escape(name)),
                    rule: "required",
                    message: format!("missing required argument `{name}`"),
                });
            }
        }
    }
    for (name, item) in map {
        let here = format!("{at}/{}", escape(name));
        match props.and_then(|p| p.get(name)) {
            Some(sub) => validate_at(sub, item, &here, depth.saturating_add(1), out),
            None => match schema.get("additionalProperties") {
                Some(Value::Bool(false)) => out.push(Violation {
                    pointer: here,
                    rule: "additionalProperties",
                    message: format!("unknown argument `{name}`"),
                }),
                Some(sub @ Value::Object(_)) => {
                    validate_at(sub, item, &here, depth.saturating_add(1), out)
                }
                _ => {}
            },
        }
    }
}

/// Fills top-level properties that are absent from `args` with their schema `default`.
/// Nested defaults are not applied. Call after validation succeeded.
pub(crate) fn apply_defaults(schema: &Value, args: &mut Value) {
    let (Some(props), Some(map)) = (
        schema.get("properties").and_then(Value::as_object),
        args.as_object_mut(),
    ) else {
        return;
    };
    for (name, sub) in props {
        if let Some(default) = sub.get("default") {
            if !map.contains_key(name) {
                map.insert(name.clone(), default.clone());
            }
        }
    }
}

fn type_matches(ty: &Value, value: &Value) -> bool {
    match ty {
        Value::String(name) => is_type(name, value),
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .any(|name| is_type(name, value)),
        _ => false,
    }
}

fn is_type(name: &str, value: &Value) -> bool {
    match name {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        "number" => value.is_number(),
        "integer" => match value {
            Value::Number(n) => {
                n.is_i64() || n.is_u64() || n.as_f64().is_some_and(|f| f.fract() == 0.0)
            }
            _ => false,
        },
        _ => false,
    }
}

fn type_label(ty: &Value) -> String {
    match ty {
        Value::String(name) => name.clone(),
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" or "),
        other => other.to_string(),
    }
}

fn type_of(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::String(_) => "string",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
    }
}

/// Escapes one JSON pointer reference token (RFC 6901).
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::{check_schema, validate};
    use serde_json::json;

    fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "minLength": 4, "maxLength": 4},
                "count": {"type": "integer", "minimum": 1, "maximum": 10},
                "mode": {"type": "string", "enum": ["a", "b"]},
                "list": {"type": "array", "items": {"type": "string"}, "minItems": 2},
                "flag": {"type": "boolean", "default": true},
                "a/b": {"type": "string"}
            },
            "required": ["name"],
            "additionalProperties": false
        })
    }

    fn rules(value: serde_json::Value) -> Vec<(String, &'static str)> {
        validate(&schema(), &value)
            .into_iter()
            .map(|v| (v.pointer, v.rule))
            .collect()
    }

    #[test]
    fn schema_is_supported() {
        check_schema("t", &schema()).unwrap();
    }

    #[test]
    fn valid_value_passes() {
        assert!(rules(
            json!({"name": "WEAP", "count": 3, "mode": "a", "list": ["x", "y"], "flag": false})
        )
        .is_empty());
    }

    #[test]
    fn each_rule_reports_pointer() {
        assert_eq!(rules(json!({})), vec![("/name".into(), "required")]);
        assert_eq!(rules(json!([])), vec![(String::new(), "type")]);
        assert_eq!(rules(json!({"name": 4})), vec![("/name".into(), "type")]);
        assert_eq!(
            rules(json!({"name": "WEAPON"})),
            vec![("/name".into(), "maxLength")]
        );
        assert_eq!(
            rules(json!({"name": "W"})),
            vec![("/name".into(), "minLength")]
        );
        assert_eq!(
            rules(json!({"name": "WEAP", "count": 0})),
            vec![("/count".into(), "minimum")]
        );
        assert_eq!(
            rules(json!({"name": "WEAP", "count": 11})),
            vec![("/count".into(), "maximum")]
        );
        assert_eq!(
            rules(json!({"name": "WEAP", "count": 1.5})),
            vec![("/count".into(), "type")]
        );
        assert_eq!(
            rules(json!({"name": "WEAP", "mode": "c"})),
            vec![("/mode".into(), "enum")]
        );
        assert_eq!(
            rules(json!({"name": "WEAP", "list": ["x"]})),
            vec![("/list".into(), "minItems")]
        );
        assert_eq!(
            rules(json!({"name": "WEAP", "list": ["x", 1]})),
            vec![("/list/1".into(), "type")]
        );
        assert_eq!(
            rules(json!({"name": "WEAP", "extra": 1})),
            vec![("/extra".into(), "additionalProperties")]
        );
        assert_eq!(
            rules(json!({"name": "WEAP", "a/b": 1})),
            vec![("/a~1b".into(), "type")]
        );
    }

    #[test]
    fn integer_accepts_whole_floats() {
        assert!(rules(json!({"name": "WEAP", "count": 2.0})).is_empty());
    }

    #[test]
    fn unsupported_keyword_is_refused() {
        let err = check_schema("t", &json!({"type": "object", "patternProperties": {}}));
        assert!(err.is_err());
        let err = check_schema(
            "t",
            &json!({"type": "object", "properties": {"x": {"format": "uri"}}}),
        );
        assert!(err.is_err());
        let err = check_schema("t", &json!({"type": "objekt"}));
        assert!(err.is_err());
    }
}
