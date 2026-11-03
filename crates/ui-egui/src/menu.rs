//! The menu bar (M7.M1), rendered from the catalog: one item per `CommandSpec`, in its
//! `menu_path`'s top-level menu, in catalog order. Toolkit-free model first (tested
//! without a window), then its drawing.
//!
//! Items of a CK menu (M7.M1 rest): the `menu item` rows of `docs/ck-parity.md` first, in
//! the observed order ([`CK_ITEMS`], a test holds it equal to the rows), each mapped to the
//! command its row names (opening that command's panel, enabled or disabled as the command
//! is) or drawn disabled with its `gap:`; then the catalog's own commands of that menu that
//! no observed item names. A command whose `menu_path` is an observed item is that item,
//! so each catalog command is still one item of its own.
//!
//! Top-level order: the Creation Kit's menus in the order `docs/ck-parity.md` "Interface
//! parity" records, as sitting `ck-20261101-01` observed them (M7.K3, filed in
//! `ck-20261102-01`); then KitKraft's own menus (Record, Tools, Archive, ...) in the
//! order the catalog first names them, before Help (so the CK menus keep their relative
//! order); a CK menu no command uses yet is still drawn, with a disabled line saying so.
//!
//! What an item does: it opens the panel or window that runs its command. It runs
//! nothing by itself, so it can never send a request the gate would refuse. Items whose
//! command the window cannot run are drawn disabled with the reason (deferred or planned;
//! writing outside ADR 0007 Amendment 3's scope; inside the scope but not yet permitted;
//! a read the window does not offer).

use crate::gate::{self, CommandSpec, CommandStatus};

/// The CK's top-level menus as `docs/ck-parity.md` records them (first "Interface
/// parity" row), in the order sitting `ck-20261101-01` observed (M7.K3). A test holds
/// this list equal to that row, so a sitting that changes the row changes this.
pub const CK_MENU_ORDER: &[&str] = &[
    "File",
    "Edit",
    "View",
    "Docks",
    "Galaxy",
    "World",
    "Terrain",
    "NavMesh",
    "Character",
    "Gameplay",
    "Audio",
    "Help",
];

/// Why a CK menu has no KitKraft command yet, by menu (its items are the `menu item`
/// rows of `docs/ck-parity.md`).
pub fn empty_menu_note(menu: &str) -> &'static str {
    match menu {
        "NavMesh" => "gap: navmesh generation is out of scope (ROADMAP M7)",
        "Docks" => "no KitKraft command here: window layouts are workbench settings (ui.json)",
        "Galaxy" => "gap: world generation, tests and exports are not in M7",
        "Terrain" => "gap: terrain editing is not in M7",
        "Audio" => "gap: audio tools (Wwise, FaceFX, voice files) are not in M7",
        _ => "no KitKraft command here yet; the CK's items are listed in docs/ck-parity.md",
    }
}

