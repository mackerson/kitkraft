//! Which `ObjectModProperty` values the armor object mods (`OMOD`) of the 14 official
//! ESMs carry (KK-011): the survey prints, per route, how many armor OMODs exist, then
//! the property codes of the union's OMOD `DATA` payloads and of the `OBTS` template
//! items of `ARMO` records themselves, and, for properties whose values are FormIDs
//! (value types 4 and 6), the record types those FormIDs name.
//!
//! The routes to the armor OMODs:
//!
//! - `template`: every OMOD an `ARMO`'s object template names (the template item's
//!   `Includes.Mod`);
//! - `attach-point`: every OMOD whose `DATA` attach point keyword is one some `ARMO`'s
//!   `APPR` lists.
//!
//! Records are keyed by the file their FormID *resolves to*, not the file they were read
//! from: a DLC that overrides a base-game OMOD stores the base record's FormID, so the
//! override and the original share one key, and the file processed later in the
//! manifest (the game's load order) holds the data. As in `omod_properties.rs`, every
//! record read (the `ARMO` and `OMOD` records) must decode with nothing left in `extra`
//! and re-encode byte for byte. Every FormID that resolves to nothing — template
//! includes, `APPR` keywords, attach points, and property values — is counted on one
//! `armor-omods unresolved` line (with a few raw examples per property), never panicked
//! on. The property tables are sorted by the property code's display name.
//!
//! `cargo test -p kitkraft-esm --release --test armor_omods -- --ignored --nocapture`

// Test-only code: a panic fails the test.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used
)]

mod common;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use kitkraft_esm::bridge::{Ctx, RawSub};
use kitkraft_esm::generated::records::{
    decode, ObjectModProperty, ObjectModPropertyValue1, ObteBlockCombinationTemplateItem, OmodData,
    Record as Typed,
};
use kitkraft_esm::index::{index_file, read_indexed, IndexOptions};
use kitkraft_esm::tes4::{Tes4, FLAG_LOCALIZED};
use kitkraft_esm::DecodeLimits;

/// Property entries of one object mod or template item, aggregated by property code.
#[derive(Default)]
struct PropAgg {
    count: u64,
    value_types: BTreeSet<u8>,
    /// FormID-valued entries (value types 4 and 6): the raw FormID and the file it was
    /// read from.
    form_refs: Vec<(u32, String)>,
}

/// What the routes and the union need from one OMOD `DATA`.
struct OmodInfo {
    /// The attach point keyword, resolved to (file, object ID), or `None` when it names
    /// nothing (counted in `dropped_refs`).
    attach_point: Option<(String, u32)>,
    properties: Vec<ObjectModProperty>,
}

#[derive(Default)]
struct Survey {
    failures: Vec<String>,
    /// Every record of the corpus, keyed by (owner file, object ID), to its signature.
    sig_by_file: BTreeMap<(String, u32), [u8; 4]>,
    /// Per file: its master list, in order.
    masters: BTreeMap<String, Vec<String>>,
    /// The attach parent slot keywords of every `ARMO` `APPR`, resolved.
    app_keywords: HashSet<(String, u32)>,
    /// Route `template`: the OMODs the `ARMO` template items name, resolved to
    /// (owner file, object ID).
    template_refs: Vec<(String, u32)>,
    /// Every OMOD, keyed by (owner file, object ID); the value is the file the data was
    /// read from (later in the manifest, i.e. the load order, wins).
    omods: BTreeMap<(String, u32), (String, OmodInfo)>,
    /// The properties of the `ARMO` template items, by property code display name.
    template_props: BTreeMap<String, PropAgg>,
    /// FormIDs that resolve to nothing: template includes, `APPR` keywords, OMOD attach
    /// points, and record owners that name no file in the load order.
    dropped_refs: u64,
}

impl Survey {
    fn fail(&mut self, message: String) {
        if self.failures.len() < 20 {
            self.failures.push(message);
        }
    }
}

/// A four-character property code, or the value in hex.
fn property_name(p: u32) -> String {
    let b = p.to_le_bytes();
    if b.iter()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        String::from_utf8_lossy(&b).into_owned()
    } else {
        format!("{p:#x}")
    }
}

