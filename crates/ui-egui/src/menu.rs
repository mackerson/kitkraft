//! The menu bar (M7.M1), rendered from the catalog: one item per `CommandSpec`, in its
//! `menu_path`'s top-level menu, in catalog order. Toolkit-free model first (tested
//! without a window), then its drawing.
//!
//! Top-level order: the Creation Kit's menus in the order `docs/ck-parity.md` "Interface
//! parity" records, which is the owner's recollection, not an observation: the bar built
//! to it is **`built, unobserved`** (ADR 0012 Amendment 1, director decision 2026-10-16,
//! pending owner) and is never counted as parity until CK sitting 1 (M7.O2) observes the
//! CK's order; then KitKraft's own menus (Record, Tools, Archive, ...) in the
//! order the catalog first names them, before Help (so the CK menus keep their relative
//! order); a CK menu no command uses yet is still drawn, with a disabled line saying so.
//!
//! What an item does: it opens the panel or window that runs its command. It runs
//! nothing by itself, so it can never send a request the gate would refuse. Items whose
//! command the window cannot run are drawn disabled with the reason (deferred or planned;
//! writing outside ADR 0007 Amendment 3's scope; inside the scope but not yet permitted;
//! a read the window does not offer).

use crate::gate::{self, CommandSpec, CommandStatus};

/// The CK's top-level menus as `docs/ck-parity.md` records them today (first
/// "Interface parity" row): a recollection, not an observation (the row is
/// `built, unobserved`, ADR 0012 Amendment 1). A test
/// holds this list equal to that row, so a sitting that changes the row changes this.
pub const CK_MENU_ORDER: &[&str] = &[
    "File",
    "Edit",
    "View",
    "World",
    "NavMesh",
    "Character",
    "Gameplay",
    "Help",
];

/// Why a CK menu has no KitKraft command yet, by menu (`None`: none of its items is
/// known before sitting 1).
pub fn empty_menu_note(menu: &str) -> &'static str {
    match menu {
        "NavMesh" => "gap: navmesh generation is out of scope (ROADMAP M7); items unresearched",
        _ => {
            "no KitKraft command here yet; the CK's items are unresearched until sitting 1 (M7.O2)"
        }
    }
}

/// Where an enabled item takes the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A workbench tab, by its label (`app.rs` `TABS`); it may be floating as a window.
    Tab(&'static str),
    /// One of the CK-style windows (`crate::windows`).
    Window(crate::windows::WindowKind),
}

/// One menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// Canonical catalog id.
    pub id: &'static str,
    /// The text after the last `/` of `menu_path`.
    pub label: &'static str,
    /// The catalog's default shortcut (shown; bound by M7.M5).
    pub shortcut: Option<&'static str>,
    /// Where it goes, or why it is disabled.
    pub target: Result<Target, String>,
}

/// One top-level menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    pub name: String,
    /// One of [`CK_MENU_ORDER`] (its place is the recorded CK order: a recollection,
    /// `built, unobserved`).
    pub ck: bool,
    pub items: Vec<MenuItem>,
}

/// The panel or window that runs command `id`, or `None` when no part of the window
/// offers it.
fn panel_of(id: &str) -> Option<Target> {
    use crate::windows::WindowKind;
    Some(match id {
        "load" => Target::Tab("Load"),
        "profiles" => Target::Tab("Start"),
        "inspect" | "query" => Target::Tab("Records"),
        "refs" => Target::Window(WindowKind::UseInfo),
        "cell" => Target::Window(WindowKind::CellView),
        "planet" => Target::Tab("Planet"),
        "diff" | "conflicts" => Target::Tab("Diff"),
        "validate" | "roundtrip" => Target::Tab("Validate"),
        "coverage" => Target::Tab("Coverage"),
        "schema" => Target::Tab("Schema"),
        "ledger" => Target::Tab("Ledger"),
        "deploy" | "deploy.rollback" | "deploy.status" | "game.session" | "game.probe"
        | "game.stop" | "game.status" => Target::Tab("Cockpit"),
        _ => return None,
    })
}

/// The text a scope write gets while the gate does not permit it in the window yet.
fn scope_not_yet_permitted() -> String {
    "writes: in ADR 0007 Amendment 3's scope, not yet permitted in the window (M6.G4, M6.G5)"
        .to_string()
}

