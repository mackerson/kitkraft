//! The command engine.
//!
//! Every user-visible action is one [`CommandSpec`] in [`catalog()`]. Menus, toolbar,
//! scripts, the CLI and the MCP server all resolve a name to the same spec and call the
//! same `execute`, so behaviour cannot drift between front ends. `execute` takes its
//! arguments as JSON matching `args_schema` and never opens a dialog; interactive
//! front ends gather arguments first, then call it.
//!
//! [`execute`] resolves an id or alias, validates the arguments against the command's
//! `args_schema` (the subset documented in [`args`]), dispatches to the registered
//! [`CommandHandler`], and returns the result envelope from `docs/mcp.md`:
//! `{command, data, coverage, limitations, evidence}`.
//!
//! An [`Engine`] owns a [`Session`] (the loaded load order and caches) that
//! [`SessionHandler`]s read and change; the free [`execute`] runs on a fresh session.
//! Undo lands after M2 (see `plan/execution-plan.md`).
//!
//! LAYER: L4

pub mod args;
pub mod build;
pub mod cmdline;
mod deploy;
pub mod edit;
mod envelope;
mod exec;
mod handlers;
mod params;
mod session;
pub mod validate;

pub use args::Violation;
pub use cmdline::{parse_line, parse_words, Invocation};
pub use envelope::{Coverage, CoverageStatus, Evidence, Outcome, NOT_IMPLEMENTED};
pub use exec::{execute, prepare_args, CommandHandler, Engine, EngineError, SessionHandler};
pub use params::{params, Param, ParamKind};
pub use session::Session;

/// Whether a catalog entry does anything yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Listed so every front end agrees on the name and arguments; not callable yet.
    /// None at M2 exit: the `catalog_closure` test fails on one (M2.F8).
    Planned,
    /// Callable from every front end.
    Implemented,
    /// Deliberately not in this milestone. `execute` refuses it with
    /// [`EngineError::Deferred`]; `kit help` and MCP `tools/list` show the reason.
    Deferred {
        /// Why it is not done now.
        reason: &'static str,
        /// Where it goes (`M3`).
        milestone: &'static str,
    },
}

impl Status {
    /// Short label for help text and listings: `implemented`, `planned`, `deferred`.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Implemented => "implemented",
            Self::Deferred { .. } => "deferred",
        }
    }

    /// The note front ends append to a command's summary: empty when implemented.
    pub fn note(&self) -> String {
        match self {
            Self::Planned => "planned: not implemented yet".to_string(),
            Self::Implemented => String::new(),
            Self::Deferred { reason, milestone } => format!("deferred to {milestone}: {reason}"),
        }
    }
}

/// One user-visible action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    /// Stable dotted id: `inspect`, `ba2.extract`. The CLI spells it `kit ba2 extract`.
    pub id: &'static str,
    /// Human label for menus and palettes.
    pub label: &'static str,
    /// Menu location in the future UI, `/`-separated.
    pub menu_path: &'static str,
    /// Default keyboard shortcut, if any.
    pub shortcut: Option<&'static str>,
    /// Other names accepted by the CLI, `command_line` and the palette.
    pub aliases: &'static [&'static str],
    /// One-line description used in `kit help` and MCP `tools/list`.
    pub summary: &'static str,
    /// JSON Schema (draft 2020-12) for the arguments object.
    pub args_schema: &'static str,
    /// Whether it is callable yet.
    pub status: Status,
}

/// The full command catalog, in help order.
pub fn catalog() -> &'static [CommandSpec] {
    CATALOG
}

/// Finds a command by id or alias. CLI words are joined with `.` before lookup
/// (`kit ba2 list` -> `ba2.list`).
pub fn find_command(name: &str) -> Option<&'static CommandSpec> {
    CATALOG
        .iter()
        .find(|spec| spec.id == name || spec.aliases.contains(&name))
}

