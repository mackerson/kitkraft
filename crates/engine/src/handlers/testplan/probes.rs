//! `probes.toml` (lane C, M4.T2): the checklist's steps as machine-checkable probes for
//! `kit game session` (ADR 0010 Decision 3). One probe per checklist step, in the same
//! order and with the step's number as its row (`T-<nn>`), so a ledger row, a checklist
//! step and a probe name the same thing.
//!
//! Expected values come from the plugin's own records: the reference's EditorID and
//! runtime ID (`help <EditorID> 4` lists it), its `DATA` position (`getpos x|y|z`, within
//! [`POSITION_TOLERANCE`]), its initially-disabled flag (`isdisabled`), the base's EditorID
//! (`getisid`), an item's runtime ID (`player.getitemcount` after `player.additem`), an
//! armor's equipped state (`player.getequipped` after `player.equipitem`), a form's inline
//! `FULL` (`help "<FULL>" 4`), a quest's stage (`getstage`). The persistence
//! protocol (docs/game-observations.md) becomes `coc`, `save {save}`, `load {save}` and the
//! return checks; the controller fills `{save}` with its own `KitKraftSession-*` name. A step
//! with no console getter (an exterior cell, clicking an object without an EditorID,
//! a form with neither EditorID nor text) is `matcher = "manual"` with the reason.
//!
//! What the console prints for each command is grade D until M4.T1 observes it (the
//! notes say so); a matcher that sees no output, or no number, reports `inconclusive`.

use kitkraft_game::probes::{Matcher, Probe, ProbeFile, Step, FORMAT};

use super::{hex, Form, IdBasis, DEV_PROFILE, ITEMS};

#[cfg(test)]
mod tests;

/// The file name beside `checklist.md`.
pub(super) const FILE: &str = "probes.toml";

/// Record flag of a reference placed initially disabled.
pub(super) const FLAG_INITIALLY_DISABLED: u32 = 0x0000_0800;

/// Largest difference accepted between a `getpos` answer and the plugin's position.
pub(super) const POSITION_TOLERANCE: f64 = 0.5;

/// The cell the persistence protocol leaves to (docs/game-observations.md, step 1).
pub(super) const LEAVE_CELL: &str = "CityNewAtlantisLodgeInt";

/// A console answer of 1 (`GetInCell >> 1.00`): grade D until M4.T1.
const ONE: &str = r">>\s*1(\.0+)?\s*$";

/// A count of at least one.
const AT_LEAST_ONE: &str = r">>\s*[1-9]";

/// Any number after `>>`.
const ANY_NUMBER: &str = r">>\s*-?\d";

/// A `FULL` payload as text when it is inline UTF-8 that can sit inside a console quote.
pub(super) fn inline_text(data: &[u8]) -> Option<String> {
    let bytes = data.split(|b| *b == 0).next()?;
    let text = std::str::from_utf8(bytes).ok()?;
    let ok = !text.is_empty()
        && text.chars().count() <= 200
        && !text.chars().any(|c| c == '"' || c.is_control());
    ok.then(|| text.to_string())
}

