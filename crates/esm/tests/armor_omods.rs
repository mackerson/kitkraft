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
//! FormIDs are raw and file-relative (`docs/load-order.md`, "FormIDs"): the high byte
//! `0xFE` or `0xFD` marks the small or medium class, whose index is the next 12 or 8
//! bits and whose object ID is the low 12 or 16 bits; any other high byte is the full
//! class (index = high byte, 24-bit object ID). The index counts only the reading
//! file's masters of that class, in master-list order; the index equal to that count,
//! in the file's own class, names the file itself. A master whose scale is unknown
//! (not in the corpus) makes every FormID of the file unresolvable, never a guess.
//! Master names are matched case-insensitively, as `onam.rs` and `crates/doc` do.
//! `0x00000000` is the null FormID: it is counted as `null`, never as `unresolved`,
//! and never resolved.
//!
//! Records are keyed by the file their FormID *resolves to*, not the file they were
//! read from: a DLC that overrides a base-game record stores the base record's FormID,
//! so the override and the original share one key, and the file processed later in the
//! manifest (the game's load order) holds the data. This holds for `ARMO` records
//! too: their `APPR` keywords, template includes and template-item properties are
//! taken from the last file that stores them, so each overridden `ARMO` is counted
//! once.
//!
//! As in `omod_properties.rs`, every record read (the `ARMO` and `OMOD` records) must
//! decode with nothing left in `extra` and re-encode byte for byte. Every FormID that
//! resolves to nothing is counted, never panicked on: unresolvable record owners are
//! counted per file, once per record, in the record-index pass, and the rest —
//! template includes, `APPR` keywords, attach points, property values — per property,
//! with a few raw examples. The property tables are sorted by the property code's
//! display name.
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
use kitkraft_esm::tes4::{Scale, Tes4, FLAG_LOCALIZED};
use kitkraft_esm::DecodeLimits;

/// A scale as a map-key tag: `Scale` has no ordering, but owner keys go in `BTreeMap`s.
fn scale_tag(s: Scale) -> u8 {
    match s {
        Scale::Full => 0,
        Scale::Medium => 1,
        Scale::Small => 2,
    }
}

/// What a raw FormID read from a file names: a record of the corpus, the null FormID,
/// or nothing.
enum Ref {
    /// (owner file, the class of the reference, object ID).
    Record((String, u8, u32)),
    /// The null FormID `0x00000000`: not a form.
    Null,
    /// Names no file, or names a file but no record in it: counted, never resolved.
    Unresolved,
}

/// One `ARMO`'s data, as stored by the last file in the manifest that holds the
/// record (later in the load order wins).
struct ArmoInfo {
    /// The file the data was read from (its master list resolves the raw refs).
    source: String,
    /// The `APPR` attach parent slot keywords, raw.
    appr: Vec<u32>,
    /// Per `OBTS` item: the `Includes.Mod` FormIDs, raw, and the item's properties.
    template_items: Vec<(Vec<u32>, Vec<ObjectModProperty>)>,
}

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
    /// The attach point keyword, resolved, or `None` when it names nothing (counted in
    /// the null or unresolved totals when the OMOD is in the armor union).
    attach_point: Option<(String, u8, u32)>,
    /// The attach point is the null FormID (`None` above for that reason).
    attach_null: bool,
    properties: Vec<ObjectModProperty>,
}

/// A file's master list and scales: what its FormIDs resolve against.
#[derive(Clone)]
struct MasterInfo {
    /// The master names, in order, lowercased (matched case-insensitively).
    names: Vec<String>,
    /// Each master's own scale, aligned with `names`; `None` if the master's file is
    /// not in the corpus.
    scales: Vec<Option<Scale>>,
    /// This file's own scale.
    own: Option<Scale>,
}

impl MasterInfo {
    /// Resolve `raw` as read from `from`.
    fn resolve(&self, from: &str, raw: u32) -> Ref {
        if raw == 0 {
            return Ref::Null;
        }
        // A master not in the corpus: no FormID of the file resolves (docs/load-order.md).
        if self.scales.iter().any(|s| s.is_none()) {
            return Ref::Unresolved;
        }
        let (class, index, oid) = match raw >> 24 {
            0xFE => (Scale::Small, (raw >> 12) & 0x0FFF, raw & 0x0000_0FFF),
            0xFD => (Scale::Medium, (raw >> 16) & 0xFF, raw & 0x0000_FFFF),
            hi => (Scale::Full, hi, raw & 0x00FF_FFFF),
        };
        let mut same_class = 0u32;
        for (name, scale) in self.names.iter().zip(&self.scales) {
            if *scale == Some(class) {
                if same_class == index {
                    return Ref::Record((name.clone(), scale_tag(class), oid));
                }
                same_class += 1;
            }
        }
        if index == same_class && self.own == Some(class) {
            return Ref::Record((from.to_ascii_lowercase(), scale_tag(class), oid));
        }
        Ref::Unresolved
    }
}

