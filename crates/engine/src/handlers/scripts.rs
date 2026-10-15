//! The `scripts` argument of `place`, `set` and `new` (and of a build spec's `[[place]]`,
//! `[[set]]`, `[[new]]` and `[[clone]]`, M4.E9): `[{name, properties: {Name: value}}]`.
//!
//! Each script must exist (`validate::attached::Sources`: loose `Scripts\` next to the
//! target or in the Data folder, the target's archives, the load order's archives), else
//! the call is refused with `papyrus.missing-script`. Each property must be declared by
//! the script or a parent (its `.pex` property table, or the base game's sources through
//! `kit papyrus events`), else `papyrus.property-unresolved`. A value is typed by the
//! declaration: `Int`, `Float`, `Bool` and `String` from JSON numbers, booleans and
//! strings; an object type from a formref, an EditorID, `null`, or (in a spec) `new:<name>`
//! naming a `[[new]]` or `[[clone]]` of the build; arrays of these from JSON arrays.
//! `Var` and struct properties are refused (their layout is known, `edit::vmad`, but not
//! how a JSON value should choose a variant or member).

use std::path::Path;

use serde_json::{json, Value};

use super::failed;
use super::load::Loaded;
use super::set::find_form;
use crate::edit::patch::FormName;
use crate::edit::vmad::{Attach, AttachValue, ObjectTarget};
use crate::exec::EngineError;
use crate::validate::attached::{type_byte, Chain, Sources};

/// A script argument before it is typed: its name and `(property, JSON value)` pairs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScriptArg {
    /// The script name.
    pub name: String,
    /// Properties in the order given.
    pub properties: Vec<(String, Value)>,
}

/// The `scripts` argument of a command, typed for `target` (empty when absent).
pub(crate) fn from_args(
    command: &str,
    args: &Value,
    loaded: &Loaded,
    target: &Path,
) -> Result<Resolved, EngineError> {
    let list = match args.get("scripts") {
        None | Some(Value::Null) => return Ok(Resolved::default()),
        Some(v) => parse(command, "scripts", v)?,
    };
    if list.is_empty() {
        return Ok(Resolved::default());
    }
    let mut src = sources(loaded, target, super::data_dir(args).as_deref());
    resolve(command, "scripts", &list, loaded, &mut src, &|_| None)
}

/// Reads `value` (a `scripts` argument or spec key) as script arguments; `at` names it in
/// errors (`scripts`, `place[0].scripts`).
pub(crate) fn parse(command: &str, at: &str, value: &Value) -> Result<Vec<ScriptArg>, EngineError> {
    let bad = |path: String, why: &str| failed(command, format!("{path}: {why}"));
    let items = value
        .as_array()
        .ok_or_else(|| bad(at.to_string(), "must be an array of {name, properties}"))?;
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let here = format!("{at}[{i}]");
        let obj = item
            .as_object()
            .ok_or_else(|| bad(here.clone(), "must be an object {name, properties}"))?;
        if let Some(k) = obj
            .keys()
            .find(|k| k.as_str() != "name" && k.as_str() != "properties")
        {
            return Err(bad(
                format!("{here}.{k}"),
                "unknown key; allowed: name, properties",
            ));
        }
        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| !n.trim().is_empty())
            .ok_or_else(|| bad(format!("{here}.name"), "is required (a script name)"))?;
        let properties = match obj.get("properties") {
            None => Vec::new(),
            Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            Some(_) => {
                return Err(bad(
                    format!("{here}.properties"),
                    "must be an object of property name -> value",
                ))
            }
        };
        out.push(ScriptArg {
            name: name.trim().to_string(),
            properties,
        });
    }
    Ok(out)
}

/// The script lookup for a write into `target` over `loaded`.
pub(crate) fn sources(loaded: &Loaded, target: &Path, data_dir: Option<&Path>) -> Sources {
    let others: Vec<std::path::PathBuf> =
        loaded.lo.plugins.iter().map(|p| p.path.clone()).collect();
    Sources::new(Some(target), &others, data_dir)
}

/// What [`resolve`] found, for the result's `scripts[]`.
fn report(arg: &ScriptArg, chain: &Chain) -> Value {
    json!({
        "script": arg.name,
        "found": chain.at,
        "properties": arg.properties.iter().map(|(n, _)| {
            let d = chain.find(n);
            json!({"name": n, "type": d.map(|d| d.ty.clone()), "declared_in": d.map(|d| d.script.clone())})
        }).collect::<Vec<_>>(),
    })
}

/// What [`resolve`] returns.
#[derive(Debug, Default)]
pub(crate) struct Resolved {
    /// The scripts, typed.
    pub scripts: Vec<Attach>,
    /// One report per script (`script`, `found`, `properties[]`).
    pub reports: Vec<Value>,
    /// Limitations.
    pub limitations: Vec<String>,
}

