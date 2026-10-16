//! Help panel: the owner's QA material, read from the repo's own docs when the window
//! opens (or on Reload), so it is never a stale copy. Four sections, each one heading's
//! part of one file, rendered as plain Markdown (`crate::md`); code spans and code lines
//! are copyable. No logic: the docs are text, not game data, and nothing here runs a
//! command or writes anything.
//!
//! The checkout is found in this order: the UI prefs' `repo_dir`, `KITKRAFT_REPO_DIR`,
//! the folders above `kit-ui.exe` (a build in `target\release` sits inside the checkout),
//! the folders above the working directory. A checkout is a folder with `AGENTS.md` and
//! `docs\`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::md::{parse, section, Block};

/// Names the checkout when the prefs do not.
pub const REPO_ENV: &str = "KITKRAFT_REPO_DIR";

/// One part of one doc the Help tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpSource {
    /// The title above it.
    pub title: &'static str,
    /// Path in the checkout, `/`-separated.
    pub file: &'static str,
    /// The heading the part starts at (its text starts with this).
    pub heading: &'static str,
}

/// What the Help tab shows, in order.
pub const SOURCES: &[HelpSource] = &[
    HelpSource {
        title: "Owner next actions",
        file: "plan/STATUS.md",
        heading: "Owner next actions",
    },
    HelpSource {
        title: "Gun overhaul build 2 in game: install, player.additem, checklist C1..C14",
        file: "docs/overhaul/build-2.md",
        heading: "In game",
    },
    HelpSource {
        title: "First POI in game: setup, steps, persistence, report",
        file: "docs/poi/first-poi.md",
        heading: "In-game test plan",
    },
    HelpSource {
        title: "Persistence protocol",
        file: "docs/game-observations.md",
        heading: "Persistence protocol",
    },
];

/// Whether `dir` is a KitKraft checkout.
pub fn is_checkout(dir: &Path) -> bool {
    dir.join("AGENTS.md").is_file() && dir.join("docs").is_dir()
}

/// The checkout and how it was found, or `None`.
pub fn find_checkout(
    pref: Option<&str>,
    env: Option<OsString>,
    exe: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Option<(PathBuf, &'static str)> {
    let given = [
        (pref.map(PathBuf::from), "UI prefs"),
        (env.filter(|e| !e.is_empty()).map(PathBuf::from), REPO_ENV),
    ];
    for (dir, how) in given {
        if let Some(d) = dir.filter(|d| is_checkout(d)) {
            return Some((d, how));
        }
    }
    for (start, how) in [(exe, "above kit-ui"), (cwd, "above the working directory")] {
        if let Some(found) = start
            .as_deref()
            .and_then(|s| s.ancestors().take(12).find(|d| is_checkout(d)))
        {
            return Some((found.to_path_buf(), how));
        }
    }
    None
}

/// The checkout of this process (prefs, environment, exe, working directory).
pub fn find_checkout_here(pref: Option<&str>) -> Option<(PathBuf, &'static str)> {
    find_checkout(
        pref,
        std::env::var_os(REPO_ENV),
        std::env::current_exe().ok(),
        std::env::current_dir().ok(),
    )
}

/// A path in the checkout from its `/`-separated form.
pub fn in_checkout(root: &Path, file: &str) -> PathBuf {
    file.split('/')
        .fold(root.to_path_buf(), |p, part| p.join(part))
}

/// One loaded section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpDoc {
    /// What it is.
    pub source: HelpSource,
    /// The file read.
    pub path: PathBuf,
    /// The blocks, or why there are none.
    pub blocks: Result<Vec<Block>, String>,
}

