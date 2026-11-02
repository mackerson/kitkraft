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

use std::collections::BTreeMap;

use eframe::egui::{self, ScrollArea};
use serde_json::{Map, Value};

use super::{edit, revert, set};
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
}

/// The editor's last confirmed `set`, the one its undo may invert.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LastWrite {
    form: String,
    into: String,
    /// The write added the record to the active file (`outcome: inserted`), so dropping
    /// the override restores the file exactly.
    inserted: bool,
}

/// The editor's state (kept in `crate::windows::Shell`).
#[derive(Default)]
pub struct EditorState {
    /// The last confirmed `set` not yet undone.
    last: Option<LastWrite>,
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

    /// `Action::PlaceInCell`: start placing `base` into `cell` (formrefs).
    pub fn place(&mut self, _base: &str, _cell: &str) {}

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
                    inserted: write_outcome(env) == Some("inserted"),
                });
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
        let what = match slot {
            Slot::EditorPreview => "the preview (`set --dry-run`)",
            Slot::EditorWrite => "the write",
            _ => "the request",
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

/// What a `set` envelope says it did to the record in the active file: `inserted`,
/// `replaced` or `moved`.
fn write_outcome(env: &Value) -> Option<&str> {
    env.pointer("/data/records")?
        .as_array()?
        .iter()
        .find(|r| r.get("parent") != Some(&Value::Bool(true)))?
        .get("outcome")?
        .as_str()
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
    if env.pointer("/data/would_write") != Some(&Value::Bool(true)) {
        return Some(
            "the dry run says a real write would be refused (validate finds errors on the written record)"
                .to_string(),
        );
    }
    if active_file != Some(w.into.as_str()) {
        return Some(format!(
            "the active file is no longer {}: OK again to preview into the active file",
            file_name(&w.into)
        ));
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
    // The edit dialog's actions, until the shell takes them from it directly.
    let mut out = edit::take_dialog_actions(ctx);
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
    });
    state.awaiting = Some(Slot::EditorPreview);
    out.push(Action::Submit(Slot::EditorPreview, preview));
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
    if !last.inserted {
        state.note = Some((
            Status::Fail,
            format!(
                "undo refused: {file} already held {form} before the last write, and `revert` would drop that earlier version too (it would undo more than one write); `kit revert` drops the whole override"
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
                    } else {
                        format!("set {} into {}", w.form, file_name(&w.into))
                    };
                    ui.strong(what);
                    ui.weak(&w.into);
                });
                match &w.previewed {
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(format!(
                                "previewing: `{} --dry-run` on a staged copy",
                                w.command
                            ));
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
            let confirmed = if w.command == "revert" {
                revert::write_request(&w.preview)
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
    if command == "revert" {
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
        ui.label(theme::subheading(ui, "Fields changed (dry run)"));
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
    let would = data.get("would_write") == Some(&Value::Bool(true));
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
            .unwrap()
            .1
            .contains("refused by the read-only workbench"));
    }
}
