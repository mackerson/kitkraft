//! `Tools\ContentResources.zip` as the base import folder of `papyrus.compile` (M3.G1).
//!
//! `PapyrusCompiler.exe` reads sources from folders, so the zip's `Scripts/Source` files
//! (`.psc` and the `.flg` flags file; corpus 2026-10-14: 5,034 files, 12.3 MB, folders up to
//! five deep for namespaces such as `Fragments`) are extracted once into the KitCraft cache
//! (`KITCRAFT_CACHE_DIR`, else `%LOCALAPPDATA%\kitcraft\cache`, else the OS temp dir) under
//! `papyrus-sources\<key>`, where `<key>` is a hash of every extracted entry's name, size
//! and CRC-32 from the zip's central directory: a changed zip gets a new folder, an
//! unchanged one is reused without reading any file data. The folder is written under a
//! temporary name and renamed into place only when complete, so a crash never leaves a
//! partial import folder behind.
//!
//! The zip is read in place, read-only (ADR 0006 §3), and treated as hostile: every name is
//! checked before the first write (only ASCII letters, digits, `_`, `-`, `.` and spaces;
//! no empty, `.`/`..`, trailing-dot or Windows device-name components; at most
//! [`MAX_DEPTH`] deep), case-folding collisions, symlinks and encrypted entries are refused,
//! and sizes are capped per entry and in total before anything is read. The zip crate
//! checks each entry's CRC-32 as it is read to its end.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

/// The folder inside the zip that holds the base game's script sources (compared
/// case-insensitively, either separator).
const SOURCE_PREFIX: &str = "scripts/source/";
/// Extensions extracted, lower case: script sources and the flags file.
const EXTENSIONS: [&str; 2] = [".psc", ".flg"];
/// Largest single file extracted (the largest official source is 170,924 bytes).
pub(crate) const MAX_ENTRY_BYTES: u64 = 16 << 20;
/// Largest total extracted.
pub(crate) const MAX_TOTAL_BYTES: u64 = 512 << 20;
/// Most files extracted.
pub(crate) const MAX_FILES: usize = 100_000;
/// Most path components below `Scripts/Source` (the corpus has at most five).
pub(crate) const MAX_DEPTH: usize = 16;
/// Part of the cache key; bump when the extracted layout changes.
const LAYOUT_VERSION: u8 = 1;
/// Written last into a complete extraction.
const COMPLETE: &str = ".kitcraft-complete";
/// The cache subfolder.
const CACHE_SUBDIR: &str = "papyrus-sources";

/// The import folder made from a zip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Extracted {
    /// The folder to pass as an import directory.
    pub dir: PathBuf,
    /// Files in it.
    pub files: usize,
    /// Their total size.
    pub bytes: u64,
    /// Whether the folder already existed (nothing was read from the zip but its
    /// central directory).
    pub cached: bool,
    /// Entries under `Scripts/Source` that were not extracted (other extensions).
    pub skipped: usize,
}

/// One entry to extract.
#[derive(Debug)]
struct Planned {
    index: usize,
    /// Path components below `Scripts/Source`.
    parts: Vec<String>,
    size: u64,
}

/// The cache root: `KITCRAFT_CACHE_DIR`, `%LOCALAPPDATA%\kitcraft\cache`, or the OS temp dir.
pub(crate) fn default_cache_root() -> PathBuf {
    kitcraft_doc::IndexCache::default_location().map_or_else(
        || std::env::temp_dir().join("kitcraft-cache"),
        |c| c.dir().to_path_buf(),
    )
}