/// Reads every [`SOURCES`] part from the checkout at `root`.
pub fn load(root: &Path) -> Vec<HelpDoc> {
    SOURCES
        .iter()
        .map(|s| {
            let path = in_checkout(root, s.file);
            let blocks = std::fs::read_to_string(&path)
                .map_err(|e| format!("{}: {e}", path.display()))
                .and_then(|text| {
                    section(&text, s.heading)
                        .map(|part| parse(&part))
                        .ok_or_else(|| {
                            format!("{}: no heading starting \"{}\"", path.display(), s.heading)
                        })
                });
            HelpDoc {
                source: *s,
                path,
                blocks,
            }
        })
        .collect()
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui::{self, CollapsingHeader, RichText, ScrollArea, TextEdit};

use crate::md::Inline;
use crate::panels::common::{action_bar, panel_head, Action, TabCtx};
use crate::theme::{self, Field, Status};

/// The Help tab's state: the checkout whose docs it shows and how it was found, the
/// checkout field, and the docs as last read.
#[derive(Default)]
pub(crate) struct HelpTab {
    pub checkout: Option<(PathBuf, &'static str)>,
    pub field: String,
    pub docs: Vec<HelpDoc>,
}

impl HelpTab {
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut TabCtx<'_>) {
        panel_head(
            ui,
            "Help",
            "The owner's QA material, read from the checkout's own docs when the window opens or Reload is clicked. Click any code to copy it. In-game steps run only on the MO2 profile KitKraft Dev with a disposable save, never on Default.",
        );
        action_bar(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Checkout");
                ui.add(
                    TextEdit::singleline(&mut self.field)
                        .desired_width(theme::width(ui, Field::Long)),
                );
                if ui
                    .button("Use this checkout")
                    .on_hover_text("remembered in the UI prefs file")
                    .clicked()
                {
                    ctx.actions
                        .push(Action::UseCheckout(self.field.trim().to_string()));
                }
                if ui.button("Reload").clicked() {
                    ctx.actions.push(Action::ReloadHelp);
                }
            })
        });
        match &self.checkout {
            Some((root, how)) => {
                ui.weak(format!("docs from {} (found {how})", root.display()));
            }
            None => {
                theme::status_line(
                    ui,
                    Status::Fail,
                    &format!(
                        "no KitKraft checkout found (a folder with AGENTS.md and docs): set {REPO_ENV} or type it above"
                    ),
                );
            }
        }
        for (i, doc) in self.docs.iter().enumerate() {
            CollapsingHeader::new(doc.source.title)
                .id_salt(("help", i))
                .default_open(true)
                .show(ui, |ui| {
                    ui.weak(format!(
                        "{} (from the heading \"{}\")",
                        doc.path.display(),
                        doc.source.heading
                    ));
                    match &doc.blocks {
                        Ok(blocks) => md_blocks(ui, i, blocks),
                        Err(e) => {
                            theme::status(ui, Status::Fail, e);
                        }
                    }
                });
        }
    }
}

/// Code in the Help tab: monospace, copied on click.
fn copy_code(ui: &mut egui::Ui, code: &str) {
    let r = ui
        .add(
            egui::Label::new(RichText::new(code).monospace().color(theme::code(ui)))
                .sense(egui::Sense::click()),
        )
        .on_hover_text("click to copy");
    if r.clicked() {
        ui.ctx().copy_text(code.to_string());
    }
}

/// Inline runs, laid out in a wrapping row.
fn md_inlines(ui: &mut egui::Ui, runs: &[Inline], strong: bool) {
    theme::inline_row(ui, |ui| {
        for r in runs {
            match r {
                Inline::Text(t) if strong => {
                    ui.label(RichText::new(t).strong());
                }
                Inline::Text(t) => {
                    ui.label(t);
                }
                Inline::Strong(t) => {
                    ui.label(RichText::new(t).strong());
                }
                Inline::Struck(t) => {
                    ui.label(RichText::new(t).strikethrough().weak());
                }
                Inline::Code(c) => copy_code(ui, c),
            }
        }
    });
}

/// One table cell, wrapped at `width`.
fn md_cell(ui: &mut egui::Ui, runs: &[Inline], width: f32, strong: bool) {
    ui.vertical(|ui| {
        ui.set_max_width(width);
        md_inlines(ui, runs, strong);
    });
}