static CATALOG: &[CommandSpec] = &[
    CommandSpec {
        id: "load",
        label: "Load Order...",
        menu_path: "File/Load Order...",
        shortcut: Some("Ctrl+O"),
        aliases: &["open"],
        summary: "Load plugins from an MO2 profile or an explicit plugin list; lists plugins with index, flags, scale, masters, missing masters and cache hits",
        args_schema: r#"{"type":"object","properties":{"profile":{"type":"string","description":"MO2 profile directory (read-only)"},"plugins":{"type":"array","items":{"type":"string"},"description":"Plugin files in load order; bare names resolve against data_dir"},"data_dir":{"type":"string","description":"Game Data dir for plugin names (default KITCRAFT_DATA_DIR; for a profile: the MO2 instance's gamePath + Data, then KITCRAFT_DATA_DIR)"},"language":{"type":"string","description":"String-table language for localized lstrings (FULL, DESC), default en"},"no_cache":{"type":"boolean","description":"Index every plugin from its file instead of the index cache (default false)"}},"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "inspect",
        label: "Inspect Record",
        menu_path: "Record/Inspect",
        shortcut: Some("Enter"),
        aliases: &["show"],
        summary: "One record across the load order (every override, the winner, lstrings) or in one plugin: header fields and subrecords with offsets",
        args_schema: r#"{"type":"object","properties":{"form":{"type":"string","description":"Plugin.esm:XXXXXX, hex FormID (as stored in `plugin`; bare hex needs `plugin`), runtime:<hex> (a runtime ID), or EditorID"},"plugin":{"type":"string","description":"Single-plugin mode: the plugin file to search. With a load order: the plugin whose numbering a hex FormID uses"},"profile":{"type":"string","description":"MO2 profile to load first"},"plugins":{"type":"array","items":{"type":"string"},"description":"Plugin files to load first, in load order"},"data_dir":{"type":"string","description":"Game Data dir for profile or plugin names (default KITCRAFT_DATA_DIR)"},"no_cache":{"type":"boolean","description":"Index plugins from their files, not the index cache"}},"required":["form"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "query",
        label: "Query Records...",
        menu_path: "Record/Query...",
        shortcut: Some("Ctrl+F"),
        aliases: &["find"],
        summary: "Records of one type matching a where predicate across the load order (winner or all versions), paged",
        args_schema: r#"{"type":"object","properties":{"type":{"type":"string","minLength":4,"maxLength":4},"where":{"type":"string","description":"Predicate, e.g. FULL ~ \"Rifle\" and not has KWDA (docs/mcp.md, query)"},"scope":{"type":"string","enum":["winner","all"],"default":"winner","description":"winner: the winning version of each form; all: every version"},"limit":{"type":"integer","minimum":1,"maximum":1000,"default":50},"cursor":{"type":"string","description":"Opaque paging token from the previous result"},"profile":{"type":"string","description":"MO2 profile to load first"},"plugins":{"type":"array","items":{"type":"string"},"description":"Plugin files to load first, in load order"},"data_dir":{"type":"string","description":"Game Data dir for profile or plugin names (default KITCRAFT_DATA_DIR)"},"no_cache":{"type":"boolean","description":"Index plugins from their files, not the index cache"}},"required":["type"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "set",
        label: "Set Field",
        menu_path: "Record/Set Field...",
        shortcut: None,
        aliases: &[],
        summary: "Write field values on the winning version of a record into a patch plugin (non-localized; FULL=\"...\" shorthand)",
        args_schema: r#"{"type":"object","properties":{"form":{"type":"string","description":"Plugin.esm:XXXXXX, EditorID, or a hex FormID as stored in `plugin`"},"fields":{"type":"object","description":"Field path -> value: FULL, DNAM.Member, FULL[1]; numbers or text; {\"mul\": x} / {\"add\": x}; hex:0A0B for opaque payloads"},"into":{"type":"string","description":"Patch plugin to create or update (.esm or .esp; never a load-order plugin, the game Data dir or MO2)"},"plugin":{"type":"string","description":"The plugin whose numbering a hex FormID uses"},"profile":{"type":"string","description":"MO2 profile to load first"},"plugins":{"type":"array","items":{"type":"string"},"description":"Plugin files to load first, in load order"},"data_dir":{"type":"string","description":"Game Data dir for profile or plugin names (default KITCRAFT_DATA_DIR)"},"language":{"type":"string","description":"String-table language for inlining lstrings, default en"},"no_cache":{"type":"boolean","description":"Index plugins from their files, not the index cache"},"allow_untyped_lstrings":{"type":"boolean","default":false,"description":"Copy 4-byte subrecords of unknown layout from a localized source as stored (named in limitations) instead of refusing; they may be string IDs"}},"required":["form","fields","into"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "diff",
        label: "Diff Plugins...",
        menu_path: "Tools/Diff Plugins...",
        shortcut: None,
        aliases: &[],
        summary: "Record-level diff of two plugins: FormIDs matched through each file's masters by name; added, removed, changed records with per-subrecord byte ranges",
        args_schema: r#"{"type":"object","properties":{"a":{"type":"string"},"b":{"type":"string"},"form":{"type":"string","description":"Only this form (Plugin.esm:XXXXXX)"},"data_dir":{"type":"string","description":"Where to look for masters besides each file's own directory (default KITCRAFT_DATA_DIR); a bare a or b name resolves in the session's load order, then the working directory, then here"},"limit":{"type":"integer","minimum":1,"description":"Entries listed per category (default 200; counts are always complete)"}},"required":["a","b"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "conflicts",
        label: "Conflicts...",
        menu_path: "Tools/Conflicts...",
        shortcut: None,
        aliases: &["esmscan"],
        summary: "Records plugins override from a master (port of esmscan.py): per-plugin counts by type, and each override classed against the version before it",
        args_schema: r#"{"type":"object","properties":{"master":{"type":"string","description":"Only forms this plugin defines"},"plugin":{"type":"string","description":"Only overrides by this plugin"},"type":{"type":"string","minLength":4,"maxLength":4,"description":"Only this record type"},"only_conflicts":{"type":"boolean","default":false,"description":"Only forms two or more plugins override"},"count_classes":{"type":"boolean","default":false,"description":"Class every matching form, not just this page"},"limit":{"type":"integer","minimum":1,"maximum":1000,"default":50},"cursor":{"type":"string","description":"Opaque paging token from the previous result"},"profile":{"type":"string","description":"MO2 profile to load first"},"plugins":{"type":"array","items":{"type":"string"},"description":"Plugin files to load first, in load order"},"data_dir":{"type":"string","description":"Game Data dir for profile or plugin names (default KITCRAFT_DATA_DIR)"},"no_cache":{"type":"boolean","description":"Index plugins from their files, not the index cache"}},"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "build",
        label: "Build Patch...",
        menu_path: "File/Build Patch...",
        shortcut: Some("Ctrl+B"),
        aliases: &[],
        summary: "Compile a data-driven patch spec (TOML v2: classes, [[rule]] with operators and via, [[set]]; v1 still accepted) into one fresh plugin, validated; --plan lists the changes without writing",
        args_schema: r#"{"type":"object","properties":{"spec":{"type":"string","description":"The TOML spec (version = 2, or 1)"},"out":{"type":"string","description":"Plugin to write (.esm or .esp; never a load-order plugin, the game or user Data dir, or MO2); written only when the new one validates"},"plan":{"type":"boolean","default":false,"description":"Dry run: list every change (before and after), the classes and rules with their counts and the validation result; write nothing"},"overwrite":{"type":"boolean","default":false,"description":"Let out replace an existing file"},"allow_untyped_lstrings":{"type":"boolean","default":false,"description":"Copy 4-byte subrecords of unknown layout from a localized source as stored (named in limitations) instead of refusing; they may be string IDs"},"profile":{"type":"string","description":"MO2 profile to load first (when the spec has no [load_order])"},"plugins":{"type":"array","items":{"type":"string"},"description":"Plugin files to load first, in load order"},"data_dir":{"type":"string","description":"Game Data dir for profile or plugin names (default KITCRAFT_DATA_DIR)"},"language":{"type":"string","description":"String-table language for inlining lstrings, default en"},"no_cache":{"type":"boolean","description":"Index plugins from their files, not the index cache"}},"required":["spec","out"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "overhaul.scaffold",
        label: "New Overhaul Spec...",
        menu_path: "File/New Overhaul Spec...",
        shortcut: None,
        aliases: &["overhaul"],
        summary: "Starter build spec v2 for one record type: a class per group of winning records, with match counts and min/median/max of each typed numeric member as comments; every rule commented out",
        args_schema: r#"{"type":"object","properties":{"type":{"type":"string","minLength":4,"maxLength":4,"description":"Record type (WEAP)"},"where":{"type":"string","description":"Only winning records matching this predicate (query's where language)"},"group_by":{"type":"string","description":"A where path (KWDA, @plugin, SIG.Member): one class per value; a record with several values joins one class per value"},"out":{"type":"string","description":"The .toml spec to write (never in the game or user Data dir or MO2); omitted: the text is returned in data.spec_text"},"overwrite":{"type":"boolean","default":false,"description":"Let out replace an existing file"},"max_groups":{"type":"integer","minimum":1,"maximum":1000,"default":40,"description":"Most classes; the smaller groups are listed in a comment"},"profile":{"type":"string","description":"MO2 profile to load first (repeated in the spec's [load_order])"},"plugins":{"type":"array","items":{"type":"string"},"description":"Plugin files to load first, in load order (repeated in the spec's [load_order])"},"data_dir":{"type":"string","description":"Game Data dir for profile or plugin names (default KITCRAFT_DATA_DIR)"},"language":{"type":"string","description":"String-table language for lstrings, default en"},"no_cache":{"type":"boolean","description":"Index plugins from their files, not the index cache"}},"required":["type"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "package",
        label: "Package Mod...",
        menu_path: "File/Package Mod...",
        shortcut: None,
        aliases: &[],
        summary: "Validate a plugin, then write an MO2-ready mod folder (plugin, loose files, scripts at their Data paths, or packed into '<plugin> - Main.ba2' / '- Textures.ba2'), kitcraft-manifest.json (SHA-256 of every file, masters, record counts, KitCraft version, spec hash) and a .zip; never installs",
        args_schema: r#"{"type":"object","properties":{"plugin":{"type":"string","description":"The plugin to package (.esm, .esp or .esl); validate must pass"},"out":{"type":"string","description":"Folder that receives <name>/ and <name>[-<version>].zip; nothing is written outside it. Never in the game or user Data dir or MO2"},"name":{"type":"string","description":"Mod folder name (default: the plugin's file name without extension)"},"version":{"type":"string","description":"Version for the manifest and the zip name (ASCII letters, digits, . - _)"},"loose":{"type":"string","description":"A folder laid out as Data (Meshes/..., Textures/...): copied, or packed with ba2"},"scripts":{"type":"array","items":{"type":"string"},"description":".pex files or folders of them; placed at Scripts/<namespace>/<name>.pex from the script name inside"},"ba2":{"type":"boolean","default":false,"description":"Pack loose files and scripts into '<plugin stem> - Main.ba2' (GNRL) and textures into '<plugin stem> - Textures.ba2' (DX10)"},"ba2_version":{"type":"integer","minimum":2,"maximum":3,"default":2,"description":"Archive version: 2 (zlib, as the official plugins' archives) or 3 (LZ4)"},"ba2_compress":{"type":"boolean","default":true},"zip":{"type":"boolean","default":true,"description":"Also write the zip of the folder's contents"},"spec":{"type":"string","description":"The build spec the plugin came from; its SHA-256 goes into the manifest"},"overwrite":{"type":"boolean","default":false,"description":"Replace a folder (and zip) an earlier package wrote; a folder without kitcraft-manifest.json is never replaced"},"profile":{"type":"string","description":"MO2 profile whose load order validate searches for masters (read only)"},"plugins":{"type":"array","items":{"type":"string"},"description":"Plugin files validate searches first for masters, in load order"},"data_dir":{"type":"string","description":"Game Data dir for masters (default KITCRAFT_DATA_DIR)"},"no_cache":{"type":"boolean","description":"Index plugins from their files, not the index cache"}},"required":["plugin","out"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "validate",
        label: "Validate Plugin",
        menu_path: "Tools/Validate",
        shortcut: Some("F7"),
        aliases: &["check"],
        summary: "Missing masters, bad links, localized FULL copies, stripped FULL/NTRM, god-mode script calls",
        args_schema: r#"{"type":"object","properties":{"plugin":{"type":"string","description":"Plugin to check (give plugin, scripts, or both)"},"scripts":{"type":"array","items":{"type":"string"},"description":".pex files, directories of them, or .ba2 archives (their .pex entries) to scan"},"plugin_archives":{"type":"boolean","default":false,"description":"Also scan the scripts in the plugin's own archives (<plugin stem> - *.ba2 next to it)"},"rules":{"type":"array","items":{"type":"string"},"description":"Rule ids to run (default: every rule for the inputs given)"},"profile":{"type":"string","description":"MO2 profile to load first; its plugins are searched first for masters"},"plugins":{"type":"array","items":{"type":"string"},"description":"Plugin files to load first, in load order; searched first for masters"},"data_dir":{"type":"string","description":"Game Data dir (default KITCRAFT_DATA_DIR): a bare plugin or script name resolves in the session's load order, then the working directory, then here, and masters are searched here after the load order and the plugin's own dir"},"no_cache":{"type":"boolean","description":"Index plugins from their files, not the index cache"}},"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "roundtrip",
        label: "Roundtrip Plugin",
        menu_path: "Tools/Roundtrip",
        shortcut: None,
        aliases: &[],
        summary: "Parse and rewrite a plugin; --verify demands byte-identical output (M1 gate)",
        args_schema: r#"{"type":"object","properties":{"plugin":{"type":"string"},"verify":{"type":"boolean","default":false,"description":"Compare the rewritten bytes with the input and report the first mismatch"},"out":{"type":"string","description":"Write the rewritten plugin here; an existing file is refused unless overwrite is true. A relative path resolves against the kit process's working directory"},"overwrite":{"type":"boolean","default":false,"description":"Let out replace an existing file"},"decode":{"type":"boolean","default":false,"description":"Also inflate every compressed record strictly (declared size, end marker and checksum, nothing after the stream) and re-encode its subrecords; a body that fails is a mismatch (passed: false)"},"data_dir":{"type":"string","description":"Game Data dir (default KITCRAFT_DATA_DIR): a bare plugin name resolves in the session's load order, then the working directory, then here"}},"required":["plugin"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "coverage",
        label: "Schema Coverage",
        menu_path: "Tools/Schema Coverage",
        shortcut: None,
        aliases: &[],
        summary: "Record types and subrecords in a plugin vs what the schema defines (typed, opaque, unknown)",
        args_schema: r#"{"type":"object","properties":{"plugin":{"type":"string"},"decode":{"type":"boolean","default":true,"description":"Decompress compressed records (default limits) and count their subrecords; --no-decode skips them"},"signatures":{"type":"boolean","description":"List each record type's subrecord signatures (record_types[].signatures; null when false). Omitted: true from kit, false over MCP"},"top":{"type":"integer","minimum":0,"description":"List only the N record types with the most records; the totals still cover every type. Omitted: all from kit, 20 over MCP"},"data_dir":{"type":"string","description":"Game Data dir (default KITCRAFT_DATA_DIR): a bare plugin name resolves in the session's load order, then the working directory, then here"}},"required":["plugin"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "schema",
        label: "Record Schema",
        menu_path: "Help/Record Schema",
        shortcut: None,
        aliases: &["defs"],
        summary: "What the record schema defines for a type: members with type, source citation and confidence (top level; --deep for nested); without --type, every definition with per-confidence counts",
        args_schema: r#"{"type":"object","properties":{"type":{"type":"string","minLength":4,"maxLength":4,"description":"Record type (WEAP) or header (GRUP); omit to list every definition"},"member":{"type":"string","description":"Only this member (name or signature; also one inside a group), shown deep"},"deep":{"type":"boolean","default":false,"description":"Expand group members, struct fields, enum and flag names and union variants, and list evidence citations"}},"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "ba2.list",
        label: "List Archive",
        menu_path: "Archive/List",
        shortcut: None,
        aliases: &[],
        summary: "List files in a .ba2 archive (GNRL or DX10): sizes, compression, texture formats",
        args_schema: r#"{"type":"object","properties":{"archive":{"type":"string"},"filter":{"type":"string","description":"Case-insensitive regex over archive paths"},"limit":{"type":"integer","minimum":1,"default":1000,"description":"Most entries to return"},"start":{"type":"integer","minimum":0,"default":0,"description":"Skip this many matching entries"},"data_dir":{"type":"string","description":"Game Data dir (default KITCRAFT_DATA_DIR): a bare archive name resolves in the working directory, then here"}},"required":["archive"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "ba2.extract",
        label: "Extract from Archive...",
        menu_path: "Archive/Extract...",
        shortcut: None,
        aliases: &[],
        summary: "Extract files from a .ba2 archive (all, or those matching --pattern)",
        args_schema: r#"{"type":"object","properties":{"archive":{"type":"string"},"pattern":{"type":"string","description":"Case-insensitive regex over archive paths; omit to extract everything"},"out_dir":{"type":"string","description":"Never under the game Data directory"},"overwrite":{"type":"boolean","default":false,"description":"Replace files that already exist"},"max_entry_bytes":{"type":"integer","minimum":0,"default":1073741824,"description":"Refuse, before writing anything, an entry that extracts to more bytes than this (default 1 GiB)"},"data_dir":{"type":"string","description":"Game Data dir (default KITCRAFT_DATA_DIR): a bare archive name resolves in the working directory, then here (out_dir never does)"}},"required":["archive","out_dir"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "ba2.pack",
        label: "Pack Archive...",
        menu_path: "Archive/Pack...",
        shortcut: None,
        aliases: &[],
        summary: "Pack a directory (as the Data root) into a .ba2 archive; deterministic",
        args_schema: r#"{"type":"object","properties":{"dir":{"type":"string"},"out":{"type":"string","description":"Never under the game Data directory"},"format":{"type":"string","enum":["gnrl","dx10"],"default":"gnrl","description":"dx10 packs .dds textures"},"version":{"type":"integer","minimum":2,"maximum":3,"default":2,"description":"2: zlib, 3: LZ4 blocks"},"compress":{"type":"boolean","default":true},"overwrite":{"type":"boolean","default":false,"description":"Let out replace an existing file"}},"required":["dir","out"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "papyrus.disasm",
        label: "Disassemble Script",
        menu_path: "Papyrus/Disassemble",
        shortcut: None,
        aliases: &["pexdis"],
        summary: "Disassemble a compiled Papyrus .pex (loose or inside a .ba2) as pexdis.py does, or as JSON",
        args_schema: r#"{"type":"object","properties":{"pex":{"type":"string","description":"Script file, or its path inside archive"},"archive":{"type":"string","description":"Read the script from this .ba2"},"format":{"type":"string","enum":["text","json","both"],"default":"text","description":"text: pexdis.py format; json: the structured model with offsets"},"data_dir":{"type":"string","description":"Game Data dir (default KITCRAFT_DATA_DIR): a bare archive (or loose script) name resolves in the working directory, then here"}},"required":["pex"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "papyrus.compile",
        label: "Compile Script...",
        menu_path: "Papyrus/Compile...",
        shortcut: None,
        aliases: &[],
        summary: "Compile one script with Bethesda's PapyrusCompiler.exe by name with import dirs or Tools/ContentResources.zip (never -release)",
        args_schema: r#"{"type":"object","properties":{"source":{"type":"string","description":"Script name (MyScript, Ns:MyScript) or a .psc path (its folder is imported first); .ppj projects are refused"},"out_dir":{"type":"string","description":"Where the .pex goes; never under the game Data directory"},"imports":{"type":"array","items":{"type":"string"},"description":"Import directories, in order"},"sources_zip":{"type":"string","description":"Tools/ContentResources.zip: its Scripts/Source files are extracted once into the KitCraft cache and imported after the import directories"},"flags":{"type":"string","description":"Flags file; default: Starfield_Papyrus_Flags.flg from the first import directory that has it"},"compiler":{"type":"string","description":"PapyrusCompiler.exe; default: KITCRAFT_PAPYRUS_COMPILER, then the game's Tools folder"},"optimize":{"type":"boolean","default":false}},"required":["source","out_dir"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "papyrus.events",
        label: "Script Events and Functions...",
        menu_path: "Papyrus/Events and Functions...",
        shortcut: None,
        aliases: &[],
        summary: "Events, functions, properties, structs and custom events the base game's Papyrus sources declare (Tools/ContentResources.zip, read in place, index cached), by script with its parents, by name or by search, with file:line evidence",
        args_schema: r#"{"type":"object","properties":{"script":{"type":"string","description":"Script name (ObjectReference, Ns:Script): its members, and with inherited its parents' too"},"event":{"type":"string","description":"Only events with this name (sets kind to event)"},"function":{"type":"string","description":"Only functions with this name (sets kind to function)"},"name":{"type":"string","description":"Only members with this name"},"kind":{"type":"string","enum":["all","event","function","property","struct","custom_event"],"default":"all"},"search":{"type":"string","description":"Case-insensitive text in the script or member name"},"inherited":{"type":"boolean","default":true,"description":"With script: also list the members its parent chain declares"},"declarations_only":{"type":"boolean","default":false,"description":"Drop members that override an ancestor's (event handlers) and remote event handlers"},"with_comments":{"type":"boolean","default":false,"description":"Add each match's doc and comment, read now from the zip (Bethesda's text: never cached)"},"limit":{"type":"integer","minimum":1,"maximum":1000,"default":100},"start":{"type":"integer","minimum":0,"default":0},"sources_zip":{"type":"string","description":"Tools/ContentResources.zip; default: next to the game Data dir (data_dir, KITCRAFT_DATA_DIR), then the Steam install"},"data_dir":{"type":"string","description":"Game Data dir whose ../Tools holds the zip (default KITCRAFT_DATA_DIR)"},"no_cache":{"type":"boolean","default":false,"description":"Parse the sources again instead of reading (or writing) the cached index"}},"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "strings.dump",
        label: "Dump Strings",
        menu_path: "Strings/Dump",
        shortcut: None,
        aliases: &[],
        summary: "Dump a .STRINGS/.DLSTRINGS/.ILSTRINGS table (loose or inside a .ba2), by ID or text",
        args_schema: r#"{"type":"object","properties":{"table":{"type":"string","description":"Table file, or its path inside archive"},"archive":{"type":"string","description":"Read the table from this .ba2"},"id":{"type":"string","description":"String ID, 0x-prefixed hex or decimal"},"contains":{"type":"string","description":"Case-insensitive text filter"},"limit":{"type":"integer","minimum":1,"default":100},"start":{"type":"integer","minimum":0,"default":0},"tolerant":{"type":"boolean","default":false,"description":"Also read a table padded past its header's data size; the padding is named in limitations"},"data_dir":{"type":"string","description":"Game Data dir (default KITCRAFT_DATA_DIR): a bare archive (or loose table) name resolves in the working directory, then here"}},"required":["table"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "strings.merge",
        label: "Merge Strings...",
        menu_path: "Strings/Merge...",
        shortcut: None,
        aliases: &[],
        summary: "Merge string tables of one kind into one (later table wins; conflicts reported; deterministic)",
        args_schema: r#"{"type":"object","properties":{"tables":{"type":"array","items":{"type":"string"},"minItems":2},"out":{"type":"string","description":"Same extension as the inputs; never under the game Data directory"},"tolerant":{"type":"boolean","default":false,"description":"Also read tables padded past their header's data size; the padding is named in limitations and not written"},"data_dir":{"type":"string","description":"Game Data dir (default KITCRAFT_DATA_DIR): a bare table name resolves in the working directory, then here (out never does)"}},"required":["tables","out"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "deploy",
        label: "Deploy to MO2 (Dev)...",
        menu_path: "File/Deploy to MO2 (Dev)...",
        shortcut: None,
        aliases: &[],
        summary: "Development deployment (kit deploy --dev, ADR 0009): copy a plugin or package folder into the isolated MO2 mod `KitKraft Dev - <plugin>` (hashed manifest, staged folder swap, journal, previous kept for rollback); refuses the Default profile; never edits a profile or another mod",
        args_schema: r#"{"type":"object","properties":{"source":{"type":"string","description":"A plugin (.esm/.esp/.esl) or a package folder (plugin at the root, loose files and .ba2 at their Data paths; kitcraft-manifest.json verified when present)"},"mo2":{"type":"string","description":"The MO2 instance root (holds ModOrganizer.ini); required on every call"},"dev":{"type":"boolean","enum":[true],"description":"Required, always true: development deployment is opt-in on every call"},"profile":{"type":"string","description":"The MO2 profile to check (never Default; KitKraft Dev): read-only diagnostics of whether the mod and plugin are enabled"},"dry_run":{"type":"boolean","default":false,"description":"Report the plan, refusals and every path that would be touched; write nothing"}},"required":["source","mo2","dev"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "deploy.rollback",
        label: "Roll Back Dev Deployment",
        menu_path: "File/Roll Back Dev Deployment",
        shortcut: None,
        aliases: &[],
        summary: "Swap a dev mod's previous deployment (`KitKraft Dev - <plugin>.previous`) back in, keeping the replaced one as the new previous (ADR 0009); refuses the Default profile",
        args_schema: r#"{"type":"object","properties":{"mo2":{"type":"string","description":"The MO2 instance root (holds ModOrganizer.ini)"},"mod":{"type":"string","description":"The dev mod: `KitKraft Dev - <plugin stem>` or the plugin stem"},"dev":{"type":"boolean","enum":[true],"description":"Required, always true"},"to":{"type":"string","description":"The deployment id expected in .previous; refused if it holds another"},"profile":{"type":"string","description":"The MO2 profile to check (never Default): read-only diagnostics"},"dry_run":{"type":"boolean","default":false,"description":"Report the plan and every path that would be touched; write nothing"}},"required":["mo2","mod","dev"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
    CommandSpec {
        id: "deploy.status",
        label: "Dev Deployment Status",
        menu_path: "File/Dev Deployment Status",
        shortcut: None,
        aliases: &[],
        summary: "Read-only: the KitKraft dev mods in an MO2 instance, their manifests, hand edits, previous deployments, interrupted operations, and whether each is enabled in a profile",
        args_schema: r#"{"type":"object","properties":{"mo2":{"type":"string","description":"The MO2 instance root (holds ModOrganizer.ini)"},"mod":{"type":"string","description":"Only this dev mod (`KitKraft Dev - <plugin stem>` or the stem)"},"profile":{"type":"string","description":"Profile whose enabled state is read (default: MO2's active profile)"}},"required":["mo2"],"additionalProperties":false}"#,
        status: Status::Implemented,
    },
];

#[cfg(test)]
mod tests {
    use super::{catalog, find_command, Status};
    use std::collections::BTreeSet;

    #[test]
    fn ids_and_aliases_are_unique() {
        let mut seen = BTreeSet::new();
        for spec in catalog() {
            assert!(seen.insert(spec.id), "duplicate name {}", spec.id);
            for alias in spec.aliases {
                assert!(seen.insert(*alias), "duplicate name {alias}");
            }
        }
    }

    #[test]
    fn every_args_schema_is_a_json_object_schema() {
        for spec in catalog() {
            let value: serde_json::Value = serde_json::from_str(spec.args_schema)
                .unwrap_or_else(|e| panic!("{}: args_schema is not JSON: {e}", spec.id));
            assert_eq!(value["type"], "object", "{}", spec.id);
        }
    }

    #[test]
    fn lookup_by_id_and_alias() {
        assert_eq!(find_command("ba2.list").map(|s| s.id), Some("ba2.list"));
        assert_eq!(find_command("show").map(|s| s.id), Some("inspect"));
        assert!(find_command("nope").is_none());
    }

    /// M2.F8 (M2 exit): every command is `Implemented`, or `Deferred` with a reason and a
    /// milestone. Nothing is `Planned` any more: a command that does only part of its job
    /// is `Implemented` and says what it does not do in `limitations[]`.
    #[test]
    fn catalog_closure() {
        for spec in catalog() {
            match spec.status {
                Status::Implemented => {}
                Status::Deferred { reason, milestone } => {
                    assert!(!reason.trim().is_empty(), "{}: empty reason", spec.id);
                    assert!(
                        milestone.starts_with('M') && milestone.len() > 1,
                        "{}: milestone `{milestone}` is not like `M3`",
                        spec.id
                    );
                }
                Status::Planned => panic!(
                    "{} is Planned at M2 exit: implement it (stating what it does not do in \
                     limitations[]) or defer it with Status::Deferred {{ reason, milestone }}",
                    spec.id
                ),
            }
        }
    }

    #[test]
    fn status_notes() {
        assert_eq!(Status::Implemented.note(), "");
        assert_eq!(Status::Planned.note(), "planned: not implemented yet");
        let deferred = Status::Deferred {
            reason: "needs the renderer",
            milestone: "M4",
        };
        assert_eq!(deferred.note(), "deferred to M4: needs the renderer");
        assert_eq!(deferred.label(), "deferred");
    }

    #[test]
    fn every_args_schema_uses_only_supported_keywords() {
        for spec in catalog() {
            let schema = crate::args::parse_schema(spec.id, spec.args_schema).unwrap();
            crate::args::check_schema(spec.id, &schema)
                .unwrap_or_else(|e| panic!("{}: {e}", spec.id));
        }
    }
}

#[cfg(test)]
mod execute_tests {
    use super::{
        catalog, execute, CommandHandler, Coverage, Engine, EngineError, Evidence, Outcome,
        Session, Status,
    };
    use serde_json::{json, Value};

    #[test]
    fn unknown_id_is_a_typed_error() {
        let err = execute("nope", json!({})).unwrap_err();
        assert!(matches!(err, EngineError::UnknownCommand(ref id) if id == "nope"));
        assert_eq!(err.kind(), "unknown_command");
    }

    #[test]
    fn schema_invalid_args_are_a_typed_error_with_pointer_and_rule() {
        let err = execute("roundtrip", json!({"verify": "yes", "bogus": 1})).unwrap_err();
        let EngineError::InvalidArgs {
            command,
            violations,
        } = &err
        else {
            panic!("expected InvalidArgs, got {err:?}");
        };
        assert_eq!(command, "roundtrip");
        let got: Vec<(&str, &str)> = violations
            .iter()
            .map(|v| (v.pointer.as_str(), v.rule))
            .collect();
        assert_eq!(
            got,
            [
                ("/plugin", "required"),
                ("/bogus", "additionalProperties"),
                ("/verify", "type"),
            ]
        );
        assert_eq!(err.kind(), "invalid_args");
        assert!(err.to_string().contains("/plugin"));
    }

    #[test]
    fn catalog_accepts_paging_and_extract_all() {
        let query = crate::find_command("query").unwrap();
        crate::prepare_args(query, json!({"type": "WEAP", "cursor": "abc"})).unwrap();
        let extract = crate::find_command("ba2.extract").unwrap();
        crate::prepare_args(extract, json!({"archive": "a.ba2", "out_dir": "o"})).unwrap();
        let coverage = crate::find_command("coverage").unwrap();
        assert_eq!(coverage.status, Status::Implemented);
    }

    #[test]
    fn args_must_be_an_object() {
        let err = execute("roundtrip", json!(["x.esm"])).unwrap_err();
        assert_eq!(err.kind(), "invalid_args");
    }

    #[test]
    fn command_without_handler_is_not_implemented() {
        // No catalog command is Planned any more (M2.F8); an engine without the handler
        // is the not-implemented case.
        let engine = Engine::empty();
        let err = engine
            .execute("build", json!({"spec": "a.toml", "out": "b.esm"}))
            .unwrap_err();
        assert!(matches!(err, EngineError::NotImplemented(ref id) if id == "build"));
        // Validation still runs first.
        assert_eq!(
            engine.execute("build", json!({})).unwrap_err().kind(),
            "invalid_args"
        );
    }

    #[test]
    fn stub_handler_returns_the_full_envelope() {
        let mut engine = Engine::empty();
        engine
            .register(
                "validate",
                Box::new(|_: &Value| Ok(Outcome::not_implemented())),
            )
            .unwrap();
        let result = engine
            .execute("validate", json!({"plugin": "Constellation.esm"}))
            .unwrap();
        assert_eq!(
            result,
            json!({
                "command": "validate",
                "data": null,
                "coverage": {
                    "status": "unknown",
                    "decoded_subrecords": null,
                    "opaque_subrecords": null,
                    "not_examined": []
                },
                "limitations": ["not implemented"],
                "evidence": []
            })
        );
    }

    #[test]
    fn alias_resolves_to_canonical_id_in_envelope() {
        let mut engine = Engine::empty();
        engine
            .register(
                "inspect",
                Box::new(|_: &Value| Ok(Outcome::not_implemented())),
            )
            .unwrap();
        let result = engine
            .execute("show", json!({"form": "0001A2B3", "plugin": "X.esm"}))
            .unwrap();
        assert_eq!(result["command"], "inspect");
    }

    struct Echo;
    impl CommandHandler for Echo {
        fn run(&self, args: &Value) -> Result<Outcome, EngineError> {
            Ok(Outcome {
                data: args.clone(),
                coverage: Coverage::unknown(),
                limitations: Vec::new(),
                evidence: vec![Evidence {
                    file: "X.esm".into(),
                    offset: 24,
                    length: 6,
                    record: Some("WEAP:0001A2B3".into()),
                    subrecord: Some("FULL".into()),
                }],
            })
        }
    }

    #[test]
    fn handlers_see_defaults_and_evidence_is_serialized() {
        let mut engine = Engine::empty();
        engine.register("roundtrip", Box::new(Echo)).unwrap();
        let result = engine
            .execute("roundtrip", json!({"plugin": "X.esm"}))
            .unwrap();
        assert_eq!(
            result["data"],
            json!({"plugin": "X.esm", "verify": false, "overwrite": false, "decode": false})
        );
        assert_eq!(result["limitations"], json!([]));
        assert_eq!(
            result["evidence"],
            json!([{"file": "X.esm", "offset": 24, "length": 6,
                    "record": "WEAP:0001A2B3", "subrecord": "FULL"}])
        );
    }

    #[test]
    fn null_args_mean_empty_object() {
        let mut engine = Engine::empty();
        engine.register("load", Box::new(Echo)).unwrap();
        let result = engine.execute("load", Value::Null).unwrap();
        assert_eq!(result["data"], json!({}));
    }

    #[test]
    fn registration_rejects_unknown_and_duplicate_ids() {
        let mut engine = Engine::empty();
        assert_eq!(
            engine.register("nope", Box::new(Echo)).unwrap_err().kind(),
            "registration"
        );
        assert_eq!(
            engine.register("show", Box::new(Echo)).unwrap_err().kind(),
            "registration"
        );
        engine.register("inspect", Box::new(Echo)).unwrap();
        assert_eq!(
            engine
                .register("inspect", Box::new(Echo))
                .unwrap_err()
                .kind(),
            "registration"
        );
    }

    #[test]
    fn handler_failures_propagate() {
        let mut engine = Engine::empty();
        engine
            .register(
                "validate",
                Box::new(|_: &Value| -> Result<Outcome, EngineError> {
                    Err(EngineError::Failed {
                        command: "validate".into(),
                        message: "bad record at offset 0x18".into(),
                    })
                }),
            )
            .unwrap();
        let err = engine
            .execute("validate", json!({"plugin": "X.esm"}))
            .unwrap_err();
        assert_eq!(err.kind(), "failed");
    }

    /// A session handler that counts its calls in a session slot and mirrors the count in
    /// the load-order listing.
    fn counter(session: &mut Session, _: &Value) -> Result<Outcome, EngineError> {
        struct Calls(u64);
        let calls = match session.get_mut::<Calls>() {
            Some(calls) => {
                calls.0 = calls.0.saturating_add(1);
                calls.0
            }
            None => {
                session.insert(Calls(1));
                1
            }
        };
        session.set_load_order(Some(json!({ "calls": calls })));
        Ok(Outcome {
            data: json!({ "calls": calls }),
            ..Outcome::not_implemented()
        })
    }

    #[test]
    fn session_handler_keeps_state_across_calls_on_one_engine() {
        let mut engine = Engine::empty();
        engine.register_session("load", Box::new(counter)).unwrap();
        engine.register("validate", Box::new(Echo)).unwrap();
        assert!(engine.has_handler("load"));
        assert_eq!(
            engine.execute("load", json!({})).unwrap()["data"]["calls"],
            1
        );
        // A stateless command in between leaves the session alone.
        engine
            .execute("validate", json!({"plugin": "X.esm"}))
            .unwrap();
        assert_eq!(
            engine.execute("open", json!({})).unwrap()["data"]["calls"],
            2
        );
        let listing = engine.with_session(|s| s.load_order().cloned());
        assert_eq!(listing, Some(json!({"calls": 2})));

        // A caller-owned session is separate from the engine's.
        let mut fresh = Session::new();
        assert!(fresh.is_empty());
        let result = engine.execute_in(&mut fresh, "load", json!({})).unwrap();
        assert_eq!(result["data"]["calls"], 1);
        assert!(!fresh.is_empty());
        fresh.clear();
        assert!(fresh.is_empty());
        assert_eq!(
            engine.execute("load", json!({})).unwrap()["data"]["calls"],
            3
        );
    }

    #[test]
    fn session_slots_are_typed() {
        let mut session = Session::new();
        assert_eq!(session.insert(5u32), None);
        assert_eq!(session.insert(7u32), Some(5));
        session.insert(String::from("x"));
        assert_eq!(session.get::<u32>(), Some(&7));
        assert_eq!(session.get::<String>().map(String::as_str), Some("x"));
        assert_eq!(session.get::<u64>(), None);
        assert_eq!(session.remove::<u32>(), Some(7));
        assert_eq!(session.get::<u32>(), None);
    }

    #[test]
    fn deferred_commands_are_refused_with_their_reason() {
        let spec = crate::CommandSpec {
            id: "render",
            label: "Render",
            menu_path: "View/Render",
            shortcut: None,
            aliases: &[],
            summary: "Render a cell",
            args_schema: r#"{"type":"object"}"#,
            status: Status::Deferred {
                reason: "needs crates/render",
                milestone: "M4",
            },
        };
        let err = crate::exec::refuse_deferred(&spec).unwrap_err();
        assert_eq!(err.kind(), "deferred");
        assert_eq!(
            err.to_string(),
            "`render` is deferred to M4: needs crates/render"
        );
        let implemented = crate::find_command("roundtrip").unwrap();
        assert!(crate::exec::refuse_deferred(implemented).is_ok());
    }

    #[test]
    fn default_handlers_cover_every_implemented_command() {
        let engine = Engine::with_default_handlers();
        assert!(engine.has_handler("roundtrip"));
        assert!(engine.has_handler("inspect"));
        assert!(engine.has_handler("coverage"));
        for spec in catalog() {
            assert_eq!(
                spec.status == Status::Implemented,
                engine.has_handler(spec.id),
                "{}: status and handler disagree",
                spec.id
            );
        }
    }
}
