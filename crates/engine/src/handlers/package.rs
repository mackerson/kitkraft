//! `package` (M4: the plan's M4.E5 shape, owned by lane G): an MO2-ready mod folder for one
//! plugin, an optional `.zip` of it, and `kitcraft-manifest.json`.
//!
//! - `validate` runs first, on the plugin (and the scripts given), with the same load-order
//!   arguments as `validate`; a plugin that fails it is refused and nothing is written.
//! - The folder is `<out>\<name>`: the plugin at the root, loose files (`loose`, a folder laid
//!   out as `Data`) and scripts (`scripts`: `.pex` files or folders of them, placed at
//!   `Scripts\<namespace>\<name>.pex` from the object name inside each `.pex`) at their Data
//!   paths, or, with `ba2`, packed through `kitcraft-ba2` into `<plugin stem> - Main.ba2`
//!   (GNRL) and `<plugin stem> - Textures.ba2` (DX10, the `.dds` files): the names the
//!   official plugins' archives use (`docs/corpus.md` "Archive names", M4.G4; grade C until
//!   the M4.O2 in-game session).
//! - The manifest lists every file of the folder with its size and SHA-256, the plugin's
//!   masters and record counts per type, the KitCraft version, the SHA-256 of the build spec
//!   (`spec`, when given), the `validate` result, and what each archive holds. No time, no
//!   absolute path: two runs on the same inputs give the same bytes, folder and zip.
//! - The zip holds the folder's contents at its root (what MO2's "install a mod from an
//!   archive" expects), sorted, deflated, every entry dated 1980-01-01.
//! - Everything is written under `out` and nowhere else: the folder is built under a
//!   temporary name in `out` and renamed into place; `out` itself is refused inside the
//!   game's Data folder, the user Data folder, or a Mod Organizer instance or profile
//!   (`edit::patch::guard_location`): packaging is target-independent and never installs
//!   (M4.O5 default: the owner installs through MO2). An existing folder is replaced only
//!   with `overwrite` and only when it holds a `kitcraft-manifest.json`.

mod sha256;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use kitcraft_ba2::{Kind, PackOptions};
use serde_json::{json, Value};

use super::{failed, file_name, str_arg};
use crate::edit::patch::absolute;
use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::session::Session;

const ID: &str = "package";
/// The manifest's file name, at the root of the folder.
pub(crate) const MANIFEST: &str = "kitcraft-manifest.json";
/// Bump when the manifest's shape changes.
const MANIFEST_FORMAT: u32 = 1;
/// Deepest folder walked under `loose` or a scripts folder.
const MAX_DEPTH: usize = 32;
/// Most files in one package: the limit `deploy --dev` reads a package with, so every
/// package `package` writes can be deployed (`docs/packaging.md`).
const MAX_FILES: usize = crate::deploy::MAX_PACKAGE_FILES;
/// Largest `.pex` read to find its object name.
const MAX_PEX_BYTES: u64 = 64 << 20;

/// One file of the package: its Data path (`/`-separated, as given) and where it comes from.
#[derive(Debug, Clone)]
struct Entry {
    rel: String,
    src: PathBuf,
}

fn bool_arg(args: &Value, name: &str, default: bool) -> bool {
    args.get(name).and_then(Value::as_bool).unwrap_or(default)
}

fn opt_str<'v>(args: &'v Value, name: &str) -> Option<&'v str> {
    args.get(name).and_then(Value::as_str)
}

/// Whether `c` is a safe single path component on Windows (no separators, `.`/`..`,
/// trailing dots or spaces, control characters or device names).
fn valid_component(c: &str) -> bool {
    const DEVICES: [&str; 22] = [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    if c.is_empty() || c.len() > 255 || c == "." || c == ".." {
        return false;
    }
    if c.ends_with(' ') || c.ends_with('.') || c.starts_with(' ') {
        return false;
    }
    if c.chars().any(|ch| {
        ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }) {
        return false;
    }
    let stem = c.split('.').next().unwrap_or_default().trim_end();
    !DEVICES.iter().any(|d| stem.eq_ignore_ascii_case(d))
}

/// Every regular file under `root`, as `(parts below root, path)`, sorted; symbolic links
/// and other non-files are refused.
fn walk(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<(Vec<String>, PathBuf)>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "`{}` is nested more than {MAX_DEPTH} folders deep",
            dir.display()
        ));
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("reading `{}`: {e}", dir.display()))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("reading `{}`: {e}", dir.display()))?;
    entries.sort();
    for path in entries {
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| format!("reading `{}`: {e}", path.display()))?;
        if meta.is_dir() {
            walk(root, &path, depth.saturating_add(1), out)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| format!("`{}` is outside `{}`", path.display(), root.display()))?;
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            if let Some(bad) = parts.iter().find(|p| !valid_component(p)) {
                return Err(format!(
                    "`{}`: the name `{bad}` is not a safe file name",
                    path.display()
                ));
            }
            if out.len() >= MAX_FILES {
                return Err(format!("more than {MAX_FILES} files"));
            }
            out.push((parts, path));
        } else {
            return Err(format!(
                "`{}` is neither a file nor a folder (a link?); package copies plain files only",
                path.display()
            ));
        }
    }
    Ok(())
}

