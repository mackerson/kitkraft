//! `cargo xtask observe` (M3.H1, M3.H2): the Creation Kit observation harness (lane H).
//!
//! - `cargo xtask observe <fixture-dir>...` reads one fixture folder the owner filled during
//!   an M3.O1 session (CK-saved plugins, screenshots, `fixture.toml`) and writes
//!   `report.md` and `report.json` **inside that folder only**: the SHA-256 of every file;
//!   per plugin its `TES4` header, `kit roundtrip --verify`, `kit coverage` and
//!   `kit validate`; and for every comparison (the fixture's base, `[[compare]]` pairs, and
//!   our own equivalent from `kit_equivalent`, run in both write styles) a positional byte
//!   diff, a `TES4` diff and `kit diff`. Our equivalent is written under `<fixture>/kit/`,
//!   and `kit`'s index cache goes to `<fixture>/kit/cache`, so nothing lands outside the
//!   folder. The report holds no timings, so rerunning gives the same bytes.
//! - `cargo xtask observe new <F-id> [--root <dir>] [--date <yyyy-mm-dd>]` creates
//!   `<root>/<date>-<id>/fixture.toml` from the template for that fixture of the M3 matrix
//!   (`docs/ck-observations.md`), with the forms and sentinels M3.H1 resolved. `--root`
//!   defaults to `observe/` in the workspace (gitignored).
//!
//! Findings are written to the ledger in `docs/ck-observations.md` by hand (M3.H3), from
//! these reports. Nothing from a fixture folder is ever committed.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::evidence::{
    arg, byte_diff, sha256_hex, strip_volatile, table, tes4, tes4_diff, KitBinary, KitOut, Runner,
};

/// Files the harness writes into a fixture folder (never hashed as fixture content).
const REPORT_JSON: &str = "report.json";
const REPORT_MD: &str = "report.md";
const FIXTURE_TOML: &str = "fixture.toml";
/// Our own outputs and `kit`'s cache, inside the fixture folder.
const KIT_DIR: &str = "kit";
/// Differing byte runs listed per comparison.
const RANGE_CAP: usize = 64;
/// Write styles our equivalent is run in (M3.A3, M3.E5).
const STYLES: [&str; 2] = ["kit", "ck"];

/// `cargo xtask observe ...`.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => {
            println!("cargo xtask observe <fixture-dir>...");
            println!("cargo xtask observe new <F00..F14> [--root <dir>] [--date <yyyy-mm-dd>]");
            println!();
            println!("  The kit binary is KITCRAFT_KIT, else `cargo run --release -p kit-cli`.");
            println!("  Masters are found through KITCRAFT_DATA_DIR (read only).");
            Ok(())
        }
        Some("new") => {
            let rest = args.get(1..).unwrap_or_default();
            let id = rest
                .first()
                .context("observe new: name a fixture id (F00..F14)")?;
            let mut out_root = root.join("observe");
            let mut date = None;
            let mut it = rest.iter().skip(1);
            while let Some(flag) = it.next() {
                let value = it
                    .next()
                    .with_context(|| format!("observe new: {flag} needs a value"))?;
                match flag.as_str() {
                    "--root" => out_root = PathBuf::from(value),
                    "--date" => date = Some(value.clone()),
                    other => bail!("observe new: unknown argument `{other}`"),
                }
            }
            let date = match date {
                Some(d) => d,
                None => today().context("observe new: could not read the clock; pass --date")?,
            };
            let path = new_fixture(&out_root, id, &date)?;
            println!("observe: wrote {}", path.display());
            Ok(())
        }
        Some(_) => {
            let kit = KitBinary::from_env(root);
            for dir in args {
                let dir =
                    std::path::absolute(dir).with_context(|| format!("observe: bad path {dir}"))?;
                let report = observe(&dir, &kit)?;
                println!(
                    "observe: {} ({} files, {} plugins, {} comparisons)",
                    dir.join(REPORT_MD).display(),
                    count(&report, "files"),
                    count(&report, "plugins"),
                    count(&report, "comparisons")
                );
            }
            Ok(())
        }
    }
}

