//! `papyrus.compile` (M2.G7): runs Bethesda's `PapyrusCompiler.exe` as a black box (ADR
//! 0006) on one script, by name, with import directories.
//!
//! The argument builder ([`compiler_args`]) emits only the object name, `-f=`, `-i=`,
//! `-o=`, `-ignorecwd` and optionally `-op`. It has no way to ask for release or final mode
//! (`-release`, `-r`, `-final`), checks its own output for them, and the catalog schema
//! rejects any extra argument. Project files (`.ppj`) are refused because they can turn
//! release mode on themselves. Output is never written under the game or user Data
//! directory or an MO2 instance (the shared output guard, `edit::patch::guard_location`), and
//! the produced `.pex` is parsed and disassembled with `kitkraft-pex` as a check.
//!
//! Paths follow the official compiler's command line (papyrus oracle, `docs/oracles.md`):
//! every import entry may hold several folders separated by `;` (its `-i=` form), and every
//! path is made absolute against the caller's working directory before the call, because
//! the compiler runs in `out_dir` and would resolve a relative one there. The compiler
//! treats its working directory as the first import unless told `-ignorecwd`; a stale
//! `.psc` in `out_dir` then shadowed the requested source (observed 2026-10-15), so
//! `-ignorecwd` is always passed and only the listed imports are searched. A `.psc` path
//! compiles as the name its `ScriptName` statement declares, with the folder that holds
//! its namespace folders imported first (`KitKraft\HelloOnLoad.psc` declaring
//! `KitKraft:HelloOnLoad` imports the folder above `KitKraft`).
//!
//! `sources_zip` (M3.G1) names `Tools\ContentResources.zip`: its `Scripts/Source` files are
//! extracted once into the KitKraft cache ([`super::sources`]) and that folder is imported
//! after every other import directory, so the base game's scripts need no manual
//! extraction.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use kitkraft_pex::{disassemble, Pex};
use serde_json::{json, Value};

use crate::envelope::{Coverage, CoverageStatus, Evidence, Outcome};
use crate::exec::EngineError;
use crate::handlers::ba2::refuse_game_data;
use crate::handlers::{failed, file_name, str_arg};

const ID: &str = "papyrus.compile";
/// The flags file the Starfield sources use (`Tools\ContentResources.zip`,
/// `Scripts/Source/Starfield_Papyrus_Flags.flg`).
pub(crate) const FLAGS_FILE: &str = "Starfield_Papyrus_Flags.flg";
/// The compiler below the game folder.
const COMPILER_REL: [&str; 3] = ["Tools", "Papyrus Compiler", "PapyrusCompiler.exe"];
/// Default Steam install.
pub(crate) const STEAM_GAME: &str = r"C:\Program Files (x86)\Steam\steamapps\common\Starfield";
/// The compiler is killed after this long.
const TIMEOUT: Duration = Duration::from_secs(300);

/// What to compile; built from the command arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    /// Script name (`MyScript`, `MyNamespace:MyScript`).
    pub object: String,
    /// Import directories, in order.
    pub imports: Vec<PathBuf>,
    /// Flags file, if any.
    pub flags: Option<PathBuf>,
    /// Output directory.
    pub out_dir: PathBuf,
    /// `-op`.
    pub optimize: bool,
}

/// The compiler's arguments for `req`. Never contains a release or final switch.
pub(crate) fn compiler_args(req: &Request) -> Result<Vec<String>, String> {
    if !valid_script_name(&req.object) {
        return Err(format!(
            "`{}` is not a script name (letters, digits, `_`, `:` namespaces)",
            req.object
        ));
    }
    let mut args = vec![req.object.clone()];
    if let Some(flags) = &req.flags {
        args.push(format!("-f={}", plain(flags)?));
    }
    if !req.imports.is_empty() {
        let dirs: Vec<String> = req
            .imports
            .iter()
            .map(|d| plain(d))
            .collect::<Result<_, _>>()?;
        if let Some(bad) = dirs.iter().find(|d| d.contains(';')) {
            return Err(format!(
                "import directory `{bad}` contains `;`, the separator"
            ));
        }
        args.push(format!("-i={}", dirs.join(";")));
    }
    args.push(format!("-o={}", plain(&req.out_dir)?));
    args.push("-ignorecwd".to_string());
    if req.optimize {
        args.push("-op".to_string());
    }
    refuse_release(&args)?;
    Ok(args)
}

