//! Walks every panel against a real MO2 profile without opening a window: the same
//! requests the panels build, through the same gate and worker, into the same
//! view-models, printed as text (`docs/workbench.md` was written from this output).
//!
//! Read-only, and `#[ignore]`d: an owner's check (plan/STATUS.md, "Owner next actions"),
//! not part of `cargo xtask ci`. Run it on request with `KITKRAFT_WORKBENCH_PROFILE`
//! naming a profile directory (use `KitKraft Dev`, never a profile you are playing on);
//! without it the test skips. Release build: a debug one loads a profile in about a
//! minute. PowerShell:
//!
//! ```text
//! $env:KITKRAFT_WORKBENCH_PROFILE = 'C:\Modding\MO2-Starfield\profiles\KitKraft Dev'
//! cargo test --release -p kitkraft-ui-egui --test dev_profile -- --ignored --nocapture
//! ```
#![allow(clippy::arithmetic_side_effects)]
// The helper outside `#[test]` may fail the test as the test body does.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use kitkraft_ui_egui::cmdline::command_line;
use kitkraft_ui_egui::envelope::EnvelopeView;
use kitkraft_ui_egui::gate::Request;
use kitkraft_ui_egui::panels::diag::{self, DiagView};
use kitkraft_ui_egui::panels::diff::{DiffForm, DiffMode, DiffView};
use kitkraft_ui_egui::panels::links::{LinksForm, LinksView};
use kitkraft_ui_egui::panels::load::{LoadForm, LoadView};
use kitkraft_ui_egui::panels::planet::{PlanetForm, PlanetView};
use kitkraft_ui_egui::panels::records::{
    coverage_request, inspect_request, schema_request, types_from_coverage, types_from_schema,
    QueryForm, QueryView, RecordView,
};
use kitkraft_ui_egui::panels::schema::{SchemaForm, SchemaView};
use kitkraft_ui_egui::panels::validate::{RoundtripView, ValidateForm, ValidateView};
use kitkraft_ui_egui::worker::Worker;
use serde_json::{json, Value};

fn run(worker: &mut Worker, req: Request) -> Value {
    let line = command_line(&req.id, &req.args).text;
    worker.submit(req).unwrap();
    let reply = worker.wait(Duration::from_secs(1800)).expect("reply");
    let env = reply.result.unwrap_or_else(|e| panic!("{line}: {e}"));
    let v = EnvelopeView::from_value(&env);
    println!("\n$ {line}    ({} ms)", reply.elapsed.as_millis());
    println!("  summary: {}", v.summary.as_deref().unwrap_or_default());
    println!("  coverage {}", v.coverage_line());
    println!(
        "  limitations: {}, evidence ranges: {}",
        v.limitations.len(),
        v.evidence.len()
    );
    for l in v.limitations.iter().take(3) {
        println!("    - {}", l.chars().take(160).collect::<String>());
    }
    env
}