/// Makes (or reuses) the import folder for the `Scripts/Source` files of `zip_path` under
/// `cache_root`. Errors are messages naming the zip and, where it applies, the entry.
pub(crate) fn extract_sources(zip_path: &Path, cache_root: &Path) -> Result<Extracted, String> {
    let shown = zip_path.display();
    let file = File::open(zip_path).map_err(|e| format!("{shown}: {e}"))?;
    let mut archive =
        ZipArchive::new(BufReader::new(file)).map_err(|e| format!("{shown}: not a zip: {e}"))?;
    let (plan, skipped, key) = plan(&archive).map_err(|e| format!("{shown}: {e}"))?;
    if plan.is_empty() {
        return Err(format!(
            "{shown}: no .psc or .flg files under Scripts/Source"
        ));
    }
    let files = plan.len();
    let bytes = plan.iter().fold(0u64, |sum, p| sum.saturating_add(p.size));
    let base = cache_root.join(CACHE_SUBDIR);
    let dir = base.join(format!("{key:016x}"));
    let done = |cached| Extracted {
        dir: dir.clone(),
        files,
        bytes,
        cached,
        skipped,
    };
    if dir.join(COMPLETE).is_file() {
        return Ok(done(true));
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let tmp = base.join(format!("{key:016x}.tmp-{}-{nanos}", std::process::id()));
    let result = write_all(&mut archive, &plan, &tmp)
        .map_err(|e| format!("{shown}: extracting into `{}`: {e}", tmp.display()));
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }
    if dir.exists() && !dir.join(COMPLETE).is_file() {
        // Not written by this code (it renames complete folders only); replace it.
        let _ = fs::remove_dir_all(&dir);
    }
    match fs::rename(&tmp, &dir) {
        Ok(()) => Ok(done(false)),
        Err(_) if dir.join(COMPLETE).is_file() => {
            // Another process finished the same extraction first.
            let _ = fs::remove_dir_all(&tmp);
            Ok(done(true))
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            Err(format!(
                "{shown}: moving the extraction to `{}`: {e}",
                dir.display()
            ))
        }
    }
}

/// Checks every entry name and size; returns what to extract, how many `Scripts/Source`
/// entries were skipped, and the cache key.
fn plan<R: Read + std::io::Seek>(
    archive: &ZipArchive<R>,
) -> Result<(Vec<Planned>, usize, u64), String> {
    let mut plan = Vec::new();
    let mut skipped = 0usize;
    let mut total = 0u64;
    let mut seen = BTreeSet::new();
    let mut key = Fnv::new();
    key.write(&[LAYOUT_VERSION]);
    for index in 0..archive.len() {
        let entry = archive
            .by_index_data(index)
            .map_err(|e| format!("entry {index}: {e}"))?;
        let name = entry
            .name()
            .map_err(|e| format!("entry {index}: {e}"))?
            .replace('\\', "/");
        let Some(rel) = strip_prefix_ascii_ci(&name, SOURCE_PREFIX) else {
            continue;
        };
        if entry.is_dir() || rel.is_empty() || rel.ends_with('/') {
            continue;
        }
        let lower = rel.to_ascii_lowercase();
        if !EXTENSIONS.iter().any(|ext| lower.ends_with(ext)) {
            skipped = skipped.saturating_add(1);
            continue;
        }
        let parts: Vec<String> = rel.split('/').map(str::to_string).collect();
        if parts.len() > MAX_DEPTH {
            return Err(format!(
                "`{name}` is nested deeper than {MAX_DEPTH} folders"
            ));
        }
        if let Some(bad) = parts.iter().find(|p| !valid_component(p)) {
            return Err(format!(
                "`{name}`: refusing the path component `{bad}` (only ASCII letters, digits, \
                 `_`, `-`, `.` and spaces; no `.`/`..`, trailing dots or spaces, or device \
                 names)"
            ));
        }
        if entry.is_symlink() {
            return Err(format!("`{name}` is a symbolic link"));
        }
        if entry.encrypted() {
            return Err(format!("`{name}` is encrypted"));
        }
        if !seen.insert(lower.clone()) {
            return Err(format!(
                "`{name}` collides with another entry when case is ignored"
            ));
        }
        let size = entry.size();
        if size > MAX_ENTRY_BYTES {
            return Err(format!(
                "`{name}` is {size} bytes, over the {MAX_ENTRY_BYTES}-byte limit"
            ));
        }
        total = total.saturating_add(size);
        if total > MAX_TOTAL_BYTES {
            return Err(format!(
                "the sources exceed {MAX_TOTAL_BYTES} bytes in total"
            ));
        }
        if plan.len() >= MAX_FILES {
            return Err(format!("more than {MAX_FILES} source files"));
        }
        key.write(lower.as_bytes());
        key.write(&[0]);
        key.write(&size.to_le_bytes());
        key.write(&entry.crc32().to_le_bytes());
        plan.push(Planned { index, parts, size });
    }
    Ok((plan, skipped, key.finish()))
}