/// A console-safe token (EditorIDs and hex IDs are; anything else is quoted or refused).
fn token(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn set(command: String) -> Step {
    Step {
        command,
        matcher: None,
        expected: None,
        tolerance: None,
        why: None,
    }
}

fn check(command: String, matcher: Matcher, expected: String, why: &str) -> Step {
    Step {
        command,
        matcher: Some(matcher),
        expected: Some(expected),
        tolerance: None,
        why: Some(why.to_string()),
    }
}

fn number(command: String, expected: f32, why: &str) -> Step {
    Step {
        command,
        matcher: Some(Matcher::Number),
        expected: Some(format!("{expected}")),
        tolerance: Some(POSITION_TOLERANCE),
        why: Some(why.to_string()),
    }
}

fn manual(row: String, title: String, forms: Vec<String>, reason: &str) -> Probe {
    Probe {
        row,
        title,
        forms,
        screenshot: false,
        matcher: Some("manual".into()),
        reason: Some(reason.to_string()),
        steps: Vec::new(),
    }
}

fn probe(
    row: String,
    title: String,
    forms: Vec<String>,
    screenshot: bool,
    steps: Vec<Step>,
) -> Probe {
    Probe {
        row,
        title,
        forms,
        screenshot,
        matcher: None,
        reason: None,
        steps,
    }
}

fn is_ref(f: &Form) -> bool {
    f.signature == "REFR" || f.signature == "ACHR"
}

/// The `prid` target of a reference: its EditorID, else its runtime ID.
fn target(f: &Form) -> Option<String> {
    match (&f.editor_id, f.runtime) {
        (Some(e), _) if token(e) => Some(e.clone()),
        (_, Some(r)) => Some(hex(r)),
        _ => None,
    }
}

fn getpos(t: &str, p: [f32; 3]) -> Vec<Step> {
    let why = "the reference's DATA position in the plugin";
    vec![
        set(format!("prid {t}")),
        number("getpos x".into(), p[0], why),
        number("getpos y".into(), p[1], why),
        number("getpos z".into(), p[2], why),
    ]
}

/// The probes for `forms`, numbered as the checklist numbers its steps.
pub(super) fn build(plugin: &str, sha: &str, forms: &[Form], id_basis: &IdBasis) -> ProbeFile {
    let mut probes = Vec::new();
    let mut n: u32 = 1;
    let next = |n: &mut u32| {
        let row = format!("T-{:02}", *n);
        *n = n.saturating_add(1);
        row
    };
    for f in forms {
        let label = if f.new {
            format!("{plugin}:{}", hex(f.stored).get(2..).unwrap_or_default())
        } else {
            format!("{} {} as stored in {plugin}", f.signature, hex(f.stored))
        };
        let mut ids = vec![label.clone()];
        if let Some(e) = &f.editor_id {
            ids.push(e.clone());
        }
        if let Some(r) = f.runtime {
            ids.push(hex(r));
        }
        let what = format!(
            "{} `{}`",
            f.signature,
            f.editor_id.as_deref().unwrap_or(&label)
        );
        if is_ref(f) {
            if let Some(cell) = &f.cell {
                let row = next(&mut n);
                match (&cell.editor_id, cell.interior) {
                    (Some(c), true) if token(c) => probes.push(probe(
                        row,
                        format!("coc {c}: the cell of {what}"),
                        vec![cell.label.clone(), c.clone()],
                        false,
                        vec![
                            set(format!("coc {c}")),
                            check(
                                format!("player.getincell {c}"),
                                Matcher::Regex,
                                ONE.into(),
                                "the player is in the reference's cell (answer format grade D)",
                            ),
                        ],
                    )),
                    _ => probes.push(manual(
                        row,
                        format!("go to cell {} (exterior)", cell.label),
                        vec![cell.label.clone()],
                        "an exterior cell has no `coc` target here; the next row's player.moveto takes the player there",
                    )),
                }
            }
            let t = target(f);
            if f.new {
                let row = next(&mut n);
                match &t {
                    Some(t) => probes.push(probe(
                        row,
                        format!("player.moveto {t}: stand at {what}"),
                        ids.clone(),
                        false,
                        vec![
                            set(format!("player.moveto {t}")),
                            set(format!("prid {t}")),
                            Step {
                                command: "getdistance player".into(),
                                matcher: Some(Matcher::Number),
                                expected: Some("0".into()),
                                tolerance: Some(500.0),
                                why: Some(
                                    "player.moveto puts the player beside the reference".into(),
                                ),
                            },
                        ],
                    )),
                    None => probes.push(manual(
                        row,
                        format!("stand at {what}"),
                        ids.clone(),
                        "the reference has neither an EditorID nor a runtime ID",
                    )),
                }
            }
            // The checklist's "click the object" step: its reference ID.
            let row = next(&mut n);
            match (&f.editor_id, f.runtime, &f.base_edid) {
                (Some(e), Some(r), _) if token(e) => probes.push(probe(
                    row,
                    format!("help {e} 4 names {what} with its runtime ID ({})", id_basis.short),
                    ids.clone(),
                    false,
                    vec![check(
                        format!("help {e} 4"),
                        Matcher::Contains,
                        hex(r),
                        "the runtime ID the load order gives (confirmed or predicted, see runtime_id_basis)",
                    )],
                )),
                (_, Some(r), Some(b)) if token(b) => probes.push(probe(
                    row,
                    format!("prid {} selects a {b}", hex(r)),
                    ids.clone(),
                    false,
                    vec![
                        set(format!("prid {}", hex(r))),
                        check(format!("getisid {b}"), Matcher::Regex, ONE.into(), "the reference's base in the plugin"),
                    ],
                )),
                _ => probes.push(manual(
                    row,
                    format!("click {what} in the console and read its reference ID"),
                    ids.clone(),
                    "clicking needs input injection (deferred, ADR 0010 Amendment 1 (b)) and the reference has no EditorID or base EditorID to check by",
                )),
            }
            // prid, getpos, isdisabled; screenshot.
            let row = next(&mut n);
            match (&t, f.position) {
                (Some(t), Some(p)) => {
                    let mut steps = getpos(t, p);
                    steps.push(check(
                        "isdisabled".into(),
                        Matcher::Regex,
                        if f.disabled {
                            ONE.into()
                        } else {
                            r">>\s*0(\.0+)?\s*$".into()
                        },
                        "the record's initially-disabled flag",
                    ));
                    if let (Some(e), Some(r)) = (&f.editor_id, f.runtime) {
                        if token(e) && *t == *e {
                            steps.push(set(format!("prid {}", hex(r))));
                            steps.push(number(
                                "getpos x".into(),
                                p[0],
                                "the numeric ID selects the same reference",
                            ));
                        }
                    }
                    probes.push(probe(
                        row,
                        format!(
                            "prid {t}; getpos x/y/z: {what} at ({:.1}, {:.1}, {:.1})",
                            p[0], p[1], p[2]
                        ),
                        ids.clone(),
                        true,
                        steps,
                    ));
                }
                _ => probes.push(manual(
                    row,
                    format!("prid and getpos on {what}"),
                    ids.clone(),
                    "no position or no console target for the reference",
                )),
            }
        } else if (f.new && ITEMS.iter().any(|s| s.as_slice() == f.signature.as_bytes()))
            || f.signature == "ARMO"
        {
            let row = next(&mut n);
            match f.runtime {
                Some(r) => probes.push(probe(
                    row,
                    format!("player.additem {} 1: {what} in the inventory", hex(r)),
                    ids.clone(),
                    false,
                    vec![
                        set(format!("player.additem {} 1", hex(r))),
                        check(
                            format!("player.getitemcount {}", hex(r)),
                            Matcher::Regex,
                            AT_LEAST_ONE.into(),
                            "the item was added",
                        ),
                    ],
                )),
                None => probes.push(manual(
                    row,
                    format!("add {what}"),
                    ids.clone(),
                    "the form has no runtime ID",
                )),
            }
            // Armor is an outfit: it must be worn and looked at, not only owned (new or
            // overridden).
            if f.signature == "ARMO" {
                let row = next(&mut n);
                match f.runtime {
                    Some(r) => probes.push(probe(
                        row,
                        format!("player.equipitem {}: {what} is worn", hex(r)),
                        ids.clone(),
                        false,
                        vec![
                            set(format!("player.equipitem {}", hex(r))),
                            check(
                                format!("player.getequipped {}", hex(r)),
                                Matcher::Regex,
                                ONE.into(),
                                "the armor is equipped (answer format grade D)",
                            ),
                        ],
                    )),
                    None => probes.push(manual(
                        row,
                        format!("equip {what}"),
                        ids.clone(),
                        "the form has no runtime ID",
                    )),
                }
                let row = next(&mut n);
                probes.push(manual(
                    row,
                    format!(
                        "look at {what}: third person, `tfc` free camera, a screenshot into `observe\\`"
                    ),
                    ids.clone(),
                    "the outfit's look needs a screenshot and a person",
                ));
            }
        } else {
            let row = next(&mut n);
            let edid = f.editor_id.as_deref().filter(|e| token(e));
            let command = match (&f.full, edid) {
                (Some(full), _) => Some(format!("help \"{full}\" 4")),
                (None, Some(e)) => Some(format!("help {e} 4")),
                (None, None) => None,
            };
            let expected = f.runtime.map(hex).or_else(|| edid.map(str::to_string));
            match (command, expected) {
                (Some(c), Some(x)) => {
                    let mut steps = vec![check(
                        c.clone(),
                        Matcher::Contains,
                        x,
                        if f.full.is_some() {
                            "the plugin's FULL finds the form (its runtime ID in the answer)"
                        } else {
                            "the EditorID finds the form (its runtime ID in the answer)"
                        },
                    )];
                    if let (Some(e), "QUST") = (edid, f.signature.as_str()) {
                        steps.push(check(
                            format!("getstage {e}"),
                            Matcher::Regex,
                            ANY_NUMBER.into(),
                            "the quest exists and reports a stage",
                        ));
                    }
                    probes.push(probe(
                        row,
                        format!("{c}: {what}"),
                        ids.clone(),
                        false,
                        steps,
                    ));
                }
                _ => probes.push(manual(
                    row,
                    format!("look for {what}"),
                    ids.clone(),
                    "the form has no EditorID, no inline FULL and no runtime ID to search by",
                )),
            }
        }
    }
    // Persistence (docs/game-observations.md, "Persistence protocol", steps 1 to 4).
    let row = next(&mut n);
    probes.push(probe(
        row,
        "persistence 1-3: leave, save to the session's own save, load it".into(),
        Vec::new(),
        false,
        vec![
            set(format!("coc {LEAVE_CELL}")),
            set("save {save}".into()),
            set("load {save}".into()),
            check(
                format!("player.getincell {LEAVE_CELL}"),
                Matcher::Regex,
                ONE.into(),
                "the save loaded where it was made (the one-hour wait of the protocol is not sent: no console command chosen, grade D)",
            ),
        ],
    ));
    let row = next(&mut n);
    let mut back = Vec::new();
    let mut back_ids = Vec::new();
    for f in forms.iter().filter(|f| is_ref(f) && f.new) {
        let (Some(t), Some(p)) = (target(f), f.position) else {
            continue;
        };
        if let Some(c) = f
            .cell
            .as_ref()
            .filter(|c| c.interior)
            .and_then(|c| c.editor_id.clone())
            .filter(|c| token(c))
        {
            back.push(set(format!("coc {c}")));
        }
        back.extend(getpos(&t, p));
        back_ids.push(t);
    }
    if back.is_empty() {
        probes.push(manual(
            row,
            "persistence 4: return and look".into(),
            Vec::new(),
            "the plugin places no new reference with a position to check after the reload",
        ));
    } else {
        probes.push(probe(
            row,
            "persistence 4: return; every new reference where it was".into(),
            back_ids,
            true,
            back,
        ));
    }
    ProbeFile {
        format: FORMAT,
        plugin: plugin.to_string(),
        sha256: sha.to_string(),
        generated_by: "kit testplan (M4.T2)".into(),
        profile: DEV_PROFILE.into(),
        runtime_id_basis: id_basis.short.clone(),
        notes: vec![
            "Run by `kit game session` on the dev profile only, from the base save; `{save}` is the session's own KitKraftSession-* save.".into(),
            "What the console prints for each command is grade D until M4.T1 observes it; a matcher with no output or no number is inconclusive, never pass.".into(),
            format!("Positions match within {POSITION_TOLERANCE} units; numeric runtime IDs are {}.", id_basis.text),
        ],
        probes,
    }
}