#[test]
#[ignore = "owner's check against a real MO2 profile; see the module docs"]
fn every_panel_against_a_profile() {
    let Some(profile) = std::env::var_os("KITKRAFT_WORKBENCH_PROFILE") else {
        eprintln!("skipped: KITKRAFT_WORKBENCH_PROFILE is not set");
        return;
    };
    let mut worker = Worker::spawn(None).unwrap();

    println!("== Load");
    let form = LoadForm {
        profile: profile.to_string_lossy().into_owned(),
        ..LoadForm::default()
    };
    let load = LoadView::from_envelope(&run(&mut worker, form.request().unwrap()));
    println!("  {} plugins from {}", load.plugins.len(), load.source);
    for p in load
        .plugins
        .iter()
        .take(3)
        .chain(load.plugins.iter().rev().take(2))
    {
        println!(
            "  [{}] {} scale={} flags=[{}] records={:?} slot={} active={} cache={}",
            p.index, p.name, p.scale, p.flags, p.records, p.runtime_slot, p.active, p.cache
        );
    }
    // The biggest plugin the profile itself activates: likely to override something.
    let last = load
        .plugins
        .iter()
        .filter(|p| p.active == "plugins.txt")
        .max_by_key(|p| p.records.unwrap_or(0))
        .unwrap_or_else(|| load.plugins.last().unwrap())
        .name
        .clone();
    let last_path = load.path_of(&last);

    println!("\n== Records: types");
    let types = types_from_schema(&run(&mut worker, Request::new("schema", json!({}))));
    println!("  {} defined types (schema)", types.len());
    let cov = types_from_coverage(&run(&mut worker, coverage_request(&last_path)));
    for t in cov.iter().take(5) {
        println!(
            "  {} {:?} records={:?} {:?}",
            t.signature, t.definition, t.records, t.occurrences
        );
    }

    println!("\n== Records: query");
    let q = QueryForm {
        record_type: "WEAP".into(),
        where_text: "FULL ~ 'Rifle'".into(),
        limit: 5,
        ..QueryForm::default()
    };
    let page = QueryView::from_envelope(&run(&mut worker, q.request(None).unwrap()));
    for r in &page.rows {
        println!(
            "  {} {} \"{}\" in {} raw {} runtime {}",
            r.form, r.editor_id, r.full, r.plugin, r.raw_form_id, r.runtime_id
        );
    }
    let form = page.rows.first().map(|r| r.form.clone()).unwrap();

    println!("\n== Records: inspect + schema join");
    let mut rec =
        RecordView::from_envelope(&run(&mut worker, inspect_request(&form, None).unwrap()));
    rec.join_schema(&run(&mut worker, schema_request(&rec.signature)));
    println!(
        "  {} {} {} winner {} runtime {}",
        rec.signature, rec.global, rec.editor_id, rec.winner, rec.runtime_id
    );
    for v in &rec.versions {
        println!(
            "  version {} raw {} {} changes {:?}",
            v.plugin,
            v.raw_form_id,
            v.class,
            v.changes.iter().take(4).collect::<Vec<_>>()
        );
        for s in v.subrecords.iter().take(6) {
            println!(
                "    {} {} = {} @{:?} [{}]",
                s.signature,
                s.member,
                s.value.chars().take(60).collect::<String>(),
                s.offset,
                s.confidence.as_deref().unwrap_or("not named")
            );
        }
    }
    println!(
        "  unconfirmed (reference) signatures: {:?}",
        rec.unconfirmed()
    );

    for (title, links) in [
        ("outgoing links", LinksForm::outgoing(&form)),
        ("use info", LinksForm::use_info(&form)),
    ] {
        println!("\n== Links: {title} of {form}");
        let lv = LinksView::from_envelope(&run(&mut worker, links.request(None).unwrap()));
        println!(
            "  {} {} total {}; {}; unrecognized {}",
            lv.global, lv.label, lv.total, lv.counts, lv.unrecognized
        );
        for e in lv.edges.iter().take(6) {
            println!(
                "  [{}] {} {} {} {} -> {} {} {}",
                e.depth,
                e.from.as_deref().unwrap_or("?"),
                e.from_label,
                e.path,
                e.status,
                e.to.as_deref().unwrap_or(""),
                e.to_label,
                e.placed.as_deref().unwrap_or("")
            );
        }
    }

    println!("\n== Planet: PCM_QuestRequest");
    let pf = PlanetForm {
        form: "PCM_QuestRequest".into(),
        limit: 20,
    };
    let pv = PlanetView::from_envelope(&run(&mut worker, pf.request(None).unwrap()));
    println!(
        "  {} {} [{}] {}",
        pv.global,
        pv.label,
        pv.kind,
        pv.counts.as_deref().unwrap_or("")
    );
    for n in pv.nodes.iter().take(8) {
        println!(
            "  {}{} {} {} sel={} cond={} {}",
            "  ".repeat(usize::try_from(n.depth.min(8)).unwrap()),
            n.form,
            n.label,
            n.order,
            n.selection,
            n.conditions,
            n.content.as_ref().map_or("", |c| c.0.as_str())
        );
    }
    println!(
        "  {} links, {} placements",
        pv.links.len(),
        pv.placements.len()
    );

    println!("\n== Diff: {last} vs the versions before it (conflicts)");
    let d = DiffForm {
        mode: DiffMode::VsMasters,
        a: last.clone(),
        ..DiffForm::default()
    };
    let dv = DiffView::from_envelope(&run(&mut worker, d.request(None).unwrap()));
    println!(
        "  counts: {:?}",
        dv.counts.iter().take(3).collect::<Vec<_>>()
    );
    for r in dv.records.iter().take(3) {
        println!(
            "  {} {} {} [{}] {} rows",
            r.signature,
            r.form,
            r.sides,
            r.class,
            r.rows.len()
        );
        for f in r.rows.iter().take(3) {
            println!(
                "    {} {} {}: {} -> {}",
                f.subrecord, f.change, f.path, f.before, f.after
            );
        }
    }

    println!("\n== Diagnostics: {form}");
    let mut dg = DiagView::from_inspect(
        &run(&mut worker, inspect_request(&form, None).unwrap()),
        Some(&load),
    );
    for i in &dg.identities {
        println!(
            "  {} scale={} raw={} resolved_by={} winner={}",
            i.plugin, i.scale, i.raw_form_id, i.resolved_by, i.winner
        );
    }
    for r in dg.references.iter().take(5) {
        println!("  ref {} raw {} -> {}", r.at, r.raw, r.resolved);
    }
    dg.with_validate(&run(
        &mut worker,
        diag::validate_request(&last_path).unwrap(),
    ));
    println!("  findings: {}", dg.findings.len());

    println!("\n== Validate: {last}");
    let vf = ValidateForm {
        plugin: last_path.clone(),
        rules: String::new(),
    };
    let vv = ValidateView::from_envelope(&run(&mut worker, vf.request().unwrap()));
    println!(
        "  passed {:?}, {} errors, {} warnings; {}",
        vv.passed, vv.errors, vv.warnings, vv.checked_line
    );
    for g in vv.groups.iter().filter(|g| !g.findings.is_empty()).take(4) {
        let f = &g.findings[0];
        println!(
            "  {} ({}e/{}w) e.g. {} {} @{:?}: {}",
            g.rule, g.errors, g.warnings, f.record, f.subrecord, f.offset, f.message
        );
    }
    let rt = RoundtripView::from_envelope(&run(&mut worker, vf.roundtrip_request().unwrap()));
    println!(
        "  roundtrip passed {:?}; {}; mismatch {:?}",
        rt.passed, rt.counts, rt.mismatch
    );

    println!("\n== Schema: WEAP");
    let sv = SchemaView::from_envelope(&run(
        &mut worker,
        SchemaForm {
            record_type: "WEAP".into(),
            ..SchemaForm::default()
        }
        .request(),
    ));
    println!(
        "  {} [{}] {}; {} rows",
        sv.name,
        sv.confidence,
        sv.confidence_counts,
        sv.members.len()
    );
    println!(
        "  unconfirmed rows: {}",
        sv.members.iter().filter(|m| m.unconfirmed()).count()
    );

    println!("\n== Refused");
    for (id, args) in [
        (
            "set",
            json!({"form": form, "fields": {"FULL": "x"}, "into": "x.esp"}),
        ),
        (
            "roundtrip",
            json!({"plugin": last, "verify": true, "out": "x.esm"}),
        ),
    ] {
        let err = worker.submit(Request::new(id, args)).unwrap_err();
        println!("  {id}: {err}");
    }
}
