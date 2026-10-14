//! A front-end-neutral view of a command's arguments, derived from its `args_schema`.
//!
//! The CLI builds its flags from this, a future UI builds its forms from it, so neither
//! re-parses JSON Schema on its own.

use serde_json::Value;

use crate::args;
use crate::{CommandSpec, EngineError};

/// The JSON shape of one argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// JSON string.
    String,
    /// JSON integer.
    Integer,
    /// JSON number.
    Number,
    /// JSON boolean.
    Boolean,
    /// JSON array of strings.
    StringList,
    /// JSON object (or any other shape): passed through as JSON text.
    Json,
}

/// One top-level argument of a command.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Property name in the arguments object.
    pub name: String,
    /// JSON shape.
    pub kind: ParamKind,
    /// Listed in the schema's `required`.
    pub required: bool,
    /// Allowed string values (`enum`), empty when unrestricted.
    pub choices: Vec<String>,
    /// Schema `description`.
    pub description: Option<String>,
    /// Schema `default`.
    pub default: Option<Value>,
}

/// The top-level arguments of `spec`: required ones first, in `required` order, then the
/// optional ones by name.
pub fn params(spec: &CommandSpec) -> Result<Vec<Param>, EngineError> {
    let schema = args::parse_schema(spec.id, spec.args_schema)?;
    args::check_schema(spec.id, &schema)?;
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<Param> = props
        .iter()
        .map(|(name, sub)| Param {
            name: name.clone(),
            kind: kind_of(sub),
            required: required.contains(&name.as_str()),
            choices: sub
                .get("enum")
                .and_then(Value::as_array)
                .map(|vals| {
                    vals.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            description: sub
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string),
            default: sub.get("default").cloned(),
        })
        .collect();
    out.sort_by_key(|p| {
        let rank = required
            .iter()
            .position(|r| *r == p.name)
            .unwrap_or(usize::MAX);
        (rank, p.name.clone())
    });
    Ok(out)
}

fn kind_of(schema: &Value) -> ParamKind {
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => ParamKind::String,
        Some("integer") => ParamKind::Integer,
        Some("number") => ParamKind::Number,
        Some("boolean") => ParamKind::Boolean,
        Some("array")
            if schema
                .get("items")
                .and_then(|i| i.get("type"))
                .and_then(Value::as_str)
                == Some("string") =>
        {
            ParamKind::StringList
        }
        _ => ParamKind::Json,
    }
}

#[cfg(test)]
mod tests {
    use super::{params, ParamKind};
    use crate::find_command;

    #[test]
    fn required_first_in_required_order() {
        let spec = find_command("ba2.extract").unwrap();
        let names: Vec<String> = params(spec).unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(
            names,
            [
                "archive",
                "out_dir",
                "data_dir",
                "max_entry_bytes",
                "overwrite",
                "pattern"
            ]
        );

        let spec = find_command("roundtrip").unwrap();
        let ps = params(spec).unwrap();
        let names: Vec<&str> = ps.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["plugin", "data_dir", "out", "overwrite", "verify"]);
        assert_eq!(ps[4].kind, ParamKind::Boolean);
        assert_eq!(ps[4].default, Some(serde_json::Value::Bool(false)));
    }

    #[test]
    fn kinds_and_choices() {
        let ps = params(find_command("ba2.pack").unwrap()).unwrap();
        let format = ps.iter().find(|p| p.name == "format").unwrap();
        assert_eq!(format.choices, ["gnrl", "dx10"]);
        let ps = params(find_command("strings.merge").unwrap()).unwrap();
        assert_eq!(ps[0].kind, ParamKind::StringList);
        let ps = params(find_command("set").unwrap()).unwrap();
        assert_eq!(
            ps.iter().find(|p| p.name == "fields").unwrap().kind,
            ParamKind::Json
        );
        let ps = params(find_command("query").unwrap()).unwrap();
        assert_eq!(
            ps.iter().find(|p| p.name == "limit").unwrap().kind,
            ParamKind::Integer
        );
    }
}