/// Fails if any argument asks for release or final mode. The builder cannot produce one;
/// this keeps it that way.
pub(crate) fn refuse_release(args: &[String]) -> Result<(), String> {
    for arg in args {
        let lower = arg.to_ascii_lowercase();
        let switch = lower
            .trim_start_matches(['-', '/'])
            .split(['=', ':'])
            .next()
            .unwrap_or_default();
        let is_switch = lower.starts_with('-') || lower.starts_with('/');
        if is_switch && matches!(switch, "r" | "release" | "final") {
            return Err(format!(
                "refusing compiler argument `{arg}`: KitKraft never compiles in release or final mode"
            ));
        }
    }
    Ok(())
}

fn valid_script_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(':')
        && !name.ends_with(':')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
}

/// A path as text, refused if it could be read as a switch or breaks quoting.
fn plain(path: &Path) -> Result<String, String> {
    let text = path.display().to_string();
    if text.starts_with('-')
        || (text.starts_with('/') && !text.starts_with("//"))
        || text.contains('"')
    {
        return Err(format!("path `{text}` cannot be passed to the compiler"));
    }
    Ok(text)
}

/// Where the compiler is looked for, in order.
fn compiler_candidates(explicit: Option<&str>) -> Vec<PathBuf> {
    if let Some(p) = explicit {
        return vec![PathBuf::from(p)];
    }
    let mut out = Vec::new();
    if let Some(p) = crate::handlers::env_var_os("KITKRAFT_PAPYRUS_COMPILER") {
        out.push(PathBuf::from(p));
    }
    if let Some(data) = crate::handlers::default_data_dir() {
        if let Some(game) = Path::new(&data).parent() {
            out.push(
                COMPILER_REL
                    .iter()
                    .fold(game.to_path_buf(), |p, c| p.join(c)),
            );
        }
    }
    out.push(
        COMPILER_REL
            .iter()
            .fold(PathBuf::from(STEAM_GAME), |p, c| p.join(c)),
    );
    out
}

/// `path` made absolute against the working directory (no symlink resolution), without a
/// verbatim prefix.
fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path)
        .map(|p| strip_verbatim(&p))
        .map_err(|e| format!("`{}`: {e}", path.display()))
}

/// The import entries as folders: each entry may list several separated by `;` (the
/// compiler's `-i=` form); empty pieces are dropped.
fn split_imports(entries: &[String]) -> Vec<PathBuf> {
    entries
        .iter()
        .flat_map(|e| e.split(';'))
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// What a `.psc` path compiles as: the name its `ScriptName` statement declares, and the
/// import root that holds it (for `A:B:Name`, the folder above `A\B\`). The file's stem
/// and its folders must spell the name (ignoring case), as the compiler requires. A source
/// the declaration reader cannot read falls back to the file stem and its own folder, and
/// the compiler reports the problem.
pub(crate) fn psc_object(path: &Path) -> Result<(String, PathBuf), String> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let cap = u64::try_from(kitkraft_pex::psc::MAX_SOURCE_BYTES).unwrap_or(u64::MAX);
    let Ok(bytes) = super::read_capped(path, cap) else {
        return Ok((stem, dir));
    };
    let Ok(script) = kitkraft_pex::psc::parse(&bytes) else {
        return Ok((stem, dir));
    };
    let mut parts: Vec<&str> = script.name.split(':').collect();
    let Some(last) = parts.pop() else {
        return Ok((stem, dir));
    };
    if !last.eq_ignore_ascii_case(&stem) {
        return Err(format!(
            "`{}` declares `ScriptName {}`, but the file is named `{stem}`; the compiler \
             needs the file name to match the script name",
            path.display(),
            script.name
        ));
    }
    let mut root = dir;
    for ns in parts.iter().rev() {
        let folder = root
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !folder.eq_ignore_ascii_case(ns) {
            return Err(format!(
                "`{}` declares `ScriptName {}`, so it must sit in a `{}` folder below an \
                 import root (it is in `{folder}`); move it, or pass the name and import \
                 the root",
                path.display(),
                script.name,
                parts.join("\\")
            ));
        }
        root = root
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    }
    Ok((script.name, root))
}