/// The checked `Scripts/Source` entries of a zip, read in memory without extracting
/// anything (`papyrus.events`, M4.G2). Names, sizes and the key are checked and computed
/// exactly as for [`extract_sources`].
pub(crate) struct ZipSources {
    archive: ZipArchive<BufReader<File>>,
    plan: Vec<Planned>,
    /// Hash of every entry's name, size and CRC-32 (the central directory only).
    pub key: u64,
}

impl ZipSources {
    /// Opens `zip_path` and checks its directory; reads no file data.
    pub(crate) fn open(zip_path: &Path) -> Result<ZipSources, String> {
        let shown = zip_path.display();
        let file = File::open(zip_path).map_err(|e| format!("{shown}: {e}"))?;
        let archive = ZipArchive::new(BufReader::new(file))
            .map_err(|e| format!("{shown}: not a zip: {e}"))?;
        let (plan, _, key) = plan(&archive).map_err(|e| format!("{shown}: {e}"))?;
        if !plan.iter().any(|p| is_psc(&p.parts)) {
            return Err(format!("{shown}: no .psc files under Scripts/Source"));
        }
        Ok(ZipSources { archive, plan, key })
    }

    /// Every `.psc`, as `(path below Scripts/Source with '/', bytes)`, in zip order.
    pub(crate) fn read_all_psc(&mut self) -> Result<Vec<(String, Vec<u8>)>, String> {
        let mut out = Vec::new();
        for p in self.plan.iter().filter(|p| is_psc(&p.parts)) {
            out.push((p.parts.join("/"), read_entry(&mut self.archive, p)?));
        }
        Ok(out)
    }

    /// One file by its path below `Scripts/Source` (case-insensitive, either separator).
    pub(crate) fn read(&mut self, rel: &str) -> Result<Option<Vec<u8>>, String> {
        let want = rel.replace('\\', "/");
        let Some(p) = self
            .plan
            .iter()
            .find(|p| p.parts.join("/").eq_ignore_ascii_case(&want))
        else {
            return Ok(None);
        };
        read_entry(&mut self.archive, p).map(Some)
    }
}

fn is_psc(parts: &[String]) -> bool {
    parts
        .last()
        .is_some_and(|n| n.to_ascii_lowercase().ends_with(".psc"))
}

/// Reads one planned entry, checking its size against the directory (the zip crate checks
/// the CRC-32 as the entry is read to its end).
fn read_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    p: &Planned,
) -> Result<Vec<u8>, String> {
    let shown = p.parts.join("/");
    let mut entry = archive
        .by_index(p.index)
        .map_err(|e| format!("`{shown}`: {e}"))?;
    // At most one byte past the declared size, so a lying header is caught without
    // reading an unbounded stream; reading to the end lets the zip crate check the CRC.
    let mut data = Vec::with_capacity(usize::try_from(p.size).unwrap_or(0));
    (&mut entry)
        .take(p.size.saturating_add(1))
        .read_to_end(&mut data)
        .map_err(|e| format!("`{shown}`: {e}"))?;
    if data.len() as u64 != p.size {
        return Err(format!(
            "`{shown}`: {} bytes, the directory says {}",
            data.len(),
            p.size
        ));
    }
    Ok(data)
}

