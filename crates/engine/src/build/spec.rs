//! The `build` spec (TOML): version 1 (M2.E9) and version 2 (M3.E6).
//!
//! ```toml
//! version = 2                         # 1 is still accepted, with M2's meaning
//! strict = false                      # true: a rule that matches nothing is an error
//!
//! [plugin]
//! localized = false                   # the only value
//! style = "kit"                       # the only value until the `ck` style (M3.A3/M3.E5)
//!
//! [load_order]                        # optional; else the arguments or the session
//! profile = 'C:\Modding\MO2-Starfield\profiles\Default'
//!
//! [class.ballistic_rifle]             # a named set: one record type and a `where`
//! type = "WEAP"
//! where = 'KWDA == Starfield.esm:000ABC and not (EDID ~ "Test")'   # optional: every winner
//! brief = "B3"                        # the brief row this class serves (docs/overhaul/)
//!
//! [[rule]]
//! class = "ballistic_rifle"
//! brief = "B3"
//! via = ["KWDA"]                      # optional: follow typed FormID members, hop by hop
//! fields = { "SIG.Member" = { mul = 0.8 } }   # a value, mul, add, min, max, clamp = [lo, hi]
//! allow_reference = false             # true: may write a `reference`-only member
//!
//! [[set]]                             # one record, as `kit set`; applies after the rules
//! form = "Starfield.esm:0021BBCD"
//! fields = { FULL = "Old Soviet Marksman Rifle" }
//! ```
//!
//! Version 1 keeps M2's shape: `[[rule]]` entries carry `type` and `where` themselves (an
//! anonymous class each), there are no classes, `via`, `strict`, `style` or
//! `allow_reference`, and a field written twice is an error even when both writes agree.
//! Every key is checked; errors name the TOML key (`rule[0].fields.FULL`), and TOML
//! `inf`/`nan` values are range errors.

use kitcraft_doc::query::{Expr, Path, Query};
use serde_json::{json, Map, Value};

use crate::edit::fields::{parse_op, parse_path};
use crate::edit::patch::Edit;

/// A spec error: the TOML key and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError {
    /// `rule[0].fields.FULL`, `class.rifles.where`, `(toml)`.
    pub key: String,
    /// What is wrong.
    pub message: String,
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "spec: {}: {}", self.key, self.message)
    }
}

fn error(key: &str, message: impl std::fmt::Display) -> SpecError {
    SpecError {
        key: key.to_string(),
        message: message.to_string(),
    }
}

/// A named set of records: the winning records of one type matching `where`.
#[derive(Debug, Clone)]
pub struct Class {
    /// The class name (`ballistic_rifle`; in a version 1 spec, the rule's key).
    pub name: String,
    /// The TOML key of the class (`class.ballistic_rifle`, or `rule[0]` in version 1).
    pub key: String,
    /// The record type.
    pub signature: [u8; 4],
    /// The `where` predicate; `None` selects every winning record of the type.
    pub query: Option<Query>,
    /// The brief row it serves.
    pub brief: Option<String>,
}

/// One hop of a rule's `via`: a typed FormID member to follow.
#[derive(Debug, Clone)]
pub struct Via {
    /// As written (`KWDA`).
    pub text: String,
    /// As a `where` path.
    pub path: Path,
}

/// A rule: edits applied to every record its class matches (or reaches through `via`).
#[derive(Debug, Clone)]
pub struct Rule {
    /// `rule[0]`.
    pub key: String,
    /// Index into [`Spec::classes`].
    pub class: usize,
    /// Hops to follow from each matched record, in order.
    pub via: Vec<Via>,
    /// The edits.
    pub edits: Vec<Edit>,
    /// May write `reference`-only members.
    pub allow_reference: bool,
    /// The brief row it serves.
    pub brief: Option<String>,
}

/// An explicit `[[set]]` entry.
#[derive(Debug, Clone)]
pub struct SetEntry {
    /// `set[0]`.
    pub key: String,
    /// The form, as `kit set` takes it.
    pub form: String,
    /// The plugin whose numbering a hex FormID uses.
    pub plugin: Option<String>,
    /// The edits.
    pub edits: Vec<Edit>,
    /// May write `reference`-only members.
    pub allow_reference: bool,
    /// The brief row it serves (version 2).
    pub brief: Option<String>,
}