/// A parsed doc section (`crate::md`), drawn plainly.
fn md_blocks(ui: &mut egui::Ui, doc: usize, blocks: &[Block]) {
    for (i, b) in blocks.iter().enumerate() {
        match b {
            Block::Heading { level, text } => {
                let t = crate::md::plain(text);
                theme::heading_gap(ui);
                match level {
                    1 | 2 => ui.label(RichText::new(t).heading()),
                    3 => ui.label(theme::subheading(ui, t)),
                    _ => ui.label(RichText::new(t).strong()),
                };
            }
            Block::Para(text) => md_inlines(ui, text, false),
            Block::Item {
                depth,
                marker,
                text,
            } => {
                ui.horizontal(|ui| {
                    theme::list_indent(ui, *depth);
                    ui.label(format!("{marker} "));
                    ui.vertical(|ui| md_inlines(ui, text, false));
                });
            }
            Block::Table { header, rows } => {
                let width = theme::doc_cell(ui, header.len());
                ScrollArea::horizontal()
                    .id_salt(("md-table-scroll", doc, i))
                    .show(ui, |ui| {
                        theme::table(ui, ("md-table", doc, i)).show(ui, |ui| {
                            for c in header {
                                md_cell(ui, c, width, true);
                            }
                            ui.end_row();
                            for row in rows {
                                for c in row {
                                    md_cell(ui, c, width, false);
                                }
                                ui.end_row();
                            }
                        });
                    });
            }
            Block::Code(lines) => {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    if ui.small_button("copy all").clicked() {
                        ui.ctx().copy_text(lines.join("\n"));
                    }
                    for l in lines {
                        copy_code(ui, l);
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::md::{plain, Inline};

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf()
    }

    fn code_spans(blocks: &[Block]) -> Vec<String> {
        let mut out = Vec::new();
        let mut take = |runs: &[Inline]| {
            for r in runs {
                if let Inline::Code(c) = r {
                    out.push(c.clone());
                }
            }
        };
        for b in blocks {
            match b {
                Block::Heading { text, .. } | Block::Para(text) | Block::Item { text, .. } => {
                    take(text);
                }
                Block::Table { header, rows } => {
                    for c in header.iter().chain(rows.iter().flatten()) {
                        take(c);
                    }
                }
                Block::Code(_) => {}
            }
        }
        out
    }

    /// The repo's own docs have every section the Help tab shows, with the parts the
    /// owner copies from: a doc edit that drops one fails here, not in the window.
    #[test]
    fn every_section_is_in_the_repo_docs() {
        let root = repo();
        assert!(is_checkout(&root), "{}", root.display());
        let docs = load(&root);
        assert_eq!(docs.len(), SOURCES.len());
        for d in &docs {
            let blocks = d.blocks.as_ref().unwrap_or_else(|e| panic!("{e}"));
            assert!(
                blocks.len() > 3,
                "{}: {} blocks",
                d.source.file,
                blocks.len()
            );
        }
        let owner = docs[0].blocks.as_ref().unwrap();
        assert!(owner
            .iter()
            .any(|b| matches!(b, Block::Table { rows, .. } if rows.len() > 3)));

        let overhaul = docs[1].blocks.as_ref().unwrap();
        let additem: Vec<String> = code_spans(overhaul)
            .into_iter()
            .filter(|c| c.starts_with("player.additem "))
            .collect();
        assert!(additem.len() >= 17, "{additem:?}");
        assert!(additem.contains(&"player.additem 0026D96C 1".to_string()));
        let checks: Vec<String> = overhaul
            .iter()
            .filter_map(|b| match b {
                Block::Table { rows, .. } => Some(rows),
                _ => None,
            })
            .flatten()
            .filter_map(|r| r.first().map(|c| plain(c)))
            .filter(|c| c.starts_with('C'))
            .collect();
        let want: Vec<String> = (1..=14).map(|n| format!("C{n}")).collect();
        assert_eq!(checks, want);

        let poi = docs[2].blocks.as_ref().unwrap();
        assert!(code_spans(poi).iter().any(|c| c.contains("player.moveto")));
        let persistence = docs[3].blocks.as_ref().unwrap();
        assert!(
            persistence
                .iter()
                .filter(|b| matches!(b, Block::Item { .. }))
                .count()
                >= 5
        );
    }

    #[test]
    fn the_checkout_is_found_by_prefs_env_exe_then_cwd() {
        let root = repo();
        let exe = root.join("target").join("release").join("kit-ui.exe");
        let elsewhere = std::env::temp_dir();
        let r = |p: Option<&str>, e: Option<OsString>, x: Option<PathBuf>, c: Option<PathBuf>| {
            find_checkout(p, e, x, c).map(|(_, how)| how)
        };
        let root_s = root.display().to_string();
        assert_eq!(r(Some(&root_s), None, None, None), Some("UI prefs"));
        assert_eq!(
            r(
                Some("Z:/nope"),
                Some(root.clone().into_os_string()),
                None,
                None
            ),
            Some(REPO_ENV)
        );
        assert_eq!(r(None, None, Some(exe), None), Some("above kit-ui"));
        assert_eq!(
            r(None, None, Some(elsewhere.clone()), Some(root.join("docs"))),
            Some("above the working directory")
        );
        assert_eq!(r(None, None, None, Some(elsewhere)), None);
    }
}
