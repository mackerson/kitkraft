//! M4.W0: the documented JSON shape of every inspection command's envelope
//! (`docs/mcp.md`, "Data shapes"), checked by tests against real envelopes.
//!
//! Two promises a front end can key on, instead of guessing from a value's shape:
//!
//! - **Markers.** Every JSON object that stands for something other than a struct carries
//!   `"kind"` with one of the [`kind`] names: a stored FormID value is a `formref`, a
//!   string ID an `lstring`, undecoded bytes `opaque`, a typed FormID link a `link`, an
//!   incoming referrer a `referrer`, and each envelope `evidence` entry is `evidence`.
//!   A decoded value is otherwise numbers, strings, arrays and structs, and a struct never
//!   has a member named `kind` (schema member names are PascalCase; [`check_envelope`]
//!   reports an unmarked object that looks like a FormID value).
//! - **Shapes.** [`data_shape`] gives the members of `data` for each command in
//!   [`INSPECTION_COMMANDS`], with their JSON types; [`check_envelope`] lists every place
//!   an envelope differs from it, including members the shape does not document.
//!
//! `limitations` entries stay strings: they are prose for a reader, never a value to
//! format, and every front end (the CLI's text mode, MCP clients) reads them as such.

use kitkraft_doc::LoadOrder;
use serde_json::{json, Map as JsonMap, Value};

/// The member that names a marker object's kind.
pub const KIND: &str = "kind";

/// A FormID as `plugin` stores it, as a [`kind::FORMREF`] object resolved through that
/// plugin's masters: `{kind, raw, form}`, `{kind, raw, form: null, null: true}` for the
/// null FormID, `{kind, raw, form: null, error}` when it does not resolve.
pub(crate) fn formref(lo: &LoadOrder, plugin: u16, raw: u32) -> Value {
    if raw == 0 {
        return json!({"kind": kind::FORMREF, "raw": "00000000", "form": null, "null": true});
    }
    match lo.resolve(plugin, raw) {
        Ok(g) => {
            json!({"kind": kind::FORMREF, "raw": format!("{raw:08X}"), "form": lo.form_label(g)})
        }
        Err(e) => json!({
            "kind": kind::FORMREF, "raw": format!("{raw:08X}"), "form": null, "error": e.to_string()
        }),
    }
}

/// [`formref`] for a FormID stored in a file outside the load order (`set`'s dry run,
/// M7.S6), resolved through its master map; its own forms are named by `file`.
pub(crate) fn formref_in(
    lo: &LoadOrder,
    map: &kitkraft_doc::MasterMap,
    file: &str,
    raw: u32,
) -> Value {
    if raw == 0 {
        return json!({"kind": kind::FORMREF, "raw": "00000000", "form": null, "null": true});
    }
    match map.resolve(raw) {
        Ok(g) if g.plugin == map.own_index => json!({
            "kind": kind::FORMREF, "raw": format!("{raw:08X}"),
            "form": format!("{file}:{:06X}", g.object_id)
        }),
        Ok(g) => {
            json!({"kind": kind::FORMREF, "raw": format!("{raw:08X}"), "form": lo.form_label(g)})
        }
        Err(e) => json!({
            "kind": kind::FORMREF, "raw": format!("{raw:08X}"), "form": null, "error": e.to_string()
        }),
    }
}

/// Marker names (the value of [`KIND`]).
pub mod kind {
    /// A stored FormID value: `{kind, form, raw?, null?, error?, editor_id?}`. `form` is
    /// the formref (`Starfield.esm:01A2B3`), or `null` for the null FormID (`null: true`)
    /// and for one that does not resolve (`error` says why); `raw` is the FormID as
    /// stored, where the command reads it.
    pub const FORMREF: &str = "formref";
    /// A localized string ID inside a decoded value: `{kind, string_id}`.
    pub const LSTRING: &str = "lstring";
    /// Bytes the schema keeps undecoded: `{kind, opaque}`, where `opaque` is a byte range
    /// `{payload_offset, length, file_offset}` (`inspect`, `diff`, `conflicts`) or the
    /// bytes as hex (`planet` conditions).
    pub const OPAQUE: &str = "opaque";
    /// A typed FormID link (`refs` outgoing edges, `planet` content and node links):
    /// `{kind, path, raw, status, to, to_signature, to_editor_id, targets,
    /// target_allowed, resolved_by, error, offset, body_offset}`.
    pub const LINK: &str = "link";
    /// A record whose typed members name the form (`refs --incoming` edges).
    pub const REFERRER: &str = "referrer";
    /// An envelope `evidence` entry: `{kind, file, offset, length, record, subrecord}`.
    pub const EVIDENCE: &str = "evidence";
    /// Every marker name.
    pub const ALL: &[&str] = &[FORMREF, LSTRING, OPAQUE, LINK, REFERRER, EVIDENCE];
    /// The markers that occur inside a decoded value (`Shape::Value`).
    pub const IN_VALUES: &[&str] = &[FORMREF, LSTRING, OPAQUE];
}

/// The commands whose `data` has a documented shape (M4.W0): the read-only inspection
/// surface the workbench is built on.
pub const INSPECTION_COMMANDS: &[&str] = &[
    "load",
    "inspect",
    "query",
    "diff",
    "conflicts",
    "refs",
    "planet",
    "coverage",
    "schema",
    "validate",
    "cell",
];