/// The `.pex` files named by `scripts` (files, or folders searched for `*.pex`), each placed
/// at `Scripts/<object name with ':' as '/'>.pex`.
fn script_entries(scripts: &[String]) -> Result<Vec<Entry>, String> {
    let mut files: Vec<PathBuf> = Vec::new();
    for s in scripts {
        let p = PathBuf::from(s);
        if p.is_dir() {
            let mut found = Vec::new();
            walk(&p, &p, 0, &mut found)?;
            files.extend(
                found
                    .into_iter()
                    .map(|(_, f)| f)
                    .filter(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("pex"))),
            );
        } else if p.is_file() {
            files.push(p);
        } else {
            return Err(format!("script `{s}` does not exist"));
        }
    }
    let mut out = Vec::new();
    for f in files {
        let bytes = super::papyrus::read_capped(&f, MAX_PEX_BYTES)
            .map_err(|e| format!("reading `{}`: {e}", f.display()))?;
        let pex = kitcraft_pex::Pex::parse(&bytes)
            .map_err(|e| format!("`{}` is not a readable .pex: {e}", f.display()))?;
        let name = pex
            .objects
            .first()
            .map(|o| pex.string(o.name).to_string())
            .filter(|n| !n.is_empty())
            .ok_or_else(|| format!("`{}` declares no script object", f.display()))?;
        let parts: Vec<&str> = name.split(':').collect();
        if parts.iter().any(|p| !valid_component(p)) {
            return Err(format!(
                "`{}`: script name `{name}` is not a safe path",
                f.display()
            ));
        }
        out.push(Entry {
            rel: format!("Scripts/{}.pex", parts.join("/")),
            src: f,
        });
    }
    Ok(out)
}

/// Copies `src` to `dst`, creating folders.
fn copy_into(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("creating `{}`: {e}", parent.display()))?;
    }
    fs::copy(src, dst)
        .map(|_| ())
        .map_err(|e| format!("copying `{}`: {e}", src.display()))
}

/// SHA-256 and size of a file.
fn hash_file(path: &Path) -> Result<(String, u64), String> {
    let file = File::open(path).map_err(|e| format!("reading `{}`: {e}", path.display()))?;
    sha256::digest(BufReader::new(file)).map_err(|e| format!("reading `{}`: {e}", path.display()))
}

/// The folder path of `rel` under `root`.
fn under(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(root.to_path_buf(), |p, c| p.join(c))
}

/// Packs `entries` into `path`; returns the manifest's description of the archive.
fn pack(
    entries: &[Entry],
    path: &Path,
    kind: Kind,
    version: u32,
    compress: bool,
) -> Result<Value, String> {
    let inputs: Vec<(String, String)> = entries
        .iter()
        .map(|e| (e.rel.clone(), e.src.display().to_string()))
        .collect();
    let file = File::create(path).map_err(|e| format!("creating `{}`: {e}", path.display()))?;
    let mut writer = BufWriter::new(file);
    let report = kitcraft_ba2::pack(
        &inputs,
        |p| fs::read(p),
        &PackOptions {
            kind,
            version,
            compress,
        },
        &mut writer,
    )
    .map_err(|e| format!("packing `{}`: {e}", file_name(path)))?;
    writer
        .flush()
        .map_err(|e| format!("writing `{}`: {e}", path.display()))?;
    let mut files = Vec::new();
    for p in &report.files {
        let src = entries
            .iter()
            .find(|e| e.rel.replace('\\', "/").eq_ignore_ascii_case(&p.name))
            .map(|e| e.src.clone());
        let sha = match &src {
            Some(s) => Some(hash_file(s)?.0),
            None => None,
        };
        // The hash of the source file packed (the archive's entry bytes may be
        // compressed, and a texture's header is rewritten), hence `source_sha256`.
        files.push(json!({"name": p.name, "size": p.size, "source_sha256": sha}));
    }
    Ok(json!({
        "file": file_name(path),
        "format": match kind { Kind::Textures => "dx10", Kind::General => "gnrl" },
        "version": version,
        "files": files,
    }))
}