/// What an observed CK menu item does in KitKraft (its `docs/ck-parity.md` row's "Maps to").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CkMapping {
    /// The command's own panel, enabled or disabled as the command is.
    Command(&'static str),
    /// The command, opening this target instead of its panel (File > Data... opens the Data
    /// dialog, M7.M2).
    CommandIn(&'static str, Target),
    /// No command: disabled, with the row's `gap:` reason.
    Gap(&'static str),
    /// No command of its own: disabled, with this note.
    Note(&'static str),
}

/// One observed item of a CK menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CkItem {
    /// One of [`CK_MENU_ORDER`].
    pub menu: &'static str,
    /// The label as the CK names it.
    pub label: &'static str,
    pub mapping: CkMapping,
}

const fn ck(menu: &'static str, label: &'static str, mapping: CkMapping) -> CkItem {
    CkItem {
        menu,
        label,
        mapping,
    }
}

/// File > Save Plugin: KitKraft writes each confirmed edit when it is made. KitKraft's own
/// note (ADR 0007 Amendment 4, model A: "saved at each OK"), not CK text, and a declared
/// departure from "a command or its `gap:`": the row maps `set` and `build`.
const SAVED_AT_EACH_OK: &str = "KitKraft: no command: each confirmed edit is written into the active file when it is made (ADR 0007 Amendment 4, model A), so the CK's Save has nothing left to do";

/// The CK menus' items, in the order sitting `ck-20261101-01` observed (M7.K3, filed in
/// `ck-20261102-01`), one per `menu item` row of `docs/ck-parity.md`; the reasons are the
/// rows' own words (a test holds this table equal to the rows).
#[rustfmt::skip]
pub const CK_ITEMS: &[CkItem] = &[
    ck("File", "Data...", CkMapping::CommandIn("load", Target::Window(WindowKind::DataDialog))),
    ck("File", "Save Plugin", CkMapping::Note(SAVED_AT_EACH_OK)),
    ck("File", "Import/Export", CkMapping::Gap("not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)")),
    ck("File", "Preferences...", CkMapping::Gap("no command: the Preferences window (M7.M3) keeps workbench settings in ui.json")),
    ck("File", "Theme (experimental)", CkMapping::Gap("no command: the workbench theme is a UI setting (M6)")),
    ck("File", "Validate Loaded Data", CkMapping::Command("validate")),
    ck("File", "Compact Active File Form IDs for ...", CkMapping::Gap("no command compacts FormIDs")),
    ck("File", "Convert Active File to ...", CkMapping::Gap("no command converts an existing file's header (see the row \"Convert the active file to a master\")")),
    ck("File", "Create Archive", CkMapping::Command("ba2.pack")),
    ck("File", "Upload Plugin and Archive to Bethesda.net", CkMapping::Gap("not offered: KitKraft has no Bethesda.net upload or login. Observed disabled (data loaded, no active file)")),
    ck("File", "Login to Bethesda.net", CkMapping::Gap("not offered: KitKraft has no Bethesda.net upload or login")),
    ck("File", "Logout", CkMapping::Gap("not offered: KitKraft has no Bethesda.net upload or login. Observed disabled (data loaded, no active file)")),
    ck("File", "Exit", CkMapping::Gap("no command: closing the window")),
    ck("Edit", "Undo", CkMapping::Command("revert")),
    ck("Edit", "Redo", CkMapping::Gap("no redo (M7.E6 undoes the last edit only)")),
    ck("Edit", "Cut Render", CkMapping::Gap("no command: moving and copying references in the Render Window is M8 (ROADMAP M7)")),
    ck("Edit", "Copy Render", CkMapping::Gap("no command: moving and copying references in the Render Window is M8 (ROADMAP M7)")),
    ck("Edit", "Paste Render", CkMapping::Gap("no command: moving and copying references in the Render Window is M8 (ROADMAP M7)")),
    ck("Edit", "Paste in Place", CkMapping::Gap("no command: moving and copying references in the Render Window is M8 (ROADMAP M7)")),
    ck("Edit", "Duplicate", CkMapping::Command("clone")),
    ck("Edit", "Search & Replace...", CkMapping::Gap("no search-and-replace command")),
    ck("Edit", "Find Text", CkMapping::Command("query")),
    ck("Edit", "Find by Condition", CkMapping::Gap("no search by condition")),
    ck("Edit", "Render Window Hotkeys", CkMapping::Gap("no command: the Render Window tab lists its own bindings (M7.R1)")),
    ck("Edit", "Picking Preferences", CkMapping::Gap("not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)")),
    ck("View", "Toggle Render Features", CkMapping::Gap("no command: the Render Window tab is read-only (M7.R) and has no such view option yet")),
    ck("View", "Refresh Render Window", CkMapping::Gap("no command: the Render Window tab is read-only (M7.R) and has no such view option yet")),
    ck("View", "Object Windows", CkMapping::CommandIn("query", Target::Window(WindowKind::ObjectWindow))),
    ck("View", "Render Windows", CkMapping::CommandIn("cell", Target::Tab("Render Window"))),
    ck("View", "Packin", CkMapping::Gap("not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)")),
    ck("View", "Toolbar", CkMapping::Gap("no command: KitKraft has no toolbar")),
    ck("View", "Cell View", CkMapping::Command("cell")),
    ck("View", "Statusbar", CkMapping::Gap("no command: window arrangement is workbench UI state, kept in ui.json (M7.M0)")),
    ck("View", "Scene Info Window", CkMapping::Gap("not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)")),
    ck("View", "Open Windows...", CkMapping::Gap("no command: window arrangement is workbench UI state, kept in ui.json (M7.M0)")),
    ck("View", "Preview Window", CkMapping::Gap("no command: no preview window yet")),
    ck("View", "Preview Animations", CkMapping::Gap("no command: animation is not in M7")),
    ck("View", "Show/Hide", CkMapping::Gap("not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)")),
    ck("View", "Reference Batch Action Window", CkMapping::Gap("no batch action over references")),
    ck("View", "Layers", CkMapping::Gap("no command for editor layers")),
    ck("View", "Default Layer Manager", CkMapping::Gap("no command for editor layers. Observed disabled (data loaded, no active file)")),
    ck("View", "Object Window Layouts", CkMapping::Gap("no command: window arrangement is workbench UI state, kept in ui.json (M7.M0)")),
    ck("View", "Material Editor", CkMapping::Gap("no command: materials are M8 (ROADMAP M7)")),
    ck("View", "Material Palette", CkMapping::Gap("no command: materials are M8 (ROADMAP M7)")),
    ck("View", "Material Swap Editor", CkMapping::Gap("no command: materials are M8 (ROADMAP M7)")),
    ck("View", "AVMS", CkMapping::Gap("not known yet: grouped with the material items, its window not opened (sitting 1 rest, M7.O2)")),
    ck("View", "Current Cell Only", CkMapping::Gap("no command: the Render Window tab is read-only (M7.R) and has no such view option yet. Observed disabled (data loaded, no active file)")),
    ck("View", "Isometric", CkMapping::Gap("no command: the Render Window camera (M7.R1) has no fixed views yet")),
    ck("View", "Top", CkMapping::Gap("no command: the Render Window camera (M7.R1) has no fixed views yet")),
    ck("View", "Depth Biasing", CkMapping::Gap("no command: the Render Window tab is read-only (M7.R) and has no such view option yet")),
    ck("View", "Warnings", CkMapping::CommandIn("validate", Target::Window(WindowKind::Warnings))),
    ck("View", "Inspector", CkMapping::Gap("not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)")),
    ck("View", "Properties", CkMapping::Gap("not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)")),
    ck("View", "Particle Editor", CkMapping::Gap("no command: particle effects are not in M7")),
    ck("View", "Volumes", CkMapping::Gap("not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)")),
    ck("View", "Reflection Probe", CkMapping::Gap("not known yet: its window or submenu was not opened (sitting 1 rest, M7.O2)")),
    ck("Docks", "Save Current Layout", CkMapping::Gap("no command: window arrangement is workbench UI state, kept in ui.json (M7.M0)")),
    ck("Docks", "Save Layout as...", CkMapping::Gap("no command: window arrangement is workbench UI state, kept in ui.json (M7.M0)")),
    ck("Docks", "Recent Layouts", CkMapping::Gap("no command: window arrangement is workbench UI state, kept in ui.json (M7.M0)")),
    ck("Docks", "Manage Layouts...", CkMapping::Gap("no command: window arrangement is workbench UI state, kept in ui.json (M7.M0)")),
    ck("Galaxy", "Galaxy View", CkMapping::Gap("no command: no galaxy view (`planet` reads one planet's content)")),
    ck("Galaxy", "Block Pattern Editor", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("Galaxy", "Generate Locations", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("Galaxy", "Gen Overlay Placement Data", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("Galaxy", "Generate Biome Placements", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("World", "World Spaces...", CkMapping::Command("cell")),
    ck("World", "Cells...", CkMapping::Command("cell")),
    ck("World", "Go to Location", CkMapping::Gap("no command: the Render Window shows the cell chosen in the Cell View")),
    ck("World", "World Testing And Reports", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("World", "View Render Test Failures...", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("World", "View BetaComment Data", CkMapping::Gap("no command")),
    ck("World", "Run Havok Sim", CkMapping::Gap("no command: physics simulation is not in M7")),
    ck("World", "Animate Lighting and Effects", CkMapping::Gap("no command: lighting is M8 (ROADMAP M7)")),
    ck("World", "Create Local Maps", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("World", "Validate Room/Portal Alignment", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("World", "Align Tanget Space at NIF Intersection", CkMapping::Gap("no command: world generation, tests and exports are not in M7. Observed disabled (data loaded, no active file)")),
    ck("World", "Generate LOS", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("World", "Generate Max Height Data", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("World", "Force Weather", CkMapping::Gap("no command: the Render Window draws no weather")),
    ck("World", "Toggle Wind", CkMapping::Gap("no command: the Render Window draws no weather")),
    ck("World", "Export MapMarker Data", CkMapping::Gap("no command: world generation, tests and exports are not in M7")),
    ck("Terrain", "Landscape Editing", CkMapping::Gap("no command: terrain editing is not in M7")),
    ck("Terrain", "Landscape Cutting", CkMapping::Gap("no command: terrain editing is not in M7")),
    ck("Terrain", "Object Palette Editing (Qt)", CkMapping::Gap("no command: object palettes are not in M7")),
    ck("Terrain", "Heightmap Autopaint", CkMapping::Gap("no command: terrain editing is not in M7")),
    ck("Terrain", "Heightmap Import", CkMapping::Gap("no command: terrain editing is not in M7")),
    ck("Terrain", "Export Landscape...", CkMapping::Gap("no command: terrain editing is not in M7")),
    ck("Terrain", "Reimport All Blocks", CkMapping::Gap("no command: terrain editing is not in M7")),
    ck("Terrain", "Cell Terrain Data Viewer", CkMapping::Gap("no command: terrain editing is not in M7")),
    ck("NavMesh", "Generate Current Cell", CkMapping::Gap("navmesh generation is out of scope (ROADMAP M7)")),
    ck("NavMesh", "Generate Loaded Area", CkMapping::Gap("navmesh generation is out of scope (ROADMAP M7)")),
    ck("NavMesh", "Generate WorldSpace", CkMapping::Gap("navmesh generation is out of scope (ROADMAP M7)")),
    ck("NavMesh", "Cycle View Mode", CkMapping::Gap("navmesh generation is out of scope (ROADMAP M7)")),
    ck("NavMesh", "Draw Cover", CkMapping::Gap("navmesh generation is out of scope (ROADMAP M7)")),
    ck("NavMesh", "Draw Traversal", CkMapping::Gap("navmesh generation is out of scope (ROADMAP M7)")),
    ck("NavMesh", "Other Options", CkMapping::Gap("navmesh generation is out of scope (ROADMAP M7)")),
    ck("NavMesh", "Clean NavMesh Splines", CkMapping::Gap("navmesh generation is out of scope (ROADMAP M7)")),
    ck("Character", "Filtered Dialogue...", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Character", "Export Relationship Data", CkMapping::Gap("no command: character data exports are not in M7")),
    ck("Character", "Export Dialogue...", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Character", "Export Speech Challenge Scenes...", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Character", "Export Dialogue For Specific Voices", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Character", "Export Companion Affinity Event", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Character", "List Neutral Emotion Dialogue", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Character", "List Dialogue Needing Review", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Character", "Quest Voice Asset Totals", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Character", "Update NPC Body Tint Colors", CkMapping::Gap("no command: character appearance tools are not in M7")),
    ck("Character", "Update FaceBones Model Availability", CkMapping::Gap("no command: character appearance tools are not in M7")),
    ck("Character", "Export FaceGen Customization Textures", CkMapping::Gap("no command: character appearance tools are not in M7. Observed disabled (data loaded, no active file)")),
    ck("Character", "Edit Player Dialogue", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Character", "Quest Aliases", CkMapping::Gap("no command for quest aliases")),
    ck("Character", "Generate Player Address Lip Files", CkMapping::Gap("no command: dialogue and voice tools are not in M7")),
    ck("Gameplay", "Settings...", CkMapping::Command("query")),
    ck("Gameplay", "Papyrus Script Manager", CkMapping::Gap("no script list; `papyrus.disasm` reads one compiled script")),
    ck("Gameplay", "Compile Papyrus Scripts...", CkMapping::Command("papyrus.compile")),
    ck("Gameplay", "Migrate Papyrus Property Data...", CkMapping::Gap("no command")),
    ck("Gameplay", "Export Papyrus Attach Data...", CkMapping::Gap("no command: no export of script attachments")),
    ck("Gameplay", "Animations...", CkMapping::Gap("no command: animation is not in M7")),
    ck("Gameplay", "Creature Creator", CkMapping::Gap("no command: creature tools are not in M7")),
    ck("Gameplay", "Camera Paths...", CkMapping::Gap("no command")),
    ck("Gameplay", "Default Objects...", CkMapping::Gap("no command for the default objects list")),
    ck("Gameplay", "Validate Forms", CkMapping::Command("validate")),
    ck("Gameplay", "Recipe Manager", CkMapping::Command("query")),
    ck("Gameplay", "Workshop Editor", CkMapping::Gap("no command: workshop tools are not in M7")),
    ck("Audio", "Animation Sound Tag Overrides...", CkMapping::Gap("no command: audio tools (Wwise, FaceFX, voice files) are not in M7")),
    ck("Audio", "Process Local Voice WAVs", CkMapping::Gap("no command: audio tools (Wwise, FaceFX, voice files) are not in M7")),
    ck("Audio", "Build Soundbank for Active File", CkMapping::Gap("no command: audio tools (Wwise, FaceFX, voice files) are not in M7")),
    ck("Audio", "Reload Wwise Data", CkMapping::Gap("no command: audio tools (Wwise, FaceFX, voice files) are not in M7")),
    ck("Audio", "Run FaceFX Compiler", CkMapping::Gap("no command: audio tools (Wwise, FaceFX, voice files) are not in M7")),
    ck("Help", "Contents", CkMapping::Gap("no CK help text is reproduced; KitKraft's Help menu has its own (`schema`, `doctor`)")),
    ck("Help", "Snap Windows", CkMapping::Gap("no command: window arrangement is workbench UI state, kept in ui.json (M7.M0)")),
    ck("Help", "Third-Party Software Licenses", CkMapping::Gap("no command: KitKraft lists its own in ATTRIBUTION.md")),
    ck("Help", "About", CkMapping::Gap("no command yet")),
];

/// Where an enabled item takes the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A workbench tab, by its label (`app.rs` `TABS`); it may be floating as a window.
    Tab(&'static str),
    /// One of the CK-style windows (`crate::windows`).
    Window(crate::windows::WindowKind),
    /// Pipeline E's form for this editor write (the shell sends `Action::OpenEditor`;
    /// E's ask B1: File > Build Patch opens the build spec form, M7.E5).
    Editor(&'static str),
}

/// One menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// Canonical catalog id (empty for an observed CK item that maps to no command).
    pub id: &'static str,
    /// The text after the last `/` of `menu_path`, or the CK's label of an observed item.
    pub label: &'static str,
    /// This is the catalog command's own item (its `menu_path`): each command has one.
    pub catalog: bool,
    /// An observed CK item ([`CK_ITEMS`]).
    pub observed: bool,
    /// The catalog's default shortcut (shown; bound by M7.M5).
    pub shortcut: Option<&'static str>,
    /// Where it goes, or why it is disabled.
    pub target: Result<Target, String>,
}

/// One top-level menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    pub name: String,
    /// One of [`CK_MENU_ORDER`] (its place is the observed CK order).
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
        "cell" => Target::Tab("Cell View"),
        "planet" => Target::Tab("Planet"),
        "diff" | "conflicts" => Target::Tab("Diff"),
        "validate" | "roundtrip" => Target::Tab("Validate"),
        "coverage" => Target::Tab("Coverage"),
        "schema" => Target::Tab("Schema"),
        "ledger" => Target::Tab("Ledger"),
        "deploy" | "deploy.rollback" | "deploy.status" | "game.session" | "game.probe"
        | "game.stop" | "game.status" => Target::Tab("Cockpit"),
        // Step 0 of sprint 2026-11-02: pipeline E's writes open the editor in the Edit
        // dialog (M7.E2 to M7.E6) once the gate permits each; until then `target`
        // disables them before this match is asked.
        "set" | "new" | "clone" | "place" | "revert" => Target::Window(WindowKind::EditDialog),
        // E's ask B1 (M7.E5): `build` opens its spec form through the editor.
        "build" => Target::Editor("build"),
        // Pipeline R's NIF bounds (M7.S4, M7.R3) belong to the Render Window.
        "mesh.bounds" => Target::Tab("Render Window"),
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
        } else if gate::EDITOR_SCOPE.contains(&id) {
            return Err(
                "writes: in the editor's write scope (ADR 0007 Amendment 4); permitted with its M7.E ticket"
                    .to_string(),
            );
        } else {
            return Err(
                "writes: not in the window's write scope (ADR 0007 Amendments 3 and 4)".to_string(),
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
        .map(|(name, ck)| {
            let mut items: Vec<MenuItem> = Vec::new();
            let mut merged: Vec<&str> = Vec::new();
            for c in CK_ITEMS.iter().filter(|c| ck && c.menu == name) {
                // The catalog command whose `menu_path` is this item, if any, is this item.
                let own = specs
                    .iter()
                    .find(|s| s.menu_path.split_once('/') == Some((c.menu, c.label)));
                if let Some(s) = own {
                    merged.push(s.id);
                }
                items.push(ck_item(c, own, specs));
            }
            items.extend(
                specs
                    .iter()
                    .filter(|s| top_of(s.menu_path) == name && !merged.contains(&s.id))
                    .map(|s| MenuItem {
                        id: s.id,
                        label: s.menu_path.rsplit('/').next().unwrap_or(s.label),
                        catalog: true,
                        observed: false,
                        shortcut: s.shortcut,
                        target: target(s),
                    }),
            );
            Menu {
                name: name.to_string(),
                ck,
                items,
            }
        })
        .collect()
}

/// The item of observed CK item `c`; `own` is the catalog command whose `menu_path` it is.
fn ck_item(c: &CkItem, own: Option<&CommandSpec>, specs: &[CommandSpec]) -> MenuItem {
    let of = |id: &str| -> Result<&CommandSpec, String> {
        specs
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("`{id}` is not in the catalog"))
    };
    let (id, target) = match c.mapping {
        CkMapping::Command(id) => (id, of(id).and_then(target)),
        CkMapping::CommandIn(id, t) => (id, of(id).and_then(target).map(|_| t)),
        CkMapping::Gap(why) => ("", Err(format!("gap: {why}"))),
        CkMapping::Note(why) => ("", Err(why.to_string())),
    };
    MenuItem {
        id: own.map_or(id, |s| s.id),
        label: c.label,
        catalog: own.is_some(),
        observed: true,
        shortcut: own.and_then(|s| s.shortcut),
        target,
    }
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
                                ui.weak("CK menu, as observed (ck-20261101-01)");
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
                            let mut was_observed = false;
                            for item in &menu.items {
                                if was_observed && !item.observed {
                                    ui.separator();
                                    ui.weak("KitKraft");
                                }
                                was_observed = item.observed;
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
                if item.id.is_empty() {
                    item.label
                } else {
                    item.id
                },
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
            let what = if item.id.is_empty() {
                item.label
            } else {
                item.id
            };
            ui.add_enabled(false, button)
                .on_disabled_hover_text(format!("`{what}`: {why}"));
            None
        }
    }
}

/// A disabled item's reason in one or two words, shown beside it.
pub fn short_reason(why: &str) -> &'static str {
    let why = why.trim_start_matches("KitKraft: ");
    if why.starts_with("writes") {
        "writes"
    } else if why.starts_with("gap:") {
        "gap"
    } else if why.starts_with("no command") {
        "no command"
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
            for i in m.items.iter().filter(|i| i.catalog) {
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
        let head = CK_MENU_ORDER.len().saturating_sub(1);
        assert_eq!(&names[..head], &CK_MENU_ORDER[..head]);
        assert_eq!(names.last(), Some(&"Help"));
        for n in ["Record", "Tools", "Archive", "Papyrus", "Strings", "Game"] {
            let at = names.iter().position(|x| *x == n).unwrap();
            assert!(
                (head..names.len() - 1).contains(&at),
                "{n} at {at}: {names:?}"
            );
        }
    }

    /// No enabled item names a command the gate would refuse in every form; every
    /// disabled one says why, and writing commands outside the scope say "write scope".
    #[test]
    fn disabled_items_say_why_and_enabled_ones_pass_the_gate() {
        for m in menus() {
            for i in m.items.iter().filter(|i| !i.id.is_empty()) {
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
        // M7.E2 (ADR 0007 Amendment 4): `set` is permitted, so Record > Set Field opens
        // the edit dialog.
        assert_eq!(set.target, Ok(Target::Window(WindowKind::EditDialog)));
        // A writing command not yet permitted stays disabled and says why (review L3).
        let new = menus()
            .into_iter()
            .flat_map(|m| m.items)
            .find(|i| i.id == "new")
            .unwrap();
        assert!(
            new.target
                .as_ref()
                .is_err_and(|w| w.contains("write scope")),
            "{:?}",
            new.target
        );
        let ledger = menus()
            .into_iter()
            .flat_map(|m| m.items)
            .find(|i| i.id == "ledger")
            .unwrap();
        assert_eq!(ledger.target, Ok(Target::Tab("Ledger")));
    }

    /// The recorded CK menu order in `docs/ck-parity.md` is this file's order, and every
    /// `menu` row there names a menu the bar draws and maps to a catalog command that
    /// exists, a documented `gap:`, or "per item" (its items are `menu item` rows).
    #[test]
    fn every_ck_menu_row_in_the_parity_doc_maps_to_the_bar() {
        let rows = parity_rows();
        let menus = menus();
        let names: Vec<&str> = menus.iter().map(|m| m.name.as_str()).collect();
        let order_row = rows
            .iter()
            .find(|r| r.0 == "menu" && r.1.starts_with("Main menu bar"))
            .expect("the main menu bar row");
        let observed = order_row
            .1
            .split("observed:")
            .nth(1)
            .unwrap()
            .trim_end_matches(')')
            .split(',')
            .map(|s| s.trim().trim_start_matches("and ").trim())
            .collect::<Vec<_>>();
        assert_eq!(observed, CK_MENU_ORDER, "update CK_MENU_ORDER with the row");
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

    /// Owner finding 1 (2026-11-01): the Cell View is a tab, reachable from the tab bar,
    /// and the `cell` item opens that tab.
    #[test]
    fn the_cell_view_is_a_tab_and_the_cell_item_opens_it() {
        assert_eq!(panel_of("cell"), Some(Target::Tab("Cell View")));
    }

    /// Step 0 of sprint 2026-11-02: the editor's six writes have the Edit dialog as their
    /// panel but stay disabled until the gate permits each (ADR 0007 Amendment 4), and
    /// `mesh.bounds` (read-only, allowed) opens the Render Window from Tools.
    #[test]
    fn the_editor_writes_wait_on_the_gate_and_mesh_bounds_opens_the_render_window() {
        for id in ["set", "new", "clone", "place", "build", "revert"] {
            let want = if id == "build" {
                Target::Editor("build")
            } else {
                Target::Window(WindowKind::EditDialog)
            };
            assert_eq!(panel_of(id), Some(want), "{id}");
            let spec = gate::find_command(id).unwrap();
            // A guarded id runs (`target` lets it through), so only an id neither permitted
            // nor guarded must be disabled (stub review M1: `build` enters `GUARDED`, M7.E5).
            if gate::permitted(id).is_none() && !gate::GUARDED.iter().any(|g| g.id == id) {
                let why = target(spec).unwrap_err();
                assert!(why.starts_with("writes"), "{id}: {why}");
            }
        }
        let bounds = gate::find_command("mesh.bounds").unwrap();
        assert_eq!(top_of(bounds.menu_path), "Tools");
        assert_eq!(target(bounds), Ok(Target::Tab("Render Window")));
        let geometry = target(gate::find_command("mesh.geometry").unwrap()).unwrap_err();
        assert!(geometry.starts_with("deferred to M7.S5"), "{geometry}");
    }

    /// M7.M1 rest: every `menu item` row of `docs/ck-parity.md` is one entry of
    /// [`CK_ITEMS`], in the row order, mapped as the row maps it (its first catalog id, or
    /// its `gap:`); and the bar draws each CK menu's observed items first, in that order.
    #[test]
    fn every_observed_menu_item_row_is_drawn_in_its_menu_in_order() {
        let rows: Vec<(String, String)> = parity_rows()
            .into_iter()
            .filter(|r| r.0 == "menu item")
            .map(|r| (r.1, r.2))
            .collect();
        assert_eq!(rows.len(), CK_ITEMS.len(), "one entry per menu item row");
        for ((element, maps), item) in rows.iter().zip(CK_ITEMS) {
            assert_eq!(
                element,
                &format!("{} > {}", item.menu, item.label),
                "row order"
            );
            assert!(CK_MENU_ORDER.contains(&item.menu), "{element}");
            let first_id = maps
                .split('`')
                .skip(1)
                .step_by(2)
                .find(|s| !s.ends_with(':'));
            match item.mapping {
                CkMapping::Gap(why) => {
                    assert!(maps.starts_with("`gap:`"), "{element}: {maps}");
                    assert_eq!(
                        maps,
                        &format!("`gap:` {why}"),
                        "{element}: the row's own reason"
                    );
                }
                CkMapping::Command(id) | CkMapping::CommandIn(id, _) => {
                    assert_eq!(first_id, Some(id), "{element}: {maps}");
                    assert!(gate::find_command(id).is_some(), "{element}: {id}");
                }
                CkMapping::Note(_) => {
                    assert_eq!(element, "File > Save Plugin", "the one note");
                    assert_eq!(first_id, Some("set"), "{element}: {maps}");
                    assert!(maps.contains("written when it is made"), "{maps}");
                }
            }
        }
        for m in menus().iter().filter(|m| m.ck) {
            let observed: Vec<&str> = m
                .items
                .iter()
                .take_while(|i| i.observed)
                .map(|i| i.label)
                .collect();
            let want: Vec<&str> = CK_ITEMS
                .iter()
                .filter(|c| c.menu == m.name)
                .map(|c| c.label)
                .collect();
            assert_eq!(observed, want, "{}: the observed items first", m.name);
            assert!(
                m.items
                    .iter()
                    .skip(want.len())
                    .all(|i| !i.observed && i.catalog),
                "{}: then the catalog's own",
                m.name
            );
        }
    }

    /// An observed item is enabled or disabled as its command is; a `gap:` item is
    /// disabled with its reason; File > Data... opens the Data dialog (M7.M2) and View >
    /// Warnings the Warnings window.
    #[test]
    fn observed_items_follow_their_command_or_say_their_gap() {
        let menus = menus();
        let item = |menu: &str, label: &str| {
            menus
                .iter()
                .find(|m| m.name == menu)
                .and_then(|m| m.items.iter().find(|i| i.observed && i.label == label))
                .cloned()
                .unwrap_or_else(|| panic!("{menu} > {label}"))
        };
        assert_eq!(
            item("File", "Data...").target,
            Ok(Target::Window(WindowKind::DataDialog))
        );
        assert_eq!(
            item("View", "Warnings").target,
            Ok(Target::Window(WindowKind::Warnings))
        );
        assert_eq!(
            item("View", "Cell View").target,
            Ok(Target::Tab("Cell View"))
        );
        assert_eq!(
            item("View", "Object Windows").target,
            Ok(Target::Window(WindowKind::ObjectWindow))
        );
        assert_eq!(item("Edit", "Find Text").target, Ok(Target::Tab("Records")));
        let exit = item("File", "Exit");
        assert!(exit.id.is_empty() && !exit.catalog);
        assert!(exit.target.unwrap_err().starts_with("gap: no command"));
        let save = item("File", "Save Plugin").target.unwrap_err();
        assert_eq!(short_reason(&save), "no command");
        assert!(save.contains("Amendment 4"), "{save}");
        for c in CK_ITEMS {
            let i = item(c.menu, c.label);
            match c.mapping {
                CkMapping::Command(id) => {
                    let own = target(gate::find_command(id).unwrap());
                    assert_eq!(i.target, own, "{} > {}", c.menu, c.label);
                }
                CkMapping::CommandIn(id, t) => {
                    let own = target(gate::find_command(id).unwrap());
                    assert_eq!(i.target.is_ok(), own.is_ok(), "{} > {}", c.menu, c.label);
                    if own.is_ok() {
                        assert_eq!(i.target, Ok(t));
                    }
                }
                CkMapping::Gap(_) => {
                    let why = i.target.unwrap_err();
                    assert_eq!(short_reason(&why), "gap", "{why}");
                }
                CkMapping::Note(_) => assert!(i.target.is_err()),
            }
        }
    }

    /// A command whose `menu_path` is an observed item is that item (one item, the
    /// catalog's, with its shortcut): what the M7.S1 `menu_path` table does to `load`.
    #[test]
    fn a_command_at_an_observed_place_is_that_item() {
        let mut specs: Vec<CommandSpec> = gate::catalog().to_vec();
        for s in &mut specs {
            if s.id == "load" {
                s.menu_path = "File/Data...";
            }
        }
        let menus = build(&specs);
        let file = menus.iter().find(|m| m.name == "File").unwrap();
        let loads: Vec<&MenuItem> = file.items.iter().filter(|i| i.id == "load").collect();
        assert_eq!(loads.len(), 1, "{loads:?}");
        let data = loads[0];
        assert!(data.observed && data.catalog);
        assert_eq!(data.label, "Data...");
        assert_eq!(data.shortcut, gate::find_command("load").unwrap().shortcut);
        assert_eq!(data.target, Ok(Target::Window(WindowKind::DataDialog)));
    }

    /// W2 review I1: a catalog command that sits at an observed item is the command that
    /// item's row maps (checked for the catalog and for the eight M7.S1 moves).
    #[test]
    fn a_merged_item_is_the_command_its_row_maps() {
        let moves = [
            ("load", "File/Data..."),
            ("validate", "File/Validate Loaded Data"),
            ("ba2.pack", "File/Create Archive"),
            ("revert", "Edit/Undo"),
            ("clone", "Edit/Duplicate"),
            ("query", "Edit/Find Text"),
            ("cell", "View/Cell View"),
            ("papyrus.compile", "Gameplay/Compile Papyrus Scripts..."),
        ];
        let mut moved: Vec<CommandSpec> = gate::catalog().to_vec();
        for s in &mut moved {
            if let Some((_, to)) = moves.iter().find(|(id, _)| *id == s.id) {
                s.menu_path = to;
            }
        }
        for specs in [gate::catalog().to_vec(), moved] {
            for c in CK_ITEMS {
                let Some(own) = specs
                    .iter()
                    .find(|s| s.menu_path.split_once('/') == Some((c.menu, c.label)))
                else {
                    continue;
                };
                match c.mapping {
                    CkMapping::Command(id) | CkMapping::CommandIn(id, _) => {
                        assert_eq!(own.id, id, "{} > {}", c.menu, c.label)
                    }
                    other => panic!("{} > {}: {other:?} at a command", c.menu, c.label),
                }
            }
        }
    }

    /// E1 ask A3: the reasons a not-yet-permitted write gives name Amendment 4 as decided.
    #[test]
    fn write_reasons_name_amendment_4_as_decided() {
        let editor: CommandSpec = *gate::find_command("place").unwrap();
        if gate::permitted("place").is_none() && !gate::GUARDED.iter().any(|g| g.id == "place") {
            let why = target(&editor).unwrap_err();
            assert!(why.contains("editor's write scope"), "{why}");
            assert!(!why.contains("waits on"), "{why}");
        }
        let pack = target(gate::find_command("ba2.pack").unwrap()).unwrap_err();
        assert_eq!(
            pack,
            "writes: not in the window's write scope (ADR 0007 Amendments 3 and 4)"
        );
    }

    // ---- M7.K5: the parity check ---------------------------------------------------------

    /// The statuses of ADR 0012 section 4 and its Amendment 1 (`built, unobserved`).
    const STATUSES: [&str; 5] = [
        "unresearched",
        "built, unobserved",
        "observed",
        "reproduced",
        "improved",
    ];

    /// One "Interface parity" row: kind, element, maps to, status, source, evidence.
    #[derive(Debug)]
    struct Row {
        kind: String,
        element: String,
        maps: String,
        status: String,
        source: String,
        evidence: String,
    }

    /// The rows of the "Interface parity" table of a `docs/ck-parity.md` text.
    fn interface_rows(parity: &str) -> Vec<Row> {
        let Some(section) = parity.split("## Interface parity").nth(1) else {
            return Vec::new();
        };
        let section = section.split("\n## ").next().unwrap_or(section);
        section
            .lines()
            .filter(|l| l.starts_with('|'))
            .map(|l| {
                l.trim()
                    .trim_matches('|')
                    .split('|')
                    .map(|c| c.trim().to_string())
                    .collect::<Vec<_>>()
            })
            .filter(|c| c.len() >= 6 && c[0] != "Kind" && !c[0].starts_with('-'))
            .map(|c| Row {
                kind: c[0].clone(),
                element: c[1].clone(),
                maps: c[2].clone(),
                status: c[3].clone(),
                source: c[4].clone(),
                evidence: c[5..].join("|"),
            })
            .collect()
    }

    /// The backticked tokens of a cell.
    fn ticked(cell: &str) -> Vec<&str> {
        cell.split('`').skip(1).step_by(2).collect()
    }

    /// Every full sitting id (`ck-YYYYMMDD-NN`) a cell names.
    fn sitting_ids(cell: &str) -> Vec<String> {
        let b = cell.as_bytes();
        let mut out = Vec::new();
        for (at, _) in cell.match_indices("ck-") {
            let id = b
                .get(at..at.saturating_add(14))
                .and_then(|w| std::str::from_utf8(w).ok());
            if let Some(id) = id.filter(|id| {
                let d = id.as_bytes();
                d[3..11].iter().all(u8::is_ascii_digit)
                    && d[11] == b'-'
                    && d[12..14].iter().all(u8::is_ascii_digit)
            }) {
                out.push(id.to_string());
            }
        }
        out
    }

    /// The kinds of row the Interface parity table uses (W3 review L4).
    const KINDS: [&str; 7] = [
        "menu",
        "menu item",
        "window",
        "dialog",
        "workflow",
        "shortcut",
        "control",
    ];

    /// The text of ledger entry `### <id>` of `observations`, up to the next heading.
    fn entry<'a>(observations: &'a str, id: &str) -> Option<&'a str> {
        let head = format!("### {id}");
        let at = observations.find(&head)?;
        let rest = observations.get(at..)?;
        let end = rest
            .get(4..)
            .and_then(|r| r.find("\n### ").or_else(|| r.find("\n## ")))
            .map_or(rest.len(), |e| e.saturating_add(4));
        rest.get(..end)
    }

    /// A desk entry (no CK run): its heading or first lines say so.
    fn is_desk(entry: &str) -> bool {
        let head: String = entry.lines().take(3).collect::<Vec<_>>().join(" ");
        head.contains("desk entry") || head.contains("no CK run") || head.contains("Desk work")
    }

    /// Whether `id` may stand as a Source (ruling R24): a recorded sitting, or a desk entry
    /// that names the recorded sitting it was filed from.
    fn source_ok(observations: &str, id: &str) -> bool {
        let Some(e) = entry(observations, id) else {
            return false;
        };
        if !is_desk(e) {
            return true;
        }
        sitting_ids(e)
            .iter()
            .any(|other| other != id && entry(observations, other).is_some_and(|o| !is_desk(o)))
    }

    /// The item labels a ledger entry records for menu `menu` (its `| <menu> | ... |` row,
    /// split on " / " and ", ", the `*(...)*` marks dropped).
    fn ledger_items(entry: &str, menu: &str) -> Vec<String> {
        let head = format!("| {menu} |");
        let Some(line) = entry.lines().find(|l| l.starts_with(&head)) else {
            return Vec::new();
        };
        let mut cell = line
            .trim_start_matches(&head)
            .trim_end()
            .trim_end_matches('|')
            .to_string();
        while let Some(a) = cell.find(" *(") {
            match cell[a..].find(")*") {
                Some(b) => cell.replace_range(a..a.saturating_add(b).saturating_add(2), ""),
                None => break,
            }
        }
        cell.split(" / ")
            .flat_map(|g| g.split(", "))
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    }

    /// What is wrong with the "Interface parity" rows of `parity`, judged against the
    /// sittings `observations` records (its `### ck-...` ledger entries), the catalog, and
    /// `test_exists` (whether a cited test function exists): ADR 0012 section 4 and the
    /// parity text contract.
    fn parity_problems(
        parity: &str,
        observations: &str,
        test_exists: &dyn Fn(&str) -> bool,
    ) -> Vec<String> {
        let mut out = Vec::new();
        let rows = interface_rows(parity);
        if rows.is_empty() {
            out.push("no Interface parity rows".to_string());
        }
        for r in &rows {
            let at = format!("`{}` row \"{}\"", r.kind, r.element);
            if !KINDS.contains(&r.kind.as_str()) {
                out.push(format!("{at}: kind `{}` is none of {KINDS:?}", r.kind));
            }
            if !STATUSES.contains(&r.status.as_str()) {
                out.push(format!(
                    "{at}: status `{}` is none of {STATUSES:?}",
                    r.status
                ));
            }
            if r.kind == "menu item" {
                let ids: Vec<&str> = ticked(&r.maps)
                    .into_iter()
                    .filter(|t| !t.ends_with(':'))
                    .collect();
                for id in &ids {
                    if gate::find_command(id).is_none() {
                        out.push(format!("{at}: `{id}` is no catalog command"));
                    }
                }
                if ids.is_empty() && !r.maps.contains("`gap:`") {
                    out.push(format!("{at}: maps to no catalog id and no `gap:`"));
                }
            }
            if ["observed", "reproduced", "improved"].contains(&r.status.as_str()) {
                let ids = sitting_ids(&r.source);
                if ids.is_empty() {
                    out.push(format!(
                        "{at}: `{}` but its Source names no sitting",
                        r.status
                    ));
                }
                for id in &ids {
                    if entry(observations, id).is_none() {
                        out.push(format!("{at}: Source names `{id}`, which docs/ck-observations.md does not record"));
                    }
                }
                if !ids.is_empty() && !ids.iter().any(|id| source_ok(observations, id)) {
                    out.push(format!(
                        "{at}: Source names only desk entries with no sitting behind them (R24)"
                    ));
                }
                // The label as a sitting recorded it (W3 review L1): a `menu item` row's
                // label is one of its menu's items in a cited ledger entry.
                if r.kind == "menu item" {
                    if let Some((menu, label)) = r.element.split_once(" > ") {
                        let found = ids
                            .iter()
                            .filter_map(|id| entry(observations, id))
                            .any(|e| ledger_items(e, menu).iter().any(|l| l == label));
                        if !found {
                            out.push(format!(
                                "{at}: no cited ledger entry records `{label}` among the {menu} items"
                            ));
                        }
                    }
                }
            }
            if ["built, unobserved", "reproduced", "improved"].contains(&r.status.as_str()) {
                let tokens = ticked(&r.evidence);
                let tests: Vec<&str> = tokens
                    .iter()
                    .copied()
                    .filter(|t| t.contains("::"))
                    .collect();
                let images = tokens.iter().any(|t| t.ends_with(".png"));
                if tests.is_empty() && !images {
                    out.push(format!("{at}: `{}` names no test or tour image", r.status));
                }
                for t in tests {
                    let name = t.rsplit("::").next().unwrap_or(t);
                    if !test_exists(name) {
                        out.push(format!("{at}: its test `{t}` does not exist"));
                    }
                }
            }
        }
        out
    }

    /// Whether a test function `name` exists in the workspace: `fn <name>(` under
    /// `#[test]` (attributes such as `#[ignore]` may sit between) in `crates/*/src` or
    /// `crates/*/tests` (W3 review L3).
    fn workspace_has_test(name: &str) -> bool {
        let want = format!("fn {name}(");
        let mut stack: Vec<PathBuf> = std::fs::read_dir(repo().join("crates"))
            .unwrap()
            .filter_map(Result::ok)
            .flat_map(|c| [c.path().join("src"), c.path().join("tests")])
            .collect();
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries {
                let path = e.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|x| x.to_str()) != Some("rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                let lines: Vec<&str> = text.lines().collect();
                for (i, l) in lines.iter().enumerate() {
                    if l.trim_start().starts_with(&want) {
                        let above = lines[i.saturating_sub(4)..i]
                            .iter()
                            .any(|a| a.trim() == "#[test]");
                        if above {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    const FIXTURE_HEAD: &str = "# CK parity\n\n## Interface parity\n\n| Kind | CK element (as named in the CK) | Maps to | Status | Source | Evidence |\n| ---- | ---- | ---- | ---- | ---- | ---- |\n";
    const FIXTURE_OBSERVATIONS: &str = "## Ledger\n\n### ck-20261101-01: a sitting\n\nAgent-run.\n\n| File | Data..., Save / Exit |\n\n### ck-20261102-01: the menus of `ck-20261101-01` (desk entry; no CK run)\n\n| File | Data... *(off)* / Exit |\n\n### ck-20261102-02: a desk note (desk entry; no CK run)\n\nNo sitting.\n";
    const FIXTURE_GOOD: &str = "| menu item | File > Data... | `load` (the Data dialog) | observed | `ck-20261101-01` | - |\n| menu item | File > Exit | `gap:` no command | observed | `ck-20261101-01` | - |\n| menu | Edit menu, every item | per item | unresearched | - | - |\n| dialog | Data dialog | `load` | reproduced | `ck-20261101-01` | test `menu::tests::the_parity_doc_passes_the_parity_check` |\n";

    fn fixture(rows: &str) -> String {
        format!(
            "{FIXTURE_HEAD}{FIXTURE_GOOD}{rows}\n## Next section\n\n| a | b | c | d | e | f |\n"
        )
    }

    /// M7.K5 (ROADMAP M7 exit, ADR 0012 section 4): the real `docs/ck-parity.md` passes the
    /// parity check, and each kind of bad row planted into a small fixture fails it.
    #[test]
    fn the_parity_doc_passes_the_parity_check() {
        let parity = std::fs::read_to_string(repo().join("docs").join("ck-parity.md")).unwrap();
        let observations =
            std::fs::read_to_string(repo().join("docs").join("ck-observations.md")).unwrap();
        let problems = parity_problems(&parity, &observations, &workspace_has_test);
        assert!(problems.is_empty(), "{problems:#?}");
        assert!(
            interface_rows(&parity).len() >= 150,
            "the whole table is read"
        );

        let has = |name: &str| name == "the_parity_doc_passes_the_parity_check";
        assert_eq!(
            parity_problems(&fixture(""), FIXTURE_OBSERVATIONS, &has),
            Vec::<String>::new(),
            "the good fixture passes"
        );
        // Each planted row, and what the check must say about it.
        let planted = [
            // A status ADR 0012 does not define.
            ("| menu item | File > Exit | `gap:` no command | observd | `ck-20261101-01` | - |", "none of"),
            ("| window | Warnings window | `validate` | reproduced (layout unobserved) | `ck-20261101-01` | test `menu::tests::the_parity_doc_passes_the_parity_check` |", "none of"),
            // A menu item that maps to no command that exists, or to nothing.
            ("| menu item | File > Data... | `no.such.command` | observed | `ck-20261101-01` | - |", "no catalog command"),
            ("| menu item | File > Exit | closes the window | observed | `ck-20261101-01` | - |", "no `gap:`"),
            // Observed with no sitting, or a sitting no ledger entry records.
            ("| menu item | File > Exit | `gap:` no command | observed | owner, 2026-10-16 (recollection) | - |", "names no sitting"),
            ("| window | Object Window | `query` | observed | `ck-20991231-99` | - |", "does not record"),
            // Reproduced with no evidence, or with a test that does not exist.
            ("| dialog | Data dialog | `load` | reproduced | `ck-20261101-01` | - |", "no test or tour image"),
            ("| dialog | Data dialog | `load` | improved | `ck-20261101-01` | test `menu::tests::no_such_test` |", "does not exist"),
            // W3 review: a kind typo, a built row with no evidence, an image only in prose.
            ("| menu-item | File > Data... | `no.such.command` | observed | `ck-20261101-01` | - |", "kind `menu-item`"),
            ("| window | Warnings window | `validate` | built, unobserved | - | - |", "no test or tour image"),
            ("| dialog | Data dialog | `load` | improved | `ck-20261101-01` | no .png yet |", "no test or tour image"),
            // A label no cited ledger entry records (renamed in the doc and the table both).
            ("| menu item | File > Exit Now | `gap:` no command | observed | `ck-20261101-01` | - |", "records `Exit Now`"),
            // R24: a desk entry with no sitting behind it is no Source.
            ("| window | Object Window | `query` | observed | `ck-20261102-02` | - |", "no sitting behind"),
        ];
        for (row, want) in planted {
            let problems = parity_problems(&fixture(row), FIXTURE_OBSERVATIONS, &has);
            assert!(
                problems.iter().any(|p| p.contains(want)),
                "planted {row}\n  wanted `{want}`, got {problems:#?}"
            );
        }
        // A desk entry filed from a recorded sitting is a Source, its labels included (R24).
        let chained =
            "| menu item | File > Exit | `gap:` no command | observed | `ck-20261102-01` | - |";
        assert_eq!(
            parity_problems(&fixture(chained), FIXTURE_OBSERVATIONS, &has),
            Vec::<String>::new()
        );
        // The rows of the next section are not read; an empty table is a problem.
        assert_eq!(interface_rows(&fixture("")).len(), 4);
        assert!(!parity_problems("# no table", FIXTURE_OBSERVATIONS, &has).is_empty());
    }
}