/// A JSON type in a documented shape.
#[derive(Debug, Clone, Copy)]
pub enum Shape {
    /// Anything (an open part, documented in prose).
    Any,
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool,
    /// An integer.
    Int,
    /// Any number.
    Num,
    /// A string.
    Str,
    /// Exactly this string.
    Lit(&'static str),
    /// One of these strings.
    Enum(&'static [&'static str]),
    /// The inner shape or `null`.
    Nullable(&'static Shape),
    /// An array of the inner shape.
    List(&'static Shape),
    /// An object with any keys, each value of the inner shape.
    Map(&'static Shape),
    /// An object with exactly these members (groups of members, concatenated).
    Obj(&'static [&'static [Member]]),
    /// An object with at least these members; others may follow.
    Open(&'static [&'static [Member]]),
    /// A decoded schema value: numbers, strings, arrays, structs (objects without `kind`)
    /// and the [`kind::IN_VALUES`] markers.
    Value,
    /// An object carrying this marker (its shape: [`marker_shape`]).
    Marker(&'static str),
    /// The first of these that matches.
    Either(&'static [Shape]),
    /// An object whose shape depends on the value of one member.
    By(&'static str, &'static [(Case, Shape)]),
}

/// One member of an object shape.
#[derive(Debug, Clone, Copy)]
pub struct Member {
    /// The member name.
    pub name: &'static str,
    /// Its shape.
    pub shape: Shape,
    /// Whether it is always present.
    pub required: bool,
}

/// The value a [`Shape::By`] case matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// The member is absent.
    Absent,
    /// The member is this string.
    Str(&'static str),
    /// The member is this boolean.
    Bool(bool),
}

const fn req(name: &'static str, shape: Shape) -> Member {
    Member {
        name,
        shape,
        required: true,
    }
}

const fn opt(name: &'static str, shape: Shape) -> Member {
    Member {
        name,
        shape,
        required: false,
    }
}

use Shape::{
    Any, Bool, By, Either, Enum, Int, List, Lit, Map, Marker, Null, Nullable, Num, Obj, Open, Str,
};

const NSTR: Shape = Nullable(&Str);
const NINT: Shape = Nullable(&Int);
const STRS: Shape = List(&Str);

/// The marker objects' own shapes (open: a position may add members, which its own
/// shape then documents).
pub fn marker_shape(name: &str) -> Option<&'static Shape> {
    match name {
        kind::FORMREF => Some(&FORMREF),
        kind::LSTRING => Some(&LSTRING),
        kind::OPAQUE => Some(&OPAQUE),
        kind::LINK => Some(&LINK),
        kind::REFERRER => Some(&REFERRER),
        kind::EVIDENCE => Some(&EVIDENCE),
        _ => None,
    }
}

const FORMREF: Shape = Open(&[&[
    req("kind", Lit(kind::FORMREF)),
    req("form", NSTR),
    opt("raw", Str),
    opt("null", Bool),
    opt("error", Str),
    opt("editor_id", NSTR),
]]);
const LSTRING: Shape = Obj(&[&[req("kind", Lit(kind::LSTRING)), req("string_id", Str)]]);
const BYTE_RANGE: Shape = Obj(&[&[
    req("payload_offset", Int),
    req("length", Int),
    req("file_offset", NINT),
]]);
const OPAQUE: Shape = Obj(&[&[
    req("kind", Lit(kind::OPAQUE)),
    req("opaque", Either(&[BYTE_RANGE, Str])),
]]);
const LINK_STATUS: Shape = Enum(&[
    "resolved",
    "null",
    "none",
    "missing",
    "engine_reserved",
    "unresolvable",
]);
const LINK_MEMBERS: &[Member] = &[
    req("kind", Lit(kind::LINK)),
    req("path", Str),
    req("raw", Str),
    req("status", LINK_STATUS),
    req("to", NSTR),
    req("to_signature", NSTR),
    req("to_editor_id", NSTR),
    req("targets", STRS),
    req("target_allowed", Nullable(&Bool)),
    req("resolved_by", NSTR),
    req("error", NSTR),
    req("offset", NINT),
    req("body_offset", Int),
];
const LINK: Shape = Open(&[LINK_MEMBERS]);
const PLACED: Shape = Obj(&[&[
    req("cell", Marker(kind::FORMREF)),
    req("world", Nullable(&Marker(kind::FORMREF))),
    req("persistent", Bool),
    req("group_type", Int),
]]);
const REFERRER_MEMBERS: &[Member] = &[
    req("kind", Lit(kind::REFERRER)),
    req("depth", Int),
    req("group", Enum(&["referrer", "placed"])),
    req("from", NSTR),
    req("from_raw", Str),
    req("from_signature", Str),
    req("from_editor_id", NSTR),
    req("plugin", Str),
    req("winner", Bool),
    req("to", Str),
    req("placed", Nullable(&PLACED)),
    // The link that names the form; absent when the reverse index lists a record whose
    // schema walk places no such member (a limitation says so).
    opt("path", Str),
    opt("raw", Str),
    opt("status", LINK_STATUS),
    opt("resolved_by", NSTR),
    opt("offset", NINT),
    opt("body_offset", Int),
];
const REFERRER: Shape = Obj(&[REFERRER_MEMBERS]);
const EVIDENCE: Shape = Obj(&[&[
    req("kind", Lit(kind::EVIDENCE)),
    req("file", Str),
    req("offset", Int),
    req("length", Int),
    req("record", NSTR),
    req("subrecord", NSTR),
]]);

const COVERAGE: Shape = Obj(&[&[
    req("status", Enum(&["complete", "partial", "unknown"])),
    req("decoded_subrecords", NINT),
    req("opaque_subrecords", NINT),
    req("not_examined", STRS),
]]);
const ENVELOPE: Shape = Obj(&[&[
    req("command", Str),
    req("data", Any),
    req("coverage", COVERAGE),
    req("limitations", STRS),
    req("evidence", List(&Marker(kind::EVIDENCE))),
]]);

// ---- load ------------------------------------------------------------------------------

const LOAD_PLUGIN: Shape = Obj(&[&[
    req("index", Int),
    req("name", Str),
    req("path", Str),
    req("active", NSTR),
    req("provider", NSTR),
    req("flags", NSTR),
    req("flag_names", Nullable(&STRS)),
    req("scale", Nullable(&Enum(&["full", "medium", "small"]))),
    req("localized", Nullable(&Bool)),
    req("form_version", NINT),
    req(
        "masters",
        List(&Obj(&[&[req("name", Str), req("index", NINT)]])),
    ),
    req("missing_masters", STRS),
    req("late_masters", STRS),
    req("resolvable", Bool),
    req("resolve_error", NSTR),
    req("records", Int),
    req("unresolved_records", Int),
    req("full_style_self_records", Int),
    req("full_style_self_masked_records", Int),
    req(
        "assumed_records",
        List(&Obj(&[&[
            req("raw_form_id", Str),
            req("form", Str),
            req("basis", Str),
        ]])),
    ),
    req("duplicate_records", Int),
    req("runtime_slot", NSTR),
    req("runtime_basis", RUNTIME_BASIS),
    req("cache", Enum(&["hit", "built", "uncached"])),
    req("error", NSTR),
]]);
const RUNTIME_BASIS: Shape = Enum(&["inferred", "confirmed-by-save"]);
/// `load --from-save`: the save read and how its lists compare with the load order.
const LOAD_SAVE: Shape = Obj(&[&[
    req("path", Str),
    req("game_version", Str),
    req("full", Int),
    req("small", Int),
    req("medium", Int),
    // In the decompressed data, not the file.
    req("plugin_block_offset", Int),
    req("plugin_block_size", Int),
    req("confirmed", Bool),
    req("only_in_load_order", STRS),
    req("only_in_save", STRS),
    req("scale_mismatch", STRS),
    req("differs_from_inferred", STRS),
]]);
const LOAD: Shape = Obj(&[&[
    req("runtime_basis", RUNTIME_BASIS),
    req("save", Nullable(&LOAD_SAVE)),
    req("loaded", Bool),
    req("source", Str),
    req("language", Str),
    req("plugins", List(&LOAD_PLUGIN)),
    req("inactive", STRS),
    req("missing_active_plugins", STRS),
    req("records", Int),
    req("versions", Int),
    req("cache_dir", NSTR),
    req("cache_hits", Int),
    req("index_ms", Int),
    req("table_ms", Int),
    req("elapsed_ms", Int),
    req("summary", Str),
]]);

// ---- inspect ---------------------------------------------------------------------------

/// A subrecord difference (`diff`, `conflicts`, `inspect`'s `vs_previous`).
const FIELD: Shape = Obj(&[&[
    req("path", Str),
    req("before", Nullable(&Shape::Value)),
    req("after", Nullable(&Shape::Value)),
]]);
const SUB_DIFF_MEMBERS: &[Member] = &[
    req("signature", Str),
    req("occurrence", Int),
    req("change", Enum(&["added", "removed", "changed"])),
    opt("member", Str),
    opt("fields", List(&FIELD)),
    opt("as_bytes", Str),
];
const SPAN: Shape = Obj(&[&[
    req("file_offset", NINT),
    req("body_offset", Int),
    req("length", Int),
]]);
const SUB_DIFF_SPANS: &[Member] = &[
    req("a", Nullable(&SPAN)),
    req("b", Nullable(&SPAN)),
    req("first_difference", NINT),
];
const SUB_DIFF: Shape = Obj(&[SUB_DIFF_MEMBERS]);
const SUB_DIFF_WITH_SPANS: Shape = Obj(&[SUB_DIFF_MEMBERS, SUB_DIFF_SPANS]);

const RECORD_HEADER_MEMBERS: &[Member] = &[
    req("signature", Str),
    req("flags", Str),
    req("compressed", Bool),
    req("offset", Int),
    req("data_size", Int),
    req("timestamp", Int),
    req("version_control", Int),
    req("form_version", Int),
    req("unknown", Int),
];
const SUBRECORD_MEMBERS: &[Member] = &[
    req("signature", Str),
    req("size", Int),
    req("oversized", Bool),
    req("offset", Int),
    req("data_offset", Int),
    req("payload_hex", Str),
    req("payload_truncated", Bool),
];
/// Text of `EDID`, and an lstring's members for `FULL`/`DESC` and the other lstrings.
const SUBRECORD_TEXT: &[Member] = &[
    opt("text", NSTR),
    opt("string_id", Str),
    opt("string_table", Str),
    opt("string_kind", Str),
    opt("string_offset", NINT),
    opt("malformed_len", Int),
];
const SUBRECORD_TYPED: &[Member] = &[
    opt("member", Str),
    opt("value", Shape::Value),
    // `inspect --full` (M5.D6 addition): the decoded VMAD, an open object.
    opt("vmad", Any),
];
const VERSION: Shape = Obj(&[&[
    req("plugin", Str),
    req("load_index", Int),
    req("winner", Bool),
    req("deleted", Bool),
    req("resolved_by", Str),
    req("raw_form_id", Str),
    req("record", Obj(&[RECORD_HEADER_MEMBERS])),
    req("subrecord_offsets", Enum(&["file", "decompressed"])),
    req(
        "subrecords",
        List(&Obj(&[SUBRECORD_MEMBERS, SUBRECORD_TEXT, SUBRECORD_TYPED])),
    ),
    req(
        "vs_previous",
        Nullable(&Obj(&[&[
            req(
                "class",
                Enum(&["identical-to-previous", "deleted", "differs"]),
            ),
            opt("flags_differ", Bool),
            opt("order_differs", Bool),
            opt("subrecords", List(&SUB_DIFF)),
        ]])),
    ),
    req("payloads_comparable", Nullable(&Bool)),
]]);
const INSPECT_LOAD_ORDER: Shape = Obj(&[&[
    req("mode", Lit("load_order")),
    req("load_order", Str),
    req("language", Str),
    req("form", Str),
    req(
        "matched_by",
        Enum(&["formref", "form_id", "runtime_id", "editor_id"]),
    ),
    req("global", Str),
    req("defining_plugin", Str),
    req("object_id", Str),
    req("runtime_id", NSTR),
    req("runtime_id_basis", NSTR),
    req("signature", Str),
    req("editor_id", NSTR),
    req("winner", Str),
    req("winner_deleted", Bool),
    req(
        "winner_source",
        Obj(&[&[
            req("safe", Bool),
            req("blocking", STRS),
            req("unrelated", STRS),
        ]]),
    ),
    req("summary", Str),
    req("versions", List(&VERSION)),
]]);
const INSPECT_SINGLE: Shape = Obj(&[&[
    req("plugin", Str),
    req("form", Str),
    req("matched_by", Enum(&["form_id", "editor_id"])),
    req(
        "record",
        Obj(&[
            RECORD_HEADER_MEMBERS,
            &[
                req("form_id", Str),
                req("editor_id", NSTR),
                req(
                    "groups",
                    List(&Obj(&[&[
                        req("offset", Int),
                        req("type", Int),
                        req("label", Str),
                    ]])),
                ),
            ],
        ]),
    ),
    req("subrecord_offsets", Enum(&["file", "decompressed"])),
    req("subrecords", List(&Obj(&[SUBRECORD_MEMBERS]))),
]]);
const INSPECT: Shape = By(
    "mode",
    &[
        (Case::Str("load_order"), INSPECT_LOAD_ORDER),
        (Case::Absent, INSPECT_SINGLE),
    ],
);

// ---- query -----------------------------------------------------------------------------

/// An lstring as `inspect` and `query` show it: inline `{text}`, a string ID with the
/// table's text, or `{malformed_len}`.
const LSTRING_TEXT: Shape = Obj(&[&[
    opt("text", NSTR),
    opt("string_id", Str),
    opt("string_table", Str),
    opt("string_kind", Str),
    opt("string_offset", NINT),
    opt("malformed_len", Int),
]]);
const QUERY: Shape = Obj(&[&[
    req("type", Str),
    req("where", NSTR),
    req("scope", Enum(&["winner", "all"])),
    req("limit", Int),
    req("matched", Int),
    req("unknown", Int),
    req("scanned", Int),
    req("candidates", Int),
    req("next_cursor", NSTR),
    req(
        "results",
        List(&Obj(&[&[
            req("form", Str),
            req("signature", NSTR),
            req("editor_id", NSTR),
            req("full", Nullable(&LSTRING_TEXT)),
            req("plugin", Str),
            req("load_index", Int),
            req("raw_form_id", NSTR),
            req("offset", NINT),
            req("runtime_id", NSTR),
            req("winner", Bool),
            req("deleted", Bool),
            req("resolved_by", Str),
        ]])),
    ),
    req(
        "unknown_records",
        List(&Obj(&[&[
            req("form", Str),
            req("plugin", Str),
            req("reasons", STRS),
        ]])),
    ),
    req("elapsed_ms", Int),
    req("summary", Str),
]]);

// ---- diff ------------------------------------------------------------------------------

const DIFF_SIDE: Shape = Obj(&[&[
    req("raw_form_id", Str),
    req("offset", Int),
    req("flags", Str),
    req("compressed", Bool),
]]);
const DIFF_LISTED: Shape = Obj(&[&[
    req("form", Str),
    req("signature", Str),
    req("raw_form_id", Str),
    req("offset", Int),
]]);
const DIFF: Shape = Obj(&[&[
    req("a", Str),
    req("b", Str),
    req("form", NSTR),
    req("keyed_by", Enum(&["name", "raw"])),
    req(
        "counts",
        Obj(&[&[
            req("added", Int),
            req("removed", Int),
            req("changed", Int),
            req("unchanged", Int),
            req("equal_by_value", Int),
        ]]),
    ),
    req("added", List(&DIFF_LISTED)),
    req("removed", List(&DIFF_LISTED)),
    req(
        "changed",
        List(&Obj(&[&[
            req("form", Str),
            req("signature", Str),
            req(
                "class",
                Enum(&["identical-to-previous", "deleted", "differs"]),
            ),
            req("a", DIFF_SIDE),
            req("b", DIFF_SIDE),
            req("flags_differ", Bool),
            req("order_differs", Bool),
            req("subrecords", List(&SUB_DIFF_WITH_SPANS)),
        ]])),
    ),
    req("summary", Str),
]]);

// ---- conflicts -------------------------------------------------------------------------

const CLASS_COUNTS: Shape = Map(&Int);
const CONFLICTS: Shape = Obj(&[&[
    req("master", NSTR),
    req("plugin", NSTR),
    req("type", NSTR),
    req("only_conflicts", Bool),
    req("detail", Enum(&["summary", "fields", "full"])),
    req(
        "counts",
        List(&Obj(&[&[
            req("master", Str),
            req("plugin", Str),
            req("total", Int),
            req("types", Map(&Int)),
        ]])),
    ),
    req("override_records", Int),
    req("forms", Int),
    req(
        "items",
        List(&Obj(&[&[
            req("form", Str),
            req("signature", NSTR),
            req("editor_id", NSTR),
            req("base_present", Bool),
            req("versions", STRS),
            req("winner", NSTR),
            req(
                "overrides",
                List(&Obj(&[&[
                    req("plugin", Str),
                    req("previous", NSTR),
                    req(
                        "class",
                        Enum(&[
                            "identical-to-previous",
                            "deleted",
                            "differs",
                            "first-version",
                        ]),
                    ),
                    req("resolved_by", Str),
                    req("comparable", Bool),
                    req("changes", Map(&Map(&Int))),
                    opt("subrecords", List(&SUB_DIFF)),
                    req("flags_differ", Bool),
                ]])),
            ),
        ]])),
    ),
    req("class_counts", CLASS_COUNTS),
    req("class_counts_all", Nullable(&CLASS_COUNTS)),
    req("next_cursor", NSTR),
    req("summary", Str),
]]);

// ---- cell (M7.S3) -----------------------------------------------------------------------

/// One `cell` item: a world, an interior or exterior cell, or a reference (by `mode`).
const CELL_ITEM: Shape = Obj(&[&[
    req("form", Str),
    opt("signature", NSTR),
    opt("editor_id", NSTR),
    opt("cells", Int),
    opt("child_records", Int),
    req("versions", Int),
    opt("x", NINT),
    opt("y", NINT),
    opt("base", Nullable(&Marker(kind::FORMREF))),
    opt("base_signature", NSTR),
    opt("base_editor_id", NSTR),
    opt("position", Nullable(&List(&Nullable(&Num)))),
    opt("rotation", Nullable(&List(&Nullable(&Num)))),
    opt("scale", Nullable(&Num)),
    opt("persistent", Bool),
    opt("group", Enum(&["persistent", "temporary"])),
    opt("flags", Str),
    opt("deleted", Bool),
    opt("plugin", Str),
]]);
const CELL: Shape = Obj(&[&[
    req("mode", Enum(&["worlds", "interior", "world", "cell"])),
    req("total", Int),
    req("items", List(&CELL_ITEM)),
    opt("interior_cells", Int),
    opt("world", NSTR),
    opt("world_editor_id", NSTR),
    opt("cell", Str),
    opt("cell_signature", NSTR),
    opt("cell_editor_id", NSTR),
    opt("moved_out", Int),
    req("next_cursor", NSTR),
    req("summary", Str),
    req("elapsed_ms", Int),
    req("index_ms", Int),
]]);

// ---- refs ------------------------------------------------------------------------------

const REFS_HEAD: &[Member] = &[
    req("form", Str),
    req("matched_by", Str),
    req("global", Str),
    req("signature", NSTR),
    req("editor_id", NSTR),
    req("depth", Int),
    req("total", Int),
    req("next_cursor", NSTR),
    req("truncated", Bool),
    req("summary", Str),
    req("elapsed_ms", Int),
];
const OUT_EDGE: Shape = Obj(&[
    LINK_MEMBERS,
    &[
        req("depth", Int),
        req("from", Str),
        req("from_signature", NSTR),
        req("from_editor_id", NSTR),
        req("plugin", Str),
    ],
]);
const REFS_OUT: Shape = Obj(&[
    REFS_HEAD,
    &[
        req("direction", Lit("outgoing")),
        req("forms_read", Int),
        req("counts", Obj(&[&[req("by_status", Map(&Int))]])),
        req("edges", List(&OUT_EDGE)),
    ],
]);
const REFS_IN: Shape = Obj(&[
    REFS_HEAD,
    &[
        req("direction", Lit("incoming")),
        req("scope", Enum(&["winner", "all"])),
        req(
            "counts",
            Obj(&[&[
                req("referrers", Int),
                req("placed", Int),
                req("by_signature", Map(&Int)),
            ]]),
        ),
        req("edges", List(&REFERRER)),
        req(
            "placed_by_cell",
            List(&Obj(&[&[
                req("world", NSTR),
                req("cell", NSTR),
                req("count", Int),
            ]])),
        ),
        req(
            "index",
            Obj(&[&[
                req("plugins", Int),
                req("cache_hits", Int),
                req("built", Int),
                req("uncached", Int),
                req("elapsed_ms", Int),
            ]]),
        ),
        req(
            "not_searched",
            Obj(&[&[
                req(
                    "no_definition",
                    List(&Obj(&[&[req("type", Str), req("records", Int)]])),
                ),
                req("records_with_raw_subrecords", Int),
            ]]),
        ),
    ],
]);
const REFS: Shape = By(
    "direction",
    &[
        (Case::Str("outgoing"), REFS_OUT),
        (Case::Str("incoming"), REFS_IN),
    ],
);

// ---- planet ----------------------------------------------------------------------------

const NLINK: Shape = Nullable(&Marker(kind::LINK));
const LINKS: Shape = List(&Marker(kind::LINK));
/// `{form, signature, editor_id}` of a record.
const BRIEF: Shape = Obj(&[&[
    req("form", Str),
    req("signature", NSTR),
    req("editor_id", NSTR),
]]);
/// A `CTDA` as stored: a struct of numbers and opaque unions, plus its `member` path.
const CONDITION: Shape = Shape::Value;
const CONTENT_DETAIL: Shape = Nullable(&Either(&[
    Obj(&[&[req("cell", NLINK)]]),
    Obj(&[&[req(
        "entries",
        List(&Obj(&[&[
            req("path", Str),
            req("pack_in", Str),
            req("status", LINK_STATUS),
        ]])),
    )]]),
    Obj(&[&[req("location", NLINK), req("planets", List(&BRIEF))]]),
]));
const NODE_MEMBERS: &[Member] = &[req("form", Str), req("depth", Int), req("parent", NSTR)];
const NODE_READ: &[Member] = &[
    req("signature", Str),
    req("editor_id", NSTR),
    req(
        "order",
        Enum(&[
            "root",
            "first",
            "chained",
            "shared-previous",
            "unchained",
            "ancestor",
        ]),
    ),
    req("node_type", Nullable(&Shape::Value)),
    req("child_selection", Nullable(&Shape::Value)),
    req("consume_request_on_failure", Nullable(&Shape::Value)),
    req("count_curve", NLINK),
    req("distribution_curve", NLINK),
    req("parent_link", NLINK),
    req("previous", NLINK),
    req("content", NLINK),
    req("override_content_properties", Bool),
    req("keywords", STRS),
    req("conditions", List(&CONDITION)),
    opt("content_detail", CONTENT_DETAIL),
];
/// A tree node; one that cannot be read is `{form, depth, parent, readable: false}`.
const NODE: Shape = Either(&[
    Obj(&[NODE_MEMBERS, NODE_READ]),
    Obj(&[NODE_MEMBERS, &[req("readable", Lit("false"))]]),
]);
const CHAIN_ENTRY: Shape = Obj(&[&[
    req("form", Str),
    req("signature", NSTR),
    req("editor_id", NSTR),
    req("child_selection", Nullable(&Shape::Value)),
    req("conditions", List(&CONDITION)),
]]);
const PLACEMENT: Shape = Obj(&[&[
    req("node", Str),
    req("editor_id", NSTR),
    req("content", NLINK),
    req("tree", NSTR),
    req("chain", List(&CHAIN_ENTRY)),
]]);
const KEYWORD_CONDITION: Shape = Obj(&[&[
    req("node", Str),
    req("signature", Str),
    req("editor_id", NSTR),
    req("condition", Int),
    req("member", Str),
    req("function", Nullable(&Shape::Value)),
    req("parameter", Str),
    req("raw", Str),
    req("keyword", Str),
    req("match", Lit("raw-bytes")),
]]);
const PLANET_HEAD: &[Member] = &[
    req("form", Str),
    req("matched_by", Str),
    req("global", Str),
    req("signature", NSTR),
    req("editor_id", NSTR),
    req(
        "kind",
        Enum(&[
            "tree",
            "planet",
            "star",
            "biome",
            "location",
            "worldspace",
            "pack-in",
        ]),
    ),
    req("next_cursor", NSTR),
    req("records_read", Int),
    req("tree_nodes_indexed", Int),
    req("elapsed_ms", Int),
    req("summary", Str),
];
const PLANET_TREE: Shape = Obj(&[
    PLANET_HEAD,
    &[
        req("tree", Str),
        req("ancestors", List(&NODE)),
        opt(
            "default_objects",
            List(&Obj(&[&[
                req("form", Str),
                req("editor_id", NSTR),
                req("subrecord", Str),
                req("payload_offset", Int),
                req("offset", NINT),
                req("match", Lit("raw-bytes")),
            ]])),
        ),
        req("nodes", List(&NODE)),
        req(
            "counts",
            Obj(&[&[
                req("nodes", Int),
                req("branches", Int),
                req("content_nodes", Int),
                req("max_depth", Int),
                req("by_content", Map(&Int)),
            ]]),
        ),
        req("total", Int),
        req(
            "unresolved",
            List(&Obj(&[&[
                req("node", Str),
                req("editor_id", NSTR),
                req("path", Str),
                req("raw", Str),
                req("status", LINK_STATUS),
                req("to", NSTR),
                req("error", NSTR),
            ]])),
        ),
        req(
            "target_mismatches",
            List(&Obj(&[&[
                req("node", Str),
                req("editor_id", NSTR),
                req("path", Str),
                req("to", Str),
                req("targets", STRS),
            ]])),
        ),
    ],
]);
const PLANET_PLANET: Shape = Obj(&[
    PLANET_HEAD,
    &[
        req(
            "planet",
            Obj(&[&[
                req("worldspaces", LINKS),
                req("biomes", LINKS),
                req("resource_links", Int),
                req("galaxy", Nullable(&Shape::Value)),
                req("star", Nullable(&BRIEF)),
                req("other_links", LINKS),
            ]]),
        ),
        req("placements", List(&PLACEMENT)),
    ],
]);
const PLANET_STAR: Shape = Obj(&[
    PLANET_HEAD,
    &[
        req(
            "star",
            Obj(&[&[
                req("system_id", Nullable(&Shape::Value)),
                req("links", LINKS),
            ]]),
        ),
        req(
            "planets",
            List(&Obj(&[&[
                req("planet", BRIEF),
                req("worldspaces", STRS),
                req("placements", List(&PLACEMENT)),
            ]])),
        ),
    ],
]);
const PLANET_BIOME: Shape = Obj(&[
    PLANET_HEAD,
    &[
        req(
            "biome",
            Obj(&[&[
                req("objects", Map(&Int)),
                req("flora", Int),
                req("keywords", STRS),
                req("links", LINKS),
            ]]),
        ),
        req(
            "planets",
            List(&Obj(&[&[
                req("planet", Str),
                req("editor_id", NSTR),
                req("paths", STRS),
            ]])),
        ),
        req("keyword_conditions", List(&KEYWORD_CONDITION)),
    ],
]);
const PLANET_LOCATION: Shape = Obj(&[
    PLANET_HEAD,
    &[
        req("location", Obj(&[&[req("links", LINKS)]])),
        req("worldspaces", List(&BRIEF)),
        req("planets", List(&BRIEF)),
        req("placements", List(&PLACEMENT)),
        req("keyword_conditions", List(&KEYWORD_CONDITION)),
    ],
]);
const PLANET_PLACED: Shape = Obj(&[
    PLANET_HEAD,
    &[
        req("detail", CONTENT_DETAIL),
        req("placements", List(&PLACEMENT)),
        opt(
            "in_leveled",
            List(&Obj(&[&[
                req("leveled", Str),
                req("editor_id", NSTR),
                req("paths", STRS),
            ]])),
        ),
        opt(
            "in_biomes",
            List(&Obj(&[&[
                req("biome", Str),
                req("editor_id", NSTR),
                req("objects", Int),
            ]])),
        ),
    ],
]);
const PLANET: Shape = By(
    "kind",
    &[
        (Case::Str("tree"), PLANET_TREE),
        (Case::Str("planet"), PLANET_PLANET),
        (Case::Str("star"), PLANET_STAR),
        (Case::Str("biome"), PLANET_BIOME),
        (Case::Str("location"), PLANET_LOCATION),
        (Case::Str("worldspace"), PLANET_PLACED),
        (Case::Str("pack-in"), PLANET_PLACED),
    ],
);

// ---- coverage --------------------------------------------------------------------------

const OCCURRENCES: Shape = Obj(&[&[
    req("typed", Int),
    req("failed", Int),
    req("opaque", Int),
    req("unknown", Int),
]]);
const BY_CONFIDENCE: Shape = Obj(&[&[
    req("reference", Int),
    req("container-reference", Int),
    req("corpus", Int),
    req("ck-diff", Int),
    req("verified", Int),
]]);
const COVERAGE_DATA: Shape = Obj(&[&[
    req("plugin", Str),
    req("file", Str),
    req("bytes", Int),
    req("decode", Bool),
    req("records", Int),
    req("groups", Int),
    req("compressed_records", Int),
    req("compressed_examined", Int),
    req("record_types_seen", Int),
    req("record_types_defined", Int),
    req("record_types_listed", Int),
    req("typed_records", Int),
    req("subrecords", Int),
    req("occurrences", OCCURRENCES),
    req("decoded_by_confidence", BY_CONFIDENCE),
    req("reencode_conflicts", Int),
    req(
        "typed_failures",
        List(&Obj(&[&[
            req("record", Str),
            req("signature", Str),
            req("index", Int),
            req("offset", NINT),
            req("kind", Enum(&["not_decoded", "reencoded"])),
        ]])),
    ),
    req("signatures", Bool),
    req("top", NINT),
    req("summary", Str),
    req(
        "record_types",
        List(&Obj(&[&[
            req("signature", Str),
            req("definition", NSTR),
            req("records", Int),
            req("compressed", Int),
            req("compressed_examined", Int),
            req("typed_records", Int),
            req("subrecords", Int),
            req("occurrences", OCCURRENCES),
            req("decoded_by_confidence", BY_CONFIDENCE),
            req("reencode_conflicts", Int),
            req(
                "distinct",
                Obj(&[&[
                    req("seen", Int),
                    req("typed", Int),
                    req("opaque", Int),
                    req("unknown", Int),
                ]]),
            ),
            req("first_offset", Int),
            req(
                "signatures",
                Nullable(&List(&Obj(&[&[
                    req("signature", Str),
                    req("count", Int),
                    req("class", Enum(&["typed", "opaque", "unknown"])),
                    req("failed", Int),
                    req("outside_components", Int),
                    req("confidence", NSTR),
                    req("members", STRS),
                    req("first_record", Str),
                ]]))),
            ),
        ]])),
    ),
]]);

// ---- schema ----------------------------------------------------------------------------

const CONFIDENCE: Shape = Enum(&[
    "reference",
    "container-reference",
    "corpus",
    "ck-diff",
    "verified",
]);
/// A member of a record definition (top level; `deep` adds `members`, `fields`, `values`,
/// `bits`, `variants`, `evidence` and `note`, documented in prose).
const SCHEMA_MEMBER: Shape = Open(&[&[
    req("kind", Enum(&["subrecord", "group", "any_until"])),
    req("name", Str),
    req("confidence", CONFIDENCE),
]]);
const COUNTS: &[Member] = &[
    req("subrecords", Int),
    req("typed", Int),
    req("opaque", Int),
    req("confidence_counts", BY_CONFIDENCE),
];
const SCHEMA_LIST: Shape = Obj(&[
    COUNTS,
    &[
        req(
            "records",
            List(&Obj(&[
                COUNTS,
                &[
                    req("type", Str),
                    req("name", Str),
                    req("confidence", CONFIDENCE),
                ],
            ])),
        ),
        req(
            "headers",
            List(&Obj(&[&[
                req("type", NSTR),
                req("name", Str),
                req("size", Int),
                req("confidence", CONFIDENCE),
            ]])),
        ),
        req(
            "common",
            List(&Obj(&[&[
                req("name", Str),
                req("kind", Enum(&["group", "type"])),
                req("confidence", CONFIDENCE),
            ]])),
        ),
        req("summary", Str),
    ],
]);
const SCHEMA_RECORD: Shape = Obj(&[
    COUNTS,
    &[
        req("type", Str),
        req("defined", Lit("true")),
        req("kind", Lit("record")),
        req("name", Str),
        req("confidence", CONFIDENCE),
        req("source", Str),
        req("evidence", STRS),
        req("note", NSTR),
        req(
            "header_flags",
            List(&Obj(&[&[req("name", Str), req("mask", Str)]])),
        ),
        req("deep", Bool),
        req("member", NSTR),
        req("members", List(&SCHEMA_MEMBER)),
        req("summary", Str),
    ],
]);
const SCHEMA_HEADER: Shape = Obj(&[&[
    req("type", Str),
    req("defined", Bool),
    req("kind", Lit("header")),
    req("name", Str),
    req("confidence", CONFIDENCE),
    req("source", Str),
    req("evidence", STRS),
    req("size", Int),
    req("deep", Bool),
    req(
        "fields",
        List(&Open(&[&[req("name", Str), req("type", Str)]])),
    ),
    req("summary", Str),
]]);
const SCHEMA_UNDEFINED: Shape = Obj(&[&[
    req("type", Str),
    req("defined", Bool),
    req("defined_types", STRS),
    req("summary", Str),
]]);
const SCHEMA: Shape = By(
    "kind",
    &[
        (Case::Str("record"), SCHEMA_RECORD),
        (Case::Str("header"), SCHEMA_HEADER),
        (
            Case::Absent,
            By(
                "defined",
                &[
                    (Case::Bool(false), SCHEMA_UNDEFINED),
                    (Case::Absent, SCHEMA_LIST),
                ],
            ),
        ),
    ],
);

// ---- validate --------------------------------------------------------------------------

const VALIDATE: Shape = Obj(&[&[
    req("plugin", NSTR),
    req("scripts", STRS),
    req("passed", Bool),
    req("errors", Int),
    req("warnings", Int),
    req("rules", STRS),
    req(
        "findings",
        List(&Obj(&[&[
            req("rule", Str),
            req("severity", Enum(&["error", "warning"])),
            req("plugin", Str),
            req("record", NSTR),
            req("subrecord", NSTR),
            req("offset", NINT),
            req("message", Str),
        ]])),
    ),
    req("findings_total", Int),
    req(
        "masters",
        List(&Obj(&[&[
            req("name", Str),
            req("path", NSTR),
            req("flags", NSTR),
            req("localized", Nullable(&Bool)),
        ]])),
    ),
    req("records", Int),
    req("groups", Int),
    req("subrecords", Int),
    req("checked_form_ids", Int),
    req("unresolved_form_ids", Int),
    req("reserved_form_ids", Int),
    req("reserved_by_member", Map(&Int)),
    req("reserved_ids_by_member", Map(&STRS)),
    req("unchecked_subrecords", Int),
    req("scripts_checked", Int),
    req("summary", Str),
]]);

/// The documented shape of `data` for an inspection command (by canonical id).
pub fn data_shape(command: &str) -> Option<&'static Shape> {
    match command {
        "load" => Some(&LOAD),
        "inspect" => Some(&INSPECT),
        "query" => Some(&QUERY),
        "diff" => Some(&DIFF),
        "conflicts" => Some(&CONFLICTS),
        "refs" => Some(&REFS),
        "planet" => Some(&PLANET),
        "coverage" => Some(&COVERAGE_DATA),
        "schema" => Some(&SCHEMA),
        "validate" => Some(&VALIDATE),
        "cell" => Some(&CELL),
        _ => None,
    }
}

/// Deepest nesting the checks follow (the engine's own values are far shallower).
const MAX_DEPTH: u16 = 64;

/// Every place `env` differs from the envelope and its command's documented data shape,
/// and every marker problem anywhere in `data` (a marker object off its shape, an
/// unmarked object that looks like a FormID value). Empty when it conforms.
pub fn check_envelope(env: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    check(&ENVELOPE, env, "", &mut errs, 0);
    let data = env.get("data").unwrap_or(&Value::Null);
    if let Some(shape) = env
        .get("command")
        .and_then(Value::as_str)
        .and_then(data_shape)
    {
        check(shape, data, "data", &mut errs, 0);
    }
    walk_markers(data, "data", &mut errs, 0);
    errs
}

/// Every place `v` differs from `shape` (`path` names `v` in the messages).
pub fn check_shape(shape: &Shape, v: &Value, path: &str) -> Vec<String> {
    let mut errs = Vec::new();
    check(shape, v, path, &mut errs, 0);
    errs
}

fn brief(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 60 {
        format!("{}...", s.chars().take(60).collect::<String>())
    } else {
        s
    }
}

fn check(shape: &Shape, v: &Value, path: &str, errs: &mut Vec<String>, depth: u16) {
    if depth > MAX_DEPTH {
        errs.push(format!("{path}: nested deeper than {MAX_DEPTH}"));
        return;
    }
    let next = depth.saturating_add(1);
    let mut bad = |what: &str| errs.push(format!("{path}: expected {what}, found {}", brief(v)));
    match shape {
        Any => {}
        Null => {
            if !v.is_null() {
                bad("null");
            }
        }
        Bool => {
            if !v.is_boolean() {
                bad("bool");
            }
        }
        Int => {
            if !(v.is_u64() || v.is_i64()) {
                bad("int");
            }
        }
        Num => {
            if !v.is_number() {
                bad("number");
            }
        }
        Str => {
            if !v.is_string() {
                bad("str");
            }
        }
        Lit(s) => {
            // `"true"`/`"false"` stand for the booleans in a literal position.
            let ok = match *s {
                "true" => v == &Value::Bool(true),
                "false" => v == &Value::Bool(false),
                s => v.as_str() == Some(s),
            };
            if !ok {
                bad(&format!("\"{s}\""));
            }
        }
        Enum(list) => {
            if !v.as_str().is_some_and(|s| list.contains(&s)) {
                bad(&inline(shape));
            }
        }
        Nullable(inner) => {
            if !v.is_null() {
                check(inner, v, path, errs, next);
            }
        }
        List(inner) => match v.as_array() {
            Some(items) => {
                for (i, item) in items.iter().enumerate() {
                    check(inner, item, &format!("{path}[{i}]"), errs, next);
                }
            }
            None => bad("array"),
        },
        Map(inner) => match v.as_object() {
            Some(m) => {
                for (k, item) in m {
                    check(inner, item, &format!("{path}.{k}"), errs, next);
                }
            }
            None => bad("object"),
        },
        Obj(groups) | Open(groups) => match v.as_object() {
            Some(m) => {
                for member in groups.iter().flat_map(|g| g.iter()) {
                    match m.get(member.name) {
                        Some(x) => {
                            check(
                                &member.shape,
                                x,
                                &format!("{path}.{}", member.name),
                                errs,
                                next,
                            );
                        }
                        None if member.required => {
                            errs.push(format!("{path}.{}: missing", member.name));
                        }
                        None => {}
                    }
                }
                if matches!(shape, Obj(_)) {
                    for k in m.keys() {
                        if !groups.iter().flat_map(|g| g.iter()).any(|x| x.name == k) {
                            errs.push(format!("{path}.{k}: not in the documented shape"));
                        }
                    }
                }
            }
            None => bad("object"),
        },
        Shape::Value => check_value(v, path, errs, depth),
        Marker(name) => {
            if v.get(KIND).and_then(Value::as_str) == Some(name) {
                if let Some(s) = marker_shape(name) {
                    check(s, v, path, errs, next);
                }
            } else {
                bad(&format!("a `{name}` marker object"));
            }
        }
        Either(options) => {
            let fits = options.iter().any(|o| {
                let mut e = Vec::new();
                check(o, v, path, &mut e, next);
                e.is_empty()
            });
            if !fits {
                bad(&inline(shape));
            }
        }
        By(member, cases) => {
            let found = v.get(*member);
            let case = cases.iter().find(|(c, _)| match (c, found) {
                (Case::Absent, None) => true,
                (Case::Str(s), Some(Value::String(x))) => s == x,
                (Case::Bool(b), Some(Value::Bool(x))) => b == x,
                _ => false,
            });
            match case {
                Some((_, s)) => check(s, v, path, errs, next),
                None => errs.push(format!(
                    "{path}.{member}: no documented case for {}",
                    found.map_or_else(|| "absent".to_string(), brief)
                )),
            }
        }
    }
}

/// What an unmarked object looks like, if it looks like a marker object.
fn looks_marked(m: &JsonMap<String, Value>) -> Option<&'static str> {
    const FORMREF_KEYS: &[&str] = &["form", "raw", "null", "error", "editor_id"];
    if m.contains_key("form")
        && (m.contains_key("raw") || m.contains_key("null"))
        && m.keys().all(|k| FORMREF_KEYS.contains(&k.as_str()))
    {
        return Some(kind::FORMREF);
    }
    if m.len() == 1 && m.contains_key("string_id") {
        return Some(kind::LSTRING);
    }
    if m.len() == 1 && m.contains_key("opaque") {
        return Some(kind::OPAQUE);
    }
    None
}