/// Resolve a FormID read from file `from` to (holding file, object ID): the high byte
/// indexes `from`'s master list (0-based); the high byte equal to the master count
/// names `from` itself. Full-scale files only, as the official ESMs are.
fn resolve(from: &str, raw: u32, masters: &[String]) -> Option<(String, u32)> {
    if raw == 0 {
        return None;
    }
    let hi = usize::try_from(raw >> 24).unwrap_or(usize::MAX);
    let oid = raw & 0x00FF_FFFF;
    if hi < masters.len() {
        Some((masters[hi].clone(), oid))
    } else if hi == masters.len() {
        Some((from.to_string(), oid))
    } else {
        None
    }
}

fn tally(agg: &mut BTreeMap<String, PropAgg>, p: &ObjectModProperty, file: &str) {
    let entry = agg.entry(property_name(p.property)).or_default();
    entry.count += 1;
    let vt = p.value_type.to_raw();
    entry.value_types.insert(vt);
    if matches!(vt, 4 | 6) {
        if let ObjectModPropertyValue1::FormId(id) = &p.value1 {
            entry.form_refs.push((id.0, file.to_string()));
        }
    }
}

fn check_file(survey: &mut Survey, name: &str, path: &Path) {
    let index = index_file(path, &IndexOptions::default()).unwrap();
    let masters: Vec<String> = Tes4::new(&index.header)
        .masters()
        .iter()
        .map(|m| m.name_lossy())
        .collect();
    survey.masters.insert(name.to_string(), masters.clone());
    for r in index.records.iter().skip(1) {
        match resolve(name, r.form_id, &masters) {
            Some(owner) => {
                survey.sig_by_file.insert(owner, r.signature.bytes());
            }
            None => survey.dropped_refs += 1,
        }
    }

    let localized = index.header.header.flags & FLAG_LOCALIZED != 0;
    let limits = DecodeLimits::default();
    let mut input = BufReader::new(File::open(path).unwrap());
    for loc in &index.records {
        let sig = loc.signature.bytes();
        if sig != *b"ARMO" && sig != *b"OMOD" {
            continue;
        }
        let record = read_indexed(&mut input, loc, Some(&limits)).unwrap();
        let subs: Vec<RawSub<'_>> = record
            .subrecords()
            .expect("decoded")
            .iter()
            .map(RawSub::from)
            .collect();
        let ctx = Ctx::for_record(&record, localized);
        let what = format!("{name} {} {:08X}", loc.signature, loc.form_id);
        let typed = Typed::decode(sig, &subs, ctx).expect("a definition exists");
        if !typed.extra().is_empty() {
            let sigs: Vec<String> = typed
                .extra()
                .iter()
                .map(|(i, s)| format!("{}@{i}", String::from_utf8_lossy(&s.signature)))
                .collect();
            survey.fail(format!("{what}: not decoded: {}", sigs.join(", ")));
        }
        let encoded = typed.encode();
        let same = encoded.len() == subs.len()
            && encoded.iter().zip(&subs).all(|(e, r)| {
                e.signature == r.signature && e.data == r.data && e.oversized == r.oversized
            });
        if !same {
            survey.fail(format!("{what}: re-encoding differs"));
        }
        let Some(owner) = resolve(name, loc.form_id, &masters) else {
            survey.dropped_refs += 1;
            continue;
        };
        match &typed {
            Typed::Armo(armo) => {
                if let Some(appr) = &armo.attach_parent_slots {
                    for k in &appr.value.0 {
                        match resolve(name, k.0, &masters) {
                            Some(t) => {
                                survey.app_keywords.insert(t);
                            }
                            None => survey.dropped_refs += 1,
                        }
                    }
                }
                for s in &subs {
                    if s.signature == *b"OBTS" {
                        match decode::<ObteBlockCombinationTemplateItem>(s.data, "OBTS", ctx) {
                            Ok(item) => {
                                for inc in &item.includes {
                                    match resolve(name, inc.mod_.0, &masters) {
                                        Some(t) => survey.template_refs.push(t),
                                        None => survey.dropped_refs += 1,
                                    }
                                }
                                for p in &item.properties {
                                    tally(&mut survey.template_props, p, name);
                                }
                            }
                            Err(e) => survey.fail(format!("{what}: OBTS: {e}")),
                        }
                    }
                }
            }
            Typed::Omod(_) => {
                for s in &subs {
                    if s.signature == *b"DATA" {
                        match decode::<OmodData>(s.data, "OMOD.DATA", ctx) {
                            Ok(d) => {
                                let attach_point = match resolve(name, d.attach_point.0, &masters) {
                                    Some(t) => Some(t),
                                    None => {
                                        survey.dropped_refs += 1;
                                        None
                                    }
                                };
                                survey.omods.insert(
                                    owner.clone(),
                                    (
                                        name.to_string(),
                                        OmodInfo {
                                            attach_point,
                                            properties: d.properties,
                                        },
                                    ),
                                );
                            }
                            Err(e) => survey.fail(format!("{what}: DATA: {e}")),
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Print the routes, the union, the unresolved total, and the two property tables.
/// Returns the union size.
fn finish(survey: &Survey) -> usize {
    let route_template: HashSet<(String, u32)> = survey.template_refs.iter().cloned().collect();
    let mut route_attach_point: HashSet<(String, u32)> = HashSet::new();
    for ((file, oid), (_, info)) in &survey.omods {
        if info
            .attach_point
            .as_ref()
            .is_some_and(|ap| survey.app_keywords.contains(ap))
        {
            route_attach_point.insert((file.clone(), *oid));
        }
    }
    let union: HashSet<(String, u32)> =
        route_template.union(&route_attach_point).cloned().collect();
    println!("armor-omods route template: {} OMODs", route_template.len());
    println!(
        "armor-omods route attach-point: {} OMODs",
        route_attach_point.len()
    );

    let mut props: BTreeMap<String, PropAgg> = BTreeMap::new();
    let mut total = 0u64;
    for key in &union {
        if let Some((from, info)) = survey.omods.get(key) {
            for p in &info.properties {
                total += 1;
                tally(&mut props, p, from);
            }
        }
    }
    println!("armor-omods: {} OMODs, {} properties", union.len(), total);
    let omod_unresolved = print_props("armor-omod", &props, survey);
    let template_unresolved = print_props("armor-template", &survey.template_props, survey);
    println!(
        "armor-omods unresolved: {}",
        survey.dropped_refs + omod_unresolved + template_unresolved
    );
    union.len()
}

/// Print one property table, sorted by code display name. Returns how many of the
/// table's FormID values did not resolve to a record of the corpus.
fn print_props(prefix: &str, props: &BTreeMap<String, PropAgg>, survey: &Survey) -> u64 {
    let mut unresolved_total = 0u64;
    for (code, agg) in props {
        let vts: Vec<String> = agg.value_types.iter().map(|v| v.to_string()).collect();
        println!(
            "{prefix} property {}: {} (value types {})",
            code,
            agg.count,
            vts.join(", ")
        );
        if agg.form_refs.is_empty() {
            continue;
        }
        let mut targets: BTreeMap<String, u64> = BTreeMap::new();
        let mut unresolved: Vec<(u32, String)> = Vec::new();
        for (raw, from) in &agg.form_refs {
            let name = survey
                .masters
                .get(from)
                .and_then(|m| {
                    let (file, oid) = resolve(from, *raw, m)?;
                    survey
                        .sig_by_file
                        .get(&(file, oid))
                        .map(|sig| String::from_utf8_lossy(sig).into_owned())
                })
                .unwrap_or_else(|| "unresolved".to_string());
            if name == "unresolved" {
                unresolved.push((*raw, from.clone()));
            }
            *targets.entry(name).or_default() += 1;
        }
        let list: Vec<String> = targets.iter().map(|(k, v)| format!("{k} {v}")).collect();
        println!("{prefix} property {} targets: {}", code, list.join(", "));
        if !unresolved.is_empty() {
            unresolved.sort();
            let examples: Vec<String> = unresolved
                .iter()
                .take(5)
                .map(|(raw, from)| format!("0x{raw:08X} in {from}"))
                .collect();
            println!(
                "{} property {} unresolved examples: {}",
                prefix,
                code,
                examples.join(", ")
            );
            unresolved_total += u64::try_from(unresolved.len()).unwrap_or(u64::MAX);
        }
    }
    unresolved_total
}

#[test]
#[ignore = "reads all 14 official ESMs; run in release"]
fn armor_omod_properties_full() {
    if std::env::var_os(common::DATA_DIR_VAR).is_none()
        && std::env::var_os(common::OLD_DATA_DIR_VAR).is_none()
    {
        println!("skipped: KITKRAFT_DATA_DIR is not set (see docs/corpus.md)");
        return;
    }
    let Some(files) = common::all_files("armor_omod_properties_full") else {
        return;
    };
    let mut survey = Survey::default();
    for f in &files {
        check_file(&mut survey, &f.name, &f.path);
    }
    let union = finish(&survey);
    assert!(survey.failures.is_empty(), "{:#?}", survey.failures);
    assert!(union > 0, "no armor OMOD found");
}
