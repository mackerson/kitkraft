//! `overhaul.scaffold` (M3.E7): render a starter `build` spec v2 for one record type.
//!
//! The load-order half (matching, grouping, statistics) is `handlers/overhaul.rs`; this
//! module turns its result into TOML text: a header comment, `[plugin]`, the
//! `[load_order]` the scaffold was made from, one `[class.<name>]` per group with its match
//! count and the min/median/max of each typed numeric member as comments, and one
//! commented-out `[[rule]]` per class. Every rule is commented out, so `build --plan` on a
//! fresh scaffold reports 0 changes. The output is deterministic: no timestamps, groups in
//! a fixed order.

use kitkraft_schema::Confidence;

use super::members::Numeric;

/// Statistics of one typed numeric member over a group's records.
#[derive(Debug, Clone, PartialEq)]
pub struct Stat {
    /// The member.
    pub member: Numeric,
    /// Every value (one per occurrence or element).
    pub values: Vec<f64>,
    /// Records whose value could not be decoded.
    pub unknown: usize,
    /// Records that have the member.
    pub records: usize,
}

/// One class of the scaffold.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// The class name (a TOML bare key).
    pub name: String,
    /// The class's `where` (`None`: every winning record of the type).
    pub where_text: Option<String>,
    /// What the group stands for (`KWDA == Starfield.esm:0012AB (WeaponTypeBallistic)`).
    pub label: String,
    /// Winning records in the group.
    pub count: usize,
    /// Per numeric member.
    pub stats: Vec<Stat>,
}

/// Everything the renderer needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Scaffold {
    /// The record type (`WEAP`).
    pub record_type: String,
    /// The base `where`, if any.
    pub base_where: Option<String>,
    /// The `group_by` path, if any.
    pub group_by: Option<String>,
    /// The `[load_order]` keys, rendered in order (`profile`, `plugins`, `data_dir`, ...).
    pub load_order: Vec<(String, toml::Value)>,
    /// Winning records the base `where` matched.
    pub total: usize,
    /// Records the base `where` could not decide.
    pub undecided: u64,
    /// Records with no value for `group_by` (in no class).
    pub ungrouped: usize,
    /// Records whose `group_by` value could not be decoded (in no class).
    pub group_unknown: usize,
    /// Values that could not be written as a `where` literal (in no class).
    pub unexpressible: usize,
    /// The classes, largest first.
    pub groups: Vec<Group>,
    /// Groups left out past `max_groups`: (label, count).
    pub omitted: Vec<(String, usize)>,
    /// The type's typed numeric members.
    pub numeric: Vec<Numeric>,
}

/// A number for a comment: the shortest form of the `f32` it came from when it is one.
pub fn short(v: f64) -> String {
    let narrow = v as f32;
    if f64::from(narrow) == v {
        format!("{narrow}")
    } else {
        format!("{v}")
    }
}

/// Min, median (the mean of the two middle values for an even count) and max.
pub fn summary(values: &[f64]) -> Option<(f64, f64, f64)> {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let min = *sorted.first()?;
    let max = *sorted.last()?;
    let mid = n / 2;
    let median = if n % 2 == 1 {
        *sorted.get(mid)?
    } else {
        let lo = *sorted.get(mid.checked_sub(1)?)?;
        let hi = *sorted.get(mid)?;
        lo / 2.0 + hi / 2.0
    };
    Some((min, median, max))
}

/// A TOML basic string.
fn quoted(text: &str) -> String {
    toml::Value::String(text.to_string()).to_string()
}

/// A comment line (or several), each prefixed `# `.
fn comment(out: &mut String, text: &str) {
    for line in text.lines() {
        if line.is_empty() {
            out.push_str("#\n");
        } else {
            out.push_str("# ");
            out.push_str(line);
            out.push('\n');
        }
    }
}

fn member_line(m: &Numeric) -> String {
    let mut s = format!("{} ({}, {}", m.path, m.kind, m.confidence);
    if m.confidence == Confidence::Reference {
        s.push_str(": writing it needs allow_reference = true");
    }
    if m.in_array {
        s.push_str("; array elements: a rule names one by index");
    }
    s.push(')');
    s
}

/// The member a commented-out rule suggests: the first numeric one that is not an array
/// element or a count field.
fn suggestion(numeric: &[Numeric]) -> Option<&Numeric> {
    numeric
        .iter()
        .find(|m| !m.in_array && !m.label.ends_with(".Count") && m.kind != "union")
        .or_else(|| {
            numeric
                .iter()
                .find(|m| !m.in_array && !m.label.ends_with(".Count"))
        })
}