/// Writes the zip of `folder`'s files (`files`: Data paths, sorted) to `path`.
fn write_zip(folder: &Path, files: &[String], path: &Path) -> Result<(), String> {
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, DateTime, ZipWriter};
    let file = File::create(path).map_err(|e| format!("creating `{}`: {e}", path.display()))?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    for rel in files {
        zip.start_file(rel.as_str(), options)
            .map_err(|e| format!("zip `{rel}`: {e}"))?;
        let mut src =
            File::open(under(folder, rel)).map_err(|e| format!("reading `{rel}`: {e}"))?;
        std::io::copy(&mut src, &mut zip).map_err(|e| format!("zip `{rel}`: {e}"))?;
    }
    let mut inner = zip
        .finish()
        .map_err(|e| format!("finishing the zip: {e}"))?;
    inner
        .flush()
        .map_err(|e| format!("writing `{}`: {e}", path.display()))
}

/// Records per type (`TES4` not counted).
type RecordCounts = BTreeMap<String, u64>;

/// The plugin's masters and record counts per type, from its index.
fn plugin_facts(plugin: &Path) -> Result<(Vec<String>, RecordCounts), String> {
    let index =
        kitcraft_esm::index::index_file(plugin, &kitcraft_esm::index::IndexOptions::default())
            .map_err(|e| format!("{}: {e}", plugin.display()))?;
    let masters = kitcraft_esm::tes4::Tes4::new(&index.header)
        .masters()
        .iter()
        .map(kitcraft_esm::tes4::Master::name_lossy)
        .collect();
    let mut counts = RecordCounts::new();
    for r in index.records.iter().skip(1) {
        let n = counts.entry(r.signature.to_string()).or_insert(0);
        *n = n.saturating_add(1);
    }
    Ok((masters, counts))
}