/// `papyrus.compile`.
pub(crate) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let source = str_arg(ID, args, "source")?;
    let out_dir = absolute(Path::new(str_arg(ID, args, "out_dir")?)).map_err(|e| failed(ID, e))?;
    let optimize = args
        .get("optimize")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let entries: Vec<String> = args
        .get("imports")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let mut imports = split_imports(&entries)
        .iter()
        .map(|d| absolute(d))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| failed(ID, e))?;

    let lower = source.to_ascii_lowercase();
    if lower.ends_with(".ppj") {
        return Err(failed(
            ID,
            "project files (.ppj) are refused: they can switch on release mode",
        ));
    }
    let object = if lower.ends_with(".psc") || source.contains(['/', '\\']) {
        let path = Path::new(source);
        if !path.is_file() {
            return Err(failed(ID, format!("`{source}` does not exist")));
        }
        let path = absolute(path).map_err(|e| failed(ID, e))?;
        let (name, root) = psc_object(&path).map_err(|e| failed(ID, e))?;
        imports.insert(0, root);
        name
    } else {
        source.to_string()
    };
    let zip = match args.get("sources_zip").and_then(Value::as_str) {
        Some(z) => {
            let zip_path = PathBuf::from(z);
            if !zip_path.is_file() {
                return Err(failed(ID, format!("sources zip `{z}` does not exist")));
            }
            let extracted =
                super::sources::extract_sources(&zip_path, &super::sources::default_cache_root())
                    .map_err(|e| failed(ID, e))?;
            imports.push(absolute(&extracted.dir).map_err(|e| failed(ID, e))?);
            Some((zip_path, extracted))
        }
        None => None,
    };
    for dir in &imports {
        if !dir.is_dir() {
            return Err(failed(
                ID,
                format!("import directory `{}` does not exist", dir.display()),
            ));
        }
    }
    let explicit_flags = match args.get("flags").and_then(Value::as_str) {
        Some(f) => {
            let p = absolute(Path::new(f)).map_err(|e| failed(ID, e))?;
            if !p.is_file() {
                return Err(failed(ID, format!("flags file `{f}` does not exist")));
            }
            Some(p)
        }
        None => None,
    };

    refuse_game_data(ID, &out_dir)?;
    // Nothing writes under an MO2 instance or profile but `deploy --dev` (AGENTS.md): the
    // shared output guard every writing command uses (Fable review of lane B, finding 3).
    let pex_target = object
        .split(':')
        .fold(out_dir.clone(), |p, part| p.join(part))
        .with_extension("pex");
    crate::edit::patch::guard_location(&pex_target).map_err(|e| failed(ID, e.to_string()))?;
    fs::create_dir_all(&out_dir)
        .map_err(|e| failed(ID, format!("creating `{}`: {e}", out_dir.display())))?;
    let out_dir = strip_verbatim(&fs::canonicalize(&out_dir).unwrap_or(out_dir));

    let candidates = compiler_candidates(args.get("compiler").and_then(Value::as_str));
    let compiler = candidates.iter().find(|p| p.is_file()).ok_or_else(|| {
        let looked: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
        failed(
            ID,
            format!(
                "PapyrusCompiler.exe not found; looked at: {} (pass `compiler`, or set \
                 KITKRAFT_PAPYRUS_COMPILER)",
                looked.join("; ")
            ),
        )
    })?;
    let compiler = absolute(compiler).map_err(|e| failed(ID, e))?;
    let (flags, flags_note) = choose_flags(
        explicit_flags,
        zip.as_ref().map(|(_, x)| x.dir.as_path()),
        &imports,
    );

    let request = Request {
        object,
        imports,
        flags,
        out_dir: out_dir.clone(),
        optimize,
    };
    let compiler_args = compiler_args(&request).map_err(|e| failed(ID, e))?;
    let (exit_code, stdout, stderr) =
        run_compiler(&compiler, &compiler_args, &out_dir).map_err(|e| failed(ID, e))?;
    let passed = exit_code == Some(0);

    let pex_path = request
        .object
        .split(':')
        .fold(out_dir.clone(), |p, part| p.join(part))
        .with_extension("pex");
    let mut limitations = flags_note;
    let mut evidence = Vec::new();
    let sources_zip = match &zip {
        Some((zip_path, x)) => {
            let length = fs::metadata(zip_path).map_or(0, |m| m.len());
            evidence.push(Evidence {
                file: file_name(zip_path),
                offset: 0,
                length,
                record: None,
                subrecord: None,
            });
            json!({
                "zip": zip_path.display().to_string(),
                "dir": x.dir.display().to_string(),
                "files": x.files,
                "bytes": x.bytes,
                "skipped": x.skipped,
                "cached": x.cached,
            })
        }
        None => Value::Null,
    };
    let pex = if passed && pex_path.is_file() {
        let bytes = fs::read(&pex_path)
            .map_err(|e| failed(ID, format!("reading `{}`: {e}", pex_path.display())))?;
        evidence.push(Evidence {
            file: file_name(&pex_path),
            offset: 0,
            length: bytes.len() as u64,
            record: None,
            subrecord: None,
        });
        match Pex::parse(&bytes) {
            Ok(p) => json!({
                "path": pex_path.display().to_string(),
                "bytes": bytes.len(),
                "parsed": true,
                "text": disassemble(&p),
            }),
            Err(e) => {
                limitations.push(format!("the compiled .pex does not parse: {e}"));
                json!({"path": pex_path.display().to_string(), "bytes": bytes.len(),
                       "parsed": false, "error": e.to_string()})
            }
        }
    } else {
        if passed {
            limitations.push(format!(
                "the compiler reported success but `{}` is missing",
                pex_path.display()
            ));
        }
        Value::Null
    };
    let summary = if passed {
        format!("compiled {} -> {}", request.object, pex_path.display())
    } else {
        format!(
            "{} did not compile (exit code {}); see stdout/stderr",
            request.object,
            exit_code.map_or_else(|| "none".to_string(), |c| c.to_string())
        )
    };
    Ok(Outcome {
        data: json!({
            "source": source,
            "script": request.object,
            "out_dir": out_dir.display().to_string(),
            "compiler": compiler.display().to_string(),
            "sources_zip": sources_zip,
            "args": compiler_args,
            "exit_code": exit_code,
            "passed": passed,
            "stdout": stdout,
            "stderr": stderr,
            "pex": pex,
            "summary": summary,
        }),
        coverage: Coverage {
            status: if passed {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Unknown
            },
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: Vec::new(),
        },
        limitations,
        evidence,
    })
}

