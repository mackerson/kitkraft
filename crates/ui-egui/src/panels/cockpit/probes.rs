//! What a session's probes found (M6.G5): the probe rows and the screenshots
//! `game.status` lists. The view model above the `// ---- drawing` line is
//! toolkit-free — it turns the reply into rows, each with its verdict and its
//! screenshots resolved against the paths the reply listed — so it is tested
//! without a window. Below that line the rows go into a table and the selected
//! row's screenshots are drawn as images.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::status_of;
use crate::theme::Status;

/// A screenshot one probe row named, resolved against the paths `game.status`
/// listed: pass when the reply listed it and the file is on disk, `inconclusive`
/// otherwise. A screenshot a row names is never left out — not listed, or its
/// listed path missing on disk, it is still shown, with its reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeShot {
    /// The file name the row named.
    pub file: String,
    /// The path the reply listed it under, if it listed it.
    pub path: Option<PathBuf>,
    /// Pass when listed and on disk; `inconclusive` otherwise.
    pub status: Status,
    /// Why it is not pass ("not listed", "listed by game.status but missing on
    /// disk"); empty when it is pass.
    pub reason: String,
}

/// One probe row of a session's status, as `game.status` reported it: id, title,
/// forms, expected, seen, the verdict word and reason, and the row's screenshots
/// resolved. A re-run is a row too, marked by [`Self::rerun`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeRow {
    /// The row id (`"T-02"`).
    pub id: String,
    /// The title the probe plan gave the row.
    pub title: String,
    /// The forms the row works on.
    pub forms: Vec<String>,
    /// What the row expected.
    pub expected: String,
    /// What the row saw.
    pub seen: String,
    /// The verdict word (`pass`, `fail`, `inconclusive`, `not-run`).
    pub verdict: String,
    /// The verdict's reason, when it has one.
    pub reason: String,
    /// The verdict's status, from [`super::status_of`].
    pub status: Status,
    /// The row is a re-run (a row of the status's `reruns`).
    pub rerun: bool,
    /// The row's screenshots, resolved against the paths the reply listed.
    pub shots: Vec<ProbeShot>,
}

/// Every probe row of a `game.status` reply, in the order the reply gives them:
/// the rows first, then the re-runs, each marked as one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProbeRows {
    /// The rows, in the reply's order.
    pub rows: Vec<ProbeRow>,
}

impl ProbeRows {
    /// One row per probe of the reply (re-runs after, marked as re-runs): row id,
    /// title, forms, expected, seen, the verdict word and reason, and the row's
    /// screenshots resolved against the paths the reply listed. `exists` says
    /// which of the listed paths are on disk.
    pub fn from_status(env: &Value, exists: &dyn Fn(&Path) -> bool) -> Self {
        let data = env.get("data");
        let listed = listed_shots(data);
        let mut rows = Vec::new();
        push_rows(&mut rows, data, false, &listed, exists);
        push_rows(&mut rows, data, true, &listed, exists);
        Self { rows }
    }
}

/// The rows or re-runs of the reply, one [`ProbeRow`] per object in the list.
fn push_rows(
    rows: &mut Vec<ProbeRow>,
    data: Option<&Value>,
    rerun: bool,
    listed: &HashMap<String, PathBuf>,
    exists: &dyn Fn(&Path) -> bool,
) {
    let Some(list) = data
        .and_then(|d| d.get(if rerun { "reruns" } else { "rows" }))
        .and_then(Value::as_array)
    else {
        return;
    };
    for v in list {
        if v.is_object() {
            rows.push(row_of(v, rerun, listed, exists));
        }
    }
}