fn check_value(v: &Value, path: &str, errs: &mut Vec<String>, depth: u16) {
    if depth > MAX_DEPTH {
        errs.push(format!("{path}: nested deeper than {MAX_DEPTH}"));
        return;
    }
    let next = depth.saturating_add(1);
    match v {
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                check_value(item, &format!("{path}[{i}]"), errs, next);
            }
        }
        Value::Object(m) => match m.get(KIND) {
            Some(Value::String(k)) if kind::IN_VALUES.contains(&k.as_str()) => {
                if let Some(s) = marker_shape(k) {
                    check(s, v, path, errs, next);
                }
            }
            Some(other) => errs.push(format!(
                "{path}.kind: {} is no value marker ({}); a decoded struct has no member `kind`",
                brief(other),
                kind::IN_VALUES.join(", ")
            )),
            None => {
                if let Some(k) = looks_marked(m) {
                    errs.push(format!(
                        "{path}: looks like a `{k}` but has no `kind` marker"
                    ));
                }
                for (k, item) in m {
                    check_value(item, &format!("{path}.{k}"), errs, next);
                }
            }
        },
        _ => {}
    }
}

/// Every marker object in `v` against its marker's shape, and every unmarked object that
/// looks like a FormID value.
fn walk_markers(v: &Value, path: &str, errs: &mut Vec<String>, depth: u16) {
    if depth > MAX_DEPTH {
        return;
    }
    let next = depth.saturating_add(1);
    match v {
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                walk_markers(item, &format!("{path}[{i}]"), errs, next);
            }
        }
        Value::Object(m) => {
            match m.get(KIND).and_then(Value::as_str).and_then(marker_shape) {
                Some(s) => check(s, v, path, errs, next),
                None => {
                    if looks_marked(m) == Some(kind::FORMREF) {
                        errs.push(format!(
                            "{path}: looks like a `formref` but has no `kind` marker"
                        ));
                    }
                }
            }
            for (k, item) in m {
                walk_markers(item, &format!("{path}.{k}"), errs, next);
            }
        }
        _ => {}
    }
}