/// The handler.
pub(super) fn run(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let fail = |m: String| failed(ID, m);
    let plugin = PathBuf::from(str_arg(ID, args, "plugin")?);
    let out = PathBuf::from(str_arg(ID, args, "out")?);
    if !plugin.is_file() {
        return Err(fail(format!(
            "plugin `{}` does not exist",
            plugin.display()
        )));
    }
    let plugin_name = file_name(&plugin);
    let ext = plugin
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "esm" | "esp" | "esl") {
        return Err(fail(format!(
            "`{plugin_name}` is not a .esm, .esp or .esl plugin"
        )));
    }
    let stem = plugin
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = opt_str(args, "name").map_or_else(|| stem.clone(), str::to_string);
    if !valid_component(&name) {
        return Err(fail(format!("`{name}` cannot be a folder name")));
    }
    let version = opt_str(args, "version");
    if let Some(v) = version {
        if v.is_empty()
            || !v
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        {
            return Err(fail(format!(
                "version `{v}`: use ASCII letters, digits, `.`, `-` and `_`"
            )));
        }
    }
    let want_zip = bool_arg(args, "zip", true);
    let use_ba2 = bool_arg(args, "ba2", false);
    let overwrite = bool_arg(args, "overwrite", false);
    let ba2_version = args
        .get("ba2_version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(2);
    let ba2_compress = bool_arg(args, "ba2_compress", true);

    // Where things go, and what may be replaced: decided before anything is read or written.
    let mod_dir = out.join(&name);
    let zip_name = version.map_or_else(|| format!("{name}.zip"), |v| format!("{name}-{v}.zip"));
    let zip_path = out.join(&zip_name);
    for target in [mod_dir.join(MANIFEST), zip_path.clone()] {
        crate::edit::patch::guard_location(&target).map_err(|e| fail(e.to_string()))?;
    }
    if absolute(&plugin).starts_with(absolute(&mod_dir)) {
        return Err(fail(format!(
            "the plugin is inside `{}`, the folder package writes; build it elsewhere",
            mod_dir.display()
        )));
    }
    if mod_dir.exists() {
        if !overwrite {
            return Err(fail(format!(
                "`{}` exists; pass overwrite to replace a folder package made",
                mod_dir.display()
            )));
        }
        if !mod_dir.join(MANIFEST).is_file() {
            return Err(fail(format!(
                "`{}` exists and has no {MANIFEST}: package only replaces folders it made",
                mod_dir.display()
            )));
        }
    }
    if want_zip && zip_path.exists() && !overwrite {
        return Err(fail(format!(
            "`{}` exists; pass overwrite to replace it",
            zip_path.display()
        )));
    }

    // Inputs.
    let mut entries = vec![Entry {
        rel: plugin_name.clone(),
        src: plugin.clone(),
    }];
    if let Some(loose) = opt_str(args, "loose") {
        let root = PathBuf::from(loose);
        if !root.is_dir() {
            return Err(fail(format!("loose folder `{loose}` does not exist")));
        }
        let (root_abs, mod_abs) = (absolute(&root), absolute(&mod_dir));
        if mod_abs.starts_with(&root_abs) || root_abs.starts_with(&mod_abs) {
            return Err(fail(format!(
                "the loose folder `{loose}` and the package folder `{}` overlap",
                mod_dir.display()
            )));
        }
        let mut found = Vec::new();
        walk(&root, &root, 0, &mut found).map_err(fail)?;
        for (parts, src) in found {
            let rel = parts.join("/");
            let is_plugin = parts.len() == 1
                && [".esm", ".esp", ".esl"]
                    .iter()
                    .any(|x| rel.to_ascii_lowercase().ends_with(x));
            if is_plugin {
                return Err(fail(format!(
                    "the loose folder holds the plugin `{rel}`: package takes one plugin, as `plugin`"
                )));
            }
            if rel.eq_ignore_ascii_case(MANIFEST) {
                return Err(fail(format!("the loose folder holds a `{MANIFEST}`")));
            }
            entries.push(Entry { rel, src });
        }
    }
    let script_args: Vec<String> = args
        .get("scripts")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let scripts = script_entries(&script_args).map_err(fail)?;
    let script_files: Vec<String> = scripts
        .iter()
        .map(|e| e.src.display().to_string())
        .collect();
    entries.extend(scripts);
    // The folder holds every entry (or, with `ba2`, the plugin and at most two archives).
    if !use_ba2 && entries.len() > MAX_FILES {
        return Err(fail(format!(
            "{} files to package: more than {MAX_FILES}, the most `deploy --dev` reads; pack them with ba2",
            entries.len()
        )));
    }
    let mut seen = BTreeSet::new();
    for e in &entries {
        if !seen.insert(e.rel.to_ascii_lowercase()) {
            return Err(fail(format!("two inputs go to `{}`", e.rel)));
        }
    }

    // Validate first; a failing plugin is never packaged.
    let mut vargs = json!({ "plugin": plugin.display().to_string() });
    for k in ["profile", "plugins", "data_dir", "no_cache"] {
        if let (Some(v), Some(obj)) = (args.get(k), vargs.as_object_mut()) {
            obj.insert(k.to_string(), v.clone());
        }
    }
    if !script_files.is_empty() {
        if let Some(obj) = vargs.as_object_mut() {
            obj.insert("scripts".into(), json!(script_files));
        }
    }
    let checked = super::validate::run(session, &vargs)?;
    let v = &checked.data;
    let passed = v.get("passed").and_then(Value::as_bool) == Some(true);
    if !passed {
        let errors: Vec<String> = v
            .get("findings")
            .and_then(Value::as_array)
            .map(|fs| {
                fs.iter()
                    .filter(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
                    .take(5)
                    .map(|f| {
                        format!(
                            "{}: {}",
                            f.get("rule").and_then(Value::as_str).unwrap_or("?"),
                            f.get("message").and_then(Value::as_str).unwrap_or("")
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        return Err(fail(format!(
            "`{plugin_name}` fails validate ({} error(s)); nothing was written. {}",
            v.get("errors").and_then(Value::as_u64).unwrap_or(0),
            errors.join("; ")
        )));
    }
    let (masters, record_counts) = plugin_facts(&plugin).map_err(fail)?;
    let records: u64 = record_counts.values().sum();
    let spec = match opt_str(args, "spec") {
        Some(s) => {
            let p = PathBuf::from(s);
            let (sha, size) = hash_file(&p).map_err(fail)?;
            Some(json!({"file": file_name(&p), "size": size, "sha256": sha}))
        }
        None => None,
    };

    // Build the folder under a temporary name in `out`.
    fs::create_dir_all(&out).map_err(|e| fail(format!("creating `{}`: {e}", out.display())))?;
    let tmp = out.join(format!(".{name}.partial-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let built = build_folder(&tmp, &entries, use_ba2, &stem, ba2_version, ba2_compress);
    let archives = match built {
        Ok(a) => a,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            return Err(fail(e));
        }
    };
    let finished = finish_folder(
        &tmp,
        &mod_dir,
        Manifest {
            name: &name,
            version,
            plugin: &plugin_name,
            masters: &masters,
            record_counts: &record_counts,
            records,
            spec: spec.clone(),
            validate: json!({
                "passed": true,
                "errors": v.get("errors").cloned().unwrap_or(Value::Null),
                "warnings": v.get("warnings").cloned().unwrap_or(Value::Null),
                "rules": v.get("rules").cloned().unwrap_or(Value::Null),
            }),
            archives: &archives,
        },
    );
    let (manifest, files) = match finished {
        Ok(x) => x,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            return Err(fail(e));
        }
    };

    // The zip, from the finished folder.
    let zip_written = if want_zip {
        let partial = out.join(format!("{zip_name}.partial-{}", std::process::id()));
        let mut names: Vec<String> = files.iter().map(|(rel, _, _)| rel.clone()).collect();
        names.push(MANIFEST.to_string());
        names.sort();
        // A file rename replaces an existing zip in one step (MoveFileEx with
        // REPLACE_EXISTING on Windows, rename(2) elsewhere): never a moment without one.
        let result = write_zip(&mod_dir, &names, &partial).and_then(|()| {
            fs::rename(&partial, &zip_path)
                .map_err(|e| format!("renaming to `{}`: {e}", zip_path.display()))
        });
        if let Err(e) = result {
            let _ = fs::remove_file(&partial);
            return Err(fail(e));
        }
        Some(hash_file(&zip_path).map_err(fail)?)
    } else {
        None
    };

    let bytes: u64 = files.iter().map(|(_, size, _)| *size).sum();
    let mut limitations: Vec<String> = Vec::new();
    if !archives.is_empty() {
        limitations.push(
            "archive names follow docs/corpus.md \"Archive names\" (`<plugin stem> - Main.ba2`, \
             `- Textures.ba2`): grade C until the M4.O2 in-game session sees the game load them"
                .into(),
        );
    }
    limitations.push(
        "package does not install: add the folder (or the zip) to MO2 yourself; it never \
         writes into MO2, the game's Data folder or Documents\\My Games\\Starfield"
            .into(),
    );
    limitations.extend(checked.limitations.iter().map(|l| format!("validate: {l}")));
    let summary = format!(
        "{name}: {} file(s), {bytes} bytes in `{}`{}; validate passed ({} warning(s))",
        files.len().saturating_add(1),
        mod_dir.display(),
        zip_written
            .as_ref()
            .map_or_else(String::new, |(_, size)| format!(
                ", zip `{}` ({size} bytes)",
                zip_path.display()
            )),
        v.get("warnings").and_then(Value::as_u64).unwrap_or(0)
    );
    let plugin_len = fs::metadata(&plugin).map(|m| m.len()).unwrap_or(0);
    Ok(Outcome {
        data: json!({
            "plugin": plugin.display().to_string(),
            "name": name,
            "version": version,
            "folder": mod_dir.display().to_string(),
            "zip": zip_written.as_ref().map(|(sha, size)| json!({
                "path": zip_path.display().to_string(), "size": size, "sha256": sha,
            })),
            "manifest": manifest,
            "files": files.len().saturating_add(1),
            "bytes": bytes,
            "summary": summary,
        }),
        coverage: Coverage {
            status: CoverageStatus::Complete,
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence: vec![Evidence {
            file: plugin_name,
            offset: 0,
            length: plugin_len,
            record: None,
            subrecord: None,
        }],
    })
}

/// Copies or packs `entries` into the new folder `tmp`; returns the archives' manifest
/// entries.
fn build_folder(
    tmp: &Path,
    entries: &[Entry],
    use_ba2: bool,
    stem: &str,
    version: u32,
    compress: bool,
) -> Result<Vec<Value>, String> {
    fs::create_dir_all(tmp).map_err(|e| format!("creating `{}`: {e}", tmp.display()))?;
    let (plugin, rest) = entries.split_first().ok_or("nothing to package")?;
    copy_into(&plugin.src, &under(tmp, &plugin.rel))?;
    if !use_ba2 {
        for e in rest {
            copy_into(&e.src, &under(tmp, &e.rel))?;
        }
        return Ok(Vec::new());
    }
    let is_dds = |e: &&Entry| e.rel.to_ascii_lowercase().ends_with(".dds");
    let textures: Vec<Entry> = rest.iter().filter(is_dds).cloned().collect();
    let general: Vec<Entry> = rest.iter().filter(|e| !is_dds(e)).cloned().collect();
    let mut archives = Vec::new();
    if !general.is_empty() {
        let path = tmp.join(format!("{stem} - Main.ba2"));
        archives.push(pack(&general, &path, Kind::General, version, compress)?);
    }
    if !textures.is_empty() {
        let path = tmp.join(format!("{stem} - Textures.ba2"));
        archives.push(pack(&textures, &path, Kind::Textures, version, compress)?);
    }
    Ok(archives)
}

/// (Data path, size, SHA-256) of one file of the folder.
type FileRow = (String, u64, String);

/// What the manifest records besides the file list.
struct Manifest<'a> {
    name: &'a str,
    version: Option<&'a str>,
    plugin: &'a str,
    masters: &'a [String],
    record_counts: &'a BTreeMap<String, u64>,
    records: u64,
    spec: Option<Value>,
    validate: Value,
    archives: &'a [Value],
}

/// Hashes the folder, writes the manifest, and moves `tmp` to `mod_dir` (replacing a
/// folder package made). Returns the manifest and `(path, size, sha256)` of every other
/// file, sorted by path.
fn finish_folder(
    tmp: &Path,
    mod_dir: &Path,
    m: Manifest<'_>,
) -> Result<(Value, Vec<FileRow>), String> {
    let mut found = Vec::new();
    walk(tmp, tmp, 0, &mut found)?;
    let mut files: Vec<FileRow> = Vec::new();
    for (parts, path) in found {
        let (sha, size) = hash_file(&path)?;
        files.push((parts.join("/"), size, sha));
    }
    files.sort();
    let plugin_sha = files
        .iter()
        .find(|(rel, _, _)| rel == m.plugin)
        .map(|(_, _, sha)| sha.clone());
    let manifest = json!({
        "format": MANIFEST_FORMAT,
        "kitcraft_version": env!("CARGO_PKG_VERSION"),
        "name": m.name,
        "version": m.version,
        "plugins": [{
            "file": m.plugin,
            "sha256": plugin_sha,
            "masters": m.masters,
            "records": m.records,
            "record_types": m.record_counts,
        }],
        "spec": m.spec,
        "validate": m.validate,
        "archives": m.archives,
        "files": files.iter().map(|(rel, size, sha)| json!({"path": rel, "size": size, "sha256": sha})).collect::<Vec<_>>(),
    });
    let mut text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    text.push('\n');
    fs::write(tmp.join(MANIFEST), text).map_err(|e| format!("writing the manifest: {e}"))?;
    // `overwrite` replaces the old folder by renames only: old aside, new in, then the old
    // one removed. A failure before the new folder is in puts the old one back, so the
    // folder is never half deleted (removing it first could stop midway on a locked file).
    let aside = if mod_dir.exists() {
        if !mod_dir.join(MANIFEST).is_file() {
            return Err(format!(
                "`{}` appeared and was not made by package",
                mod_dir.display()
            ));
        }
        let name = mod_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let aside = mod_dir.with_file_name(format!(".{name}.old-{}", std::process::id()));
        fs::rename(mod_dir, &aside).map_err(|e| {
            format!(
                "replacing `{}` (moving it aside; nothing was changed): {e}",
                mod_dir.display()
            )
        })?;
        Some(aside)
    } else {
        None
    };
    if let Err(e) = fs::rename(tmp, mod_dir) {
        let restored = aside.as_ref().map_or(Ok(()), |a| {
            fs::rename(a, mod_dir).map_err(|r| r.to_string())
        });
        return Err(match (aside, restored) {
            (Some(a), Err(r)) => format!(
                "renaming to `{}`: {e}; putting the old folder back failed too ({r}): it is at `{}`",
                mod_dir.display(),
                a.display()
            ),
            _ => format!("renaming to `{}`: {e}", mod_dir.display()),
        });
    }
    if let Some(a) = aside {
        // The new folder is in place; a leftover old copy stays a hidden `.<name>.old-<pid>`.
        let _ = fs::remove_dir_all(&a);
    }
    Ok((manifest, files))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
mod tests;
