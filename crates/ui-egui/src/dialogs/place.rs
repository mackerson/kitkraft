//! `place` (M7.E4, pipeline E): a reference of a base object placed into the Cell
//! View's cell at a typed position, previewed and confirmed per click.
//!
//! Step 0 of sprint 2026-11-02 declared this file and the Cell View's one call to
//! [`cell_view_bar`] (`windows/cell_view.rs`), whose answer the shell turns into
//! `Action::PlaceInCell`; the editor (`super::editor`) opens the Place form on it
//! ([`form_ui`]). The form's labels are KitKraft's, not observed.

use eframe::egui;
use serde_json::{json, Value};

use crate::gate::Request;
use crate::theme::{self, Status};

/// The preview of placing a reference of `base` in `cell` at `position` into `into`, the
/// active file: `place --dry-run` (lane D round 5), which writes and validates a staged
/// copy and leaves the active file untouched; the gate passes it like a read.
pub(crate) fn preview_request(base: &str, cell: &str, position: [f64; 3], into: &str) -> Request {
    Request::new(
        "place",
        json!({"base": base, "cell": cell, "position": position, "into": into, "dry_run": true}),
    )
}

/// The confirmed placement of a previewed `place`: the preview's arguments with `dry_run`
/// dropped, built once per confirm click; the only confirmed `place` in the crate (the
/// gate's per-entry source scan holds it). `None` when `preview` is not a `place` dry run.
pub(crate) fn write_request(preview: &Request) -> Option<Request> {
    if preview.id != "place" || preview.args.get("dry_run") != Some(&Value::Bool(true)) {
        return None;
    }
    let mut args = preview.args.clone();
    args.as_object_mut()?.remove("dry_run");
    Some(Request::confirmed("place", args))
}

/// What the place bar asks for: place `base` into `cell` (formrefs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceAsk {
    pub base: String,
    pub cell: String,
}

/// The Cell View's place bar, drawn above the references of `cell`: "Place <inspected>
/// here", with the shared selection (the record inspected) as the base to place.
/// Disabled without a selected record. Its answer becomes `Action::PlaceInCell`.
pub fn cell_view_bar(ui: &mut egui::Ui, cell: &str, inspected: Option<&str>) -> Option<PlaceAsk> {
    let base = inspected
        .map(str::trim)
        .filter(|b| b.contains(':') && *b != cell.trim());
    let label = match base {
        Some(b) => format!("Place {b} here"),
        None => "Place here".to_string(),
    };
    let clicked = ui
        .add_enabled(base.is_some(), egui::Button::new(label))
        .on_hover_text("a new reference of the inspected record in this cell, into the active file: `place --dry-run` first, then a confirm")
        .on_disabled_hover_text("inspect the base to place first (Records, or a Use Info row)")
        .clicked();
    match (clicked, base) {
        (true, Some(b)) => Some(PlaceAsk {
            base: b.to_string(),
            cell: cell.trim().to_string(),
        }),
        _ => None,
    }
}

/// The Place form's fields: the base, the cell and the typed position (X, Y, Z as text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaceForm {
    pub base: String,
    pub cell: String,
    pub xyz: [String; 3],
}

impl PlaceForm {
    /// The form on `base` and `cell`, at 0, 0, 0.
    pub(crate) fn open(base: &str, cell: &str) -> Self {
        Self {
            base: base.trim().to_string(),
            cell: cell.trim().to_string(),
            xyz: ["0".to_string(), "0".to_string(), "0".to_string()],
        }
    }

    /// The typed position, or which coordinate is not a number.
    pub(crate) fn position(&self) -> Result<[f64; 3], String> {
        let mut out = [0.0; 3];
        for ((slot, text), axis) in out.iter_mut().zip(&self.xyz).zip(["X", "Y", "Z"]) {
            *slot = text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("position {axis}: `{}` is not a number", text.trim()))?;
        }
        Ok(out)
    }
}

/// What the Place form's buttons asked for this frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FormAsk {
    pub preview: bool,
    pub close: bool,
}