/// Where `spec`'s item goes, or why it is disabled.
pub fn target(spec: &CommandSpec) -> Result<Target, String> {
    match spec.status {
        CommandStatus::Implemented => {}
        other => return Err(other.note()),
    }
    let id = spec.id;
    let runs = if spec.writes {
        if gate::permitted(id).is_some() || gate::GUARDED.iter().any(|g| g.id == id) {
            true
        } else if gate::COCKPIT_SCOPE.contains(&id) {
            return Err(scope_not_yet_permitted());
        } else {
            return Err(
                "writes: not in the window's write scope (ADR 0007 Amendment 3); editing waits on Amendment 4 (M7.O4)"
                    .to_string(),
            );
        }
    } else {
        gate::ALLOWLIST.iter().any(|a| a.id == id)
    };
    if !runs {
        return Err(format!(
            "not on the window's read-only list (ADR 0007 Amendment 1); run `kit {}` or MCP",
            gate::command_words(spec)
        ));
    }
    panel_of(id).ok_or_else(|| {
        format!(
            "no panel shows it yet; run `kit {}` or MCP",
            gate::command_words(spec)
        )
    })
}

/// The menu model of `specs` (the catalog in the window).
pub fn build(specs: &[CommandSpec]) -> Vec<Menu> {
    let mut extra: Vec<&str> = Vec::new();
    for s in specs {
        let top = top_of(s.menu_path);
        if !CK_MENU_ORDER.contains(&top) && !extra.contains(&top) {
            extra.push(top);
        }
    }
    let (ck_head, ck_tail) = CK_MENU_ORDER.split_at(CK_MENU_ORDER.len().saturating_sub(1));
    let names = ck_head
        .iter()
        .map(|n| (*n, true))
        .chain(extra.iter().map(|n| (*n, false)))
        .chain(ck_tail.iter().map(|n| (*n, true)));
    names
        .map(|(name, ck)| Menu {
            name: name.to_string(),
            ck,
            items: specs
                .iter()
                .filter(|s| top_of(s.menu_path) == name)
                .map(|s| MenuItem {
                    id: s.id,
                    label: s.menu_path.rsplit('/').next().unwrap_or(s.label),
                    shortcut: s.shortcut,
                    target: target(s),
                })
                .collect(),
        })
        .collect()
}

/// The top-level menu of a `menu_path` (`Record/Inspect` -> `Record`).
pub fn top_of(path: &str) -> &str {
    path.split('/').next().unwrap_or(path)
}

// ---- drawing ---------------------------------------------------------------------------

use eframe::egui;

use crate::theme::{self, Status};
use crate::windows::{Shell, WindowKind};

/// What a click in the menu bar asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuPick {
    /// Open this target (a tab, or a CK-style window).
    Open(Target),
    /// Toggle a workbench tab between the central panel and a floating window.
    FloatTab(&'static str),
}

/// Draws the menu bar (one top panel above the tabs) and returns the click, if any.
/// `tabs` are the workbench's tab labels; `selected` names the inspected record (the
/// Edit dialog's and Use Info's subject).
pub fn menu_bar(
    ui: &mut egui::Ui,
    menus: &[Menu],
    shell: &mut Shell,
    tabs: &[&'static str],
    selected: Option<&str>,
) -> Option<MenuPick> {
    let mut pick = None;
    let frame = theme::top_frame(ui);
    egui::Panel::top("kitkraft-menu-bar")
        .frame(frame)
        .show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                shell.menu_ids.clear();
                for menu in menus {
                    let (resp, _) =
                        egui::containers::menu::MenuButton::new(menu.name.as_str()).ui(ui, |ui| {
                            ui.set_min_width(theme::combo_width(ui));
                            if menu.ck {
                                ui.weak(
                                    "CK menu: built, unobserved; place and items wait on sitting 1",
                                );
                            } else {
                                ui.weak("KitKraft menu");
                            }
                            ui.separator();
                            if menu.items.is_empty() {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(empty_menu_note(&menu.name)),
                                );
                            }
                            for item in &menu.items {
                                if let Some(p) = item_ui(ui, item) {
                                    pick = Some(p);
                                    ui.close();
                                }
                            }
                            if menu.name == "View" {
                                if let Some(p) = view_extras(ui, shell, tabs, selected) {
                                    pick = Some(p);
                                }
                            }
                        });
                    shell.menu_ids.push((menu.name.clone(), resp.id));
                }
            });
        });
    pick
}

fn item_ui(ui: &mut egui::Ui, item: &MenuItem) -> Option<MenuPick> {
    let mut button = egui::Button::new(item.label);
    if let Some(s) = item.shortcut {
        button = button.shortcut_text(s);
    }
    match &item.target {
        Ok(t) => {
            let resp = ui.add(button).on_hover_text(format!(
                "`{}`{}",
                item.id,
                if item.shortcut.is_some() {
                    "; the shortcut is the catalog's, shown only (bindings: M7.M5)"
                } else {
                    ""
                }
            ));
            resp.clicked().then_some(MenuPick::Open(*t))
        }
        Err(why) => {
            // Disabled is said in a word too (never color-only): why, in short.
            if item.shortcut.is_none() {
                button = button.right_text(short_reason(why));
            }
            ui.add_enabled(false, button)
                .on_disabled_hover_text(format!("`{}`: {why}", item.id));
            None
        }
    }
}