/// A parsed spec.
#[derive(Debug, Clone)]
pub struct Spec {
    /// 1 or 2.
    pub version: u8,
    /// The `[load_order]` table as JSON load-order arguments.
    pub load_order: Option<Value>,
    /// A rule matching nothing is an error.
    pub strict: bool,
    /// Classes in key order (version 1: one per rule).
    pub classes: Vec<Class>,
    /// Rules in spec order.
    pub rules: Vec<Rule>,
    /// Explicit sets in spec order.
    pub sets: Vec<SetEntry>,
}

/// TOML -> JSON (field values and load-order arguments).
pub fn to_json(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => json!(i),
        toml::Value::Float(f) => {
            serde_json::Number::from_f64(*f).map_or(Value::Null, Value::Number)
        }
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(a) => Value::Array(a.iter().map(to_json).collect()),
        toml::Value::Table(t) => Value::Object(
            t.iter()
                .map(|(k, v)| (k.clone(), to_json(v)))
                .collect::<Map<String, Value>>(),
        ),
    }
}

fn only_keys(table: &toml::Table, key: &str, allowed: &[&str]) -> Result<(), SpecError> {
    for k in table.keys() {
        if !allowed.contains(&k.as_str()) {
            let at = if key.is_empty() {
                k.clone()
            } else {
                format!("{key}.{k}")
            };
            return Err(error(
                &at,
                format!("unknown key; allowed: {}", allowed.join(", ")),
            ));
        }
    }
    Ok(())
}

fn string<'t>(table: &'t toml::Table, key: &str, at: &str) -> Result<&'t str, SpecError> {
    match table.get(key) {
        Some(toml::Value::String(s)) => Ok(s),
        Some(_) => Err(error(&format!("{at}.{key}"), "must be a string")),
        None => Err(error(&format!("{at}.{key}"), "is required")),
    }
}

fn optional_string(table: &toml::Table, key: &str, at: &str) -> Result<Option<String>, SpecError> {
    match table.get(key) {
        None => Ok(None),
        Some(_) => string(table, key, at).map(|s| Some(s.to_string())),
    }
}

fn boolean(table: &toml::Table, key: &str, at: &str) -> Result<bool, SpecError> {
    match table.get(key) {
        None => Ok(false),
        Some(toml::Value::Boolean(b)) => Ok(*b),
        Some(_) => Err(error(
            &if at.is_empty() {
                key.to_string()
            } else {
                format!("{at}.{key}")
            },
            "must be true or false",
        )),
    }
}

/// The dotted key of the first float in `value` that is not finite (TOML `inf`, `nan`).
fn non_finite(value: &toml::Value, at: &str) -> Option<(String, f64)> {
    match value {
        toml::Value::Float(f) if !f.is_finite() => Some((at.to_string(), *f)),
        toml::Value::Table(t) => t
            .iter()
            .find_map(|(k, v)| non_finite(v, &format!("{at}.{k}"))),
        toml::Value::Array(a) => a
            .iter()
            .enumerate()
            .find_map(|(i, v)| non_finite(v, &format!("{at}[{i}]"))),
        _ => None,
    }
}

/// The `fields` table of an entry as edits; errors name `<at>.fields.<path>`.
fn fields(table: &toml::Table, at: &str) -> Result<Vec<Edit>, SpecError> {
    let Some(toml::Value::Table(f)) = table.get("fields") else {
        return Err(error(&format!("{at}.fields"), "is required (a table)"));
    };
    if let Some((key, value)) = f
        .iter()
        .find_map(|(k, v)| non_finite(v, &format!("{at}.fields.{k}")))
    {
        return Err(error(
            &key,
            format!("{value} is not a finite number (range): field values and operator arguments must be finite"),
        ));
    }
    if f.is_empty() {
        return Err(error(&format!("{at}.fields"), "no fields to set"));
    }
    f.iter()
        .map(|(key, value)| {
            let bad = |e: crate::edit::fields::FieldError| {
                error(&format!("{at}.fields.{key}"), format!("{e}"))
            };
            let path = parse_path(key).map_err(bad)?;
            let op = parse_op(key, &to_json(value)).map_err(bad)?;
            Ok(Edit { path, op })
        })
        .collect()
}