/// A shape as one short type: `str`, `int | null`, `[str]`, `{a, b?}`, `<formref>`.
pub fn inline(shape: &Shape) -> String {
    match shape {
        Any => "any".into(),
        Null => "null".into(),
        Bool => "bool".into(),
        Int => "int".into(),
        Num => "number".into(),
        Str => "str".into(),
        Lit(s) if *s == "true" || *s == "false" => (*s).to_string(),
        Lit(s) => format!("\"{s}\""),
        Enum(list) => list
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(" | "),
        Nullable(inner) => format!("{} | null", inline(inner)),
        List(inner) => format!("[{}]", inline(inner)),
        Map(inner) => format!("map<{}>", inline(inner)),
        Obj(groups) | Open(groups) => {
            let names: Vec<String> = groups
                .iter()
                .flat_map(|g| g.iter())
                .map(|m| format!("{}{}", m.name, if m.required { "" } else { "?" }))
                .collect();
            let more = if matches!(shape, Open(_)) {
                ", ..."
            } else {
                ""
            };
            format!("{{{}{more}}}", names.join(", "))
        }
        Shape::Value => "value".into(),
        Marker(name) => format!("<{name}>"),
        Either(options) => options.iter().map(inline).collect::<Vec<_>>().join(" | "),
        By(member, _) => format!("object (by `{member}`)"),
    }
}

