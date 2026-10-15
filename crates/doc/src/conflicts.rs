//! The conflict graph (M2.D6): forms that plugins override, each override classed against
//! the version before it in load order. Subrecords the schema gives a layout are compared
//! member by member, FormIDs resolved through each version's plugin (M3.D2,
//! [`classify_versions`]); the rest as bytes.

use std::collections::BTreeMap;

use kitkraft_schema::Schema;

use crate::compare::{classify_with, edid_first, FieldCompare, OverrideClass};
use crate::error::Result;
use crate::fields::{FormValue, Side};
use crate::form::GlobalForm;
use crate::load_order::LoadOrder;
use crate::overrides::Version;
use crate::record::{LoadedRecord, RecordReader};
use crate::typed;

/// A FormID stored in plugin `plugin`, resolved for comparison: the key is the resolved
/// form's (plugin index and object ID), the label its formref.
pub fn resolve_for_compare(lo: &LoadOrder, plugin: u16, raw: u32) -> FormValue {
    match lo.resolve(plugin, raw) {
        Ok(g) => FormValue::Form {
            key: format!("{:012X}", g.key()),
            label: lo.form_label(g),
        },
        Err(e) => FormValue::Unresolved {
            raw,
            error: e.to_string(),
        },
    }
}

/// The decoding context of a version.
fn ctx_of(lo: &LoadOrder, plugin: u16, rec: &LoadedRecord) -> typed::Ctx {
    typed::Ctx {
        form_version: rec.record.header.form_version,
        localized: lo
            .plugin(plugin)
            .and_then(|p| p.header())
            .is_some_and(|h| h.flags.localized()),
        edid_first: edid_first(rec),
    }
}

/// Class of version `current` against `previous` (two versions of one form, as read):
/// members of typed subrecords compared by value with FormIDs resolved through each
/// version's plugin when `schema` is given, else bytes.
pub fn classify_versions(
    lo: &LoadOrder,
    schema: Option<&Schema>,
    previous: (&Version, &LoadedRecord),
    current: (&Version, &LoadedRecord),
) -> OverrideClass {
    let (pv, pr) = previous;
    let (cv, cr) = current;
    let Some(schema) = schema else {
        return classify_with(pr, cr, None);
    };
    let ra = |raw: u32| resolve_for_compare(lo, pv.plugin, raw);
    let rb = |raw: u32| resolve_for_compare(lo, cv.plugin, raw);
    let cmp = FieldCompare {
        schema,
        a: Side {
            ctx: ctx_of(lo, pv.plugin, pr),
            resolve: &ra,
        },
        b: Side {
            ctx: ctx_of(lo, cv.plugin, cr),
            resolve: &rb,
        },
        same_encoding: lo.encodings_compatible(pv.plugin, cv.plugin),
    };
    classify_with(pr, cr, Some(&cmp))
}

/// Which overridden forms to consider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Filter {
    /// Only forms this plugin defines.
    pub master: Option<u16>,
    /// Only forms this plugin overrides.
    pub plugin: Option<u16>,
    /// Only forms of this record type (the defining version's, or the first version's).
    pub signature: Option<[u8; 4]>,
    /// Only forms with two or more overriding plugins.
    pub only_conflicts: bool,
}

/// One override, classed against the version before it.
#[derive(Debug, Clone, PartialEq)]
pub struct Override {
    /// This version.
    pub version: Version,
    /// The version before it in load order, `None` when it is the first version.
    pub previous: Option<Version>,
    /// The class, `None` without a previous version.
    pub class: Option<OverrideClass>,
    /// Whether payload FormIDs of the two versions are encoded alike (otherwise byte
    /// differences may be encoding, not content).
    pub comparable: bool,
}

/// A form with its versions and classed overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct FormConflict {
    /// The form.
    pub form: GlobalForm,
    /// Every version, in load order.
    pub versions: Vec<Version>,
    /// Whether the defining plugin has the record.
    pub base_present: bool,
    /// Versions held by plugins other than the defining one.
    pub overrides: Vec<Override>,
}

/// Signature of a version's record.
pub fn signature_of(lo: &LoadOrder, v: &Version) -> Option<[u8; 4]> {
    lo.record(v.plugin, v.record).map(|r| r.signature)
}