/// The position a `place` request writes, as the confirm box shows it: "at (x, y, z)".
pub(crate) fn position_line(request: &Request) -> String {
    let axes: Vec<String> = request
        .args
        .get("position")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|v| {
                    v.as_f64()
                        .map_or_else(|| "?".to_string(), |f| f.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    format!("at ({})", axes.join(", "))
}

/// The Place form: the base, the cell, the position typed as X, Y and Z (game units),
/// the active file it goes into, and Preview and Close. Nothing is sent from here.
/// `locked` while a preview of it awaits its confirm: the position cannot change under
/// the confirm box (M7.E4 review M1); closing the box unlocks it.
pub(crate) fn form_ui(
    ui: &mut egui::Ui,
    f: &mut PlaceForm,
    active_file: Option<&str>,
    locked: bool,
) -> FormAsk {
    let mut ask = FormAsk::default();
    ui.horizontal(|ui| {
        ui.label("Base");
        ui.monospace(&f.base);
    });
    ui.horizontal(|ui| {
        ui.label("Cell");
        ui.monospace(&f.cell);
    });
    ui.add_enabled_ui(!locked, |ui| {
        ui.horizontal(|ui| {
            for (axis, text) in ["X", "Y", "Z"].iter().zip(f.xyz.iter_mut()) {
                let label = ui.label(*axis);
                ui.add(
                    egui::TextEdit::singleline(text)
                        .desired_width(theme::width(ui, theme::Field::Tiny)),
                )
                .labelled_by(label.id);
            }
        });
    });
    let position = f.position();
    if let Err(e) = &position {
        theme::status_line(ui, Status::Fail, e);
    }
    match active_file.filter(|a| !a.trim().is_empty()) {
        Some(a) => {
            ui.label(format!("into the active file: {a}"));
        }
        None => {
            theme::status_line(
                ui,
                Status::NotRun,
                "no active file: choose one in File > Data... (Set as Active File)",
            );
        }
    }
    ui.horizontal(|ui| {
        ask.preview = ui
            .add_enabled(position.is_ok() && !locked, egui::Button::new("Preview"))
            .on_disabled_hover_text(if locked {
                "previewed: close the confirm box to change the position"
            } else {
                "type a number for each of X, Y and Z"
            })
            .on_hover_text(
                "`place --dry-run`: the reference and the cell it carries, written and validated on a staged copy; nothing written",
            )
            .clicked();
        ask.close = ui.button("Close").clicked();
    });
    ask
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::check;

    #[test]
    fn the_placement_is_the_preview_without_dry_run_and_confirmed() {
        let preview = preview_request(
            "World.esp:001000",
            "World.esp:001001",
            [1.0, 2.0, 3.0],
            r"C:\out\Patch.esp",
        );
        assert!(!preview.is_confirmed());
        assert!(check(&preview).is_ok(), "the dry run passes like a read");
        assert_eq!(preview.args["position"], json!([1.0, 2.0, 3.0]));
        let write = write_request(&preview).unwrap();
        assert!(write.is_confirmed());
        assert_eq!(write.id, "place");
        assert_eq!(write.args.get("dry_run"), None);
        assert!(check(&write).is_ok());
        assert!(write_request(&write).is_none());
        assert!(write_request(&Request::new("set", json!({"dry_run": true}))).is_none());
    }

    /// The typed position is three finite numbers; the first that is not names its axis.
    #[test]
    fn the_position_is_three_numbers() {
        let mut f = PlaceForm::open(" World.esp:001000 ", "World.esp:001001");
        assert_eq!(f.base, "World.esp:001000");
        assert_eq!(f.position(), Ok([0.0, 0.0, 0.0]));
        f.xyz = ["1.5".into(), " -2 ".into(), "3e2".into()];
        assert_eq!(f.position(), Ok([1.5, -2.0, 300.0]));
        f.xyz[1] = "up".into();
        assert!(f.position().unwrap_err().contains("position Y"));
        f.xyz[1] = "NaN".into();
        assert!(f.position().is_err());
    }
}