/// The scaffold as TOML text.
pub fn render(s: &Scaffold) -> String {
    let mut out = String::new();
    let mut head = format!(
        "kit overhaul scaffold (M3.E7): a starter build spec v2 for {}; every rule is\n\
         commented out, so `kit build <this file> <out> --plan` reports 0 changes.\n\n",
        s.record_type
    );
    head.push_str(&format!(
        "where: {}\n",
        s.base_where
            .as_deref()
            .unwrap_or("(none: every winning record)")
    ));
    head.push_str(&format!(
        "group_by: {}\n",
        s.group_by.as_deref().unwrap_or("(none: one class)")
    ));
    head.push_str(&format!(
        "{} winning record(s) matched; {} class(es) below",
        s.total,
        s.groups.len()
    ));
    if s.undecided > 0 {
        head.push_str(&format!(
            "; {} record(s) the where could not decide",
            s.undecided
        ));
    }
    head.push('\n');
    if s.group_by.is_some() {
        head.push_str(&format!(
            "in no class: {} without a value, {} undecodable, {} not expressible as a where literal\n",
            s.ungrouped, s.group_unknown, s.unexpressible
        ));
        head.push_str(
            "a record with several values (an array such as KWDA) is in one class per value\n",
        );
    }
    head.push('\n');
    if s.numeric.is_empty() {
        head.push_str(&format!(
            "{} has no typed numeric member in the schema yet (lane B round 3 types the\n\
             weapon records); rules can still set typed text and FormID members.\n",
            s.record_type
        ));
    } else {
        head.push_str(&format!("Typed numeric members of {}:\n", s.record_type));
        for m in &s.numeric {
            head.push_str(&format!("  {}\n", member_line(m)));
        }
    }
    head.push_str(
        "\nBefore building: fill each class's and each rule's `brief` with its row in\n\
         docs/overhaul/brief.md (the traceability rule), then uncomment and edit the rules.\n\
         Operators: a value, { mul }, { add }, { min } (at least), { max } (at most),\n\
         { clamp = [lo, hi] }; via = [\"SIG.Member\"] follows typed FormID members.\n",
    );
    comment(&mut out, &head);
    out.push_str("\nversion = 2\n\n[plugin]\nlocalized = false\nstyle = \"kit\"\n");
    if !s.load_order.is_empty() {
        out.push_str("\n[load_order]\n");
        for (k, v) in &s.load_order {
            out.push_str(&format!("{k} = {v}\n"));
        }
    }
    for g in &s.groups {
        out.push_str(&format!(
            "\n[class.{}]\ntype = {}\n",
            g.name,
            quoted(&s.record_type)
        ));
        if let Some(w) = &g.where_text {
            out.push_str(&format!("where = {}\n", quoted(w)));
        }
        let mut notes = String::from("brief = \"B?\"   (its row in docs/overhaul/brief.md)\n");
        notes.push_str(&format!("matched: {} record(s): {}\n", g.count, g.label));
        for st in &g.stats {
            match summary(&st.values) {
                Some((min, median, max)) => notes.push_str(&format!(
                    "{}: min {}, median {}, max {} ({} record(s){})\n",
                    st.member.path,
                    short(min),
                    short(median),
                    short(max),
                    st.records,
                    if st.unknown > 0 {
                        format!(", {} undecodable", st.unknown)
                    } else {
                        String::new()
                    }
                )),
                None if st.unknown > 0 => notes.push_str(&format!(
                    "{}: {} record(s) undecodable\n",
                    st.member.path, st.unknown
                )),
                None => notes.push_str(&format!("{}: no values\n", st.member.path)),
            }
        }
        comment(&mut out, &notes);
        let example = match suggestion(&s.numeric) {
            Some(m) => format!("{} = {{ mul = 1.0 }}", quoted(&m.path)),
            None => "\"SIG.Member\" = { mul = 1.0 }".to_string(),
        };
        out.push('\n');
        comment(
            &mut out,
            &format!(
                "[[rule]]\nclass = {}\nbrief = \"B?\"\nfields = {{ {example} }}",
                quoted(&g.name)
            ),
        );
    }
    if !s.omitted.is_empty() {
        let mut rest = format!(
            "\n{} more group(s) left out (raise max_groups to list them):\n",
            s.omitted.len()
        );
        for (label, count) in &s.omitted {
            rest.push_str(&format!("  {count:>6}  {label}\n"));
        }
        out.push('\n');
        comment(&mut out, rest.trim_start_matches('\n'));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_and_short_numbers() {
        assert_eq!(summary(&[3.0, 1.0, 2.0]), Some((1.0, 2.0, 3.0)));
        assert_eq!(summary(&[4.0, 1.0, 2.0, 3.0]), Some((1.0, 2.5, 4.0)));
        assert_eq!(summary(&[]), None);
        assert_eq!(short(f64::from(0.1f32)), "0.1");
        assert_eq!(short(0.1), "0.1");
        assert_eq!(short(7.0), "7");
    }
}