fn count(report: &Value, key: &str) -> usize {
    report
        .get(key)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

// ---------------------------------------------------------------------------------------
// The fixture matrix (M3.H1). The forms and why each was chosen are in
// docs/ck-observations.md; `matrix_matches_the_ledger_doc` keeps the two in step.

/// One plugin the CK is expected to save for a fixture.
struct PluginSpec {
    file: &'static str,
    form: &'static str,
    editor_id: &'static str,
    action: &'static str,
    sentinel: &'static str,
    /// `kit` argument vectors that build our equivalent; `{out}` is our output path.
    kit: &'static [&'static [&'static str]],
}

struct FixtureSpec {
    id: &'static str,
    title: &'static str,
    /// The fixture whose saved plugin is this one's starting point.
    base: Option<(&'static str, &'static str)>,
    masters: &'static [&'static str],
    plugins: &'static [PluginSpec],
    compare: &'static [(&'static str, &'static str)],
    notes: &'static str,
}

const RIFLE: &str = "Starfield.esm:0021BBCD";
const RIFLE_EDID: &str = "RussianHuntingRifle";
const AMMO: &str = "Starfield.esm:002B559B";
const GMST: &str = "Starfield.esm:000ABEF6";
const NPC: &str = "Starfield.esm:0000BD3B";
const WRLD: &str = "Starfield.esm:0027CFA2";

const NUMERIC: &str = "float field: 1234.5 / 2345.25; integer field: 4321 / 1234";

const MATRIX: &[FixtureSpec] = &[
    FixtureSpec {
        id: "F00",
        title: "Session metadata",
        base: None,
        masters: &[],
        plugins: &[],
        compare: &[],
        notes: "Copy EditorDataFilesLoaded.txt from the game folder (if the CK wrote it); fill \
                ck_version (title bar), game_build (Starfield.exe Properties > Details) and \
                mo2_version.",
    },
    FixtureSpec {
        id: "F01",
        title: "Empty plugin, master Starfield.esm only",
        base: None,
        masters: &["Starfield.esm"],
        plugins: &[PluginSpec {
            file: "KC_F01.esp",
            form: "",
            editor_id: "",
            action: "File > Data: tick Starfield.esm only, no active file; save without \
                     editing anything. If the CK refuses an unchanged plugin, record the \
                     message.",
            sentinel: "",
            kit: &[],
        }],
        compare: &[],
        notes: "",
    },
    FixtureSpec {
        id: "F02",
        title: "Override of the rifle, FULL renamed",
        base: None,
        masters: &["Starfield.esm"],
        plugins: &[PluginSpec {
            file: "KC_F02.esp",
            form: RIFLE,
            editor_id: RIFLE_EDID,
            action: "Object Window > WEAP RussianHuntingRifle: set Name to \
                     \"Old Soviet Marksman Rifle\"",
            sentinel: "Old Soviet Marksman Rifle",
            kit: &[&[
                "set",
                RIFLE,
                "--fields",
                "{\"FULL\": \"Old Soviet Marksman Rifle\"}",
                "{out}",
                "--plugins",
                "Starfield.esm",
            ]],
        }],
        compare: &[],
        notes: "",
    },
    FixtureSpec {
        id: "F03",
        title: "Rifle: one numeric field, two sentinels; one checkbox",
        base: Some(("F02", "KC_F02.esp")),
        masters: &["Starfield.esm"],
        plugins: &[
            PluginSpec {
                file: "KC_F03a.esp",
                form: RIFLE,
                editor_id: RIFLE_EDID,
                action: "Weapon dialog: change one numeric field (record tab and label) to \
                         the first sentinel",
                sentinel: NUMERIC,
                kit: &[],
            },
            PluginSpec {
                file: "KC_F03b.esp",
                form: RIFLE,
                editor_id: RIFLE_EDID,
                action: "The same field as F03a, second sentinel",
                sentinel: NUMERIC,
                kit: &[],
            },
            PluginSpec {
                file: "KC_F03c.esp",
                form: RIFLE,
                editor_id: RIFLE_EDID,
                action: "Weapon dialog: toggle one checkbox (record tab and label)",
                sentinel: "checkbox toggled",
                kit: &[],
            },
        ],
        compare: &[("KC_F03a.esp", "KC_F03b.esp")],
        notes: "Start each variant from KC_F02.esp loaded as the active file (a fresh copy per \
                variant). kit_equivalent is added once lane B types WEAP (M3.B1).",
    },
    FixtureSpec {
        id: "F04",
        title: "The rifle's AMMO: one numeric field, two sentinels",
        base: Some(("F02", "KC_F02.esp")),
        masters: &["Starfield.esm"],
        plugins: &[
            PluginSpec {
                file: "KC_F04a.esp",
                form: AMMO,
                editor_id: "Ammo9x39mm",
                action: "Ammo dialog: change one numeric field (record the label) to the \
                         first sentinel",
                sentinel: NUMERIC,
                kit: &[],
            },
            PluginSpec {
                file: "KC_F04b.esp",
                form: AMMO,
                editor_id: "Ammo9x39mm",
                action: "The same field as F04a, second sentinel",
                sentinel: NUMERIC,
                kit: &[],
            },
        ],
        compare: &[("KC_F04a.esp", "KC_F04b.esp")],
        notes: "kit_equivalent is added once lane B types AMMO (M3.B2).",
    },
    FixtureSpec {
        id: "F05",
        title: "One float GMST, two sentinels",
        base: Some(("F01", "KC_F01.esp")),
        masters: &["Starfield.esm"],
        plugins: &[
            PluginSpec {
                file: "KC_F05a.esp",
                form: GMST,
                editor_id: "fJumpHeightMin",
                action: "Game settings editor: fJumpHeightMin = 1234.5",
                sentinel: "1234.5",
                kit: &[&[
                    "set",
                    GMST,
                    "--fields",
                    "{\"DATA\": 1234.5}",
                    "{out}",
                    "--plugins",
                    "Starfield.esm",
                ]],
            },
            PluginSpec {
                file: "KC_F05b.esp",
                form: GMST,
                editor_id: "fJumpHeightMin",
                action: "Game settings editor: fJumpHeightMin = 2345.25",
                sentinel: "2345.25",
                kit: &[&[
                    "set",
                    GMST,
                    "--fields",
                    "{\"DATA\": 2345.25}",
                    "{out}",
                    "--plugins",
                    "Starfield.esm",
                ]],
            },
        ],
        compare: &[("KC_F05a.esp", "KC_F05b.esp")],
        notes: "",
    },
    FixtureSpec {
        id: "F06",
        title: "One new GLOB",
        base: Some(("F01", "KC_F01.esp")),
        masters: &["Starfield.esm"],
        plugins: &[PluginSpec {
            file: "KC_F06.esp",
            form: "(new)",
            editor_id: "KC_F06_Global",
            action: "Object Window > Global: right-click > New; EditorID KC_F06_Global, type \
                     float, value 1234.5",
            sentinel: "1234.5",
            kit: &[],
        }],
        compare: &[],
        notes: "Our writer creates no new records in M3; this fixture only observes FormID \
                allocation and HEDR's next object ID.",
    },
    FixtureSpec {
        id: "F07",
        title: "Compressed NPC_ override (and M3.A1's candidates)",
        base: Some(("F01", "KC_F01.esp")),
        masters: &["Starfield.esm"],
        plugins: &[PluginSpec {
            file: "KC_F07.esp",
            form: NPC,
            editor_id: "EAW_AudioSlateChar01_Lydia_NoAppearanceData",
            action: "Actor dialog: change one numeric field (record tab and label); then one \
                     trivial edit on each M3.A1 candidate listed in docs/ck-observations.md",
            sentinel: NUMERIC,
            kit: &[],
        }],
        compare: &[],
        notes: "M3.A1's recompression candidates are still pending (lane A); add them to this \
                plugin when docs/ck-observations.md lists them.",
    },
    FixtureSpec {
        id: "F08",
        title: "Override of the XXXX record (field outside the large subrecords)",
        base: Some(("F01", "KC_F01.esp")),
        masters: &["Starfield.esm"],
        plugins: &[PluginSpec {
            file: "KC_F08.esp",
            form: WRLD,
            editor_id: "akilacity",
            action: "World > World Spaces > akilacity: change one numeric field or checkbox \
                     (record the label)",
            sentinel: NUMERIC,
            kit: &[],
        }],
        compare: &[],
        notes: "",
    },
    FixtureSpec {
        id: "F09",
        title: "Two masters: Starfield.esm + SFBGS004.esm (small)",
        base: None,
        masters: &["Starfield.esm", "SFBGS004.esm"],
        plugins: &[PluginSpec {
            file: "KC_F09.esp",
            form: "Starfield.esm:000FB76D, SFBGS004.esm:000EE4",
            editor_id: "CF05_EnsignTestGlobal, _SFBGS004",
            action: "Object Window > Global: CF05_EnsignTestGlobal = 1234.5; \
                     _SFBGS004 = 2345.25",
            sentinel: "1234.5, 2345.25",
            kit: &[],
        }],
        compare: &[],
        notes: "",
    },
    FixtureSpec {
        id: "F10",
        title: "Mixed tiers: Starfield.esm (full) + Constellation.esm (small) + SFBGS006.esm \
                (medium)",
        base: None,
        masters: &["Starfield.esm", "Constellation.esm", "SFBGS006.esm"],
        plugins: &[PluginSpec {
            file: "KC_F10.esp",
            form: "Starfield.esm:000FB76D, Constellation.esm:000041, SFBGS006.esm:000800",
            editor_id: "CF05_EnsignTestGlobal, ConstellationTestGlobal, SFBGS006_Global",
            action: "Object Window > Global: CF05_EnsignTestGlobal = 1234.5; \
                     ConstellationTestGlobal = 2345.25; SFBGS006_Global = 3456.75",
            sentinel: "1234.5, 2345.25, 3456.75",
            kit: &[],
        }],
        compare: &[],
        notes: "",
    },
    FixtureSpec {
        id: "F11",
        title: "Exports of F06: full, medium, small",
        base: Some(("F06", "KC_F06.esp")),
        masters: &["Starfield.esm"],
        plugins: &[
            PluginSpec {
                file: "KC_F11_full.esm",
                form: "(F06's GLOB)",
                editor_id: "KC_F06_Global",
                action: "The CK's export to master, full (record every option the dialog \
                         shows)",
                sentinel: "",
                kit: &[],
            },
            PluginSpec {
                file: "KC_F11_medium.esm",
                form: "(F06's GLOB)",
                editor_id: "KC_F06_Global",
                action: "Export as medium, if offered",
                sentinel: "",
                kit: &[],
            },
            PluginSpec {
                file: "KC_F11_small.esm",
                form: "(F06's GLOB)",
                editor_id: "KC_F06_Global",
                action: "Export as small, if offered",
                sentinel: "",
                kit: &[],
            },
        ],
        compare: &[
            ("KC_F11_full.esm", "KC_F11_medium.esm"),
            ("KC_F11_full.esm", "KC_F11_small.esm"),
        ],
        notes: "Record which export kinds the CK offers; a kind it does not offer is a \
                finding too.",
    },
    FixtureSpec {
        id: "F12",
        title: "F02's edit saved as a localized plugin",
        base: Some(("F02", "KC_F02.esp")),
        masters: &["Starfield.esm"],
        plugins: &[PluginSpec {
            file: "KC_F12.esp",
            form: RIFLE,
            editor_id: RIFLE_EDID,
            action: "As F02, saved as a localized plugin if the CK offers it; keep the \
                     string tables it writes next to the plugin (with their folder)",
            sentinel: "Old Soviet Marksman Rifle",
            kit: &[],
        }],
        compare: &[],
        notes: "",
    },
    FixtureSpec {
        id: "F13",
        title: "F02 with a non-ASCII name",
        base: None,
        masters: &["Starfield.esm"],
        plugins: &[PluginSpec {
            file: "KC_F13.esp",
            form: RIFLE,
            editor_id: RIFLE_EDID,
            action: "Object Window > WEAP RussianHuntingRifle: set Name to \"Fusil côté Ä\"",
            sentinel: "Fusil côté Ä",
            kit: &[&[
                "set",
                RIFLE,
                "--fields",
                "{\"FULL\": \"Fusil côté Ä\"}",
                "{out}",
                "--plugins",
                "Starfield.esm",
            ]],
        }],
        compare: &[],
        notes: "",
    },
    FixtureSpec {
        id: "F14",
        title: "The CK loads our rename plugin as a non-active plugin",
        base: None,
        masters: &["Starfield.esm", "OldSovietMarksmanRifle.esm"],
        plugins: &[],
        compare: &[],
        notes: "Our plugin: C:\\Modding\\kitcraft\\out\\OldSovietMarksmanRifle.esm (2,221 bytes, \
                SHA-256 9C0B28A6...4078EA27). Record every warning the CK shows on load; \
                nothing needs saving. Copy the CK's log or warning text here if it offers one.",
    },
];

/// TOML basic string.
fn q(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn list(items: &[&str]) -> String {
    format!(
        "[{}]",
        items.iter().map(|s| q(s)).collect::<Vec<_>>().join(", ")
    )
}

/// The `fixture.toml` template for one matrix fixture.
fn template(spec: &FixtureSpec, date: &str, base: Option<String>) -> String {
    let mut t = String::new();
    let _ = writeln!(
        t,
        "# fixture.toml: one CK observation fixture (docs/ck-observations.md, M3.O1 script)."
    );
    let _ = writeln!(
        t,
        "# Fill the empty fields during the session; `cargo xtask observe <this folder>` reads it."
    );
    let _ = writeln!(t, "fixture = {}", q(spec.id));
    let _ = writeln!(t, "title = {}", q(spec.title));
    let _ = writeln!(t, "date = {}", q(date));
    let _ = writeln!(t, "ck_version = \"\"        # CK title bar");
    let _ = writeln!(
        t,
        "game_build = \"\"        # Starfield.exe Properties > Details"
    );
    let _ = writeln!(t, "mo2_version = \"\"");
    let _ = writeln!(
        t,
        "masters = {}   # ticked in File > Data",
        list(spec.masters)
    );
    let _ = writeln!(
        t,
        "active_file = \"\"       # the active file set in File > Data, if any"
    );
    let _ = writeln!(
        t,
        "landed_in = \"\"         # where the CK saved (expected C:\\\\Modding\\\\MO2-Starfield\\\\overwrite)"
    );
    match (spec.base, base) {
        (Some(_), Some(path)) => {
            let _ = writeln!(t, "base = {}   # relative to this folder", q(&path));
        }
        (Some((id, file)), None) => {
            let _ = writeln!(
                t,
                "base = \"\"   # fill: ..\\\\<date>-{id}\\\\{file} (the {id} fixture's plugin)"
            );
        }
        (None, _) => {}
    }
    let _ = writeln!(
        t,
        "messages = []           # every warning or message the CK showed (text)"
    );
    let _ = writeln!(t, "screenshots = []        # file names in this folder");
    let _ = writeln!(t, "notes = {}", q(spec.notes));
    for p in spec.plugins {
        let _ = writeln!(t);
        let _ = writeln!(t, "[[plugin]]");
        let _ = writeln!(t, "file = {}", q(p.file));
        let _ = writeln!(t, "form = {}", q(p.form));
        let _ = writeln!(t, "editor_id = {}", q(p.editor_id));
        let _ = writeln!(t, "action = {}", q(p.action));
        let _ = writeln!(t, "dialog = \"\"            # dialog and tab as shown");
        let _ = writeln!(t, "field = \"\"             # field label as shown");
        let _ = writeln!(t, "sentinel = {}", q(p.sentinel));
        let _ = writeln!(
            t,
            "value_before = \"\"      # the value shown before the edit"
        );
        if p.kit.is_empty() {
            let _ = writeln!(
                t,
                "kit_equivalent = []     # kit argument vectors building our equivalent; {{out}} = our output"
            );
        } else {
            let _ = writeln!(t, "kit_equivalent = [");
            for argv in p.kit {
                let _ = writeln!(t, "  {},", list(argv));
            }
            let _ = writeln!(t, "]");
        }
    }
    for (a, b) in spec.compare {
        let _ = writeln!(t);
        let _ = writeln!(t, "[[compare]]");
        let _ = writeln!(t, "a = {}", q(a));
        let _ = writeln!(t, "b = {}", q(b));
    }
    t
}

/// The newest `<date>-<id>` folder in `root` holding `file`, relative to a sibling folder.
fn find_base(root: &Path, id: &str, file: &str) -> Option<String> {
    let suffix = format!("-{id}");
    let mut found: Vec<String> = fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|name| name.ends_with(&suffix) && root.join(name).join(file).is_file())
        .collect();
    found.sort();
    found.pop().map(|dir| {
        format!(
            "..{}{dir}{}{file}",
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        )
    })
}

/// Creates `<root>/<date>-<id>/fixture.toml`; refuses to overwrite one.
fn new_fixture(root: &Path, id: &str, date: &str) -> Result<PathBuf> {
    let spec = MATRIX
        .iter()
        .find(|s| s.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| anyhow!("observe new: `{id}` is not a matrix fixture (F00..F14)"))?;
    let valid_date = date.len() == 10
        && date.chars().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    if !valid_date {
        bail!("observe new: date `{date}` is not yyyy-mm-dd");
    }
    let dir = root.join(format!("{date}-{}", spec.id));
    let path = dir.join(FIXTURE_TOML);
    if path.exists() {
        bail!("observe new: {} exists; not overwritten", path.display());
    }
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let base = spec.base.and_then(|(bid, file)| find_base(root, bid, file));
    fs::write(&path, template(spec, date, base))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// Today's UTC date as `yyyy-mm-dd` (civil-from-days).
fn today() -> Option<String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let days = i64::try_from(secs.checked_div(86_400)?).ok()?;
    let z = days.checked_add(719_468)?;
    let era = z.checked_div(146_097)?;
    let doe = z.checked_sub(era.checked_mul(146_097)?)?;
    let yoe = doe
        .checked_sub(doe.checked_div(1_460)?)?
        .checked_add(doe.checked_div(36_524)?)?
        .checked_sub(doe.checked_div(146_096)?)?
        .checked_div(365)?;
    let doy = doe.checked_sub(
        yoe.checked_mul(365)?
            .checked_add(yoe.checked_div(4)?)?
            .checked_sub(yoe.checked_div(100)?)?,
    )?;
    let mp = doy.checked_mul(5)?.checked_add(2)?.checked_div(153)?;
    let d = doy
        .checked_sub(mp.checked_mul(153)?.checked_add(2)?.checked_div(5)?)?
        .checked_add(1)?;
    let m = if mp < 10 {
        mp.checked_add(3)?
    } else {
        mp.checked_sub(9)?
    };
    let y = yoe
        .checked_add(era.checked_mul(400)?)?
        .checked_add(i64::from(m <= 2))?;
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

// ---------------------------------------------------------------------------------------
// Processing a fixture folder (M3.H2).

/// Every file under `dir` (relative, `/`-separated), except the harness's own outputs.
fn fixture_files(dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let here = dir.join(&rel);
        for entry in fs::read_dir(&here).with_context(|| format!("reading {}", here.display()))? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let child = rel.join(name.as_ref());
            let top = rel.as_os_str().is_empty();
            if top && (name == KIT_DIR || name == REPORT_JSON || name == REPORT_MD) {
                continue;
            }
            if entry.file_type()?.is_dir() {
                stack.push(child);
            } else {
                out.push(child.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    Ok(out)
}

fn is_plugin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".esp") || lower.ends_with(".esm") || lower.ends_with(".esl")
}

fn s(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// The parts of a `kit` result a report keeps.
fn digest(out: &KitOut, keys: &[&str]) -> Value {
    if let Some(e) = out.error() {
        return json!({"error": e, "exit": out.code});
    }
    let data = out.data();
    let mut d = serde_json::Map::new();
    d.insert("exit".to_string(), json!(out.code));
    for k in keys {
        if let Some(v) = data.get(*k) {
            d.insert((*k).to_string(), v.clone());
        }
    }
    if let Some(l) = out.json.get("limitations") {
        d.insert("limitations".to_string(), l.clone());
    }
    let mut v = Value::Object(d);
    strip_volatile(&mut v);
    v
}

fn coverage_digest(out: &KitOut) -> Value {
    let mut d = digest(
        out,
        &[
            "compressed_records",
            "compressed_examined",
            "groups",
            "occurrences",
        ],
    );
    if out.error().is_none() {
        let types: Vec<Value> = out
            .data()
            .get("record_types")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|t| {
                json!({
                    "signature": t.get("signature"),
                    "records": t.get("records"),
                    "compressed": t.get("compressed"),
                })
            })
            .collect();
        let records: u64 = types
            .iter()
            .filter_map(|t| t.get("records").and_then(Value::as_u64))
            .fold(0u64, u64::saturating_add);
        if let Some(map) = d.as_object_mut() {
            map.insert("records".to_string(), json!(records));
            map.insert("record_types".to_string(), Value::Array(types));
        }
    }
    d
}

fn diff_digest(out: &KitOut) -> Value {
    let mut d = digest(out, &["counts", "keyed_by", "added", "removed", "changed"]);
    strip_volatile(&mut d);
    d
}

/// One comparison: byte diff, TES4 diff, `kit diff`.
fn compare(
    kit: &dyn Runner,
    cache: &Path,
    label: &str,
    a: &Path,
    b: &Path,
    shown_a: &str,
    shown_b: &str,
) -> Result<Value> {
    let ba = fs::read(a).with_context(|| format!("reading {}", a.display()))?;
    let bb = fs::read(b).with_context(|| format!("reading {}", b.display()))?;
    let bytes = byte_diff(&ba, &bb, RANGE_CAP);
    let tes4_lines = match (tes4(&ba), tes4(&bb)) {
        (Ok(x), Ok(y)) => json!(tes4_diff(&x, &y)),
        (x, y) => json!([format!(
            "TES4 not comparable: {} / {}",
            x.err().unwrap_or_default(),
            y.err().unwrap_or_default()
        )]),
    };
    let kit_diff = if bytes.identical {
        json!({"skipped": "identical bytes"})
    } else {
        diff_digest(&kit.kit(&["diff".to_string(), arg(a)?, arg(b)?], cache)?)
    };
    Ok(json!({
        "label": label,
        "a": shown_a,
        "b": shown_b,
        "sha256_a": sha256_hex(&ba),
        "sha256_b": sha256_hex(&bb),
        "bytes": bytes.to_json(),
        "tes4": tes4_lines,
        "kit_diff": kit_diff,
    }))
}

fn toml_strings(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect()
}

fn toml_str(t: &toml::Table, key: &str) -> String {
    t.get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Whether `out` failed because the command has no `style` argument (before M3.E5).
fn style_unsupported(out: &KitOut) -> bool {
    out.code == Some(2)
        && out
            .error()
            .is_some_and(|e| e.contains("style") || e.contains("unexpected argument"))
}

/// Builds our equivalent of one CK plugin in each write style and compares it.
fn equivalents(
    kit: &dyn Runner,
    dir: &Path,
    cache: &Path,
    file: &str,
    argvs: &[Vec<String>],
    comparisons: &mut Vec<Value>,
) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for style in STYLES {
        let out_dir = dir.join(KIT_DIR).join(style);
        fs::create_dir_all(&out_dir)?;
        let out_path = out_dir.join(file);
        if out_path.exists() {
            fs::remove_file(&out_path)
                .with_context(|| format!("clearing {}", out_path.display()))?;
        }
        let out_text = arg(&out_path)?;
        let mut status = "built".to_string();
        let mut runs = Vec::new();
        for argv in argvs {
            let words: Vec<String> = argv.iter().map(|w| w.replace("{out}", &out_text)).collect();
            let mut with_style = words.clone();
            with_style.extend(["--style".to_string(), style.to_string()]);
            let mut result = kit.kit(&with_style, cache)?;
            if style_unsupported(&result) {
                if style == "kit" {
                    // Before M3.E5 the default (and only) style is kit's own.
                    result = kit.kit(&words, cache)?;
                    status = "built (no style argument yet: default style)".to_string();
                } else {
                    status =
                        "unavailable: the command has no `style` argument yet (M3.E5)".to_string();
                    runs.push(json!({"argv": words, "error": result.error()}));
                    break;
                }
            }
            if let Some(e) = result.error() {
                status = format!("failed: {e}");
            }
            let mut d = digest(&result, &["summary", "passed"]);
            strip_volatile(&mut d);
            runs.push(json!({"argv": words, "result": d}));
        }
        let built = out_path.is_file() && status.starts_with("built");
        let ck = dir.join(file);
        let mut row = json!({
            "plugin": file,
            "style": style,
            "status": status,
            "runs": runs,
            "out": format!("{KIT_DIR}/{style}/{file}"),
        });
        if built {
            let bytes = fs::read(&out_path)?;
            if let Some(map) = row.as_object_mut() {
                map.insert("sha256".to_string(), json!(sha256_hex(&bytes)));
            }
            if ck.is_file() {
                let c = compare(
                    kit,
                    cache,
                    &format!("CK vs ours ({style} style)"),
                    &ck,
                    &out_path,
                    file,
                    &format!("{KIT_DIR}/{style}/{file}"),
                )?;
                let identical = c
                    .pointer("/bytes/identical")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if let Some(map) = row.as_object_mut() {
                    map.insert("identical_to_ck".to_string(), json!(identical));
                }
                comparisons.push(c);
            }
        }
        rows.push(row);
    }
    Ok(rows)
}

/// Processes one fixture folder and writes its reports; returns the JSON report.
pub(crate) fn observe(dir: &Path, kit: &dyn Runner) -> Result<Value> {
    if !dir.is_dir() {
        bail!("observe: {} is not a directory", dir.display());
    }
    let cache = dir.join(KIT_DIR).join("cache");
    fs::create_dir_all(&cache).with_context(|| format!("creating {}", cache.display()))?;
    let mut limitations: Vec<String> = Vec::new();

    let toml_path = dir.join(FIXTURE_TOML);
    let fixture: toml::Table = if toml_path.is_file() {
        let text = fs::read_to_string(&toml_path)?;
        toml::from_str(&text).with_context(|| format!("parsing {}", toml_path.display()))?
    } else {
        limitations.push(format!(
            "no {FIXTURE_TOML}: CK version, game build, actions and sentinels are unknown"
        ));
        toml::Table::new()
    };
    let fixture_json = serde_json::to_value(&fixture).unwrap_or(Value::Null);

    // Files.
    let names = fixture_files(dir)?;
    let mut files = Vec::new();
    for name in &names {
        let bytes = fs::read(dir.join(name))?;
        files.push(json!({"file": name, "bytes": bytes.len(), "sha256": sha256_hex(&bytes)}));
    }

    // Plugins.
    let listed: Vec<toml::Table> = fixture
        .get("plugin")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_table().cloned())
        .collect();
    let mut plugins = Vec::new();
    for name in names.iter().filter(|n| is_plugin(n)) {
        let path = dir.join(name);
        let p = arg(&path)?;
        let bytes = fs::read(&path)?;
        let header = match tes4(&bytes) {
            Ok(t) => t.to_json(),
            Err(e) => json!({"problem": e}),
        };
        let roundtrip = digest(
            &kit.kit(
                &["roundtrip".to_string(), p.clone(), "--verify".into()],
                &cache,
            )?,
            &[
                "passed",
                "identical",
                "mismatch",
                "records",
                "groups",
                "compressed_records",
            ],
        );
        let coverage = coverage_digest(&kit.kit(&["coverage".to_string(), p.clone()], &cache)?);
        let validate = digest(
            &kit.kit(&["validate".to_string(), "--plugin".into(), p], &cache)?,
            &["passed", "errors", "warnings", "findings"],
        );
        if !listed
            .iter()
            .any(|t| toml_str(t, "file").eq_ignore_ascii_case(name))
        {
            limitations.push(format!("{name} is not listed in {FIXTURE_TOML}"));
        }
        plugins.push(json!({
            "file": name,
            "tes4": header,
            "roundtrip": roundtrip,
            "coverage": coverage,
            "validate": validate,
        }));
    }

    // Comparisons.
    let mut comparisons = Vec::new();
    let base = toml_str(&fixture, "base");
    if base.is_empty() && fixture.contains_key("base") {
        limitations.push(format!(
            "base is empty in {FIXTURE_TOML}: the saved files were not compared with their base"
        ));
    }
    if !base.is_empty() {
        let base_path = dir.join(&base);
        if base_path.is_file() {
            for t in &listed {
                let file = toml_str(t, "file");
                if dir.join(&file).is_file() {
                    comparisons.push(compare(
                        kit,
                        &cache,
                        "base vs CK",
                        &base_path,
                        &dir.join(&file),
                        &base,
                        &file,
                    )?);
                }
            }
        } else {
            limitations.push(format!("base {base} not found"));
        }
    }
    for pair in fixture
        .get("compare")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
    {
        let (a, b) = (toml_str(pair, "a"), toml_str(pair, "b"));
        if dir.join(&a).is_file() && dir.join(&b).is_file() {
            comparisons.push(compare(
                kit,
                &cache,
                "variant vs variant",
                &dir.join(&a),
                &dir.join(&b),
                &a,
                &b,
            )?);
        } else {
            limitations.push(format!("compare {a} vs {b}: a file is missing"));
        }
    }

    // Our equivalents, rebuilt from scratch on every run.
    for style in STYLES {
        let out_dir = dir.join(KIT_DIR).join(style);
        if out_dir.exists() {
            fs::remove_dir_all(&out_dir)
                .with_context(|| format!("clearing {}", out_dir.display()))?;
        }
    }
    let mut ours = Vec::new();
    for t in &listed {
        let file = toml_str(t, "file");
        let argvs: Vec<Vec<String>> = t
            .get("kit_equivalent")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .map(|a| toml_strings(Some(a)))
            .filter(|a| !a.is_empty())
            .collect();
        if argvs.is_empty() || file.is_empty() {
            continue;
        }
        if !dir.join(&file).is_file() {
            limitations.push(format!(
                "{file}: kit_equivalent given but the CK's file is missing"
            ));
        }
        ours.extend(equivalents(
            kit,
            dir,
            &cache,
            &file,
            &argvs,
            &mut comparisons,
        )?);
    }

    let report = json!({
        "generated_by": "cargo xtask observe (M3.H2)",
        "fixture_dir": dir.file_name().map(|n| n.to_string_lossy().into_owned()),
        "fixture": fixture_json,
        "files": files,
        "plugins": plugins,
        "comparisons": comparisons,
        "kit_equivalent": ours,
        "limitations": limitations,
    });
    let mut text = serde_json::to_string_pretty(&report)?;
    text.push('\n');
    fs::write(dir.join(REPORT_JSON), text)?;
    fs::write(dir.join(REPORT_MD), render(&report))?;
    Ok(report)
}

fn yes(v: Option<&Value>) -> &'static str {
    match v.and_then(Value::as_bool) {
        Some(true) => "yes",
        Some(false) => "**no**",
        None => "?",
    }
}