#[derive(Default)]
struct Survey {
    failures: Vec<String>,
    /// Every record of the corpus, keyed by (owner file, class of the reference, object
    /// ID), to its signature.
    sig_by_file: BTreeMap<(String, u8, u32), [u8; 4]>,
    /// Per file (lowercased name): what its FormIDs resolve against.
    infos: BTreeMap<String, MasterInfo>,
    /// Per file: records whose owner FormID names no file, counted once each in the
    /// record-index pass.
    owners_unresolved: BTreeMap<String, u64>,
    /// The attach parent slot keywords of every `ARMO` `APPR`, resolved.
    app_keywords: HashSet<(String, u8, u32)>,
    /// Route `template`: the OMODs the `ARMO` template items name, resolved to
    /// (owner file, class, object ID).
    template_refs: BTreeSet<(String, u8, u32)>,
    /// Every OMOD, keyed by (owner file, class, object ID); the value is the file the
    /// data was read from (later in the manifest, i.e. the load order, wins).
    omods: BTreeMap<(String, u8, u32), (String, OmodInfo)>,
    /// Every `ARMO`, keyed by (owner file, class, object ID); the value is the last
    /// file in the manifest that stores the record.
    armos: BTreeMap<(String, u8, u32), ArmoInfo>,
    /// The properties of the `ARMO` template items, by property code display name.
    template_props: BTreeMap<String, PropAgg>,
    /// FormID values that resolve to nothing: template includes and `APPR` keywords
    /// (the union's attach points and property values are counted in `finish`).
    dropped_refs: u64,
    /// `0x00000000` values: not forms, not unresolved.
    null_refs: u64,
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

/// One file: build the record signature map (counting unresolvable owners per file,
/// once per record), then decode and check every `ARMO` and `OMOD`, keeping their raw
/// data keyed by owner.
fn check_file(survey: &mut Survey, name: &str, path: &Path, info: &MasterInfo) {
    let index = index_file(path, &IndexOptions::default()).unwrap();
    let mut owners_unresolved = 0u64;
    for r in index.records.iter().skip(1) {
        match info.resolve(name, r.form_id) {
            Ref::Record(owner) => {
                survey.sig_by_file.insert(owner, r.signature.bytes());
            }
            Ref::Unresolved => owners_unresolved += 1,
            Ref::Null => {}
        }
    }
    survey
        .owners_unresolved
        .insert(name.to_string(), owners_unresolved);

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
        // The owner was already counted in the record-index pass; skip without
        // counting again.
        let Ref::Record(owner) = info.resolve(name, loc.form_id) else {
            continue;
        };
        match &typed {
            Typed::Armo(armo) => {
                let mut appr = Vec::new();
                if let Some(a) = &armo.attach_parent_slots {
                    appr = a.value.0.iter().map(|f| f.0).collect();
                }
                let mut template_items = Vec::new();
                for s in &subs {
                    if s.signature == *b"OBTS" {
                        match decode::<ObteBlockCombinationTemplateItem>(s.data, "OBTS", ctx) {
                            Ok(item) => {
                                let includes: Vec<u32> =
                                    item.includes.iter().map(|i| i.mod_.0).collect();
                                template_items.push((includes, item.properties));
                            }
                            Err(e) => survey.fail(format!("{what}: OBTS: {e}")),
                        }
                    }
                }
                // Keyed by owner, later file in the manifest wins: each overridden
                // `ARMO` is counted once.
                survey.armos.insert(
                    owner,
                    ArmoInfo {
                        source: name.to_string(),
                        appr,
                        template_items,
                    },
                );
            }
            Typed::Omod(_) => {
                for s in &subs {
                    if s.signature == *b"DATA" {
                        match decode::<OmodData>(s.data, "OMOD.DATA", ctx) {
                            Ok(d) => {
                                let resolved = info.resolve(name, d.attach_point.0);
                                let attach_null = matches!(resolved, Ref::Null);
                                let attach_point = match resolved {
                                    Ref::Record(t) => Some(t),
                                    Ref::Null | Ref::Unresolved => None,
                                };
                                survey.omods.insert(
                                    owner.clone(),
                                    (
                                        name.to_string(),
                                        OmodInfo {
                                            attach_point,
                                            attach_null,
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

/// Resolve the `ARMO` data collected per owner: the union of `APPR` keywords, the
/// template route, and the template-item property table.
fn finish_armos(survey: &mut Survey) {
    let mut template_props: BTreeMap<String, PropAgg> = BTreeMap::new();
    for info in survey.armos.values() {
        let Some(master) = survey.infos.get(&info.source.to_ascii_lowercase()) else {
            continue;
        };
        for k in &info.appr {
            match master.resolve(&info.source, *k) {
                Ref::Record(t) => {
                    survey.app_keywords.insert(t);
                }
                Ref::Null => survey.null_refs += 1,
                Ref::Unresolved => survey.dropped_refs += 1,
            }
        }
        for (includes, props) in &info.template_items {
            for raw in includes {
                match master.resolve(&info.source, *raw) {
                    Ref::Record(t) => {
                        survey.template_refs.insert(t);
                    }
                    Ref::Null => survey.null_refs += 1,
                    Ref::Unresolved => survey.dropped_refs += 1,
                }
            }
            for p in props {
                tally(&mut template_props, p, &info.source);
            }
        }
    }
    survey.template_props = template_props;
}

/// Print the per-file owner counts, the routes, the union, the unresolved totals, and
/// the two property tables. Returns the union size.
fn finish(survey: &Survey) -> usize {
    for (file, n) in &survey.owners_unresolved {
        println!("armor-omods owners {file}: {n} records whose owner FormID resolves to no file");
    }
    let route_template = survey.template_refs.clone();
    let mut route_attach_point: BTreeSet<(String, u8, u32)> = BTreeSet::new();
    for ((file, scale, oid), (_, info)) in &survey.omods {
        if info
            .attach_point
            .as_ref()
            .is_some_and(|ap| survey.app_keywords.contains(ap))
        {
            route_attach_point.insert((file.clone(), *scale, *oid));
        }
    }
    let union: HashSet<(String, u8, u32)> =
        route_template.union(&route_attach_point).cloned().collect();
    println!("armor-omods route template: {} OMODs", route_template.len());
    println!(
        "armor-omods route attach-point: {} OMODs",
        route_attach_point.len()
    );

    let mut props: BTreeMap<String, PropAgg> = BTreeMap::new();
    let mut total = 0u64;
    // The union's attach points that name nothing: only armor OMODs are counted.
    let (mut attach_null, mut attach_unresolved) = (0u64, 0u64);
    for key in &union {
        if let Some((from, info)) = survey.omods.get(key) {
            if info.attach_null {
                attach_null += 1;
            } else if info.attach_point.is_none() {
                attach_unresolved += 1;
            }
            for p in &info.properties {
                total += 1;
                tally(&mut props, p, from);
            }
        }
    }
    println!("armor-omods read: {} OMODs", survey.omods.len());
    println!("armor-omods: {} OMODs, {} properties", union.len(), total);
    let omod_unresolved = print_props("armor-omod", &props, survey);
    let template_unresolved = print_props("armor-template", &survey.template_props, survey);
    let owners_total: u64 = survey.owners_unresolved.values().sum();
    println!(
        "armor-omods unresolved: {}",
        owners_total
            + survey.dropped_refs
            + attach_unresolved
            + omod_unresolved
            + template_unresolved
    );
    println!("armor-omods null: {}", survey.null_refs + attach_null);
    union.len()
}

/// The record type a property value's FormID names, resolved through the record index
/// of the file that holds it; `null` for `0x00000000`; `unresolved` when it names no
/// record of the corpus.
fn target_of(survey: &Survey, from: &str, raw: u32) -> String {
    match survey
        .infos
        .get(&from.to_ascii_lowercase())
        .map(|m| m.resolve(from, raw))
    {
        Some(Ref::Record(key)) => survey
            .sig_by_file
            .get(&key)
            .map(|sig| String::from_utf8_lossy(sig).into_owned())
            .unwrap_or_else(|| "unresolved".to_string()),
        Some(Ref::Null) => "null".to_string(),
        Some(Ref::Unresolved) | None => "unresolved".to_string(),
    }
}

/// Print one property table, sorted by code display name. Returns how many of the
/// table's FormID values did not resolve to a record of the corpus (nulls excluded).
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
            let name = target_of(survey, from, *raw);
            if name == "unresolved" {
                unresolved.push((*raw, from.clone()));
            }
            *targets.entry(name).or_default() += 1;
        }
        let list: Vec<String> = targets.iter().map(|(k, v)| format!("{k} {v}")).collect();
        println!("{prefix} property {} targets: {}", code, list.join(", "));
        if !unresolved.is_empty() {
            unresolved_total += u64::try_from(unresolved.len()).unwrap_or(u64::MAX);
            unresolved.sort();
            unresolved.dedup();
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
    // Index every file first: a master's scale is its own file's scale, known only
    // once every file is indexed.
    let mut infos: Vec<MasterInfo> = Vec::new();
    for f in &files {
        let index = index_file(&f.path, &IndexOptions::default()).unwrap();
        let tes4 = Tes4::new(&index.header);
        let names: Vec<String> = tes4
            .masters()
            .iter()
            .map(|m| m.name_lossy().to_ascii_lowercase())
            .collect();
        infos.push(MasterInfo {
            names,
            scales: vec![],
            own: tes4.scale(),
        });
    }
    let scales_by_file: BTreeMap<String, Option<Scale>> = files
        .iter()
        .zip(&infos)
        .map(|(f, i)| (f.name.to_ascii_lowercase(), i.own))
        .collect();
    for (f, info) in files.iter().zip(&mut infos) {
        info.scales = info
            .names
            .iter()
            .map(|n| scales_by_file.get(n).copied().unwrap_or(None))
            .collect();
        survey
            .infos
            .insert(f.name.to_ascii_lowercase(), info.clone());
        check_file(&mut survey, &f.name, &f.path, info);
    }
    finish_armos(&mut survey);
    let union = finish(&survey);
    assert!(survey.failures.is_empty(), "{:#?}", survey.failures);
    assert!(union > 0, "no armor OMOD found");
}