/// The screenshots the reply listed, by file name: each `data.screenshots`
/// object's `file` to its `path`.
fn listed_shots(data: Option<&Value>) -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();
    let Some(shots) = data
        .and_then(|d| d.get("screenshots"))
        .and_then(Value::as_array)
    else {
        return out;
    };
    for s in shots {
        let (Some(f), Some(p)) = (
            s.get("file").and_then(Value::as_str),
            s.get("path").and_then(Value::as_str),
        ) else {
            continue;
        };
        out.insert(f.to_string(), PathBuf::from(p));
    }
    out
}

/// One row of the reply: its fields as plain values (a missing one is empty,
/// never a skipped row), its verdict through [`super::status_of`], and its
/// screenshots resolved against the listed paths.
fn row_of(
    v: &Value,
    rerun: bool,
    listed: &HashMap<String, PathBuf>,
    exists: &dyn Fn(&Path) -> bool,
) -> ProbeRow {
    let str = |k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let forms = v
        .get("forms")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        });
    let verdict = v
        .get("verdict")
        .and_then(|o| o.get("status"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let reason = v
        .get("verdict")
        .and_then(|o| o.get("reason"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let shots = v
        .get("screenshots")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|f| shot_of(f, listed, exists))
                .collect()
        });
    let status = status_of(&verdict);
    ProbeRow {
        id: str("row"),
        title: str("title"),
        forms,
        expected: str("expected"),
        seen: str("seen"),
        verdict,
        reason,
        status,
        rerun,
        shots,
    }
}

/// One screenshot a row named, resolved: the path the reply listed it under,
/// and the status it is shown with.
fn shot_of(
    file: &str,
    listed: &HashMap<String, PathBuf>,
    exists: &dyn Fn(&Path) -> bool,
) -> ProbeShot {
    let Some(path) = listed.get(file) else {
        return ProbeShot {
            file: file.to_string(),
            path: None,
            status: Status::Inconclusive,
            reason: "not listed".to_string(),
        };
    };
    if exists(path) {
        ProbeShot {
            file: file.to_string(),
            path: Some(path.clone()),
            status: Status::Pass,
            reason: String::new(),
        }
    } else {
        ProbeShot {
            file: file.to_string(),
            path: Some(path.clone()),
            status: Status::Inconclusive,
            reason: "listed by game.status but missing on disk".to_string(),
        }
    }
}

/// Decode a screenshot's bytes into an image the window can draw: PNG only (the
/// format the session writes), into an `egui::ColorImage`. The error is the
/// decoder's reason, so a file that does not decode is shown `inconclusive`
/// with the reason, never hidden.
pub fn decode_screenshot(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let (w, h) = (img.width(), img.height());
    let rgba = img.to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [
            usize::try_from(w).unwrap_or_default(),
            usize::try_from(h).unwrap_or_default(),
        ],
        rgba.as_raw(),
    ))
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self};

use crate::panels::common::{self, Action, TabCtx};
use crate::theme::{self, View};

/// What the probes' drawing keeps between frames: the session the selection and
/// the textures are for (a new session starts both over), the selected row, and
/// the textures loaded per screenshot path.
#[derive(Default)]
pub struct ProbeUi {
    /// The session the current selection and textures are for.
    session: Option<String>,
    /// The selected row (by index into [`ProbeRows`]); `None` before a selection.
    selected: Option<usize>,
    /// Each screenshot path loaded once: the texture, or why it does not draw.
    shots: HashMap<PathBuf, Result<egui::TextureHandle, String>>,
}

impl ProbeUi {
    /// The selected row, starting the selection and the textures over when the
    /// session changes.
    fn selection(&mut self, session: Option<&str>) -> Option<usize> {
        let key = session.map(str::to_string);
        if self.session != key {
            self.session = key;
            self.selected = None;
            self.shots.clear();
        }
        self.selected
    }
}