fn signature(text: &str, key: &str) -> Result<[u8; 4], SpecError> {
    text.to_ascii_uppercase()
        .as_bytes()
        .try_into()
        .map_err(|_| error(key, "must be a four-character record type"))
}

fn query(text: &str, key: &str) -> Result<Query, SpecError> {
    Query::parse(text).map_err(|e| error(key, e))
}

/// A `via` hop: a `where` field path (`KWDA`, `DNAM.Ammo`), parsed by lane D's grammar.
fn via(text: &str, key: &str) -> Result<Via, SpecError> {
    let parsed = Query::parse(&format!("{text} == null")).map_err(|e| error(key, e))?;
    match parsed.expr {
        Expr::Cmp {
            path: path @ Path::Field { index: None, .. },
            ..
        } => Ok(Via {
            text: text.to_string(),
            path,
        }),
        _ => Err(error(
            key,
            "must be a member path (SIG or SIG.Member) naming a typed FormID member",
        )),
    }
}

fn entries(root: &toml::Table, key: &str) -> Result<Vec<toml::Table>, SpecError> {
    match root.get(key) {
        None => Ok(Vec::new()),
        Some(toml::Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(i, v)| match v {
                toml::Value::Table(t) => Ok(t.clone()),
                _ => Err(error(
                    &format!("{key}[{i}]"),
                    format!("must be a table ([[{key}]])"),
                )),
            })
            .collect(),
        Some(_) => Err(error(
            key,
            format!("must be an array of tables ([[{key}]])"),
        )),
    }
}