/// Extracts `plan` into the new folder `tmp` and marks it complete.
fn write_all<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    plan: &[Planned],
    tmp: &Path,
) -> Result<(), String> {
    fs::create_dir_all(tmp).map_err(|e| e.to_string())?;
    for p in plan {
        let target = p
            .parts
            .iter()
            .fold(tmp.to_path_buf(), |path, c| path.join(c));
        let shown = p.parts.join("/");
        let data = read_entry(archive, p)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("`{shown}`: {e}"))?;
        }
        fs::write(&target, &data).map_err(|e| format!("`{shown}`: {e}"))?;
    }
    fs::write(tmp.join(COMPLETE), format!("{}\n", plan.len())).map_err(|e| e.to_string())
}

/// `name` without `prefix`, compared ASCII case-insensitively.
fn strip_prefix_ascii_ci<'n>(name: &'n str, prefix: &str) -> Option<&'n str> {
    let head = name.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| name.get(prefix.len()..))
        .flatten()
}

/// Whether `c` is safe as one path component on Windows.
fn valid_component(c: &str) -> bool {
    const DEVICES: [&str; 22] = [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    if c.is_empty() || c.len() > 255 || c == "." || c == ".." {
        return false;
    }
    if c.starts_with(' ') || c.ends_with(' ') || c.ends_with('.') {
        return false;
    }
    if !c
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ' '))
    {
        return false;
    }
    let stem = c.split('.').next().unwrap_or_default().trim_end();
    !DEVICES.iter().any(|d| stem.eq_ignore_ascii_case(d))
}