/// The session's probe rows and, below them, the selected row's screenshots: the
/// rows in a table with the status component and an inspect link per form, the
/// screenshots scaled to the panel width.
pub(crate) fn rows_ui(
    ui: &mut egui::Ui,
    ctx: &mut TabCtx<'_>,
    session: Option<&str>,
    rows: &ProbeRows,
    probe: &mut ProbeUi,
) {
    let Some(session) = session else {
        return;
    };
    ui.label(theme::subheading(ui, "Probe rows (game.status)"));
    let selected = probe.selection(Some(session));
    let busy = ctx.busy;
    common::virtual_table(
        ui,
        "cockpit-session-probes",
        &[
            "row", "title", "forms", "expected", "seen", "verdict", "reason",
        ],
        rows.rows.len(),
        View::Medium,
        |row, i| {
            let Some(r) = rows.rows.get(i) else {
                return;
            };
            row.cell(|ui| {
                let label = if r.rerun {
                    format!("{} (re-run)", r.id)
                } else {
                    r.id.clone()
                };
                if ui.selectable_label(selected == Some(i), &label).clicked() {
                    probe.selected = if selected == Some(i) { None } else { Some(i) };
                }
            });
            row.cell(|ui| ui.label(&r.title));
            row.cell(|ui| {
                ui.horizontal_wrapped(|ui| {
                    for f in &r.forms {
                        if common::link(ui, !busy, f.as_str())
                            .on_hover_text("inspect")
                            .clicked()
                        {
                            ctx.actions.push(Action::Inspect(f.clone()));
                        }
                    }
                });
            });
            row.cell(|ui| ui.label(&r.expected));
            row.cell(|ui| ui.label(&r.seen));
            row.cell(|ui| {
                let resp = theme::status(ui, r.status, &r.verdict);
                if r.reason.is_empty() {
                    resp.on_hover_text("the verdict game.status gave the row")
                } else {
                    resp.on_hover_text(&r.reason)
                }
            });
            row.cell(|ui| ui.weak(&r.reason));
        },
    );
    let sel = selected.and_then(|i| rows.rows.get(i));
    let Some(r) = sel else {
        return;
    };
    theme::gap(ui);
    ui.label(theme::subheading(ui, format!("screenshots of {}", r.id)));
    if r.shots.is_empty() {
        ui.weak("none taken");
    } else {
        for shot in &r.shots {
            screenshot_ui(ui, shot, probe);
        }
    }
}

/// One screenshot of the selected row: the image at the path the reply listed
/// (opened only because `game.status` returned it), or, when it cannot be
/// opened or decoded, its `inconclusive` status with the reason.
fn screenshot_ui(ui: &mut egui::Ui, shot: &ProbeShot, probe: &mut ProbeUi) {
    let Some(path) = &shot.path else {
        theme::status_line(ui, shot.status, &shot.reason);
        return;
    };
    match shot_image(ui, path, &mut probe.shots) {
        Ok(()) => ui.weak(&shot.file),
        Err(why) => theme::status_line(ui, Status::Inconclusive, &why),
    };
}