/// `Tools\ContentResources.zip` as `kit doctor` finds it: next to the game Data folder
/// (`KITKRAFT_DATA_DIR`, else detected), else in the Steam install.
fn detected_sources_zip() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(data) = crate::handlers::default_data_dir() {
        if let Some(game) = Path::new(&data).parent() {
            candidates.push(game.join("Tools").join("ContentResources.zip"));
        }
    }
    candidates.push(
        PathBuf::from(STEAM_GAME)
            .join("Tools")
            .join("ContentResources.zip"),
    );
    candidates.into_iter().find(|p| p.is_file())
}

/// The flags file to compile with, and the limitation lines that name it (path, where it
/// came from, SHA-256). Order: the `flags` argument; the official one of
/// `Tools\ContentResources.zip` (`sources_zip`'s extraction, else the zip `kit doctor`
/// detects, its flags file alone cached); only then the first import directory that holds
/// one. A flags file beside a user's scripts never replaces the official one silently
/// (Fable review of lane B, finding 2).
fn choose_flags(
    explicit: Option<PathBuf>,
    extracted: Option<&Path>,
    imports: &[PathBuf],
) -> (Option<PathBuf>, Vec<String>) {
    let in_imports: Vec<PathBuf> = imports
        .iter()
        .map(|d| d.join(FLAGS_FILE))
        .filter(|p| p.is_file())
        .collect();
    let official = || -> Option<(PathBuf, String)> {
        if let Some(dir) = extracted {
            let p = dir.join(FLAGS_FILE);
            return p
                .is_file()
                .then(|| (p, "the sources_zip extraction".to_string()));
        }
        let zip = detected_sources_zip()?;
        let cache = super::sources::default_cache_root();
        let p = super::sources::cached_file(&zip, &cache, FLAGS_FILE).ok()??;
        Some((p, format!("`{}` (detected; cached)", zip.display())))
    };
    let chosen = match explicit {
        Some(p) => Some((p, "the `flags` argument".to_string())),
        None => official().or_else(|| {
            in_imports.first().map(|p| {
                (
                    p.clone(),
                    "the first import directory that holds one (no official \
                     Tools\\ContentResources.zip was found)"
                        .to_string(),
                )
            })
        }),
    };
    let mut notes = Vec::new();
    let Some((path, origin)) = chosen else {
        notes.push(format!(
            "no flags file: none given, no Tools\\ContentResources.zip found, and {FLAGS_FILE} is not in any import directory"
        ));
        return (None, notes);
    };
    let sha =
        crate::deploy::sha256::file(&path).map_or_else(|e| format!("unreadable ({e})"), |(_, h)| h);
    notes.push(format!(
        "flags file: `{}` from {origin}, SHA-256 {sha}",
        path.display()
    ));
    for other in in_imports.iter().filter(|p| **p != path) {
        let other_sha = crate::deploy::sha256::file(other)
            .map_or_else(|e| format!("unreadable ({e})"), |(_, h)| h);
        if other_sha != sha {
            notes.push(format!(
                "a different {FLAGS_FILE} in an import directory was not used: `{}`, SHA-256 {other_sha}",
                other.display()
            ));
        }
    }
    (Some(path), notes)
}

