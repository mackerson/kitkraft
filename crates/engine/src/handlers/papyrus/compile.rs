//! `papyrus.compile` (M2.G7): runs Bethesda's `PapyrusCompiler.exe` as a black box (ADR
//! 0006) on one script, by name, with import directories.
//!
//! The argument builder ([`compiler_args`]) emits only the object name, `-f=`, `-i=`,
//! `-o=` and optionally `-op`. It has no way to ask for release or final mode (`-release`,
//! `-r`, `-final`), checks its own output for them, and the catalog schema rejects any
//! extra argument. Project files (`.ppj`) are refused because they can turn release mode
//! on themselves. Output is never written under the game Data directory, and the produced
//! `.pex` is parsed and disassembled with `kitcraft-pex` as a check.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use kitcraft_pex::{disassemble, Pex};
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
const STEAM_GAME: &str = r"C:\Program Files (x86)\Steam\steamapps\common\Starfield";
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
                "refusing compiler argument `{arg}`: KitCraft never compiles in release or final mode"
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
    if let Some(p) = std::env::var_os("KITCRAFT_PAPYRUS_COMPILER") {
        out.push(PathBuf::from(p));
    }
    if let Some(data) = std::env::var_os("KITCRAFT_DATA_DIR") {
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

/// `papyrus.compile`.
pub(crate) fn run(args: &Value) -> Result<Outcome, EngineError> {
    let source = str_arg(ID, args, "source")?;
    let out_dir = PathBuf::from(str_arg(ID, args, "out_dir")?);
    let optimize = args
        .get("optimize")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut imports: Vec<PathBuf> = args
        .get("imports")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();

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
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let dir = fs::canonicalize(&dir).unwrap_or(dir);
        imports.insert(0, strip_verbatim(&dir));
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        source.to_string()
    };
    for dir in &imports {
        if !dir.is_dir() {
            return Err(failed(
                ID,
                format!("import directory `{}` does not exist", dir.display()),
            ));
        }
    }
    let flags = match args.get("flags").and_then(Value::as_str) {
        Some(f) => {
            let p = PathBuf::from(f);
            if !p.is_file() {
                return Err(failed(ID, format!("flags file `{f}` does not exist")));
            }
            Some(p)
        }
        None => imports
            .iter()
            .map(|d| d.join(FLAGS_FILE))
            .find(|p| p.is_file()),
    };

    refuse_game_data(ID, &out_dir)?;
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
                 KITCRAFT_PAPYRUS_COMPILER)",
                looked.join("; ")
            ),
        )
    })?;

    let request = Request {
        object,
        imports,
        flags,
        out_dir: out_dir.clone(),
        optimize,
    };
    let compiler_args = compiler_args(&request).map_err(|e| failed(ID, e))?;
    let (exit_code, stdout, stderr) =
        run_compiler(compiler, &compiler_args, &out_dir).map_err(|e| failed(ID, e))?;
    let passed = exit_code == Some(0);

    let pex_path = request
        .object
        .split(':')
        .fold(out_dir.clone(), |p, part| p.join(part))
        .with_extension("pex");
    let mut limitations = Vec::new();
    if request.flags.is_none() {
        limitations.push(format!(
            "no flags file: none given and {FLAGS_FILE} is not in any import directory"
        ));
    }
    let mut evidence = Vec::new();
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
    use super::{compiler_args, refuse_release, Request};
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
        assert_eq!(compiler_args(&minimal).unwrap(), ["S", "-o=out"]);
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
        let dir = std::env::temp_dir().join(format!("kitcraft-compile-{}", std::process::id()));
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

    /// With the compiler installed and `KITCRAFT_PAPYRUS_IMPORTS` naming an extraction of
    /// `Tools\ContentResources.zip`'s `Scripts\Source`, a trivial script compiles in a temp
    /// directory and disassembles through `kitcraft-pex`.
    #[test]
    fn trivial_script_compiles_and_disassembles() {
        let Some(imports) = std::env::var_os("KITCRAFT_PAPYRUS_IMPORTS") else {
            println!("skipped: KITCRAFT_PAPYRUS_IMPORTS is not set");
            return;
        };
        if super::compiler_candidates(None)
            .iter()
            .all(|p| !p.is_file())
        {
            println!("skipped: PapyrusCompiler.exe not found");
            return;
        }
        let dir = std::env::temp_dir().join(format!("kitcraft-compile-ok-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        let psc = dir.join("src").join("KitcraftTrivial.psc");
        fs::write(
            &psc,
            "Scriptname KitcraftTrivial extends ScriptObject\n\nInt Function Twice(Int a)\n  Return a * 2\nEndFunction\n",
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
        assert!(text.starts_with("KitcraftTrivial.psc v3.12"), "{text}");
        assert!(text.contains("FUNC Twice(a)"), "{text}");
        assert!(text.contains("imul"), "{text}");
        let args = r["data"]["args"].as_array().unwrap();
        assert!(args
            .iter()
            .all(|a| !a.as_str().unwrap().to_ascii_lowercase().starts_with("-r")));
        fs::remove_dir_all(&dir).unwrap();
    }
}