/// FNV-1a 64 (the cache key; not a security boundary: the folder is ours).
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
pub(super) mod tests {
    use super::{extract_sources, valid_component, COMPLETE};
    use std::fs;
    use std::io::{Cursor, Write};
    use std::path::{Path, PathBuf};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kitcraft-papyrus-sources-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A zip with the given `(name, contents)` entries, deflated.
    pub(crate) fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, data) in entries {
            out.start_file(*name, options).unwrap();
            out.write_all(data).unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    fn write_zip(dir: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join("ContentResources.zip");
        fs::write(&path, zip_bytes(entries)).unwrap();
        path
    }

    const GOOD: [(&str, &[u8]); 6] = [
        ("Scripts/Source/Alpha.psc", b"Scriptname Alpha\n"),
        (
            "Scripts/Source/Fragments/Quests/QF_Beta.psc",
            b"Scriptname Fragments:Quests:QF_Beta\n",
        ),
        (
            "Scripts/Source/Starfield_Papyrus_Flags.flg",
            b"Flag Hidden 0\n",
        ),
        ("Scripts/Source/placeholder.txt", b"x"),
        ("Scripts/Source/Genesis_Papyrus.sublime-project", b"{}"),
        ("Materials/Thing.mat", b"not a script"),
    ];

    #[test]
    fn extracts_scripts_and_flags_only_and_reuses_the_folder() {
        let dir = temp("good");
        let zip = write_zip(&dir, &GOOD);
        let cache = dir.join("cache");
        let first = extract_sources(&zip, &cache).unwrap();
        assert!(!first.cached);
        assert_eq!(first.files, 3);
        assert_eq!(first.skipped, 2);
        assert_eq!(
            first.bytes,
            GOOD[..3].iter().map(|(_, d)| d.len() as u64).sum::<u64>()
        );
        assert!(first.dir.starts_with(cache.join("papyrus-sources")));
        assert_eq!(
            fs::read(
                first
                    .dir
                    .join("Fragments")
                    .join("Quests")
                    .join("QF_Beta.psc")
            )
            .unwrap(),
            GOOD[1].1
        );
        assert!(first.dir.join("Starfield_Papyrus_Flags.flg").is_file());
        assert!(first.dir.join(COMPLETE).is_file());
        assert!(!first.dir.join("placeholder.txt").exists());
        assert!(!first.dir.join("Thing.mat").exists());
        // Only the final folder is left in the cache.
        assert_eq!(
            fs::read_dir(cache.join("papyrus-sources")).unwrap().count(),
            1
        );

        let again = extract_sources(&zip, &cache).unwrap();
        assert!(again.cached);
        assert_eq!(again.dir, first.dir);

        // A changed source is a new key, so a new folder.
        let mut changed = GOOD;
        changed[0].1 = b"Scriptname Alpha\n; edited\n";
        let zip2 = write_zip(&dir, &changed);
        let third = extract_sources(&zip2, &cache).unwrap();
        assert!(!third.cached);
        assert_ne!(third.dir, first.dir);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn hostile_names_are_refused_before_anything_is_written() {
        let cases: [(&str, &str); 7] = [
            ("Scripts/Source/../Evil.psc", ".."),
            ("Scripts/Source/a/./b.psc", "`.`"),
            ("Scripts/Source/CON.psc", "CON.psc"),
            ("Scripts/Source/x:y.psc", "x:y.psc"),
            ("Scripts/Source/trail./a.psc", "trail."),
            ("Scripts/Source/caf\u{e9}.psc", "caf"),
            ("Scripts/Source//a.psc", "``"),
        ];
        for (i, (name, needle)) in cases.iter().enumerate() {
            let dir = temp(&format!("hostile{i}"));
            let zip = write_zip(&dir, &[(GOOD[0].0, GOOD[0].1), (name, b"x")]);
            let cache = dir.join("cache");
            let err = extract_sources(&zip, &cache).unwrap_err();
            assert!(
                err.contains("refusing") && err.contains(needle),
                "{name}: {err}"
            );
            assert!(!cache.exists(), "{name}: nothing may be written");
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn case_collisions_empty_and_non_zip_inputs_fail() {
        let dir = temp("collide");
        let zip = write_zip(
            &dir,
            &[
                ("Scripts/Source/A.psc", b"1"),
                ("scripts/source/a.PSC", b"2"),
            ],
        );
        let err = extract_sources(&zip, &dir.join("cache")).unwrap_err();
        assert!(err.contains("case is ignored"), "{err}");

        let zip = write_zip(&dir, &[("Materials/Thing.mat", b"x")]);
        let err = extract_sources(&zip, &dir.join("cache")).unwrap_err();
        assert!(err.contains("no .psc or .flg"), "{err}");

        let not_zip = dir.join("not.zip");
        fs::write(&not_zip, b"PK\x03\x04 but not really").unwrap();
        let err = extract_sources(&not_zip, &dir.join("cache")).unwrap_err();
        assert!(err.contains("not a zip"), "{err}");

        let err = extract_sources(&dir.join("missing.zip"), &dir.join("cache")).unwrap_err();
        assert!(err.contains("missing.zip"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A data byte flipped inside a stored entry fails the CRC check; the half-written
    /// folder is removed.
    #[test]
    fn corrupt_entry_data_fails_and_leaves_no_folder() {
        let dir = temp("corrupt");
        let mut out = ZipWriter::new(Cursor::new(Vec::new()));
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        out.start_file("Scripts/Source/A.psc", stored).unwrap();
        out.write_all(b"Scriptname A\n").unwrap();
        let mut bytes = out.finish().unwrap().into_inner();
        let at = bytes.windows(10).position(|w| w == b"Scriptname").unwrap();
        bytes[at] ^= 0x20;
        let zip = dir.join("bad.zip");
        fs::write(&zip, &bytes).unwrap();
        let cache = dir.join("cache");
        let err = extract_sources(&zip, &cache).unwrap_err();
        assert!(err.contains("A.psc"), "{err}");
        let left: Vec<_> = fs::read_dir(cache.join("papyrus-sources"))
            .map(|d| d.collect())
            .unwrap_or_default();
        assert!(left.is_empty(), "{left:?}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn component_rules() {
        for ok in [
            "A.psc",
            "Fragments",
            "QF_Beta-1.psc",
            "My Folder",
            "x.y.psc",
        ] {
            assert!(valid_component(ok), "{ok}");
        }
        for bad in [
            "", ".", "..", "a.", "a ", " a", "a\\b", "a:b", "nul", "Com1.psc", "LPT9", "é",
        ] {
            assert!(!valid_component(bad), "{bad}");
        }
    }

    /// With `KITCRAFT_DATA_DIR` set, the official zip's sources extract (or are reused)
    /// with the counts recorded in the module docs.
    #[test]
    fn official_content_resources_zip() {
        let Some(data) = std::env::var_os("KITCRAFT_DATA_DIR") else {
            println!("skipped: KITCRAFT_DATA_DIR is not set");
            return;
        };
        let Some(zip) = Path::new(&data)
            .parent()
            .map(|g| g.join("Tools").join("ContentResources.zip"))
            .filter(|z| z.is_file())
        else {
            println!("skipped: Tools\\ContentResources.zip not found");
            return;
        };
        let cache = std::env::temp_dir().join("kitcraft-papyrus-sources-official");
        let got = extract_sources(&zip, &cache).unwrap();
        assert_eq!(got.files, 5_034);
        assert_eq!(got.bytes, 12_261_886);
        assert!(got.dir.join("Starfield_Papyrus_Flags.flg").is_file());
        assert!(got.dir.join("ObjectReference.psc").is_file());
    }

    /// M4.G1 acceptance: with `KITCRAFT_DATA_DIR` set, every official `.psc` in
    /// `Tools\ContentResources.zip` is read in memory (nothing extracted) and its
    /// declarations parse with no error and no statement left unread; failures are listed
    /// with file and line. The counts printed are the ones in `plan/STATUS.md`.
    #[test]
    fn every_official_source_parses() {
        let Some(zip) = official_zip() else {
            return;
        };
        let mut sources = super::ZipSources::open(&zip).unwrap();
        let files = sources.read_all_psc().unwrap();
        assert_eq!(files.len(), 5_033);
        let mut failures = Vec::new();
        let mut unread = Vec::new();
        let (mut functions, mut events, mut properties, mut structs) = (0, 0, 0, 0);
        let (mut custom, mut natives, mut remote) = (0, 0, 0);
        for (path, bytes) in &files {
            match kitcraft_pex::psc::parse(bytes) {
                Ok(s) => {
                    functions += s.functions.len();
                    events += s.events.len();
                    properties += s.properties.len();
                    structs += s.structs.len();
                    custom += s.custom_events.len();
                    natives += s
                        .functions
                        .iter()
                        .filter(|f| kitcraft_pex::psc::has_flag(&f.flags, "native"))
                        .count();
                    remote += s.events.iter().filter(|e| e.remote.is_some()).count();
                    unread.extend(
                        s.diagnostics
                            .iter()
                            .map(|d| format!("{path}:{}: {}", d.line, d.message)),
                    );
                }
                Err(e) => failures.push(format!("{path}: {e}")),
            }
        }
        println!(
            "{} sources: {functions} functions ({natives} native), {events} events \
             ({remote} remote), {properties} properties, {structs} structs, {custom} custom events",
            files.len()
        );
        assert!(failures.is_empty(), "{failures:#?}");
        assert!(unread.is_empty(), "{unread:#?}");
    }

    /// `Tools\ContentResources.zip` next to `KITCRAFT_DATA_DIR`, or `None` with a message.
    pub(crate) fn official_zip() -> Option<PathBuf> {
        let Some(data) = std::env::var_os("KITCRAFT_DATA_DIR") else {
            println!("skipped: KITCRAFT_DATA_DIR is not set");
            return None;
        };
        let zip = Path::new(&data)
            .parent()
            .map(|g| g.join("Tools").join("ContentResources.zip"))
            .filter(|z| z.is_file());
        if zip.is_none() {
            println!("skipped: Tools\\ContentResources.zip not found");
        }
        zip
    }
}