/// Runs the compiler with no stdin, collecting its output; kills it after [`TIMEOUT`].
fn run_compiler(
    compiler: &Path,
    args: &[String],
    cwd: &Path,
) -> Result<(Option<i32>, String, String), String> {
    refuse_release(args)?;
    let mut child = Command::new(compiler)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("starting `{}`: {e}", compiler.display()))?;
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut text);
            }
            String::from_utf8_lossy(&text).into_owned()
        })
    };
    let out = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let err = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "the compiler did not finish within {} s and was stopped",
                    TIMEOUT.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("waiting for the compiler: {e}")),
        }
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    Ok((status.code(), stdout, stderr))
}

/// `\\?\C:\x` -> `C:\x` (the compiler does not take verbatim paths).
fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::{compiler_args, refuse_release, Request, FLAGS_FILE};
    use crate::execute;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn request() -> Request {
        Request {
            object: "My:Script_1".into(),
            imports: vec![PathBuf::from(r"C:\src a"), PathBuf::from(r"C:\base")],
            flags: Some(PathBuf::from(r"C:\base\Starfield_Papyrus_Flags.flg")),
            out_dir: PathBuf::from(r"C:\out"),
            optimize: true,
        }
    }

    #[test]
    fn argument_builder() {
        assert_eq!(
            compiler_args(&request()).unwrap(),
            [
                "My:Script_1",
                r"-f=C:\base\Starfield_Papyrus_Flags.flg",
                r"-i=C:\src a;C:\base",
                r"-o=C:\out",
                "-ignorecwd",
                "-op"
            ]
        );
        let minimal = Request {
            object: "S".into(),
            imports: Vec::new(),
            flags: None,
            out_dir: PathBuf::from("out"),
            optimize: false,
        };
        assert_eq!(
            compiler_args(&minimal).unwrap(),
            ["S", "-o=out", "-ignorecwd"]
        );
        for bad in ["-release", "-r", "a b", "", "x.psc", ":x", "x;y"] {
            let r = Request {
                object: bad.into(),
                ..request()
            };
            assert!(compiler_args(&r).is_err(), "{bad}");
        }
        let semi = Request {
            imports: vec![PathBuf::from("a;b")],
            ..request()
        };
        assert!(compiler_args(&semi).is_err());
        let switch = Request {
            out_dir: PathBuf::from("-release"),
            ..request()
        };
        assert!(compiler_args(&switch).is_err());
    }

    /// Import entries take the compiler's `;` lists; relative paths become absolute against
    /// the working directory, not the compiler's (`out_dir`). Regression for the lane E
    /// request (M4.E9): `--imports docs\recipes\scripts` failed with "Cannot use import
    /// folder ...\out\recipes\Scripts\docs\recipes\scripts".
    #[test]
    fn imports_split_and_become_absolute() {
        let entries = vec![r"a;b ; ;C:\c".to_string(), "d".to_string()];
        assert_eq!(
            super::split_imports(&entries),
            ["a", "b", r"C:\c", "d"].map(PathBuf::from)
        );
        let abs = super::absolute(std::path::Path::new(r"docs\recipes\scripts")).unwrap();
        assert!(abs.is_absolute(), "{}", abs.display());
        assert!(abs.starts_with(std::env::current_dir().unwrap()));
        assert!(!abs.display().to_string().starts_with(r"\\?\"));
    }

    /// A `.psc` path compiles as its declared (namespaced) name with the folder above its
    /// namespace folders imported. Regression for the lane E request (M4.E9): the path of
    /// `KitKraft\HelloOnLoad.psc` compiled as `HelloOnLoad` and the compiler refused it.
    #[test]
    fn psc_path_names_its_namespace_root() {
        let dir = std::env::temp_dir().join(format!("kitkraft-psc-ns-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let ns = dir.join("root").join("kitkraft").join("Sub");
        fs::create_dir_all(&ns).unwrap();
        let psc = ns.join("Hello.psc");
        fs::write(
            &psc,
            "ScriptName KitKraft:Sub:Hello extends ObjectReference\n",
        )
        .unwrap();
        let (name, root) = super::psc_object(&psc).unwrap();
        assert_eq!(name, "KitKraft:Sub:Hello");
        assert_eq!(root, dir.join("root"));

        // A plain script: its own folder.
        let plain = dir.join("Plain.psc");
        fs::write(&plain, "Scriptname plain extends ScriptObject\n").unwrap();
        assert_eq!(
            super::psc_object(&plain).unwrap(),
            ("plain".to_string(), dir.clone())
        );
        // The folders must spell the namespace, the file the name.
        let wrong = dir.join("Other.psc");
        fs::write(&wrong, "ScriptName KitKraft:Other\n").unwrap();
        let err = super::psc_object(&wrong).unwrap_err();
        assert!(err.contains("`KitKraft` folder"), "{err}");
        let renamed = dir.join("Renamed.psc");
        fs::write(&renamed, "ScriptName Original\n").unwrap();
        assert!(super::psc_object(&renamed)
            .unwrap_err()
            .contains("named `Renamed`"));
        // Unreadable declarations fall back to the file name; the compiler reports.
        let broken = dir.join("Broken.psc");
        fs::write(&broken, "; no script name here\n").unwrap();
        assert_eq!(
            super::psc_object(&broken).unwrap(),
            ("Broken".to_string(), dir.clone())
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn release_and_final_are_refused_in_any_spelling() {
        for arg in [
            "-release",
            "-R",
            "/release",
            "-Final",
            "-r=1",
            "-release:true",
        ] {
            assert!(refuse_release(&["S".into(), arg.into()]).is_err(), "{arg}");
        }
        refuse_release(&["S".into(), "-o=C:\\release".into(), "-op".into()]).unwrap();
        // Every argument the builder can produce passes.
        refuse_release(&compiler_args(&request()).unwrap()).unwrap();
    }

    #[test]
    fn execute_refuses_release_and_reports_missing_compiler() {
        let dir = std::env::temp_dir().join(format!("kitkraft-compile-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let err = execute(
            "papyrus.compile",
            json!({"source": "S", "out_dir": dir.to_str().unwrap(), "release": true}),
        )
        .unwrap_err();
        assert_eq!(err.kind(), "invalid_args");
        let missing = dir.join("nope").join("PapyrusCompiler.exe");
        let err = execute(
            "papyrus.compile",
            json!({"source": "S", "out_dir": dir.join("o").to_str().unwrap(),
                   "compiler": missing.to_str().unwrap()}),
        )
        .unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("nope"), "{err}");
        let err = execute(
            "papyrus.compile",
            json!({"source": "x.ppj", "out_dir": dir.to_str().unwrap()}),
        )
        .unwrap_err();
        assert!(err.to_string().contains(".ppj"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A bad `sources_zip` fails before the compiler is looked for, and a hostile entry
    /// name is refused before anything is extracted (M3.G1).
    #[test]
    fn sources_zip_errors_come_before_the_compiler() {
        let dir = std::env::temp_dir().join(format!("kitkraft-compile-zip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let missing_compiler = dir.join("nope").join("PapyrusCompiler.exe");
        let run = |zip: &std::path::Path| {
            execute(
                "papyrus.compile",
                json!({"source": "S", "out_dir": dir.join("o").to_str().unwrap(),
                       "sources_zip": zip.to_str().unwrap(),
                       "compiler": missing_compiler.to_str().unwrap()}),
            )
            .unwrap_err()
        };
        let err = run(&dir.join("absent.zip"));
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("absent.zip"), "{err}");

        let zip = dir.join("hostile.zip");
        fs::write(
            &zip,
            super::super::sources::tests::zip_bytes(&[("Scripts/Source/../x.psc", b"x")]),
        )
        .unwrap();
        let err = run(&zip);
        assert!(err.to_string().contains("refusing"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A flags file beside a user's scripts (an import directory; the `.psc`'s namespace
    /// root is imported first) never replaces the official one: the `ContentResources`
    /// extraction's wins, and the limitations name the file used and the decoy, each with
    /// its SHA-256 (Fable review of lane B, finding 2). The `flags` argument still wins.
    #[test]
    fn official_flags_win_over_a_decoy_in_an_import() {
        use crate::deploy::sha256;
        let dir = std::env::temp_dir().join(format!("kitkraft-flags-decoy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let official = dir.join("extracted");
        let user = dir.join("user");
        fs::create_dir_all(&official).unwrap();
        fs::create_dir_all(&user).unwrap();
        fs::write(official.join(FLAGS_FILE), b"official flags").unwrap();
        fs::write(user.join(FLAGS_FILE), b"decoy flags").unwrap();
        let imports = [user.clone(), official.clone()];

        let (flags, notes) = super::choose_flags(None, Some(&official), &imports);
        assert_eq!(flags, Some(official.join(FLAGS_FILE)));
        assert!(
            notes[0].starts_with("flags file:")
                && notes[0].contains("sources_zip")
                && notes[0].contains(&sha256::hex(b"official flags")),
            "{notes:?}"
        );
        assert!(
            notes.iter().any(|n| n.contains("was not used")
                && n.contains(&user.join(FLAGS_FILE).display().to_string())
                && n.contains(&sha256::hex(b"decoy flags"))),
            "{notes:?}"
        );

        let (flags, notes) =
            super::choose_flags(Some(user.join(FLAGS_FILE)), Some(&official), &imports);
        assert_eq!(flags, Some(user.join(FLAGS_FILE)));
        assert!(notes[0].contains("`flags` argument"), "{notes:?}");
        assert!(notes[0].contains(&sha256::hex(b"decoy flags")), "{notes:?}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// `out_dir` under an MO2 instance is refused before anything is created, by the
    /// output guard every writing command uses (AGENTS.md: nothing writes under MO2 but
    /// `deploy --dev`; Fable review of lane B, finding 3).
    #[test]
    fn out_dir_under_an_mo2_instance_is_refused() {
        let dir = std::env::temp_dir().join(format!("kitkraft-compile-mo2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mo2 = dir.join("MO2-Starfield");
        fs::create_dir_all(&mo2).unwrap();
        fs::write(mo2.join("ModOrganizer.ini"), "[General]\n").unwrap();
        let out = mo2.join("mods").join("Mine").join("Scripts");
        let err = execute(
            "papyrus.compile",
            json!({"source": "KitKraft:S", "out_dir": out.to_str().unwrap(),
                   "compiler": dir.join("nope.exe").to_str().unwrap()}),
        )
        .unwrap_err();
        assert_eq!(err.kind(), "failed");
        assert!(err.to_string().contains("Mod Organizer"), "{err}");
        assert!(!mo2.join("mods").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// With the compiler installed and `KITKRAFT_DATA_DIR` set, a base-game script compiles
    /// by name given only `Tools\ContentResources.zip` (M3.G1).
    #[test]
    #[cfg_attr(not(windows), ignore = "Windows-only: PapyrusCompiler.exe")]
    fn base_game_script_compiles_from_the_zip_alone() {
        let Some(data) = crate::handlers::env_var_os("KITKRAFT_DATA_DIR") else {
            println!("skipped: KITKRAFT_DATA_DIR is not set");
            return;
        };
        let Some(zip) = std::path::Path::new(&data)
            .parent()
            .map(|g| g.join("Tools").join("ContentResources.zip"))
            .filter(|z| z.is_file())
        else {
            println!("skipped: Tools\\ContentResources.zip not found");
            return;
        };
        if super::compiler_candidates(None)
            .iter()
            .all(|p| !p.is_file())
        {
            println!("skipped: PapyrusCompiler.exe not found");
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("kitkraft-compile-zip-ok-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let r = execute(
            "papyrus.compile",
            json!({"source": "DefaultCounterQuestA", "out_dir": dir.to_str().unwrap(),
                   "sources_zip": zip.to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(r["data"]["passed"], true, "{}", r["data"]);
        assert_eq!(r["data"]["sources_zip"]["files"], 5_034);
        assert!(r["data"]["pex"]["text"]
            .as_str()
            .unwrap()
            .starts_with("DefaultCounterQuestA.psc v3.12"));
        assert_eq!(r["evidence"][0]["file"], "ContentResources.zip");

        // A namespaced `.psc` path compiles under its declared name, and a stale copy of
        // the source in `out_dir` (the compiler's working directory) is not picked up:
        // without `-ignorecwd` the compiler searched it first and compiled the decoy
        // (papyrus oracle, 2026-10-15).
        let src = dir.join("src").join("KitKraft");
        let out = dir.join("out");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(out.join("KitKraft")).unwrap();
        let probe = "ScriptName KitKraft:Probe extends ScriptObject\nInt Property Real = 1 Auto\n";
        fs::write(src.join("Probe.psc"), probe).unwrap();
        // A stray flags file in the namespace root (imported first) is not used.
        let decoy = dir.join("src").join(FLAGS_FILE);
        fs::write(&decoy, "this is not a flags file {\n").unwrap();
        fs::write(
            out.join("KitKraft").join("Probe.psc"),
            probe.replace("Real", "Decoy"),
        )
        .unwrap();
        let r = execute(
            "papyrus.compile",
            json!({"source": src.join("Probe.psc").to_str().unwrap(),
                   "out_dir": out.to_str().unwrap(), "sources_zip": zip.to_str().unwrap()}),
        )
        .unwrap();
        assert_eq!(r["data"]["passed"], true, "{}", r["data"]);
        assert_eq!(r["data"]["script"], "KitKraft:Probe");
        let text = r["data"]["pex"]["text"].as_str().unwrap();
        assert!(text.contains("PROP Int Real"), "{text}");
        assert!(!text.contains("Decoy"), "{text}");
        let limits: Vec<&str> = r["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|l| l.as_str())
            .collect();
        assert!(
            limits
                .iter()
                .any(|l| l.starts_with("flags file:") && l.contains("sources_zip")),
            "{limits:?}"
        );
        assert!(
            limits
                .iter()
                .any(|l| l.contains("was not used") && l.contains(&decoy.display().to_string())),
            "{limits:?}"
        );
        assert!(out.join("KitKraft").join("Probe.pex").is_file());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// With the compiler installed and `KITKRAFT_PAPYRUS_IMPORTS` naming an extraction of
    /// `Tools\ContentResources.zip`'s `Scripts\Source`, a trivial script compiles in a temp
    /// directory and disassembles through `kitkraft-pex`.
    #[test]
    fn trivial_script_compiles_and_disassembles() {
        let Some(imports) = crate::handlers::env_var_os("KITKRAFT_PAPYRUS_IMPORTS") else {
            println!("skipped: KITKRAFT_PAPYRUS_IMPORTS is not set");
            return;
        };
        if super::compiler_candidates(None)
            .iter()
            .all(|p| !p.is_file())
        {
            println!("skipped: PapyrusCompiler.exe not found");
            return;
        }
        let dir = std::env::temp_dir().join(format!("kitkraft-compile-ok-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        let psc = dir.join("src").join("KitkraftTrivial.psc");
        fs::write(
            &psc,
            "Scriptname KitkraftTrivial extends ScriptObject\n\nInt Function Twice(Int a)\n  Return a * 2\nEndFunction\n",
        )
        .unwrap();
        let r = execute(
            "papyrus.compile",
            json!({"source": psc.to_str().unwrap(), "out_dir": dir.join("out").to_str().unwrap(),
                   "imports": [imports.to_str().unwrap()]}),
        )
        .unwrap();
        assert_eq!(r["data"]["passed"], true, "{}", r["data"]);
        let text = r["data"]["pex"]["text"].as_str().unwrap();
        assert!(text.starts_with("KitkraftTrivial.psc v3.12"), "{text}");
        assert!(text.contains("FUNC Twice(a)"), "{text}");
        assert!(text.contains("imul"), "{text}");
        let args = r["data"]["args"].as_array().unwrap();
        assert!(args
            .iter()
            .all(|a| !a.as_str().unwrap().to_ascii_lowercase().starts_with("-r")));
        fs::remove_dir_all(&dir).unwrap();
    }
}