/// `report.md` from the JSON report.
fn render(r: &Value) -> String {
    let mut md = String::new();
    let fx = r.get("fixture").cloned().unwrap_or(Value::Null);
    let _ = writeln!(
        md,
        "# Observation report: {} {}\n",
        s(&fx, "fixture"),
        s(&fx, "title")
    );
    let _ = writeln!(
        md,
        "Written by `cargo xtask observe` (M3.H2) into this fixture folder; rerunning replaces it. \
         Findings go to the ledger in `docs/ck-observations.md` by hand.\n"
    );
    let _ = writeln!(md, "## Fixture\n");
    let rows: Vec<Vec<String>> = [
        "date",
        "ck_version",
        "game_build",
        "mo2_version",
        "masters",
        "active_file",
        "landed_in",
        "base",
        "messages",
        "notes",
    ]
    .iter()
    .map(|k| vec![(*k).to_string(), s(&fx, k)])
    .collect();
    md.push_str(&table(&["Field", "Value"], &rows));

    let _ = writeln!(md, "\n## Files\n");
    let rows: Vec<Vec<String>> = r
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|f| vec![format!("`{}`", s(f, "file")), s(f, "bytes"), s(f, "sha256")])
        .collect();
    md.push_str(&table(&["File", "Bytes", "SHA-256"], &rows));

    let _ = writeln!(md, "\n## Plugins\n");
    let rows: Vec<Vec<String>> = r
        .get("plugins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|p| {
            let t = p.get("tes4").cloned().unwrap_or(Value::Null);
            let hedr = t.get("hedr").cloned().unwrap_or(Value::Null);
            let rt = p.get("roundtrip").cloned().unwrap_or(Value::Null);
            let cov = p.get("coverage").cloned().unwrap_or(Value::Null);
            let val = p.get("validate").cloned().unwrap_or(Value::Null);
            vec![
                format!("`{}`", s(p, "file")),
                s(&t, "flags"),
                s(&t, "form_version"),
                format!(
                    "{} / {} / {}",
                    s(&hedr, "version"),
                    s(&hedr, "records"),
                    s(&hedr, "next_object_id")
                ),
                t.get("masters")
                    .and_then(Value::as_array)
                    .map(|m| {
                        m.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default(),
                match rt.get("error") {
                    Some(e) => format!("error: {e}"),
                    None => yes(rt.get("identical")).to_string(),
                },
                format!(
                    "{} ({} compressed)",
                    s(&cov, "records"),
                    s(&cov, "compressed_records")
                ),
                match val.get("error") {
                    Some(e) => format!("error: {e}"),
                    None => format!("{} / {}", s(&val, "errors"), s(&val, "warnings")),
                },
            ]
        })
        .collect();
    md.push_str(&table(
        &[
            "Plugin",
            "TES4 flags",
            "Form version",
            "HEDR version / count / next ID",
            "Masters",
            "Roundtrip identical",
            "Records",
            "Validate errors / warnings",
        ],
        &rows,
    ));
    for p in r
        .get("plugins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let _ = writeln!(md, "\n### `{}` TES4 subrecords\n", s(p, "file"));
        let rows: Vec<Vec<String>> = p
            .pointer("/tes4/subrecords")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|sub| {
                vec![
                    format!("`{}`", s(sub, "signature")),
                    s(sub, "size"),
                    match sub.get("text").and_then(Value::as_str) {
                        Some(t) => format!("\"{t}\""),
                        None => format!(
                            "`{}`{}",
                            s(sub, "hex"),
                            if sub.get("truncated") == Some(&Value::Bool(true)) {
                                "..."
                            } else {
                                ""
                            }
                        ),
                    },
                ]
            })
            .collect();
        md.push_str(&table(&["Signature", "Size", "Payload"], &rows));
        if let Some(problem) = p.pointer("/tes4/problem").and_then(Value::as_str) {
            let _ = writeln!(md, "\nProblem: {problem}");
        }
    }

    let _ = writeln!(md, "\n## Comparisons\n");
    let comparisons: Vec<&Value> = r
        .get("comparisons")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    if comparisons.is_empty() {
        let _ = writeln!(md, "None.");
    }
    for c in comparisons {
        let b = c.get("bytes").cloned().unwrap_or(Value::Null);
        let _ = writeln!(
            md,
            "### {}: `{}` -> `{}`\n",
            s(c, "label"),
            s(c, "a"),
            s(c, "b")
        );
        let _ = writeln!(
            md,
            "- Bytes: {} -> {}; identical: {}; first difference: {}; differing bytes {} in {} run(s)",
            s(&b, "size_a"),
            s(&b, "size_b"),
            yes(b.get("identical")),
            match b.get("first_difference").and_then(Value::as_u64) {
                Some(o) => format!("0x{o:X}"),
                None => "-".to_string(),
            },
            s(&b, "differing_bytes"),
            s(&b, "ranges_total"),
        );
        let _ = writeln!(
            md,
            "- SHA-256: `{}` -> `{}`",
            s(c, "sha256_a"),
            s(c, "sha256_b")
        );
        let ranges: Vec<String> = b
            .get("ranges")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|x| {
                format!(
                    "0x{:X}+{}",
                    x.get("offset").and_then(Value::as_u64).unwrap_or(0),
                    s(x, "length")
                )
            })
            .collect();
        if !ranges.is_empty() {
            let _ = writeln!(md, "- Runs (offset+length): {}", ranges.join(", "));
        }
        for line in c
            .get("tes4")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let _ = writeln!(md, "- TES4 {}", line.as_str().unwrap_or_default());
        }
        let kd = c.get("kit_diff").cloned().unwrap_or(Value::Null);
        if let Some(e) = kd.get("error") {
            let _ = writeln!(md, "- `kit diff`: error {e}");
        } else if let Some(skip) = kd.get("skipped") {
            let _ = writeln!(
                md,
                "- `kit diff`: skipped ({})",
                skip.as_str().unwrap_or("")
            );
        } else {
            let counts = kd.get("counts").cloned().unwrap_or(Value::Null);
            let _ = writeln!(
                md,
                "- `kit diff` (keyed by {}): {} added, {} removed, {} changed, {} unchanged",
                s(&kd, "keyed_by"),
                s(&counts, "added"),
                s(&counts, "removed"),
                s(&counts, "changed"),
                s(&counts, "unchanged")
            );
            for key in ["added", "removed"] {
                for rec in kd.get(key).and_then(Value::as_array).into_iter().flatten() {
                    let _ = writeln!(
                        md,
                        "  - {key}: {} `{}` (raw {})",
                        s(rec, "signature"),
                        s(rec, "form"),
                        s(rec, "raw_form_id")
                    );
                }
            }
            for ch in kd
                .get("changed")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let _ = writeln!(
                    md,
                    "  - changed: {} `{}` ({}){}{}",
                    s(ch, "signature"),
                    s(ch, "form"),
                    s(ch, "class"),
                    if ch.get("flags_differ") == Some(&Value::Bool(true)) {
                        "; flags differ"
                    } else {
                        ""
                    },
                    if ch.get("order_differs") == Some(&Value::Bool(true)) {
                        "; subrecord order differs"
                    } else {
                        ""
                    },
                );
                for sub in ch
                    .get("subrecords")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let _ = writeln!(
                        md,
                        "    - `{}`[{}] {}{}",
                        s(sub, "signature"),
                        s(sub, "occurrence"),
                        s(sub, "change"),
                        match sub.get("first_difference").and_then(Value::as_u64) {
                            Some(o) => format!(", first difference at payload byte {o}"),
                            None => String::new(),
                        }
                    );
                }
            }
        }
        let _ = writeln!(md);
    }

    let _ = writeln!(md, "## Our equivalent (`kit_equivalent`)\n");
    let rows: Vec<Vec<String>> = r
        .get("kit_equivalent")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|e| {
            vec![
                format!("`{}`", s(e, "plugin")),
                s(e, "style"),
                s(e, "status"),
                s(e, "sha256"),
                yes(e.get("identical_to_ck")).to_string(),
            ]
        })
        .collect();
    if rows.is_empty() {
        let _ = writeln!(md, "None in `{FIXTURE_TOML}`.");
    } else {
        md.push_str(&table(
            &[
                "CK plugin",
                "Style",
                "Status",
                "Our SHA-256",
                "Identical to the CK's",
            ],
            &rows,
        ));
    }

    let _ = writeln!(md, "\n## Limitations\n");
    let lims: Vec<&Value> = r
        .get("limitations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    if lims.is_empty() {
        let _ = writeln!(md, "None.");
    }
    for l in lims {
        let _ = writeln!(md, "- {}", l.as_str().unwrap_or_default());
    }
    md
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
pub(crate) mod tests {
    use super::*;
    use crate::evidence::tests::{sub, tes4_record};
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    /// A temp folder removed on drop.
    pub(crate) struct Temp(pub PathBuf);
    impl Temp {
        pub(crate) fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "kitcraft-xtask-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Every file under `dir` with its bytes.
    fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).unwrap() {
                let e = e.unwrap();
                if e.file_type().unwrap().is_dir() {
                    stack.push(e.path());
                } else {
                    out.insert(e.path(), fs::read(e.path()).unwrap());
                }
            }
        }
        out
    }

    fn glob_plugin(value: f32) -> Vec<u8> {
        let mut h = 0.96f32.to_le_bytes().to_vec();
        h.extend_from_slice(&2u32.to_le_bytes());
        h.extend_from_slice(&0xFF00_1000u32.to_le_bytes());
        let mut body = sub(b"HEDR", &h);
        body.extend(sub(b"CNAM", b"DEFAULT\0"));
        let mut out = tes4_record(0, &body);
        let mut rec_body = sub(b"EDID", b"KC_Test\0");
        rec_body.extend(sub(b"FLTV", &value.to_le_bytes()));
        let mut rec = b"GLOB".to_vec();
        rec.extend_from_slice(&(rec_body.len() as u32).to_le_bytes());
        rec.extend_from_slice(&0u32.to_le_bytes());
        rec.extend_from_slice(&0x800u32.to_le_bytes());
        rec.extend_from_slice(&[0; 4]);
        rec.extend_from_slice(&581u16.to_le_bytes());
        rec.extend_from_slice(&0u16.to_le_bytes());
        rec.extend(rec_body);
        out.extend_from_slice(b"GRUP");
        out.extend_from_slice(&((rec.len() + 24) as u32).to_le_bytes());
        out.extend_from_slice(b"GLOB");
        out.extend_from_slice(&[0; 12]);
        out.extend(rec);
        out
    }

    /// A fake kit: answers by command, writes `{out}` for `set` by copying a fixed plugin,
    /// and refuses `--style` (as kit does before M3.E5).
    struct Fake {
        calls: RefCell<Vec<Vec<String>>>,
        set_output: Vec<u8>,
    }

    impl Runner for Fake {
        fn kit(&self, args: &[String], cache: &Path) -> Result<KitOut> {
            self.calls.borrow_mut().push(args.to_vec());
            assert!(cache.ends_with("kit/cache") || cache.ends_with("kit\\cache"));
            let cmd = args[0].as_str();
            if args.iter().any(|a| a == "--style") {
                return Ok(KitOut {
                    code: Some(2),
                    json: json!({"error": {"kind": "usage", "message": "`set`: unexpected argument `--style`"}}),
                });
            }
            let data = match cmd {
                "roundtrip" => json!({"passed": true, "identical": true, "mismatch": null,
                    "records": 2, "groups": 1, "compressed_records": 0, "elapsed_ms": 7}),
                "coverage" => json!({"groups": 1, "compressed_records": 0,
                    "record_types": [{"signature": "TES4", "records": 1, "compressed": 0},
                                     {"signature": "GLOB", "records": 1, "compressed": 0}]}),
                "validate" => json!({"passed": true, "errors": 0, "warnings": 0, "findings": []}),
                "diff" => {
                    json!({"counts": {"added": 0, "removed": 0, "changed": 1, "unchanged": 0},
                    "keyed_by": "raw", "added": [], "removed": [],
                    "changed": [{"form": "KC.esp:000800", "signature": "GLOB", "class": "itm",
                        "subrecords": [{"signature": "FLTV", "occurrence": 0, "change": "changed", "first_difference": 1}]}]})
                }
                "set" => {
                    let out = args.iter().find(|a| a.ends_with(".esp")).unwrap();
                    fs::write(out, &self.set_output).unwrap();
                    json!({"summary": "created", "elapsed_ms": 3})
                }
                other => panic!("unexpected kit command {other}"),
            };
            Ok(KitOut {
                code: Some(0),
                json: json!({"command": cmd, "data": data, "limitations": []}),
            })
        }
    }

    #[test]
    fn observe_writes_only_inside_the_fixture_and_is_idempotent() {
        let tmp = Temp::new("observe");
        let root = tmp.0.join("observe");
        let base_dir = root.join("2026-10-20-F01");
        let dir = root.join("2026-10-20-F05");
        fs::create_dir_all(&base_dir).unwrap();
        fs::create_dir_all(dir.join("shots")).unwrap();
        fs::write(base_dir.join("KC_F01.esp"), glob_plugin(0.0)).unwrap();
        fs::write(dir.join("KC_F05a.esp"), glob_plugin(1234.5)).unwrap();
        fs::write(dir.join("KC_F05b.esp"), glob_plugin(2345.25)).unwrap();
        fs::write(dir.join("shots").join("before.png"), b"png").unwrap();
        // The template, filled the way the owner would.
        let spec = MATRIX.iter().find(|s| s.id == "F05").unwrap();
        let text = template(spec, "2026-10-20", find_base(&root, "F01", "KC_F01.esp"))
            .replace("ck_version = \"\"", "ck_version = \"1.15.222.0\"");
        fs::write(dir.join(FIXTURE_TOML), text).unwrap();

        let fake = Fake {
            calls: RefCell::new(Vec::new()),
            set_output: glob_plugin(1234.5),
        };
        let outside_before = snapshot(&tmp.0)
            .into_iter()
            .filter(|(p, _)| !p.starts_with(&dir))
            .collect::<BTreeMap<_, _>>();
        let report = observe(&dir, &fake).unwrap();
        let first_json = fs::read(dir.join(REPORT_JSON)).unwrap();
        let first_md = fs::read(dir.join(REPORT_MD)).unwrap();
        let outside_after = snapshot(&tmp.0)
            .into_iter()
            .filter(|(p, _)| !p.starts_with(&dir))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            outside_before, outside_after,
            "observe wrote outside the fixture"
        );

        // Files: the toml, two plugins, the screenshot; never the reports or kit/.
        let files: Vec<&str> = report["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["file"].as_str().unwrap())
            .collect();
        assert_eq!(
            files,
            vec![
                "KC_F05a.esp",
                "KC_F05b.esp",
                "fixture.toml",
                "shots/before.png"
            ]
        );
        assert_eq!(report["plugins"].as_array().unwrap().len(), 2);
        assert_eq!(report["plugins"][0]["tes4"]["hedr"]["records"], 2);
        assert_eq!(report["plugins"][0]["coverage"]["records"], 2);
        assert!(report["plugins"][0]["roundtrip"]
            .get("elapsed_ms")
            .is_none());
        // base vs a, base vs b, a vs b, CK vs ours for a (kit style) and b (kit style).
        let labels: Vec<&str> = report["comparisons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["label"].as_str().unwrap())
            .collect();
        assert_eq!(
            labels,
            vec![
                "base vs CK",
                "base vs CK",
                "variant vs variant",
                "CK vs ours (kit style)",
                "CK vs ours (kit style)"
            ]
        );
        let ab = &report["comparisons"][2];
        assert_eq!(ab["bytes"]["identical"], false);
        assert_eq!(ab["kit_diff"]["counts"]["changed"], 1);
        // Ours equals KC_F05a, not KC_F05b (the fake writes one fixed plugin).
        let eq = report["kit_equivalent"].as_array().unwrap();
        assert_eq!(eq.len(), 4);
        assert_eq!(eq[0]["style"], "kit");
        assert_eq!(eq[0]["identical_to_ck"], true);
        assert!(eq[1]["status"].as_str().unwrap().starts_with("unavailable"));
        assert_eq!(eq[2]["identical_to_ck"], false);
        assert!(dir.join("kit").join("kit").join("KC_F05a.esp").is_file());
        assert!(report["limitations"].as_array().unwrap().is_empty());
        let md = String::from_utf8(first_md.clone()).unwrap();
        assert!(md.contains("# Observation report: F05 One float GMST"));
        assert!(md.contains("| ck_version | 1.15.222.0 |"));
        assert!(md.contains("changed: GLOB `KC.esp:000800`"));

        // Rerun: identical reports, same files.
        observe(&dir, &fake).unwrap();
        assert_eq!(fs::read(dir.join(REPORT_JSON)).unwrap(), first_json);
        assert_eq!(fs::read(dir.join(REPORT_MD)).unwrap(), first_md);
        let outside_again = snapshot(&tmp.0)
            .into_iter()
            .filter(|(p, _)| !p.starts_with(&dir))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(outside_before, outside_again);
    }

    #[test]
    fn observe_without_fixture_toml_still_reports() {
        let tmp = Temp::new("observe-bare");
        fs::write(tmp.0.join("Stray.esp"), glob_plugin(1.0)).unwrap();
        fs::write(tmp.0.join("not-a-plugin.esp"), b"junk").unwrap();
        let fake = Fake {
            calls: RefCell::new(Vec::new()),
            set_output: Vec::new(),
        };
        let report = observe(&tmp.0, &fake).unwrap();
        let lims = report["limitations"].as_array().unwrap();
        assert!(lims
            .iter()
            .any(|l| l.as_str().unwrap().starts_with("no fixture.toml")));
        assert_eq!(report["plugins"][1]["file"], "not-a-plugin.esp");
        assert!(report["plugins"][1]["tes4"]["problem"].is_string());
        assert!(tmp.0.join(REPORT_MD).is_file());
        assert!(observe(&tmp.0.join("missing"), &fake).is_err());
    }

    #[test]
    fn every_matrix_template_parses() {
        let ids: Vec<&str> = MATRIX.iter().map(|s| s.id).collect();
        let expected: Vec<String> = (0..=14).map(|n| format!("F{n:02}")).collect();
        assert_eq!(ids, expected);
        for spec in MATRIX {
            let text = template(spec, "2026-10-20", None);
            let t: toml::Table =
                toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}\n{text}", spec.id));
            assert_eq!(t["fixture"].as_str(), Some(spec.id));
            let plugins = t
                .get("plugin")
                .and_then(|p| p.as_array())
                .map_or(0, Vec::len);
            assert_eq!(plugins, spec.plugins.len(), "{}", spec.id);
            for (p, ps) in t
                .get("plugin")
                .and_then(|p| p.as_array())
                .into_iter()
                .flatten()
                .zip(spec.plugins)
            {
                let kit = p["kit_equivalent"].as_array().unwrap();
                assert_eq!(kit.len(), ps.kit.len());
                for argv in kit {
                    let words = toml_strings(Some(argv));
                    assert!(words.contains(&"{out}".to_string()), "{}", spec.id);
                    // The --fields argument is valid JSON.
                    let i = words.iter().position(|w| w == "--fields").unwrap();
                    serde_json::from_str::<Value>(&words[i + 1]).unwrap();
                }
            }
            assert_eq!(t.contains_key("base"), spec.base.is_some(), "{}", spec.id);
        }
    }

    #[test]
    fn new_fixture_writes_once_and_finds_its_base() {
        let tmp = Temp::new("observe-new");
        let p = new_fixture(&tmp.0, "f02", "2026-10-20").unwrap();
        assert!(
            p.ends_with("2026-10-20-F02/fixture.toml")
                || p.ends_with("2026-10-20-F02\\fixture.toml")
        );
        assert!(
            new_fixture(&tmp.0, "F02", "2026-10-20").is_err(),
            "overwrote"
        );
        assert!(new_fixture(&tmp.0, "F99", "2026-10-20").is_err());
        assert!(new_fixture(&tmp.0, "F03", "20261020").is_err());
        // F03's base is found once F02's plugin exists.
        fs::write(tmp.0.join("2026-10-20-F02").join("KC_F02.esp"), b"x").unwrap();
        let f03 = new_fixture(&tmp.0, "F03", "2026-10-21").unwrap();
        let t: toml::Table = toml::from_str(&fs::read_to_string(f03).unwrap()).unwrap();
        let base = t["base"].as_str().unwrap();
        assert!(tmp.0.join("2026-10-21-F03").join(base).is_file(), "{base}");
    }

    #[test]
    fn today_is_a_date() {
        let d = today().unwrap();
        assert_eq!(d.len(), 10);
        assert!(d.as_str() >= "2026-01-01");
    }

    /// The matrix in code and the ledger doc name the same forms.
    #[test]
    fn matrix_matches_the_ledger_doc() {
        let doc = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("docs")
            .join("ck-observations.md");
        let text = fs::read_to_string(doc).unwrap();
        for form in [
            RIFLE,
            AMMO,
            GMST,
            NPC,
            WRLD,
            "Starfield.esm:000FB76D",
            "Constellation.esm:000041",
            "SFBGS004.esm:000EE4",
            "SFBGS006.esm:000800",
        ] {
            assert!(text.contains(form), "docs/ck-observations.md lacks {form}");
        }
        for spec in MATRIX {
            assert!(text.contains(&format!("| {} ", spec.id)), "{}", spec.id);
            for p in spec.plugins {
                assert!(text.contains(p.file), "{}", p.file);
            }
        }
    }

    /// End to end with the real `kit` from this target dir, when it has been built (it is in
    /// `cargo test --workspace`); skipped with a message otherwise.
    #[test]
    fn observe_with_the_real_kit() {
        let Some(kit) = built_kit() else {
            eprintln!("skipped: no kit binary next to the test executable (build kit-cli)");
            return;
        };
        let tmp = Temp::new("observe-real");
        fs::write(tmp.0.join("KC_A.esp"), glob_plugin(1.0)).unwrap();
        fs::write(tmp.0.join("KC_B.esp"), glob_plugin(2.0)).unwrap();
        fs::write(
            tmp.0.join(FIXTURE_TOML),
            "fixture = \"T\"\n[[plugin]]\nfile = \"KC_A.esp\"\n[[plugin]]\nfile = \"KC_B.esp\"\n\
             [[compare]]\na = \"KC_A.esp\"\nb = \"KC_B.esp\"\n",
        )
        .unwrap();
        let runner = KitBinary::at(kit);
        let report = observe(&tmp.0, &runner).unwrap();
        let first = fs::read(tmp.0.join(REPORT_JSON)).unwrap();
        for p in report["plugins"].as_array().unwrap() {
            assert_eq!(p["roundtrip"]["identical"], true, "{p}");
            assert_eq!(p["coverage"]["records"], 2, "{p}");
        }
        let c = &report["comparisons"][0];
        assert_eq!(c["bytes"]["differing_bytes"], 2, "{c}");
        assert_eq!(c["kit_diff"]["counts"]["changed"], 1, "{c}");
        observe(&tmp.0, &runner).unwrap();
        assert_eq!(fs::read(tmp.0.join(REPORT_JSON)).unwrap(), first);
    }

    /// `kit` built into the same target dir as this test, if present.
    pub(crate) fn built_kit() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let profile_dir = exe.parent()?.parent()?;
        let name = format!("kit{}", std::env::consts::EXE_SUFFIX);
        let p = profile_dir.join(name);
        p.is_file().then_some(p)
    }
}