/// Whether a shape is drawn as member lines rather than one inline type.
fn expands(shape: &Shape) -> bool {
    match shape {
        Obj(_) | Open(_) | By(..) => true,
        Nullable(inner) | List(inner) => expands(inner),
        _ => false,
    }
}

/// The documented shape of `command`'s `data`, one line per member path
/// (`versions[].subrecords[].value?: value`; `?` = not always present), as `docs/shapes.md`
/// prints it.
pub fn describe(command: &str) -> Option<String> {
    let shape = data_shape(command)?;
    let mut out = Vec::new();
    children(shape, "", &mut out, 0);
    Some(out.join("\n"))
}

/// What an expanding shape is, on the line that introduces its members.
fn header(shape: &Shape) -> String {
    match shape {
        Nullable(inner) => format!("{} or null", header(inner)),
        List(inner) => format!("[{}]", header(inner)),
        By(member, _) => format!("object (by `{member}`)"),
        _ => "object".to_string(),
    }
}

/// The member lines of an expanding shape under `path` (`?` marks a member that is not
/// always present, on its own line only).
fn children(shape: &Shape, path: &str, out: &mut Vec<String>, depth: u16) {
    if depth > MAX_DEPTH {
        return;
    }
    let next = depth.saturating_add(1);
    let at = |name: &str| {
        if path.is_empty() {
            name.to_string()
        } else {
            format!("{path}.{name}")
        }
    };
    match shape {
        Nullable(inner) => children(inner, path, out, next),
        List(inner) => children(inner, &format!("{path}[]"), out, next),
        Obj(groups) | Open(groups) => {
            for m in groups.iter().flat_map(|g| g.iter()) {
                let shown = at(&format!("{}{}", m.name, if m.required { "" } else { "?" }));
                if expands(&m.shape) {
                    out.push(format!("{shown}: {}", header(&m.shape)));
                    children(&m.shape, &at(m.name), out, next);
                } else {
                    out.push(format!("{shown}: {}", inline(&m.shape)));
                }
            }
            if matches!(shape, Open(_)) {
                out.push(format!("{}: (more members may follow)", at("...")));
            }
        }
        By(member, cases) => {
            for (case, s) in cases.iter() {
                let when = match case {
                    Case::Absent => format!("`{member}` absent"),
                    Case::Str(v) => format!("`{member}` = \"{v}\""),
                    Case::Bool(b) => format!("`{member}` = {b}"),
                };
                let whose = if path.is_empty() { "data" } else { path };
                out.push(format!("-- {whose}, when {when}:"));
                children(s, path, out, next);
            }
        }
        _ => {}
    }
}