/// Parses a spec of either version.
pub fn parse(text: &str) -> Result<Spec, SpecError> {
    let root: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| error("(toml)", e.to_string().trim_end()))?;
    let version: u8 = match root.get("version") {
        Some(toml::Value::Integer(1)) => 1,
        Some(toml::Value::Integer(2)) => 2,
        Some(_) => return Err(error("version", "must be 1 or 2")),
        None => return Err(error("version", "is required (version = 2)")),
    };
    if version == 1 {
        only_keys(
            &root,
            "",
            &["version", "plugin", "load_order", "set", "rule"],
        )?;
    } else {
        only_keys(
            &root,
            "",
            &[
                "version",
                "plugin",
                "load_order",
                "strict",
                "class",
                "rule",
                "set",
            ],
        )?;
    }
    if let Some(plugin) = root.get("plugin") {
        let toml::Value::Table(p) = plugin else {
            return Err(error("plugin", "must be a table"));
        };
        if version == 1 {
            only_keys(p, "plugin", &["localized"])?;
        } else {
            only_keys(p, "plugin", &["localized", "style"])?;
        }
        match p.get("localized") {
            None | Some(toml::Value::Boolean(false)) => {}
            Some(_) => {
                return Err(error(
                    "plugin.localized",
                    "must be false (M2 writes no string tables; masters are computed)",
                ))
            }
        }
        match p.get("style") {
            None => {}
            Some(toml::Value::String(s)) if s == "kit" => {}
            Some(toml::Value::String(s)) if s == "ck" => {
                return Err(error(
                    "plugin.style",
                    "the `ck` write style needs the CK observation ledger (M3.H3) and the writer's style knobs (M3.A3); only \"kit\" is available",
                ))
            }
            Some(_) => return Err(error("plugin.style", "must be \"kit\"")),
        }
    }
    let load_order = match root.get("load_order") {
        None => None,
        Some(toml::Value::Table(t)) => {
            only_keys(
                t,
                "load_order",
                &["profile", "plugins", "data_dir", "no_cache", "language"],
            )?;
            Some(to_json(&toml::Value::Table(t.clone())))
        }
        Some(_) => return Err(error("load_order", "must be a table")),
    };
    let strict = boolean(&root, "strict", "")?;

    let mut classes = Vec::new();
    let mut rules = Vec::new();
    if version == 1 {
        for (i, t) in entries(&root, "rule")?.iter().enumerate() {
            let at = format!("rule[{i}]");
            only_keys(t, &at, &["type", "where", "fields"])?;
            let sig = signature(string(t, "type", &at)?, &format!("{at}.type"))?;
            let q = query(string(t, "where", &at)?, &format!("{at}.where"))?;
            classes.push(Class {
                name: at.clone(),
                key: at.clone(),
                signature: sig,
                query: Some(q),
                brief: None,
            });
            rules.push(Rule {
                key: at.clone(),
                class: i,
                via: Vec::new(),
                edits: fields(t, &at)?,
                allow_reference: false,
                brief: None,
            });
        }
    } else {
        match root.get("class") {
            None => {}
            Some(toml::Value::Table(table)) => {
                for (name, v) in table {
                    let at = format!("class.{name}");
                    let toml::Value::Table(t) = v else {
                        return Err(error(&at, "must be a table ([class.<name>])"));
                    };
                    only_keys(t, &at, &["type", "where", "brief", "note"])?;
                    let sig = signature(string(t, "type", &at)?, &format!("{at}.type"))?;
                    let q = match t.get("where") {
                        None => None,
                        Some(_) => Some(query(string(t, "where", &at)?, &format!("{at}.where"))?),
                    };
                    optional_string(t, "note", &at)?;
                    classes.push(Class {
                        name: name.clone(),
                        key: at.clone(),
                        signature: sig,
                        query: q,
                        brief: optional_string(t, "brief", &at)?,
                    });
                }
            }
            Some(_) => {
                return Err(error(
                    "class",
                    "must be a table of classes ([class.<name>])",
                ))
            }
        }
        for (i, t) in entries(&root, "rule")?.iter().enumerate() {
            let at = format!("rule[{i}]");
            only_keys(
                t,
                &at,
                &["class", "via", "fields", "allow_reference", "brief", "note"],
            )?;
            let name = string(t, "class", &at)?;
            let class = classes.iter().position(|c| c.name == name).ok_or_else(|| {
                let names: Vec<&str> = classes.iter().map(|c| c.name.as_str()).collect();
                error(
                    &format!("{at}.class"),
                    format!(
                        "no class `{name}` (classes: {})",
                        if names.is_empty() {
                            "none".to_string()
                        } else {
                            names.join(", ")
                        }
                    ),
                )
            })?;
            let hops = match t.get("via") {
                None => Vec::new(),
                Some(toml::Value::Array(items)) => items
                    .iter()
                    .enumerate()
                    .map(|(j, v)| {
                        let key = format!("{at}.via[{j}]");
                        match v {
                            toml::Value::String(s) => via(s, &key),
                            _ => Err(error(&key, "must be a member path string")),
                        }
                    })
                    .collect::<Result<_, _>>()?,
                Some(_) => {
                    return Err(error(
                        &format!("{at}.via"),
                        "must be an array of member paths",
                    ))
                }
            };
            optional_string(t, "note", &at)?;
            rules.push(Rule {
                key: at.clone(),
                class,
                via: hops,
                edits: fields(t, &at)?,
                allow_reference: boolean(t, "allow_reference", &at)?,
                brief: optional_string(t, "brief", &at)?,
            });
        }
    }
    let mut sets = Vec::new();
    for (i, t) in entries(&root, "set")?.iter().enumerate() {
        let at = format!("set[{i}]");
        if version == 1 {
            only_keys(t, &at, &["form", "plugin", "fields"])?;
        } else {
            only_keys(
                t,
                &at,
                &[
                    "form",
                    "plugin",
                    "fields",
                    "allow_reference",
                    "brief",
                    "note",
                ],
            )?;
            optional_string(t, "note", &at)?;
        }
        sets.push(SetEntry {
            key: at.clone(),
            form: string(t, "form", &at)?.to_string(),
            plugin: optional_string(t, "plugin", &at)?,
            edits: fields(t, &at)?,
            allow_reference: boolean(t, "allow_reference", &at)?,
            brief: optional_string(t, "brief", &at)?,
        });
    }
    if version == 1 && rules.is_empty() && sets.is_empty() {
        return Err(error("set", "the spec has no [[set]] or [[rule]] entries"));
    }
    Ok(Spec {
        version,
        load_order,
        strict,
        classes,
        rules,
        sets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::fields::Op;

    fn err(text: &str) -> String {
        parse(text).unwrap_err().to_string()
    }

    #[test]
    fn version_two_parses_classes_rules_and_sets() {
        let spec = parse(
            "version = 2\nstrict = true\n[plugin]\nstyle = \"kit\"\n\
             [class.rifles]\ntype = \"weap\"\nwhere = 'EDID ~ \"Rifle\"'\nbrief = \"B1\"\n\
             [class.all_globals]\ntype = \"GLOB\"\n\
             [[rule]]\nclass = \"rifles\"\nvia = [\"KWDA\"]\nbrief = \"B1\"\nfields = { EDID = \"Kw\" }\n\
             [[rule]]\nclass = \"all_globals\"\nallow_reference = true\nfields = { FLTV = { clamp = [0.0, 1.5] } }\n\
             [[set]]\nform = \"Starfield.esm:000800\"\nfields = { FULL = \"X\" }\n",
        )
        .unwrap();
        assert_eq!(spec.version, 2);
        assert!(spec.strict);
        let names: Vec<&str> = spec.classes.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["all_globals", "rifles"]);
        let rifles = &spec.classes[spec.rules[0].class];
        assert_eq!(rifles.signature, *b"WEAP");
        assert_eq!(rifles.brief.as_deref(), Some("B1"));
        assert!(spec.classes[spec.rules[1].class].query.is_none());
        assert_eq!(spec.rules[0].via[0].text, "KWDA");
        assert!(spec.rules[1].allow_reference);
        assert_eq!(spec.rules[1].edits[0].op, Op::Clamp(0.0, 1.5));
        assert_eq!(spec.sets.len(), 1);
        // An empty version 2 spec parses (`plan` on a scaffold); version 1 needs entries.
        assert!(parse("version = 2\n").is_ok());
        assert!(err("version = 1\n").contains("spec: set:"));
    }

    #[test]
    fn errors_name_the_key() {
        assert!(err("version = 3\n").starts_with("spec: version:"));
        assert!(err("version = 2\nfoo = 1\n").starts_with("spec: foo:"));
        // Version 2 keys are not version 1 keys.
        assert!(err("version = 1\nstrict = true\n").starts_with("spec: strict:"));
        assert!(err("version = 2\n[plugin]\nstyle = \"ck\"\n").contains("M3.A3"));
        assert!(err("version = 2\n[class.a]\nwhere = 'EDID ~ \"x\"'\n")
            .starts_with("spec: class.a.type:"));
        assert!(
            err("version = 2\n[class.a]\ntype = \"WEAP\"\nwhere = 'EDID ~~'\n")
                .starts_with("spec: class.a.where:")
        );
        assert!(
            err("version = 2\n[[rule]]\nclass = \"nope\"\nfields = { FULL = \"x\" }\n")
                .contains("spec: rule[0].class: no class `nope` (classes: none)")
        );
        let rule = |body: &str| {
            err(&format!(
                "version = 2\n[class.a]\ntype = \"GLOB\"\n[[rule]]\nclass = \"a\"\n{body}"
            ))
        };
        assert!(rule("via = [\"@form\"]\nfields = { FLTV = 1 }\n").contains("rule[0].via[0]"));
        assert!(rule("via = \"KWDA\"\nfields = { FLTV = 1 }\n").contains("rule[0].via:"));
        assert!(rule("fields = { FLTV = { clamp = [2, 1] } }\n")
            .contains("rule[0].fields.FLTV: FLTV: `clamp` [2, 1]: lo is above hi"));
        assert!(rule("fields = { FLTV = { pow = 2 } }\n").contains("unknown operator `pow`"));
        assert!(rule("fields = { FLTV = { min = nan } }\n").contains("not a finite number"));
        assert!(rule("fields = {}\n").contains("rule[0].fields: no fields to set"));
        assert!(rule("allow_reference = 1\nfields = { FLTV = 1 }\n")
            .contains("rule[0].allow_reference: must be true or false"));
    }
}