/// The overridden forms passing `filter`, in form order (each as its run of versions).
pub fn overridden<'a>(lo: &'a LoadOrder, filter: &Filter) -> Vec<&'a [Version]> {
    lo.forms
        .runs()
        .filter(|run| {
            let Some(first) = run.first() else {
                return false;
            };
            let form = first.form();
            if filter.master.is_some_and(|m| m != form.plugin) {
                return false;
            }
            let overriders: Vec<u16> = run
                .iter()
                .map(|v| v.plugin)
                .filter(|p| *p != form.plugin)
                .collect();
            if overriders.is_empty() {
                return false;
            }
            if filter.plugin.is_some_and(|p| !overriders.contains(&p)) {
                return false;
            }
            if let Some(sig) = filter.signature {
                let base = run
                    .iter()
                    .find(|v| v.plugin == form.plugin)
                    .unwrap_or(first);
                if signature_of(lo, base) != Some(sig) {
                    return false;
                }
            }
            if filter.only_conflicts {
                let mut distinct = overriders.clone();
                distinct.dedup();
                if distinct.len() < 2 {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// Read and class every override of one form ([`classify_versions`] with `schema`).
pub fn classify_run(
    lo: &LoadOrder,
    reader: &mut RecordReader,
    run: &[Version],
    schema: Option<&Schema>,
) -> Result<FormConflict> {
    let form = run.first().map_or(
        GlobalForm {
            plugin: 0,
            object_id: 0,
        },
        Version::form,
    );
    let mut loaded = Vec::with_capacity(run.len());
    for v in run {
        let loc = lo
            .record(v.plugin, v.record)
            .copied()
            .ok_or_else(|| crate::DocError::Invalid("version not in index".into()))?;
        loaded.push(reader.read(lo, v.plugin, &loc)?);
    }
    let mut overrides = Vec::new();
    for (i, v) in run.iter().enumerate() {
        if v.plugin == form.plugin {
            continue;
        }
        let prev = i.checked_sub(1).and_then(|j| run.get(j).copied());
        let class = i.checked_sub(1).and_then(|j| {
            Some(classify_versions(
                lo,
                schema,
                (run.get(j)?, loaded.get(j)?),
                (v, loaded.get(i)?),
            ))
        });
        overrides.push(Override {
            version: *v,
            previous: prev,
            class,
            comparable: prev.is_some_and(|p| lo.encodings_compatible(p.plugin, v.plugin)),
        });
    }
    Ok(FormConflict {
        form,
        versions: run.to_vec(),
        base_present: run.iter().any(|v| v.plugin == form.plugin),
        overrides,
    })
}

/// `esmscan.py`'s counts: for each plugin, how many of its records override a form `master`
/// defines, by record type.
pub fn override_counts(lo: &LoadOrder, master: u16) -> BTreeMap<u16, BTreeMap<String, u64>> {
    let mut out: BTreeMap<u16, BTreeMap<String, u64>> = BTreeMap::new();
    for run in lo.forms.runs() {
        let Some(first) = run.first() else { continue };
        if first.form().plugin != master {
            continue;
        }
        for v in run.iter().filter(|v| v.plugin != master) {
            let sig = signature_of(lo, v).map_or_else(
                || "????".to_string(),
                |s| String::from_utf8_lossy(&s).into_owned(),
            );
            let n = out.entry(v.plugin).or_default().entry(sig).or_insert(0);
            *n = n.saturating_add(1);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::load_order::{LoadOptions, PluginSpec};
    use crate::testdata::{self, rec, rec_flags};

    /// Base defines four weapons; A and B override them in each class.
    fn lo() -> LoadOrder {
        let dir = testdata::temp_dir("conflicts");
        let fields = |n: &str| {
            vec![
                ("EDID", format!("{n}\0").into_bytes()),
                ("DATA", vec![1, 2]),
            ]
        };
        let base = testdata::plugin_with(
            0x1,
            &[],
            &[
                rec(b"WEAP", 0x801, &fields("Same")),
                rec(b"WEAP", 0x802, &fields("Diff")),
                rec(b"WEAP", 0x803, &fields("Del")),
                rec(b"KYWD", 0x804, &fields("Two")),
            ],
        );
        let a = testdata::plugin_with(
            0x1,
            &["Base.esm"],
            &[
                rec(b"WEAP", 0x801, &fields("Same")),
                rec(
                    b"WEAP",
                    0x802,
                    &[("EDID", b"Diff\0".to_vec()), ("DATA", vec![9, 2])],
                ),
                rec_flags(b"WEAP", 0x20, 0x803, &[]),
                rec(b"KYWD", 0x804, &fields("Two")),
                rec(b"KYWD", 0x0100_0900, &fields("New")),
            ],
        );
        let b = testdata::plugin_with(0x1, &["Base.esm"], &[rec(b"KYWD", 0x804, &fields("Two"))]);
        let mut specs = Vec::new();
        for (name, bytes) in [("Base.esm", base), ("A.esm", a), ("B.esm", b)] {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            specs.push(PluginSpec {
                name: name.into(),
                path,
            });
        }
        LoadOrder::load(specs, &LoadOptions::default()).unwrap()
    }

    #[test]
    fn classes_filters_and_counts() {
        let lo = lo();
        let mut reader = RecordReader::new();
        let runs = overridden(&lo, &Filter::default());
        assert_eq!(runs.len(), 4);
        let classes: Vec<Vec<&str>> = runs
            .iter()
            .map(|run| {
                let c =
                    classify_run(&lo, &mut reader, run, kitkraft_schema::builtin().ok()).unwrap();
                assert!(c.base_present);
                c.overrides
                    .iter()
                    .map(|o| {
                        // Base -> A: A encodes Base's forms as Base does. A -> B: B cannot
                        // encode A's own forms, so a payload reference to one would read as
                        // something else in B.
                        let prev = o.previous.unwrap().plugin;
                        assert_eq!(o.comparable, prev == 0, "{o:?}");
                        o.class.as_ref().unwrap().as_str()
                    })
                    .collect()
            })
            .collect();
        assert_eq!(
            classes,
            [
                vec!["identical-to-previous"],
                vec!["differs"],
                vec!["deleted"],
                vec!["identical-to-previous", "identical-to-previous"],
            ]
        );
        let only = overridden(
            &lo,
            &Filter {
                only_conflicts: true,
                ..Filter::default()
            },
        );
        assert_eq!(only.len(), 1);
        let kywd = overridden(
            &lo,
            &Filter {
                signature: Some(*b"KYWD"),
                plugin: Some(2),
                master: Some(0),
                ..Filter::default()
            },
        );
        assert_eq!(kywd.len(), 1);
        let counts = override_counts(&lo, 0);
        assert_eq!(counts[&1]["WEAP"], 3);
        assert_eq!(counts[&1]["KYWD"], 1);
        assert_eq!(counts[&2]["KYWD"], 1);
        assert!(override_counts(&lo, 1).is_empty());
    }
}