/// A disabled item's reason in one or two words, shown beside it.
pub fn short_reason(why: &str) -> &'static str {
    if why.starts_with("writes") {
        "writes"
    } else if why.starts_with("deferred") {
        "deferred"
    } else if why.starts_with("planned") {
        "planned"
    } else {
        "kit / MCP"
    }
}

/// The View menu's own part: the CK-style windows, then every tab as a floating window.
fn view_extras(
    ui: &mut egui::Ui,
    shell: &mut Shell,
    tabs: &[&'static str],
    selected: Option<&str>,
) -> Option<MenuPick> {
    let mut pick = None;
    ui.separator();
    ui.weak("Windows (CK-style, floating)");
    for kind in WindowKind::ALL {
        let open = shell.is_open(kind);
        let label = match (kind, selected) {
            (WindowKind::EditDialog | WindowKind::UseInfo, Some(s)) => {
                format!("{}: {s}", kind.title())
            }
            _ => kind.title().to_string(),
        };
        // The state is a word as well as a highlight (never color-only).
        let button =
            egui::Button::selectable(open, label).right_text(if open { "open" } else { "" });
        if ui.add(button).clicked() {
            shell.set_open(kind, !open);
        }
    }
    ui.separator();
    let sub = egui::Button::new("Workbench tabs as windows").right_text(">");
    egui::containers::menu::SubMenuButton::from_button(sub).ui(ui, |ui| {
        for tab in tabs {
            let floating = shell.is_floating(tab);
            let button = egui::Button::selectable(floating, *tab).right_text(if floating {
                "window"
            } else {
                ""
            });
            if ui.add(button).clicked() {
                pick = Some(MenuPick::FloatTab(tab));
            }
        }
    });
    if shell.is_open(WindowKind::EditDialog) && selected.is_none() {
        theme::status(
            ui,
            Status::NotRun,
            "no record selected: inspect one in Records",
        );
    }
    pick
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    fn menus() -> Vec<Menu> {
        build(gate::catalog())
    }

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf()
    }

    /// The "Interface parity" table of `docs/ck-parity.md` as (kind, element, maps to).
    fn parity_rows() -> Vec<(String, String, String, String)> {
        let text = std::fs::read_to_string(repo().join("docs").join("ck-parity.md")).unwrap();
        let section = text.split("## Interface parity").nth(1).unwrap();
        section
            .lines()
            .filter(|l| l.starts_with('|'))
            .map(|l| {
                l.trim_matches('|')
                    .split('|')
                    .map(|c| c.trim().to_string())
                    .collect::<Vec<_>>()
            })
            .filter(|c| c.len() >= 4 && c[0] != "Kind" && !c[0].starts_with('-'))
            .map(|c| (c[0].clone(), c[1].clone(), c[2].clone(), c[3].clone()))
            .collect()
    }

    #[test]
    fn every_catalog_command_lands_in_exactly_one_menu() {
        let menus = menus();
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for m in &menus {
            for i in &m.items {
                *seen.entry(i.id).or_default() += 1;
                assert_eq!(top_of(gate::find_command(i.id).unwrap().menu_path), m.name);
            }
        }
        for spec in gate::catalog() {
            assert_eq!(
                seen.get(spec.id),
                Some(&1),
                "{} ({})",
                spec.id,
                spec.menu_path
            );
            assert!(
                menus.iter().any(|m| m.name == top_of(spec.menu_path)),
                "{}: menu_path {} lands in no menu",
                spec.id,
                spec.menu_path
            );
        }
    }

    #[test]
    fn ck_menus_come_first_in_the_recorded_order_and_kitkraft_menus_before_help() {
        let menus = menus();
        let names: Vec<&str> = menus.iter().map(|m| m.name.as_str()).collect();
        let ck: Vec<&str> = names
            .iter()
            .copied()
            .filter(|n| CK_MENU_ORDER.contains(n))
            .collect();
        assert_eq!(ck, CK_MENU_ORDER);
        assert_eq!(&names[..7], &CK_MENU_ORDER[..7]);
        assert_eq!(names.last(), Some(&"Help"));
        for n in ["Record", "Tools", "Archive", "Papyrus", "Strings", "Game"] {
            let at = names.iter().position(|x| *x == n).unwrap();
            assert!((7..names.len() - 1).contains(&at), "{n} at {at}: {names:?}");
        }
    }

    /// No enabled item names a command the gate would refuse in every form; every
    /// disabled one says why, and writing commands outside the scope say "write scope".
    #[test]
    fn disabled_items_say_why_and_enabled_ones_pass_the_gate() {
        for m in menus() {
            for i in &m.items {
                let spec = gate::find_command(i.id).unwrap();
                match &i.target {
                    Ok(_) => assert!(
                        gate::ALLOWLIST
                            .iter()
                            .chain(gate::GUARDED)
                            .any(|a| a.id == i.id)
                            || gate::permitted(i.id).is_some(),
                        "{} is enabled but the gate refuses it",
                        i.id
                    ),
                    Err(why) => {
                        assert!(!why.trim().is_empty(), "{}", i.id);
                        if spec.writes && !gate::COCKPIT_SCOPE.contains(&i.id) {
                            assert!(why.contains("write scope"), "{}: {why}", i.id);
                        }
                    }
                }
            }
        }
        let set = menus()
            .into_iter()
            .flat_map(|m| m.items)
            .find(|i| i.id == "set")
            .unwrap();
        assert!(set.target.is_err());
        let ledger = menus()
            .into_iter()
            .flat_map(|m| m.items)
            .find(|i| i.id == "ledger")
            .unwrap();
        assert_eq!(ledger.target, Ok(Target::Tab("Ledger")));
    }

    /// The recorded CK menu order in `docs/ck-parity.md` is this file's order, and every
    /// `menu` row there names a menu the bar draws and maps to a catalog command that
    /// exists, a documented `gap:`, or "per item" (items unresearched until sitting 1).
    #[test]
    fn every_ck_menu_row_in_the_parity_doc_maps_to_the_bar() {
        let rows = parity_rows();
        let menus = menus();
        let names: Vec<&str> = menus.iter().map(|m| m.name.as_str()).collect();
        let order_row = rows
            .iter()
            .find(|r| r.0 == "menu" && r.1.starts_with("Main menu bar"))
            .expect("the main menu bar row");
        let recalled = order_row
            .1
            .split("recalls")
            .nth(1)
            .unwrap()
            .trim_end_matches(')')
            .split(',')
            .map(|s| s.trim().trim_start_matches("and ").trim())
            .collect::<Vec<_>>();
        assert_eq!(recalled, CK_MENU_ORDER, "update CK_MENU_ORDER with the row");
        let mut checked = 0;
        for (kind, element, maps, _status) in rows.iter().filter(|r| r.0 == "menu") {
            assert_eq!(kind, "menu");
            if element.starts_with("Main menu bar") {
                continue;
            }
            let menu = element.split_whitespace().next().unwrap();
            assert!(
                names.contains(&menu),
                "{element}: no {menu} menu in {names:?}"
            );
            let ids: Vec<&str> = maps
                .split('`')
                .skip(1)
                .step_by(2)
                .filter(|s| !s.ends_with(':'))
                .collect();
            for id in &ids {
                assert!(
                    gate::find_command(id).is_some(),
                    "{element}: `{id}` is no catalog command"
                );
            }
            assert!(
                maps.contains("gap:") || maps.starts_with("per item") || !ids.is_empty(),
                "{element}: maps to `{maps}`: a catalog id, `gap:` or per item"
            );
            checked += 1;
        }
        assert!(checked >= 8, "{checked} menu rows");
    }

    #[test]
    fn deferred_and_out_of_list_reads_are_disabled_with_their_reason() {
        let deferred = CommandSpec {
            status: CommandStatus::Deferred {
                reason: "later",
                milestone: "M9",
            },
            ..*gate::find_command("query").unwrap()
        };
        assert_eq!(target(&deferred), Err("deferred to M9: later".into()));
        let ba2 = target(gate::find_command("ba2.list").unwrap()).unwrap_err();
        assert!(ba2.contains("read-only list"), "{ba2}");
        assert!(ba2.contains("kit ba2 list"), "{ba2}");
        // M6.G5: `game.probe` is permitted now, so it runs in the Cockpit like the other
        // game commands.
        assert_eq!(
            target(gate::find_command("game.probe").unwrap()),
            Ok(Target::Tab("Cockpit"))
        );
        // All six scope ids being permitted, no catalog spec reaches the
        // not-yet-permitted branch; a scope write the gate did not permit would get
        // this text (the message's function, called directly).
        let message = scope_not_yet_permitted();
        assert!(message.contains("not yet permitted"), "{message}");
        assert!(message.contains("Amendment 3"), "{message}");
        // M6.G4: the permitted game commands and the status read all live in the Cockpit.
        for id in ["game.session", "game.probe", "game.stop", "game.status"] {
            assert_eq!(panel_of(id), Some(Target::Tab("Cockpit")), "{id}");
        }
    }

    #[test]
    fn the_cell_view_is_a_window_and_the_cell_item_opens_it() {
        use crate::windows::WindowKind;
        assert!(WindowKind::ALL.contains(&WindowKind::CellView));
        assert_eq!(panel_of("cell"), Some(Target::Window(WindowKind::CellView)));
    }
}