/// Types every script argument by its declaration (module docs). `locals` maps a spec
/// name to the EditorID of the form it writes (`new:<name>` values).
pub(crate) fn resolve(
    command: &str,
    at: &str,
    args: &[ScriptArg],
    loaded: &Loaded,
    sources: &mut Sources,
    locals: &dyn Fn(&str) -> Option<String>,
) -> Result<Resolved, EngineError> {
    let mut out = Vec::new();
    let mut reports = Vec::new();
    let mut limitations = Vec::new();
    for (i, arg) in args.iter().enumerate() {
        let here = format!("{at}[{i}]");
        let chain = sources
            .properties(&arg.name)
            .map_err(|e| failed(command, format!("{here}: {}: {e}", arg.name)))?
            .ok_or_else(|| {
                failed(
                    command,
                    format!(
                        "papyrus.missing-script: {here}: script `{}` is not found as a .pex (searched {}); compile it with papyrus.compile into a Scripts folder next to the target, or name a script of the load order",
                        arg.name,
                        sources.searched()
                    ),
                )
            })?;
        if let Some(p) = &chain.missing_parent {
            limitations.push(format!(
                "{here}: `{}` extends `{p}`, which is not found as a .pex or in the base game's sources: properties declared there are unknown, so only `{}`'s own can be set",
                arg.name, arg.name
            ));
        }
        // A compiled script names a struct type `Script#Struct`.
        let is_struct = |t: &str| t.contains('#');
        let mut properties = Vec::new();
        for (name, value) in &arg.properties {
            let key = format!("{here}.properties.{name}");
            let declared = chain.find(name).ok_or_else(|| {
                let names: Vec<&str> = chain.declared.iter().map(|d| d.name.as_str()).collect();
                failed(
                    command,
                    format!(
                        "papyrus.property-unresolved: {key}: `{}` declares no property `{name}`{} (declared: {})",
                        arg.name,
                        chain
                            .missing_parent
                            .as_ref()
                            .map(|p| format!(" (its parent `{p}` is not found)"))
                            .unwrap_or_default(),
                        if names.is_empty() { "none".to_string() } else { names.join(", ") }
                    ),
                )
            })?;
            let t = type_byte(&declared.ty, &is_struct);
            let typed = typed_value(command, &key, &declared.ty, t, value, loaded, locals)?;
            properties.push((declared.name.clone(), typed));
        }
        reports.push(report(arg, &chain));
        out.push(Attach {
            name: arg.name.clone(),
            properties,
        });
    }
    if !sources.unreadable.is_empty() {
        limitations.push(format!(
            "scripts: archives that could not be read were not searched: {}",
            sources.unreadable.join("; ")
        ));
    }
    if !out.is_empty() {
        limitations.push(
            "scripts: an object property's value is not checked against the declared object type (a formref of any record type is written)".to_string(),
        );
    }
    Ok(Resolved {
        scripts: out,
        reports,
        limitations,
    })
}

fn typed_value(
    command: &str,
    key: &str,
    ty: &str,
    t: u8,
    value: &Value,
    loaded: &Loaded,
    locals: &dyn Fn(&str) -> Option<String>,
) -> Result<AttachValue, EngineError> {
    let bad = |why: String| {
        failed(
            command,
            format!("papyrus.property-unresolved: {key}: {why}"),
        )
    };
    let scalar = |t: u8, v: &Value| -> Result<AttachValue, EngineError> {
        match (t, v) {
            (1, Value::Null) => Ok(AttachValue::Object(ObjectTarget::Null)),
            (1, Value::String(s)) => {
                if let Some(name) = crate::build::spec::local_ref(s) {
                    return locals(name)
                        .map(|edid| AttachValue::Object(ObjectTarget::Local(edid)))
                        .ok_or_else(|| {
                            bad(format!(
                                "unknown name `{name}` (a [[new]] or [[clone]] of the build)"
                            ))
                        });
                }
                let g = find_form(command, loaded, s, None)
                    .map_err(|e| bad(format!("`{s}` ({ty}): {e}")))?;
                let plugin = loaded
                    .lo
                    .plugin(g.plugin)
                    .map(|p| p.name.clone())
                    .ok_or_else(|| bad(format!("`{s}`: no plugin")))?;
                Ok(AttachValue::Object(ObjectTarget::Form(FormName {
                    plugin,
                    object_id: g.object_id,
                })))
            }
            (2, Value::String(s)) => Ok(AttachValue::String(s.clone())),
            (3, Value::Number(n)) => n
                .as_i64()
                .and_then(|i| i32::try_from(i).ok())
                .map(AttachValue::Int)
                .ok_or_else(|| bad(format!("{n} is not a 32-bit integer ({ty})"))),
            (4, Value::Number(n)) => n
                .as_f64()
                .filter(|f| f.is_finite() && f.abs() <= f64::from(f32::MAX))
                .map(|f| AttachValue::Float(f as f32))
                .ok_or_else(|| bad(format!("{n} is not a finite 32-bit float ({ty})"))),
            (5, Value::Bool(b)) => Ok(AttachValue::Bool(*b)),
            (6 | 7, _) => Err(bad(format!(
                "a {ty} property cannot be set here (Var and struct values are not attached yet)"
            ))),
            (_, v) => Err(bad(format!("{v} does not fit the declared type {ty}"))),
        }
    };
    if t > 10 {
        let element = t.saturating_sub(10);
        let items = value
            .as_array()
            .ok_or_else(|| bad(format!("{ty} takes a JSON array")))?;
        let typed = items
            .iter()
            .map(|v| scalar(element, v))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(AttachValue::Array(element, typed));
    }
    scalar(t, value)
}