/// The image at `path`: opened, decoded as PNG, and loaded into a texture
/// (the result cached per path, so each file is read and decoded once),
/// scaled to the panel width. The error is why it does not draw.
fn shot_image(
    ui: &mut egui::Ui,
    path: &Path,
    cache: &mut HashMap<PathBuf, Result<egui::TextureHandle, String>>,
) -> Result<(), String> {
    let loaded = cache.get(path).cloned().unwrap_or_else(|| {
        let read = std::fs::read(path).map_err(|e| format!("cannot open {}: {e}", path.display()));
        let result = read.and_then(|bytes| {
            decode_screenshot(&bytes).map(|img| {
                ui.ctx().load_texture(
                    format!("cockpit-probe-shot {}", path.display()),
                    img,
                    Default::default(),
                )
            })
        });
        cache.insert(path.to_path_buf(), result.clone());
        result
    });
    let handle = loaded?;
    ui.add(egui::Image::new(&handle).max_width(ui.available_width()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{decode_screenshot, ProbeRows};
    use crate::theme::Status;
    use serde_json::{json, Value};

    /// The canned `game.status` reply the view model is tested on: three rows
    /// (`T-01` pass, `T-02` fail with a reason and one screenshot, `T-03` manual
    /// `inconclusive` with one screenshot) and one re-run, the screenshots the
    /// reply lists, and the files on disk.
    fn status_env() -> Value {
        json!({
            "command": "game.status",
            "data": {
                "session": "2026-10-27-t01",
                "phase": "probing",
                "current_row": "T-02",
                "rows": [
                    {
                        "row": "T-01",
                        "title": "the load screen shows",
                        "forms": ["Game.esm:000001"],
                        "commands": ["tcl GetFormEditorID 000001"],
                        "expected": "loaded",
                        "seen": "loaded",
                        "verdict": { "status": "pass" },
                        "screenshots": []
                    },
                    {
                        "row": "T-02",
                        "title": "the cell holds the weapon",
                        "forms": ["X.esm:001000"],
                        "commands": ["tcl GetFormEditorID 001000"],
                        "expected": "001000",
                        "seen": "000000",
                        "verdict": { "status": "fail", "reason": "expected 001000, saw 000000" },
                        "screenshots": ["T-02-1.png"]
                    },
                    {
                        "row": "T-03",
                        "title": "the owner confirms the cell",
                        "forms": [],
                        "commands": [],
                        "expected": "the owner says so",
                        "seen": "a screenshot for the owner",
                        "verdict": { "status": "inconclusive", "reason": "manual: the owner confirms" },
                        "screenshots": ["T-03-1.png"]
                    }
                ],
                "reruns": [
                    {
                        "row": "T-02",
                        "title": "the cell holds the weapon",
                        "forms": ["X.esm:001000"],
                        "commands": ["tcl GetFormEditorID 001000"],
                        "expected": "001000",
                        "seen": "001000",
                        "verdict": { "status": "pass" },
                        "screenshots": []
                    }
                ],
                "screenshots": [
                    { "file": "T-02-1.png", "path": "observe/s/T-02-1.png" },
                    { "file": "T-03-1.png", "path": "observe/s/T-03-1.png" }
                ],
                "events": ["T-01 done", "T-02 done"],
                "elapsed_secs": 42,
                "report": null
            }
        })
    }

    /// The two files the canned reply lists, on disk.
    fn exists_on_disk() -> fn(&std::path::Path) -> bool {
        |p: &std::path::Path| {
            p == std::path::Path::new("observe/s/T-02-1.png")
                || p == std::path::Path::new("observe/s/T-03-1.png")
        }
    }

    #[test]
    fn probe_rows_and_screenshots_come_from_game_status() {
        let rows = ProbeRows::from_status(&status_env(), &exists_on_disk());
        // Every row is there, in the reply's order, and the re-run is marked.
        assert_eq!(rows.rows.len(), 4, "{:?}", rows.rows);
        let ids: Vec<&str> = rows.rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["T-01", "T-02", "T-03", "T-02"]);
        assert!(rows.rows.iter().take(3).all(|r| !r.rerun));
        assert!(rows.rows.get(3).unwrap().rerun);
        // Verdicts and reasons, mapped to the status component's statuses.
        let t1 = rows.rows.get(0).unwrap();
        assert_eq!(
            (t1.verdict.as_str(), t1.reason.as_str(), t1.status),
            ("pass", "", Status::Pass)
        );
        let t2 = rows.rows.get(1).unwrap();
        assert_eq!(
            (t2.verdict.as_str(), t2.reason.as_str(), t2.status),
            ("fail", "expected 001000, saw 000000", Status::Fail)
        );
        assert_eq!(t2.forms, vec!["X.esm:001000"]);
        assert_eq!(
            (t2.expected.as_str(), t2.seen.as_str()),
            ("001000", "000000")
        );
        let t3 = rows.rows.get(2).unwrap();
        assert_eq!(
            (t3.verdict.as_str(), t3.reason.as_str(), t3.status),
            (
                "inconclusive",
                "manual: the owner confirms",
                Status::Inconclusive
            )
        );
        let rr = rows.rows.get(3).unwrap();
        assert_eq!(
            (rr.verdict.as_str(), rr.reason.as_str(), rr.status),
            ("pass", "", Status::Pass)
        );
        // Each screenshot the rows name resolves to the path the reply listed it under.
        let s2 = t2.shots.first().unwrap();
        assert_eq!(s2.file, "T-02-1.png");
        assert_eq!(
            s2.path.as_deref(),
            Some(std::path::Path::new("observe/s/T-02-1.png"))
        );
        assert_eq!(s2.status, Status::Pass);
        assert_eq!(s2.reason, "");
        let s3 = t3.shots.first().unwrap();
        assert_eq!(s3.file, "T-03-1.png");
        assert_eq!(
            s3.path.as_deref(),
            Some(std::path::Path::new("observe/s/T-03-1.png"))
        );
        assert_eq!(s3.status, Status::Pass);
        assert!(t1.shots.is_empty() && rr.shots.is_empty());
        for r in &rows.rows {
            for s in &r.shots {
                assert_eq!(
                    s.path.as_deref(),
                    Some(match s.file.as_str() {
                        "T-02-1.png" => std::path::Path::new("observe/s/T-02-1.png"),
                        "T-03-1.png" => std::path::Path::new("observe/s/T-03-1.png"),
                        other => panic!("unlisted screenshot {other}"),
                    })
                );
            }
        }
    }

    #[test]
    fn a_missing_screenshot_is_inconclusive_never_hidden() {
        // The same reply, with `T-03`'s screenshot no longer listed (`T-02`'s
        // listed file is missing on disk): both are still shown, as
        // `inconclusive` with their reasons, never left out.
        let mut env = status_env();
        env["data"]["screenshots"] = json!([
            { "file": "T-02-1.png", "path": "observe/s/T-02-1.png" }
        ]);
        let rows = ProbeRows::from_status(&env, &|_: &std::path::Path| false);
        let t2 = rows.rows.get(1).unwrap();
        let missing = t2.shots.first().unwrap();
        assert_eq!(missing.file, "T-02-1.png");
        assert_eq!(
            missing.path.as_deref(),
            Some(std::path::Path::new("observe/s/T-02-1.png"))
        );
        assert_eq!(missing.status, Status::Inconclusive);
        assert_eq!(missing.reason, "listed by game.status but missing on disk");
        let t3 = rows.rows.get(2).unwrap();
        let unlisted = t3.shots.first().unwrap();
        assert_eq!(unlisted.file, "T-03-1.png");
        assert_eq!(unlisted.path, None);
        assert_eq!(unlisted.status, Status::Inconclusive);
        assert_eq!(unlisted.reason, "not listed");
        // A row that names no screenshot shows none; none are hidden anywhere.
        assert!(rows.rows.iter().all(|r| {
            r.shots
                .iter()
                .all(|s| s.path.is_some() || s.reason == "not listed")
        }));
    }

    #[test]
    fn screenshots_decode_or_say_why() {
        // A 2 x 2 PNG, encoded in the test with the `image` crate.
        let px = [
            [1, 2, 3, 255],
            [4, 5, 6, 255],
            [7, 8, 9, 255],
            [10, 11, 12, 255],
        ];
        let png = crate::shots::png(2, 2, &px).expect("the 2 x 2 png encodes");
        let img = decode_screenshot(&png).expect("the test's png decodes");
        assert_eq!((img.width(), img.height()), (2, 2));
        // The bytes the engine's fake host writes are not a PNG: an Err with a reason.
        let err = decode_screenshot(b"\x89PNG fake").expect_err("it must not decode");
        assert!(!err.is_empty(), "the error says why");
    }
}
