//! The editor (pipeline E; ADR 0007 Amendment 4): the M7 writes into the active file
//! only (`set` M7.E2, `revert` M7.E6, `build` M7.E5, `new`/`clone` M7.E3, `place`
//! M7.E4), each previewed, validated and confirmed per OK click through the gate.
//!
//! Step 0 of sprint 2026-11-02 wired this file into the shared seams: its state in
//! `Shell` (`editor`), the per-frame call to [`ui`] in `shell.rs`, its tickets
//! (`Pending::Editor`, forwarded to [`EditorState::receive`]), its slots
//! (`Slot::EditorPreview`, `EditorWrite`, `EditorValidate`, routed to
//! [`EditorState::receive_slot`] in `app.rs`), and the actions `OpenEditor` and
//! `PlaceInCell` (applied through [`EditorState::open`] and [`EditorState::place`]).
//!
//! M7.E2 (`set`): the edit dialog's OK asks for `OpenEditor { command: "set", form }`.
//! The next frame the editor takes the dialog's changed fields of `form`, refuses
//! without an active file, checks the preview against the gate and sends `set
//! --dry-run` into the active file (`Slot::EditorPreview`). Its answer opens the confirm
//! box: the paths changed with before and after, `validate` on the staged result, and
//! the dry run's limitations. The box's one confirm button is enabled only when the
//! dry run says it would write and the active file is still the preview's output; one
//! click sends the confirmed `set` (`Slot::EditorWrite`, built in `dialogs/set.rs`),
//! then the written record is read back from the active file alone. One write per OK.
//!
//! `set` starts from the load order's winner, and the active file is not in the load
//! order (the engine refuses a load-order plugin as output), so a second edit of a record
//! the active file already holds would drop the first (review finding H1). When the dry
//! run says it would replace a record in the active file, the editor reads the active
//! file's version (`inspect`, single-plugin, every byte), carries each subrecord that
//! differs from the winner as `hex:` at its `set` path, puts the dialog's edits over
//! them, and previews again: the confirm box shows the whole difference from the winner
//! that will be written, with the kept paths named.
//!
//! M7.E6 (`revert`): the edit dialog's "Undo last write" asks for
//! `OpenEditor { command: "revert", form }`. The editor undoes only its own last
//! confirmed write, and only when that write added the record to the active file (the
//! write's envelope says `inserted`): then `revert --dry-run` with `to: remove` previews
//! dropping that override (the diff of the file and `validate`), and the box's one
//! "Undo in <file>" click sends the confirmed `revert` (built in `dialogs/revert.rs`).
//! A write that replaced a version the file already held cannot be undone this way:
//! `revert` would drop the earlier version too, so the undo is refused with the reason.
//! One undo per write: the last write is forgotten once it is undone.
//!
//! M7.E5 (`build`): `OpenEditor { command: "build", .. }` opens the Build Spec form
//! (`dialogs/build.rs` draws it): the spec file and the active file it builds into.
//! "Plan" sends `build --plan` (every change, the new forms, validate; nothing written);
//! the confirm box shows the plan, and its one "Build into <file>" click sends the
//! confirmed `build`, then reads the first written record back from the active file.
//! `build` writes a fresh plugin and the window never sends `overwrite`, so the form
//! refuses an active file that already exists (the engine refuses it too).
//!
//! M7.E3 (`new`, `clone`): `OpenEditor { command: "new", form: <type> }` (the Object
//! Window's New) and `OpenEditor { command: "clone", form: <record> }` (Edit > Duplicate)
//! open the New Form form (`dialogs/new.rs` draws it): the record type or the record
//! copied, and the new EditorID. "Preview" sends `new --dry-run` or `clone --dry-run`
//! into the active file; the confirm box's one "Write to <file>" click sends the
//! confirmed write (built in `dialogs/new.rs` or `dialogs/clone.rs`), then reads the new
//! form back from the active file. A new form is not the undo's to drop (`revert` refuses
//! a form the plugin defines), so it is not offered there.
//!
//! M7.E4 (`place`): `Action::PlaceInCell { base, cell }` (the Cell View's place bar,
//! `dialogs/place.rs` `cell_view_bar`) opens the Place form: the base, the cell and the
//! typed position. "Preview" sends `place --dry-run` into the active file (the reference
//! and the cell it carries, validate); the confirm box's one click sends the confirmed
//! `place`, then reads the new reference back from the active file. Its undo is refused
//! with the reason: the reference is a new form the active file defines, which `revert`
//! does not drop, and a first placement in a cell also copies the cell (the E2 review's
//! parent rule).

use std::collections::BTreeMap;

use eframe::egui::{self, ScrollArea};
use serde_json::{Map, Value};

use super::{build, clone, edit, new, place, revert, set};
use crate::gate::{self, Request};
use crate::panels::common::{Action, Slot};
use crate::panels::fmt_value;
use crate::theme::{self, Status};

/// What an editor request sent through the shell's tickets was for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorTicket {
    /// A write's preview (dry run).
    Preview,
    /// A write's confirmed form.
    Write,
    /// The `validate` of a preview.
    Validate,
}

/// One write in progress: from its preview to its read-back.
#[derive(Debug, Clone, PartialEq)]
struct Write {
    /// The catalog command: `set`, or `revert` (the undo).
    command: &'static str,
    /// The record (formref).
    form: String,
    /// The output: the active file when the preview was sent.
    into: String,
    /// The preview request as sent; its reply is matched against it.
    preview: Request,
    /// The dialog's changed fields (field path -> value), which a rebased preview puts
    /// over what the active file already holds.
    edits: Map<String, Value>,
    /// The read of the active file's version of the record, when the dry run said it
    /// holds one (H1); its reply is matched against it.
    held: Option<Request>,
    /// The first dry run's envelope while the active file's version is read.
    first: Option<Value>,
    /// The paths carried from the active file's version (kept by this write).
    kept: Vec<String>,
    /// The preview's envelope, once it came back.
    previewed: Option<Value>,
    /// The confirmed write's envelope, once it came back.
    written: Option<Value>,
    /// Whether the read-back is still to be sent.
    readback_due: bool,
    /// The confirmed write as sent; its reply is matched against it (review finding L5).
    write_req: Option<Request>,
    /// The read-back as sent; its reply is matched against it.
    readback_req: Option<Request>,
    /// The read-back's envelope (the written record in the active file alone).
    readback: Option<Value>,
    /// The confirmed write was sent and no envelope came back: the confirm is spent, and
    /// another write needs a new OK and a new dry run (review finding M1).
    failed: bool,
    /// A build's spec file as it was when planned: the confirm covers its content, not
    /// only its path (M7.E5 review M1). `None` for the other commands.
    spec_bytes: Option<Vec<u8>>,
}

/// Why a build's spec no longer matches its plan: `spec` is the path, `planned` its
/// bytes at plan time.
fn spec_changed(spec: &str, planned: &[u8]) -> Option<String> {
    match std::fs::read(spec) {
        Ok(now) if now == planned => None,
        Ok(_) => Some("the spec changed since the plan: Plan again to preview it".to_string()),
        Err(e) => Some(format!("the spec cannot be read now ({e}): Plan again")),
    }
}

/// The editor's last confirmed `set`, the one its undo may invert.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LastWrite {
    form: String,
    into: String,
    /// Whether dropping the override restores the file exactly ([`undoable`]), or why
    /// not.
    undo: Result<(), String>,
    /// The active file's size and modification time right after the write, so an undo
    /// refuses a file another front end changed since (M7.E6 review L3).
    stamp: Option<(u64, std::time::SystemTime)>,
}

