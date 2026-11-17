//! The quest rules over a `QUST`'s `VMAD` fragment block: `quest.fragment-stage` (a stage
//! fragment must name a stage the quest has and a log entry that stage has) and
//! `quest.alias-undeclared` (an alias script entry must name an alias the quest declares).
//!
//! What the official files show (`tests/quest_vmad_corpus.rs` and the 2026-11-17 scan in
//! `docs/validate.md`): of 13,947 stage fragments, 13,813 name an existing stage's existing
//! log entry and 134 a stage the quest does not have; none names a stage without log
//! entries, though 148 stages have none. A fragment is bound to a log entry, so one on a
//! stage without that entry is not run (the bench quest's stage 10 fragment, which had no
//! log entry, wrote nothing in game: `docs/bench/verify-mod.md`, row Q-03). Of 4,404 alias
//! entries, 4 name an alias the quest does not declare (SFBGS050.esm), hence a warning.

use kitkraft_esm::Subrecord;

use super::{Finding, Severity};
use crate::edit::vmad::{quest_aliases, QuestFragments};

/// A quest's stages in record order: the stage index (`INDX`) and how many log entries
/// (`QSDT`, which opens each `LogEntry`) follow it before the next stage, objective
/// (`QOBJ`) or the aliases (`ANAM`).
pub fn stages(subs: &[Subrecord<'_>]) -> Vec<(u16, u32)> {
    let mut out: Vec<(u16, u32)> = Vec::new();
    let mut in_stage = false;
    for s in subs {
        match &s.signature().0 {
            b"INDX" => {
                let stage = s
                    .data()
                    .get(..2)
                    .and_then(|b| b.try_into().ok())
                    .map(u16::from_le_bytes);
                if let Some(stage) = stage {
                    out.push((stage, 0));
                    in_stage = true;
                } else {
                    in_stage = false;
                }
            }
            b"QSDT" if in_stage => {
                if let Some(last) = out.last_mut() {
                    last.1 = last.1.saturating_add(1);
                }
            }
            b"QOBJ" | b"ANAM" => in_stage = false,
            _ => {}
        }
    }
    out
}

/// Why the fragment at `(stage, index)` has nowhere to run, or `None` when its log entry
/// exists.
pub fn fragment_problem(stages: &[(u16, u32)], stage: u32, index: u32) -> Option<String> {
    match stages.iter().find(|(s, _)| u32::from(*s) == stage) {
        None => Some(format!("the quest has no stage {stage}")),
        Some((_, n)) if index < *n => None,
        Some((_, 0)) => Some(format!(
            "stage {stage} has no log entry (a fragment runs from a log entry; give the stage one, QSDT)"
        )),
        Some((_, n)) => Some(format!(
            "stage {stage} has {n} log entr{} (0 to {}), not {index}",
            if *n == 1 { "y" } else { "ies" },
            n.saturating_sub(1)
        )),
    }
}

/// The findings for one quest: `block` is its decoded fragment block, `earlier` the
/// previous version in load order's block (its plugin name), whose fragments an override
/// carries as warnings.
#[allow(clippy::too_many_arguments)]
pub fn check(
    record: &str,
    plugin: &str,
    subs: &[Subrecord<'_>],
    offset: Option<u64>,
    block: &QuestFragments,
    earlier: Option<(&str, &QuestFragments)>,
    stage_on: bool,
    alias_on: bool,
    findings: &mut Vec<Finding>,
) {
    let finding = |rule: &'static str, severity: Severity, message: String| Finding {
        rule,
        severity,
        plugin: plugin.to_string(),
        record: Some(record.to_string()),
        subrecord: Some("VMAD".to_string()),
        offset,
        message,
    };
    if stage_on {
        let stages = stages(subs);
        for f in &block.fragments {
            let Some(why) = fragment_problem(&stages, f.stage, f.stage_index) else {
                continue;
            };
            let function = String::from_utf8_lossy(&f.fragment_name);
            let carried = earlier.and_then(|(p, q)| {
                q.fragments
                    .iter()
                    .any(|e| (e.stage, e.stage_index) == (f.stage, f.stage_index))
                    .then_some(p)
            });
            let (severity, tail) = match carried {
                None => (Severity::Error, String::new()),
                Some(p) => (
                    Severity::Warning,
                    format!("; carried from {p}'s version of the record, so a warning"),
                ),
            };
            findings.push(finding(
                "quest.fragment-stage",
                severity,
                format!(
                    "stage fragment `{function}` (stage {}, log entry {}): {why}; the game does not run it{tail}",
                    f.stage, f.stage_index
                ),
            ));
        }
    }
    if alias_on && !block.aliases.is_empty() {
        let declared = quest_aliases(subs);
        for e in &block.aliases {
            if declared
                .iter()
                .any(|d| i64::from(d.id) == i64::from(e.object.alias))
            {
                continue;
            }
            let names: Vec<String> = e.scripts.iter().map(|s| s.name_text()).collect();
            findings.push(finding(
                "quest.alias-undeclared",
                Severity::Warning,
                format!(
                    "alias scripts ({}) are attached to alias {}, which the quest does not declare (4 official entries in SFBGS050.esm do the same; what the game does with them is not observed)",
                    names.join(", "),
                    e.object.alias
                ),
            ));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use kitkraft_esm::Signature;

    fn sub(sig: &[u8; 4], data: &[u8]) -> Subrecord<'static> {
        Subrecord::new(Signature(*sig), data.to_vec())
    }

    #[test]
    fn stages_count_their_log_entries() {
        let subs = [
            sub(b"EDID", b"Q\0"),
            sub(b"INDX", &[10, 0, 0, 0]),
            sub(b"INDX", &[20, 0, 0, 0]),
            sub(b"QSDT", &[0]),
            sub(b"CNAM", b"a\0"),
            sub(b"QSDT", &[0]),
            sub(b"QOBJ", &[0, 0]),
            sub(b"QSDT", &[0]),
            sub(b"ANAM", &[0, 0, 0, 0]),
        ];
        let st = stages(&subs);
        assert_eq!(st, [(10, 0), (20, 2)]);
        assert!(fragment_problem(&st, 20, 1).is_none());
        assert!(fragment_problem(&st, 10, 0)
            .unwrap()
            .contains("stage 10 has no log entry"));
        assert!(fragment_problem(&st, 20, 2)
            .unwrap()
            .contains("2 log entries (0 to 1), not 2"));
        assert!(fragment_problem(&st, 30, 0)
            .unwrap()
            .contains("no stage 30"));
    }

    #[test]
    fn fragments_and_alias_entries_are_checked() {
        use crate::edit::vmad::{AliasScripts, Object, QuestFragment, Script};
        let subs = [
            sub(b"INDX", &[10, 0, 0, 0]),
            sub(b"QSDT", &[0]),
            sub(b"ANAM", &[1, 0, 0, 0]),
            sub(b"ALST", &[0, 0, 0, 0]),
            sub(b"ALID", b"A\0"),
            sub(b"ALED", b""),
        ];
        let fragment = |stage: u32| QuestFragment {
            stage,
            stage_index: 0,
            unknown: 1,
            script_name: b"QF".to_vec(),
            fragment_name: b"F".to_vec(),
        };
        let entry = |alias: i16| AliasScripts {
            object: Object {
                form_id: 0x800,
                alias,
                unused: 0,
            },
            version: 6,
            object_format: 2,
            scripts: vec![Script {
                name: b"KK:A".to_vec(),
                flags: 0,
                properties: Vec::new(),
            }],
        };
        let block = QuestFragments {
            fragments: vec![fragment(10), fragment(20), fragment(30)],
            aliases: vec![entry(0), entry(5)],
            ..QuestFragments::new()
        };
        // The master already has the stage 30 fragment: carried, so a warning.
        let earlier = QuestFragments {
            fragments: vec![fragment(30)],
            ..QuestFragments::new()
        };
        let mut findings = Vec::new();
        check(
            "QUST:00000800",
            "P.esp",
            &subs,
            Some(4),
            &block,
            Some(("M.esm", &earlier)),
            true,
            true,
            &mut findings,
        );
        let got: Vec<(&str, Severity)> = findings.iter().map(|f| (f.rule, f.severity)).collect();
        assert_eq!(
            got,
            [
                ("quest.fragment-stage", Severity::Error),
                ("quest.fragment-stage", Severity::Warning),
                ("quest.alias-undeclared", Severity::Warning),
            ]
        );
        assert!(findings[1].message.contains("carried from M.esm"));
        assert!(findings[2].message.contains("KK:A") && findings[2].message.contains("alias 5"));
    }
}