/// The size and modification time of `path`, when it can be read.
fn stamp_of(path: &str) -> Option<(u64, std::time::SystemTime)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.len(), m.modified().ok()?))
}

/// The editor's state (kept in `crate::windows::Shell`).
#[derive(Default)]
pub struct EditorState {
    /// The last confirmed `set` not yet undone.
    last: Option<LastWrite>,
    /// The Build Spec form, while it is open: the spec file's path.
    build_form: Option<String>,
    /// The New Form form (`new` or `clone`), while it is open.
    new_form: Option<new::NewForm>,
    /// The Place form, while it is open.
    place_form: Option<place::PlaceForm>,
    /// A dialog's OK, taken up by the next frame (which knows the active file).
    opening: Option<(String, String)>,
    /// The write in progress, shown in the confirm box.
    write: Option<Write>,
    /// The slot whose reply the editor waits for.
    awaiting: Option<Slot>,
    /// The last outcome or refusal, shown in the box until it is closed.
    note: Option<(Status, String)>,
    /// The request the next frame sends (a reply cannot send one itself).
    next: Option<(Slot, Request)>,
}

impl EditorState {
    /// `Action::OpenEditor`: start editing `form` with `command` (a catalog id).
    pub fn open(&mut self, command: &str, form: &str) {
        self.opening = Some((command.to_string(), form.to_string()));
    }

    /// `Action::PlaceInCell`: start placing `base` into `cell` (formrefs). The Place form
    /// opens on them; nothing is sent until its Preview. The last write's note goes with
    /// its box, so the box does not cover the form.
    pub fn place(&mut self, base: &str, cell: &str) {
        self.note = None;
        // The last write's box goes too, unless a reply is still awaited (M7.E4 review L1).
        if self.awaiting.is_none() {
            self.write = None;
        }
        self.place_form = Some(place::PlaceForm::open(base, cell));
    }

    /// Takes the reply to a request sent with `Pending::Editor(ticket)`.
    pub fn receive(&mut self, _ticket: &EditorTicket, _result: Result<&Value, String>) {}

    /// Takes a successful reply that came back on `Slot::EditorPreview`,
    /// `Slot::EditorWrite` or `Slot::EditorValidate`.
    pub(crate) fn receive_slot(&mut self, slot: Slot, request: &Request, env: &Value) {
        let Some(w) = self.write.as_mut() else {
            return;
        };
        match (slot, request.id.as_str()) {
            (Slot::EditorPreview, "set") if *request == w.preview => {
                self.awaiting = None;
                // The active file holds this record already: read its version before
                // asking for the confirm (H1). Once per OK.
                let held_id = (w.held.is_none()).then(|| replaced_form_id(env)).flatten();
                match held_id.and_then(|id| set::held_request(&id, &w.into)) {
                    Some(read) => {
                        w.first = Some(env.clone());
                        w.held = Some(read.clone());
                        self.next = Some((Slot::EditorPreview, read));
                    }
                    None => w.previewed = Some(env.clone()),
                }
            }
            (Slot::EditorPreview, "inspect") if w.held.as_ref() == Some(request) => {
                self.awaiting = None;
                let before = w
                    .first
                    .as_ref()
                    .and_then(|f| f.pointer("/data/before"))
                    .unwrap_or(&Value::Null);
                match rebase(env, before, &w.edits) {
                    Ok((_, kept)) if kept.is_empty() => {
                        // Nothing to carry: the first dry run stands.
                        w.previewed = w.first.take();
                    }
                    Ok((fields, kept)) => {
                        let preview = set::preview_request(&w.form, &fields, &w.into);
                        match gate::check(&preview) {
                            Ok(()) => {
                                w.kept = kept;
                                w.preview = preview.clone();
                                self.next = Some((Slot::EditorPreview, preview));
                            }
                            Err(e) => {
                                self.write = None;
                                self.note = Some((Status::Fail, e.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        self.write = None;
                        self.note = Some((Status::Fail, e));
                    }
                }
            }
            (Slot::EditorWrite, "set")
                if w.written.is_none() && w.write_req.as_ref() == Some(request) =>
            {
                w.written = Some(env.clone());
                w.readback_due = true;
                self.awaiting = None;
                if let Some(read) = set::readback_request(env, &w.into) {
                    w.readback_req = Some(read.clone());
                    self.next = Some((Slot::EditorWrite, read));
                }
                let file = file_name(&w.into);
                self.note = Some((
                    Status::Pass,
                    format!("written: {} into {file}", changed_line(env)),
                ));
                self.last = Some(LastWrite {
                    form: w.form.clone(),
                    into: w.into.clone(),
                    undo: undoable(env, &w.form, &w.into),
                    stamp: stamp_of(&w.into),
                });
            }
            (Slot::EditorPreview, "place") if *request == w.preview => {
                self.awaiting = None;
                w.previewed = Some(env.clone());
            }
            (Slot::EditorWrite, "place")
                if w.written.is_none() && w.write_req.as_ref() == Some(request) =>
            {
                w.written = Some(env.clone());
                self.awaiting = None;
                if let Some(read) = set::readback_request(env, &w.into) {
                    w.readback_req = Some(read.clone());
                    self.next = Some((Slot::EditorWrite, read));
                }
                let reference = own_record_form(env);
                let file = file_name(&w.into);
                self.note = Some((
                    Status::Pass,
                    format!("placed: {reference} ({}) into {file}", w.form),
                ));
                // The undo of a placement is refused, with the reason: the reference is a
                // new form of the active file, which `revert` does not drop; a first
                // placement in a cell also copies the cell (`undoable`).
                let why = match undoable(env, &reference, &w.into) {
                    Err(e) => format!("{e}; and the reference is a new form {file} defines, which `revert` does not drop"),
                    Ok(()) => format!("the reference is a new form {file} defines, which `revert` does not drop (it undoes overrides only)"),
                };
                self.last = Some(LastWrite {
                    form: reference,
                    into: w.into.clone(),
                    undo: Err(why),
                    stamp: stamp_of(&w.into),
                });
            }
            (Slot::EditorPreview, "new" | "clone") if *request == w.preview => {
                self.awaiting = None;
                w.previewed = Some(env.clone());
            }
            (Slot::EditorWrite, "new" | "clone")
                if w.written.is_none() && w.write_req.as_ref() == Some(request) =>
            {
                w.written = Some(env.clone());
                self.awaiting = None;
                // A form the active file defines is not `revert`'s to drop.
                self.last = None;
                if let Some(read) = set::readback_request(env, &w.into) {
                    w.readback_req = Some(read.clone());
                    self.next = Some((Slot::EditorWrite, read));
                }
                let form = env
                    .pointer("/data/records")
                    .and_then(Value::as_array)
                    .and_then(|a| {
                        a.iter()
                            .find(|r| r.get("parent") != Some(&Value::Bool(true)))
                    })
                    .and_then(|r| r.get("form"))
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string();
                self.note = Some((
                    Status::Pass,
                    format!("written: the new form {form} into {}", file_name(&w.into)),
                ));
            }
            (Slot::EditorPreview, "build") if *request == w.preview => {
                self.awaiting = None;
                w.previewed = Some(env.clone());
            }
            (Slot::EditorWrite, "build")
                if w.written.is_none() && w.write_req.as_ref() == Some(request) =>
            {
                w.written = Some(env.clone());
                self.awaiting = None;
                // A build is not the editor's undo's to invert.
                self.last = None;
                if let Some(read) = set::readback_request(env, &w.into) {
                    w.readback_req = Some(read.clone());
                    self.next = Some((Slot::EditorWrite, read));
                }
                self.note = Some((
                    Status::Pass,
                    format!("built: {} into {}", changed_line(env), file_name(&w.into)),
                ));
            }
            (Slot::EditorPreview, "revert") if *request == w.preview => {
                self.awaiting = None;
                w.previewed = Some(env.clone());
            }
            (Slot::EditorWrite, "revert")
                if w.written.is_none() && w.write_req.as_ref() == Some(request) =>
            {
                w.written = Some(env.clone());
                self.awaiting = None;
                // One undo per write.
                self.last = None;
                self.note = Some((
                    Status::Pass,
                    format!(
                        "undone: {} ({})",
                        diff_line(env),
                        env.pointer("/data/summary")
                            .and_then(Value::as_str)
                            .unwrap_or("revert")
                    ),
                ));
            }
            (Slot::EditorWrite, "inspect") if w.readback_req.as_ref() == Some(request) => {
                w.readback = Some(env.clone());
                self.awaiting = None;
            }
            _ => {}
        }
    }

    /// A request on an editor slot that came back without an envelope: refused by the
    /// gate or failed in the engine (`error`; `None` when only its absence is known).
    pub(crate) fn failed(&mut self, slot: Slot, error: Option<&str>) {
        if self.awaiting != Some(slot) {
            return;
        }
        self.awaiting = None;
        // Named after the command in progress (review finding L2 of M7.E6).
        let command = self.write.as_ref().map_or("set", |w| w.command);
        match slot {
            Slot::EditorWrite => {
                if let Some(w) = self.write.as_mut() {
                    if w.written.is_none() {
                        w.failed = true;
                    }
                }
            }
            // A preview (or its read of the active file) that failed leaves nothing to
            // confirm: no spinner, no dead button, only the note (review finding L1).
            Slot::EditorPreview => self.write = None,
            _ => {}
        }
        let what = match (slot, command) {
            (Slot::EditorPreview, "build") => "the plan (`build --plan`)".to_string(),
            (Slot::EditorPreview, c) => format!("the preview (`{c} --dry-run`)"),
            (Slot::EditorWrite, "build") => "the build".to_string(),
            (Slot::EditorWrite, "revert") => "the undo".to_string(),
            (Slot::EditorWrite, _) => "the write".to_string(),
            _ => "the request".to_string(),
        };
        let why = error.unwrap_or("refused or failed; nothing came back");
        self.note = Some((Status::Fail, format!("{what}: {why}")));
    }
}

/// The last part of a path, either separator.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The changed fields of a `set` envelope (dry run or write), one `path before -> after`
/// each.
fn changes(env: &Value) -> Vec<(String, String, String)> {
    let data = env.get("data").unwrap_or(&Value::Null);
    if let Some(a) = data.get("changes").and_then(Value::as_array) {
        // A build plan: every change, each with its record.
        return a
            .iter()
            .map(|c| {
                (
                    format!(
                        "{} {}",
                        c.get("form").map(fmt_value).unwrap_or_default(),
                        c.get("path").map(fmt_value).unwrap_or_default()
                    ),
                    c.get("before").map(fmt_value).unwrap_or_default(),
                    c.get("after").map(fmt_value).unwrap_or_default(),
                )
            })
            .collect();
    }
    let from_changed = data.get("changed").and_then(Value::as_array).map(|a| {
        a.iter()
            .map(|c| {
                (
                    c.get("path").map(fmt_value).unwrap_or_default(),
                    c.get("before").map(fmt_value).unwrap_or_default(),
                    c.get("after").map(fmt_value).unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>()
    });
    from_changed.unwrap_or_else(|| {
        data.get("records")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|r| r.get("parent") != Some(&Value::Bool(true)))
            .flat_map(|r| {
                r.get("fields")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            })
            .map(|c| {
                (
                    c.get("path").map(fmt_value).unwrap_or_default(),
                    c.get("before").map(fmt_value).unwrap_or_default(),
                    c.get("after").map(fmt_value).unwrap_or_default(),
                )
            })
            .collect()
    })
}

fn changed_line(env: &Value) -> String {
    let c = changes(env);
    if c.is_empty() {
        return "no field changed".to_string();
    }
    c.iter()
        .map(|(p, b, a)| format!("{p} {b} -> {a}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The formref of the record a write's envelope wrote (its first non-parent record).
fn own_record_form(env: &Value) -> String {
    env.pointer("/data/records")
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter()
                .find(|r| r.get("parent") != Some(&Value::Bool(true)))
        })
        .and_then(|r| r.get("form"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string()
}

/// Whether undoing a `set` write (`env`, its envelope) with `revert --to remove`
/// restores the active file exactly, or why not. It does when the write inserted the
/// record and inserted nothing else. A record the file already held (`replaced`, a second
/// edit) would lose that earlier version too. A parent the write copied in to hold the
/// record (a placed reference's CELL and WRLD, an INFO's DIAL: `parent: true`,
/// `inserted`) stays after `revert`, which drops only the record (M7.E6 review M1).
fn undoable(env: &Value, form: &str, into: &str) -> Result<(), String> {
    let file = file_name(into);
    let records = env
        .pointer("/data/records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let outcome = |r: &Value| {
        r.get("outcome")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string()
    };
    let is_parent = |r: &Value| r.get("parent") == Some(&Value::Bool(true));
    let Some(own) = records.iter().find(|r| !is_parent(r)) else {
        return Err(format!("the write's envelope names no record in {file}"));
    };
    if outcome(own) != "inserted" {
        return Err(format!(
            "{file} already held {form} before the last write, and `revert` would drop that earlier version too (it would undo more than one write); `kit revert` drops the whole override"
        ));
    }
    let copied: Vec<String> = records
        .iter()
        .filter(|r| is_parent(r) && outcome(r) == "inserted")
        .map(|r| {
            format!(
                "{} {}",
                r.get("signature").and_then(Value::as_str).unwrap_or("?"),
                r.get("form").and_then(Value::as_str).unwrap_or("?")
            )
        })
        .collect();
    if !copied.is_empty() {
        return Err(format!(
            "the write also copied {} into {file} to hold the record; `revert` keeps those, so the undo would not restore the file; `kit revert` the record and its parents",
            copied.join(" and ")
        ));
    }
    Ok(())
}

/// A `revert` envelope's diff of the file in one line.
fn diff_line(env: &Value) -> String {
    let n = |k: &str| {
        env.pointer(&format!("/data/diff/counts/{k}"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    format!(
        "{} removed, {} changed, {} added",
        n("removed"),
        n("changed"),
        n("added")
    )
}

/// The FormID (as the active file stores it) of the record a `set` dry run says it would
/// replace in the active file: the active file holds a version already. `None` when the
/// record would be inserted (a new file, or a record new to it).
fn replaced_form_id(env: &Value) -> Option<String> {
    env.pointer("/data/records")?
        .as_array()?
        .iter()
        .filter(|r| r.get("parent") != Some(&Value::Bool(true)))
        .find(|r| {
            matches!(
                r.get("outcome").and_then(Value::as_str),
                Some("replaced" | "moved")
            )
        })?
        .get("form_id")?
        .as_str()
        .map(str::to_string)
}

/// The `set` path of each subrecord of an envelope's list (signature, `[n]` for the n-th
/// repeat) with its payload, `None` when the list cut it short.
fn payloads(subrecords: Option<&Value>) -> BTreeMap<String, Option<String>> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for s in subrecords.and_then(Value::as_array).into_iter().flatten() {
        let Some(sig) = s.get("signature").and_then(Value::as_str) else {
            continue;
        };
        let n = seen.entry(sig.to_string()).or_insert(0);
        let path = if *n == 0 {
            sig.to_string()
        } else {
            format!("{sig}[{n}]")
        };
        *n = n.saturating_add(1);
        let whole = s.get("payload_truncated") != Some(&Value::Bool(true));
        let hex = s
            .get("payload_hex")
            .and_then(Value::as_str)
            .filter(|_| whole)
            .map(str::to_string);
        out.insert(path, hex);
    }
    out
}

/// The fields of a rebased `set` (H1): every subrecord the active file's version
/// (`held`, a single-plugin `inspect` with every byte) holds differently from the load
/// order's winner (`before`, the dry run's `before`), as `hex:` at its `set` path, with
/// the dialog's `edits` over them; and the paths kept. `Err` when the active file's
/// version could not be read whole.
fn rebase(
    held: &Value,
    before: &Value,
    edits: &Map<String, Value>,
) -> Result<(Map<String, Value>, Vec<String>), String> {
    let held = payloads(held.pointer("/data/subrecords"));
    let winner = payloads(before.get("subrecords"));
    let mut fields = Map::new();
    let mut kept = Vec::new();
    for (path, hex) in held {
        if edits.contains_key(&path) {
            continue;
        }
        let Some(hex) = hex else {
            return Err(format!(
                "the active file's {path} could not be read whole; edit this record with `kit set`"
            ));
        };
        if winner.get(&path).and_then(Option::as_ref) != Some(&hex) {
            fields.insert(path.clone(), Value::String(format!("hex:{hex}")));
            kept.push(path);
        }
    }
    for (k, v) in edits {
        fields.insert(k.clone(), v.clone());
    }
    Ok((fields, kept))
}

/// Why the confirm button is disabled, or `None` when one click may write.
fn confirm_blocked(
    w: &Write,
    busy: bool,
    awaiting: bool,
    active_file: Option<&str>,
) -> Option<String> {
    let Some(env) = &w.previewed else {
        return Some("waiting for the preview".to_string());
    };
    if w.written.is_some() {
        return Some("written: one write per OK; edit and OK again for another".to_string());
    }
    if w.failed {
        return Some("the write failed: OK again for a new dry run".to_string());
    }
    let writable = if w.command == "build" {
        env.pointer("/data/passed") == Some(&Value::Bool(true))
    } else {
        env.pointer("/data/would_write") == Some(&Value::Bool(true))
    };
    if !writable {
        return Some(if w.command == "build" {
            "the plan's validate did not pass: a real build would be refused".to_string()
        } else {
            "the dry run says a real write would be refused (validate finds errors on the written record)"
                .to_string()
        });
    }
    if active_file != Some(w.into.as_str()) {
        return Some(format!(
            "the active file is no longer {}: OK again to preview into the active file",
            file_name(&w.into)
        ));
    }
    if w.command == "build" {
        // What the box shows is what is built (M7.E5 review M1), into a fresh file (L1).
        if let Some(why) = w
            .spec_bytes
            .as_deref()
            .and_then(|planned| spec_changed(&w.form, planned))
        {
            return Some(why);
        }
        if std::path::Path::new(&w.into).exists() {
            return Some(format!(
                "{} exists now: `build` writes a fresh plugin and the window never overwrites one",
                file_name(&w.into)
            ));
        }
    }
    if busy || awaiting {
        return Some("the engine is busy".to_string());
    }
    None
}

/// The editor's per-frame hook, called by the shell after the CK-style windows: draws
/// what the editor has open and returns the actions it asks for (its requests go out as
/// `Action::Submit` on the editor slots). `active_file` is the Data dialog's choice, the
/// one file the writes may go into; `selected` the inspected record.
pub(crate) fn ui(
    ctx: &egui::Context,
    state: &mut EditorState,
    busy: bool,
    active_file: Option<&str>,
    _selected: Option<&str>,
) -> Vec<Action> {
    let mut out = Vec::new();
    // A reply the editor waits for never came (the window applies only an envelope
    // here; a refusal or engine error stays in the slot's result).
    if let Some(slot) = state.awaiting {
        if !busy {
            state.failed(slot, None);
        }
    }
    if let Some((command, form)) = state.opening.take() {
        start(ctx, state, &command, &form, active_file, &mut out);
    }
    if let Some(w) = state.write.as_mut() {
        if w.readback_due {
            // Written: the dialog's edits are in the active file now.
            w.readback_due = false;
            edit::clear_edits(ctx, &w.form);
        }
    }
    if state.awaiting.is_none() {
        if let Some((slot, req)) = state.next.take() {
            state.awaiting = Some(slot);
            out.push(Action::Submit(slot, req));
        }
    }
    if state.build_form.is_some() {
        build_window(ctx, state, active_file, &mut out);
    }
    if state.new_form.is_some() {
        new_window(ctx, state, active_file, &mut out);
    }
    if state.place_form.is_some() {
        place_window(ctx, state, active_file, &mut out);
    }
    if state.write.is_some() || state.note.is_some() {
        confirm_box(ctx, state, busy, active_file, &mut out);
    }
    if !out.is_empty() || state.opening.is_some() {
        // The next step (an opened edit, a request's reply) needs another frame.
        ctx.request_repaint();
    }
    out
}

/// Takes up a dialog's OK: the preview of `form`'s changed fields into the active file,
/// or the reason there is none.
fn start(
    ctx: &egui::Context,
    state: &mut EditorState,
    command: &str,
    form: &str,
    active_file: Option<&str>,
    out: &mut Vec<Action>,
) {
    if state.awaiting.is_some() {
        state.note = Some((
            Status::Warn,
            "the editor is waiting for the engine; OK again when it has answered".to_string(),
        ));
        return;
    }
    state.write = None;
    if command == "build" {
        // The form opens; nothing is sent until its Plan.
        state.build_form.get_or_insert_with(String::new);
        return;
    }
    if command == "new" || command == "clone" {
        // The form opens on its source; nothing is sent until its Preview. The last
        // write's note goes with its box, so the box does not cover the form (M7.E3
        // review L2).
        state.note = None;
        state.new_form = Some(new::NewForm::open(command, form));
        return;
    }
    if command != "set" && command != "revert" {
        state.note = Some((
            Status::NotRun,
            format!("`{command}` is not in the editor yet (ADR 0007 Amendment 4: its M7.E ticket)"),
        ));
        return;
    }
    let Some(into) = active_file.map(str::trim).filter(|f| !f.is_empty()) else {
        state.note = Some((
            Status::Fail,
            "no active file: choose one in File > Data... (Set as Active File); the editor writes only into it"
                .to_string(),
        ));
        return;
    };
    if command == "revert" {
        start_undo(state, form, into, out);
        return;
    }
    let fields = match edit::changed_fields(ctx, form) {
        Ok(f) if f.is_empty() => {
            state.note = Some((Status::NotRun, "no field changed".to_string()));
            return;
        }
        Ok(f) => f,
        Err(e) => {
            state.note = Some((Status::Fail, e));
            return;
        }
    };
    let preview = set::preview_request(form, &fields, into);
    if let Err(e) = gate::check(&preview) {
        state.note = Some((Status::Fail, e.to_string()));
        return;
    }
    state.note = None;
    state.next = None;
    state.write = Some(Write {
        command: "set",
        form: form.to_string(),
        into: into.to_string(),
        preview: preview.clone(),
        edits: fields,
        held: None,
        first: None,
        kept: Vec::new(),
        previewed: None,
        written: None,
        readback_due: false,
        write_req: None,
        readback_req: None,
        readback: None,
        failed: false,
        spec_bytes: None,
    });
    state.awaiting = Some(Slot::EditorPreview);
    out.push(Action::Submit(Slot::EditorPreview, preview));
}

/// The Build Spec form (`build::form_ui`) in its own window; its Plan starts a build.
fn build_window(
    ctx: &egui::Context,
    state: &mut EditorState,
    active_file: Option<&str>,
    out: &mut Vec<Action>,
) {
    let mut plan = false;
    let mut close = false;
    if let Some(spec) = state.build_form.as_mut() {
        egui::Window::new("Build Spec")
            .id(egui::Id::new("kitkraft-editor-build"))
            .collapsible(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                let ask = build::form_ui(ui, spec, active_file);
                plan = ask.plan;
                close = ask.close;
            });
    }
    if close {
        state.build_form = None;
        return;
    }
    if plan {
        let spec = state.build_form.clone().unwrap_or_default();
        start_build(state, &spec, active_file, out);
    }
}

/// The Place form (`place::form_ui`) in its own window; its Preview starts the write.
fn place_window(
    ctx: &egui::Context,
    state: &mut EditorState,
    active_file: Option<&str>,
    out: &mut Vec<Action>,
) {
    let mut preview = false;
    let mut close = false;
    // Locked while its preview awaits the confirm (M7.E4 review M1).
    let locked = state
        .write
        .as_ref()
        .is_some_and(|w| w.command == "place" && w.written.is_none() && !w.failed);
    if let Some(f) = state.place_form.as_mut() {
        egui::Window::new("Place Reference")
            .id(egui::Id::new("kitkraft-editor-place"))
            .collapsible(false)
            .default_width(480.0)
            .show(ctx, |ui| {
                let ask = place::form_ui(ui, f, active_file, locked);
                preview = ask.preview;
                close = ask.close;
            });
    }
    if close {
        state.place_form = None;
        return;
    }
    if preview {
        if let Some(f) = state.place_form.clone() {
            start_place(state, &f, active_file, out);
        }
    }
}

/// Takes up the Place form's Preview: `place --dry-run` into the active file, or the
/// reason there is none.
fn start_place(
    state: &mut EditorState,
    f: &place::PlaceForm,
    active_file: Option<&str>,
    out: &mut Vec<Action>,
) {
    if state.awaiting.is_some() {
        state.note = Some((
            Status::Warn,
            "the editor is waiting for the engine; Preview again when it has answered".to_string(),
        ));
        return;
    }
    state.write = None;
    let Some(into) = active_file.map(str::trim).filter(|f| !f.is_empty()) else {
        state.note = Some((
            Status::Fail,
            "no active file: choose one in File > Data... (Set as Active File); the editor writes only into it"
                .to_string(),
        ));
        return;
    };
    let position = match f.position() {
        Ok(p) => p,
        Err(e) => {
            state.note = Some((Status::Fail, e));
            return;
        }
    };
    let preview = place::preview_request(&f.base, &f.cell, position, into);
    if let Err(e) = gate::check(&preview) {
        state.note = Some((Status::Fail, e.to_string()));
        return;
    }
    state.note = None;
    state.next = None;
    state.write = Some(Write {
        command: "place",
        form: format!("{} in {}", f.base, f.cell),
        into: into.to_string(),
        preview: preview.clone(),
        edits: Map::new(),
        held: None,
        first: None,
        kept: Vec::new(),
        previewed: None,
        written: None,
        readback_due: false,
        write_req: None,
        readback_req: None,
        readback: None,
        failed: false,
        spec_bytes: None,
    });
    state.awaiting = Some(Slot::EditorPreview);
    out.push(Action::Submit(Slot::EditorPreview, preview));
}

/// The New Form form (`new::form_ui`) in its own window; its Preview starts the write.
fn new_window(
    ctx: &egui::Context,
    state: &mut EditorState,
    active_file: Option<&str>,
    out: &mut Vec<Action>,
) {
    let mut preview = false;
    let mut close = false;
    if let Some(f) = state.new_form.as_mut() {
        let title = if f.command == "clone" {
            "Duplicate"
        } else {
            "New Form"
        };
        egui::Window::new(title)
            .id(egui::Id::new("kitkraft-editor-new"))
            .collapsible(false)
            .default_width(480.0)
            .show(ctx, |ui| {
                let ask = new::form_ui(ui, f, active_file);
                preview = ask.preview;
                close = ask.close;
            });
    }
    if close {
        state.new_form = None;
        return;
    }
    if preview {
        if let Some(f) = state.new_form.clone() {
            start_new(state, &f, active_file, out);
        }
    }
}

/// Takes up the New Form form's Preview: `new --dry-run` or `clone --dry-run` into the
/// active file, or the reason there is none.
fn start_new(
    state: &mut EditorState,
    f: &new::NewForm,
    active_file: Option<&str>,
    out: &mut Vec<Action>,
) {
    if state.awaiting.is_some() {
        state.note = Some((
            Status::Warn,
            "the editor is waiting for the engine; Preview again when it has answered".to_string(),
        ));
        return;
    }
    state.write = None;
    let Some(into) = active_file.map(str::trim).filter(|f| !f.is_empty()) else {
        state.note = Some((
            Status::Fail,
            "no active file: choose one in File > Data... (Set as Active File); the editor writes only into it"
                .to_string(),
        ));
        return;
    };
    let (source, edid) = (f.source.trim(), f.edid.trim());
    if source.is_empty() || edid.is_empty() {
        state.note = Some((
            Status::NotRun,
            "name the record type (or the record to copy) and the new EditorID".to_string(),
        ));
        return;
    }
    let (command, preview) = if f.command == "clone" {
        ("clone", clone::preview_request(source, edid, into))
    } else {
        ("new", new::preview_request(source, edid, into))
    };
    if let Err(e) = gate::check(&preview) {
        state.note = Some((Status::Fail, e.to_string()));
        return;
    }
    state.note = None;
    state.next = None;
    state.write = Some(Write {
        command,
        form: format!("{source} as {edid}"),
        into: into.to_string(),
        preview: preview.clone(),
        edits: Map::new(),
        held: None,
        first: None,
        kept: Vec::new(),
        previewed: None,
        written: None,
        readback_due: false,
        write_req: None,
        readback_req: None,
        readback: None,
        failed: false,
        spec_bytes: None,
    });
    state.awaiting = Some(Slot::EditorPreview);
    out.push(Action::Submit(Slot::EditorPreview, preview));
}

/// Takes up the Build Spec form's Plan: `build --plan` of `spec` into the active file, or
/// the reason there is none.
fn start_build(
    state: &mut EditorState,
    spec: &str,
    active_file: Option<&str>,
    out: &mut Vec<Action>,
) {
    if state.awaiting.is_some() {
        state.note = Some((
            Status::Warn,
            "the editor is waiting for the engine; Plan again when it has answered".to_string(),
        ));
        return;
    }
    state.write = None;
    let Some(into) = active_file.map(str::trim).filter(|f| !f.is_empty()) else {
        state.note = Some((
            Status::Fail,
            "no active file: choose one in File > Data... (Set as Active File); the build writes only into it"
                .to_string(),
        ));
        return;
    };
    let spec = spec.trim();
    if spec.is_empty() {
        state.note = Some((
            Status::NotRun,
            "name the build spec (a .toml file)".to_string(),
        ));
        return;
    }
    if std::path::Path::new(into).exists() {
        state.note = Some((
            Status::Fail,
            format!(
                "{} exists: `build` writes a fresh plugin and the window never overwrites one; choose a new active file",
                file_name(into)
            ),
        ));
        return;
    }
    let spec_bytes = match std::fs::read(spec) {
        Ok(b) => b,
        Err(e) => {
            state.note = Some((Status::Fail, format!("cannot read the spec {spec}: {e}")));
            return;
        }
    };
    let plan = build::plan_request(spec, into);
    if let Err(e) = gate::check(&plan) {
        state.note = Some((Status::Fail, e.to_string()));
        return;
    }
    state.note = None;
    state.next = None;
    state.write = Some(Write {
        command: "build",
        form: spec.to_string(),
        into: into.to_string(),
        preview: plan.clone(),
        edits: Map::new(),
        held: None,
        first: None,
        kept: Vec::new(),
        previewed: None,
        written: None,
        readback_due: false,
        write_req: None,
        readback_req: None,
        readback: None,
        failed: false,
        spec_bytes: Some(spec_bytes),
    });
    state.awaiting = Some(Slot::EditorPreview);
    out.push(Action::Submit(Slot::EditorPreview, plan));
}

/// Takes up "Undo last write" on `form`: the `revert` preview of the editor's last write,
/// when that write is of `form` into the active file and added the record there; else
/// the reason there is none.
fn start_undo(state: &mut EditorState, form: &str, into: &str, out: &mut Vec<Action>) {
    let file = file_name(into);
    let Some(last) = state
        .last
        .as_ref()
        .filter(|l| l.form == form && l.into == into)
    else {
        state.note = Some((
            Status::NotRun,
            format!("nothing to undo: the editor has no write of {form} into {file} this session"),
        ));
        return;
    };
    if let Err(why) = &last.undo {
        state.note = Some((Status::Fail, format!("undo refused: {why}")));
        return;
    }
    if last.stamp.is_none() || stamp_of(into) != last.stamp {
        state.note = Some((
            Status::Fail,
            format!(
                "undo refused: {file} changed since the editor's write (its size or time differs), so dropping the record could undo someone else's edit too; undo with `kit revert`"
            ),
        ));
        return;
    }
    let preview = revert::preview_request(into, form);
    if let Err(e) = gate::check(&preview) {
        state.note = Some((Status::Fail, e.to_string()));
        return;
    }
    state.note = None;
    state.next = None;
    state.write = Some(Write {
        command: "revert",
        form: form.to_string(),
        into: into.to_string(),
        preview: preview.clone(),
        edits: Map::new(),
        held: None,
        first: None,
        kept: Vec::new(),
        previewed: None,
        written: None,
        readback_due: false,
        write_req: None,
        readback_req: None,
        readback: None,
        failed: false,
        spec_bytes: None,
    });
    state.awaiting = Some(Slot::EditorPreview);
    out.push(Action::Submit(Slot::EditorPreview, preview));
}

/// The confirm box: what the preview says the write does, its one confirm button, and
/// the outcome.
fn confirm_box(
    ctx: &egui::Context,
    state: &mut EditorState,
    busy: bool,
    active_file: Option<&str>,
    out: &mut Vec<Action>,
) {
    let mut close = false;
    let mut confirm = false;
    let command = state.write.as_ref().map_or("set", |w| w.command);
    egui::Window::new(format!("Confirm {command}"))
        .id(egui::Id::new("kitkraft-editor-confirm"))
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .show(ctx, |ui| {
            if let Some((status, text)) = &state.note {
                theme::status_line(ui, *status, text);
            }
            if let Some(w) = &state.write {
                ui.horizontal_wrapped(|ui| {
                    let what = if w.command == "revert" {
                        format!("undo: revert {} in {}", w.form, file_name(&w.into))
                    } else if w.command == "build" {
                        format!("build {} into {}", file_name(&w.form), file_name(&w.into))
                    } else if w.command == "place" {
                        format!("place {} into {}", w.form, file_name(&w.into))
                    } else if w.command == "new" || w.command == "clone" {
                        let verb = if w.command == "clone" { "duplicate" } else { "new" };
                        format!("{verb} {} into {}", w.form, file_name(&w.into))
                    } else {
                        format!("set {} into {}", w.form, file_name(&w.into))
                    };
                    ui.strong(what);
                    ui.weak(&w.into);
                });
                if w.command == "place" {
                    // The exact position the confirmed request writes (M7.E4 review M1).
                    ui.monospace(place::position_line(&w.preview));
                }
                match &w.previewed {
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(if w.command == "build" {
                                "planning: `build --plan` (nothing is written)".to_string()
                            } else {
                                format!("previewing: `{} --dry-run` on a staged copy", w.command)
                            });
                        });
                    }
                    Some(env) => {
                        if !w.kept.is_empty() {
                            theme::status_line(
                                ui,
                                Status::Warn,
                                &format!(
                                    "kept from {}: {} (already written there; `set` starts from the load order's winner, so this write carries them)",
                                    file_name(&w.into),
                                    w.kept.join(", ")
                                ),
                            );
                        }
                        preview_lines(ui, env, w.command);
                    }
                }
                if let (Some(done), "revert") = (&w.written, w.command) {
                    ui.label(theme::subheading(
                        ui,
                        format!("Undone in {}: {}", file_name(&w.into), diff_line(done)),
                    ));
                }
                if let Some(back) = &w.readback {
                    readback_lines(ui, back, &w.into);
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if let Some(w) = &state.write {
                    let blocked = confirm_blocked(w, busy, state.awaiting.is_some(), active_file);
                    let (label, hover) = if w.command == "revert" {
                        (
                            format!("Undo in {}", file_name(&w.into)),
                            "one confirmed `revert`: drops the override the last write added",
                        )
                    } else if w.command == "build" {
                        (
                            format!("Build into {}", file_name(&w.into)),
                            "one confirmed `build`: the planned plugin, written as the active file",
                        )
                    } else if w.command == "place" {
                        (
                            format!("Write to {}", file_name(&w.into)),
                            "one confirmed `place`: the previewed reference (and the cell it carries), into the active file",
                        )
                    } else if w.command == "new" || w.command == "clone" {
                        (
                            format!("Write to {}", file_name(&w.into)),
                            "one confirmed write: the previewed new form, into the active file",
                        )
                    } else {
                        (
                            format!("Write to {}", file_name(&w.into)),
                            "one confirmed `set`: the previewed fields, into the active file",
                        )
                    };
                    let b = ui.add_enabled(blocked.is_none(), egui::Button::new(label));
                    let b = match &blocked {
                        Some(why) => b.on_disabled_hover_text(why.as_str()),
                        None => b.on_hover_text(hover),
                    };
                    if b.clicked() {
                        confirm = true;
                    }
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
    if confirm {
        if let Some(w) = state.write.as_mut() {
            if let Some(why) = w
                .spec_bytes
                .as_deref()
                .and_then(|planned| spec_changed(&w.form, planned))
            {
                state.note = Some((Status::Fail, why));
                return;
            }
            let confirmed = if w.command == "revert" {
                revert::write_request(&w.preview)
            } else if w.command == "build" {
                build::build_request(&w.preview)
            } else if w.command == "place" {
                place::write_request(&w.preview)
            } else if w.command == "new" {
                new::write_request(&w.preview)
            } else if w.command == "clone" {
                clone::write_request(&w.preview)
            } else {
                set::write_request(&w.preview)
            };
            match confirmed.map(|r| (gate::check(&r), r)) {
                Some((Ok(()), req)) => {
                    w.write_req = Some(req.clone());
                    state.awaiting = Some(Slot::EditorWrite);
                    out.push(Action::Submit(Slot::EditorWrite, req));
                }
                Some((Err(e), _)) => state.note = Some((Status::Fail, e.to_string())),
                None => state.note = Some((Status::Fail, "the preview is not a dry run".into())),
            }
        }
    }
    if close {
        state.write = None;
        state.note = None;
    }
}

/// The preview's changes (`set`: the fields; `revert`: the diff of the file), its
/// `validate` on the staged result, and its limitations.
fn preview_lines(ui: &mut egui::Ui, env: &Value, command: &str) {
    let data = env.get("data").unwrap_or(&Value::Null);
    if command == "place" {
        ui.label(theme::subheading(ui, "Placed (dry run)"));
        for p in data
            .get("placed")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let text = |ptr: &str| p.pointer(ptr).map(fmt_value).unwrap_or_default();
            ui.monospace(format!(
                "{} of {} in {} ({})",
                text("/form"),
                text("/base/form"),
                text("/cell/form"),
                text("/cell/kind")
            ));
        }
        for c in data
            .get("carried")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let text = |k: &str| c.get(k).map(fmt_value).unwrap_or_default();
            ui.weak(format!(
                "carries {} {} ({}) to hold it",
                text("signature"),
                text("form"),
                text("outcome")
            ));
        }
    } else if command == "revert" {
        ui.label(theme::subheading(
            ui,
            format!("Diff of the file (dry run): {}", diff_line(env)),
        ));
        for r in data
            .pointer("/diff/removed")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            ui.monospace(format!("removed {}", fmt_value(r)));
        }
    } else {
        ui.label(theme::subheading(
            ui,
            if command == "build" {
                "Changes (plan; nothing written)"
            } else {
                "Fields changed (dry run)"
            },
        ));
        let rows = changes(env);
        if rows.is_empty() {
            ui.weak("  (none: the values are what the record holds)");
        }
        egui::Grid::new("kitkraft-editor-changes")
            .striped(true)
            .show(ui, |ui| {
                for (path, before, after) in &rows {
                    ui.monospace(path);
                    ui.monospace(before);
                    ui.label("->");
                    ui.monospace(after);
                    ui.end_row();
                }
            });
    }
    let would = if command == "build" {
        data.get("passed") == Some(&Value::Bool(true))
    } else {
        data.get("would_write") == Some(&Value::Bool(true))
    };
    let validate = data.get("validate").unwrap_or(&Value::Null);
    let n = |k: &str| validate.get(k).and_then(Value::as_u64).unwrap_or(0);
    let line = format!(
        "validate on the staged result: {} error(s), {} warning(s){}",
        n("errors"),
        n("warnings"),
        if would {
            ""
        } else {
            "; a real write would be refused"
        }
    );
    theme::status_line(ui, if would { Status::Pass } else { Status::Fail }, &line);
    let findings = validate
        .get("findings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !findings.is_empty() {
        ScrollArea::vertical()
            .id_salt("kitkraft-editor-findings")
            .max_height(theme::view_height(ui, theme::View::Short))
            .show(ui, |ui| {
                for f in &findings {
                    let text = |k: &str| f.get(k).map(fmt_value).unwrap_or_default();
                    ui.label(format!(
                        "[{}] {} {}",
                        text("rule"),
                        text("severity"),
                        text("message")
                    ));
                }
            });
    }
    for l in env
        .get("limitations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        ui.weak(l);
    }
}

/// The written record as the active file now holds it (raw payloads: single-plugin
/// mode).
fn readback_lines(ui: &mut egui::Ui, env: &Value, into: &str) {
    ui.label(theme::subheading(
        ui,
        format!("Read back from {}", file_name(into)),
    ));
    let subs = env
        .pointer("/data/subrecords")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for s in &subs {
        let text = |k: &str| s.get(k).map(fmt_value).unwrap_or_default();
        ui.monospace(format!(
            "{} @{} {}",
            text("signature"),
            text("offset"),
            text("payload_hex")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write(into: &str) -> Write {
        Write {
            command: "set",
            form: "Fixture.esm:000800".into(),
            into: into.into(),
            preview: Request::new("set", json!({"dry_run": true})),
            edits: Map::new(),
            held: None,
            first: None,
            kept: Vec::new(),
            previewed: None,
            written: None,
            readback_due: false,
            write_req: None,
            readback_req: None,
            readback: None,
            failed: false,
            spec_bytes: None,
        }
    }

    /// H1: a rebase carries what the active file holds differently from the winner, as
    /// `hex:` at its path (repeats by occurrence), lets the dialog's edits win, and
    /// refuses a version it could not read whole.
    #[test]
    fn a_rebase_carries_what_the_active_file_holds_over_the_winner() {
        let sub = |sig: &str, hex: &str| json!({"signature": sig, "payload_hex": hex});
        let held = json!({"data": {"subrecords": [
            sub("EDID", "4100"), sub("FULL", "5800"), sub("FULL", "5900"), sub("FLTV", "0000E840")
        ]}});
        let before = json!({"subrecords": [
            sub("EDID", "4100"), sub("FULL", "5800"), sub("FULL", "5A00"), sub("FLTV", "0000C03F")
        ]});
        let mut edits = Map::new();
        edits.insert("EDID".into(), json!("B"));
        let (fields, kept) = rebase(&held, &before, &edits).unwrap();
        assert_eq!(kept, ["FLTV", "FULL[1]"]);
        assert_eq!(
            Value::Object(fields),
            json!({"EDID": "B", "FLTV": "hex:0000E840", "FULL[1]": "hex:5900"})
        );
        // Nothing differs: nothing kept.
        let (_, kept) = rebase(
            &held,
            &json!({"subrecords": held["data"]["subrecords"]}),
            &Map::new(),
        )
        .unwrap();
        assert!(kept.is_empty());
        // A cut payload cannot be carried.
        let cut = json!({"data": {"subrecords": [
            {"signature": "DATA", "payload_hex": "00", "payload_truncated": true}
        ]}});
        assert!(rebase(&cut, &before, &Map::new()).is_err());
        // Only a replaced record is read back first.
        let env = |outcome: &str| {
            json!({"data": {"records": [
                {"form_id": "00000801", "parent": true, "outcome": "replaced"},
                {"form_id": "00000800", "parent": false, "outcome": outcome}
            ]}})
        };
        assert_eq!(
            replaced_form_id(&env("replaced")).as_deref(),
            Some("00000800")
        );
        assert_eq!(replaced_form_id(&env("inserted")), None);
    }

    /// M7.E6 review M1: a write is undoable only when it inserted the record and nothing
    /// else; a parent copied in to hold it (CELL, WRLD) or a record already held refuses
    /// the undo with the reason; a parent the file already held does not.
    #[test]
    fn an_undo_needs_a_write_that_inserted_only_the_record() {
        let env = |records: Value| json!({"data": {"records": records}});
        let rec = |sig: &str, form: &str, parent: bool, outcome: &str| json!({"signature": sig, "form": form, "parent": parent, "outcome": outcome});
        let refr = rec("REFR", "Starfield.esm:000807", false, "inserted");
        assert_eq!(undoable(&env(json!([refr.clone()])), "f", "P.esp"), Ok(()));
        let parents = env(json!([
            rec("WRLD", "Starfield.esm:27CFA2", true, "inserted"),
            rec("CELL", "Starfield.esm:27F1A0", true, "inserted"),
            refr.clone()
        ]));
        let why = undoable(&parents, "f", r"C:\out\P.esp").unwrap_err();
        assert!(
            why.contains("WRLD Starfield.esm:27CFA2 and CELL Starfield.esm:27F1A0")
                && why.contains("P.esp"),
            "{why}"
        );
        // The file already held the parent: dropping the REFR restores it.
        let held_parent = env(json!([
            rec("CELL", "Starfield.esm:27F1A0", true, "replaced"),
            refr
        ]));
        assert_eq!(undoable(&held_parent, "f", "P.esp"), Ok(()));
        let replaced = env(json!([rec("GLOB", "F.esm:000800", false, "replaced")]));
        assert!(undoable(&replaced, "f", "P.esp")
            .unwrap_err()
            .contains("already held"));
        assert!(undoable(&env(json!([])), "f", "P.esp").is_err());
    }

    /// A build's confirm needs a plan whose validate passed; its changes list each with
    /// its record.
    #[test]
    fn a_build_confirm_needs_a_plan_that_passed() {
        let mut w = write(r"C:\out\Patch.esp");
        w.command = "build";
        let active = Some(r"C:\out\Patch.esp");
        w.previewed = Some(json!({"data": {"passed": false, "would_write": true}}));
        assert!(confirm_blocked(&w, false, false, active).is_some());
        let plan = json!({"data": {"passed": true, "changes": [
            {"form": "Fixture.esm:000800", "path": "FLTV", "before": 1.5, "after": 7.25}
        ]}});
        w.previewed = Some(plan.clone());
        assert_eq!(confirm_blocked(&w, false, false, active), None);
        assert_eq!(changed_line(&plan), "Fixture.esm:000800 FLTV 1.5 -> 7.25");
    }

    /// The confirm button waits for the preview, needs a dry run that would write and
    /// the preview's output still the active file, and allows one write per OK.
    #[test]
    fn the_confirm_needs_a_writable_preview_into_the_active_file_once() {
        let mut w = write(r"C:\out\Patch.esp");
        let active = Some(r"C:\out\Patch.esp");
        assert!(
            confirm_blocked(&w, false, false, active).is_some(),
            "no preview yet"
        );
        w.previewed = Some(json!({"data": {"would_write": false}}));
        let why = confirm_blocked(&w, false, false, active).unwrap();
        assert!(why.contains("refused"), "{why}");
        w.previewed = Some(json!({"data": {"would_write": true}}));
        assert_eq!(confirm_blocked(&w, false, false, active), None);
        assert!(confirm_blocked(&w, true, false, active).is_some(), "busy");
        let other = confirm_blocked(&w, false, false, Some(r"C:\out\Other.esp")).unwrap();
        assert!(other.contains("no longer Patch.esp"), "{other}");
        assert!(confirm_blocked(&w, false, false, None).is_some());
        w.failed = true;
        let spent = confirm_blocked(&w, false, false, active).unwrap();
        assert!(spent.contains("OK again"), "{spent}");
        w.failed = false;
        w.written = Some(json!({}));
        let once = confirm_blocked(&w, false, false, active).unwrap();
        assert!(once.contains("one write per OK"), "{once}");
    }

    /// A reply is taken only for the write in progress: the preview by its request, the
    /// write once, then the read-back; a failure ends the wait it belongs to.
    #[test]
    fn replies_join_the_write_in_progress() {
        let mut s = EditorState::default();
        let w = write("Patch.esp");
        let preview = w.preview.clone();
        s.write = Some(w);
        s.awaiting = Some(Slot::EditorPreview);
        s.receive_slot(
            Slot::EditorPreview,
            &Request::new("set", json!({})),
            &json!({}),
        );
        assert!(
            s.write.as_ref().unwrap().previewed.is_none(),
            "another request"
        );
        s.receive_slot(Slot::EditorPreview, &preview, &json!({"data": {}}));
        assert!(s.write.as_ref().unwrap().previewed.is_some());
        assert_eq!(s.awaiting, None);
        s.awaiting = Some(Slot::EditorWrite);
        let env = json!({"data": {"records": [{"form_id": "00000800",
                         "fields": [{"path": "FLTV", "before": 1.5, "after": 7.25}]}]}});
        // Only the write as sent is taken (L5): another `set` on the slot is not.
        let sent =
            set::write_request(&Request::new("set", json!({"x": 1, "dry_run": true}))).unwrap();
        s.write.as_mut().unwrap().write_req = Some(sent.clone());
        s.receive_slot(Slot::EditorWrite, &Request::new("set", json!({})), &env);
        assert!(s.write.as_ref().unwrap().written.is_none(), "another write");
        s.receive_slot(Slot::EditorWrite, &sent, &env);
        let w = s.write.as_ref().unwrap();
        assert!(w.written.is_some() && w.readback_due);
        let (status, note) = s.note.clone().unwrap();
        assert_eq!(status, Status::Pass);
        assert!(note.contains("FLTV 1.5 -> 7.25"), "{note}");
        s.receive_slot(
            Slot::EditorWrite,
            &Request::new("inspect", json!({})),
            &json!({}),
        );
        assert!(s.write.as_ref().unwrap().readback.is_none(), "another read");
        let read = s.write.as_ref().unwrap().readback_req.clone().unwrap();
        s.receive_slot(Slot::EditorWrite, &read, &json!({}));
        assert!(s.write.as_ref().unwrap().readback.is_some());
        // A failure ends only the wait it belongs to.
        s.awaiting = Some(Slot::EditorPreview);
        s.failed(Slot::EditorWrite, Some("x"));
        assert_eq!(s.awaiting, Some(Slot::EditorPreview));
        s.failed(
            Slot::EditorPreview,
            Some("refused by the read-only workbench: no"),
        );
        assert_eq!(s.awaiting, None);
        assert!(
            s.write.is_none(),
            "a failed preview leaves nothing to confirm"
        );
        assert!(s
            .note
            .clone()
            .unwrap()
            .1
            .contains("refused by the read-only workbench"));
        // The note names the command in progress (L2 of M7.E6).
        let mut u = write("Patch.esp");
        u.command = "revert";
        s.write = Some(u.clone());
        s.awaiting = Some(Slot::EditorPreview);
        s.failed(Slot::EditorPreview, None);
        let note = s.note.clone().unwrap().1;
        assert!(
            note.starts_with("the preview (`revert --dry-run`)"),
            "{note}"
        );
        s.write = Some(u);
        s.awaiting = Some(Slot::EditorWrite);
        s.failed(Slot::EditorWrite, None);
        let note = s.note.clone().unwrap().1;
        assert!(note.starts_with("the undo:"), "{note}");
        // A build says plan and build (L2 of M7.E5).
        let mut b = write("Patch.esp");
        b.command = "build";
        s.write = Some(b.clone());
        s.awaiting = Some(Slot::EditorPreview);
        s.failed(Slot::EditorPreview, None);
        let note = s.note.clone().unwrap().1;
        assert!(note.starts_with("the plan (`build --plan`)"), "{note}");
        s.write = Some(b);
        s.awaiting = Some(Slot::EditorWrite);
        s.failed(Slot::EditorWrite, None);
        let note = s.note.clone().unwrap().1;
        assert!(note.starts_with("the build:"), "{note}");
    }
}
