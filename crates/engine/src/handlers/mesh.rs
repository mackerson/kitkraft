//! `mesh.bounds` (M7.S4, lane R): a model's box from its NIF, found from a base object's
//! `MODL` or given as a path, read from the loose files or the archives of the Data dir.
//!
//! The box is what `kitkraft-nif` reads: the union of the bounds the model's geometry
//! blocks store, carried up the node tree to the model's root frame
//! (`docs/research/nif-bounds-2026-11-02.md`, claims B5 and C1). It is not the base's
//! `OBND` (`cell` with `with_bounds` reads that). `mesh.geometry` (M7.S5) is [`geometry`];
//! the integrator's A8 seam commit flips the catalog from `Deferred` to `Implemented` and
//! adds its dispatch row.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use kitkraft_ba2::Archive;
use kitkraft_doc::{IndexCache, RecordReader};
use kitkraft_nif::{Bound, Mesh, Nif};
use serde_json::{json, Value};

use super::inspect::find_form;
use super::load::acquire;
use super::refs::identity;
use super::{data_dir_source, failed};
use crate::envelope::{Coverage, CoverageStatus, Evidence};
use crate::exec::EngineError;
use crate::{Outcome, Session};

const ID: &str = "mesh.bounds";

/// Largest model read, in bytes. The largest official `.nif` is 3,466,437 bytes (study,
/// 2026-11-02); the cap stops a hostile archive entry from declaring gigabytes.
pub(crate) const MAX_MODEL_BYTES: u64 = 64 << 20;

/// At most this many geometry blocks get an evidence range each.
const MAX_EVIDENCE: usize = 1_000;

/// The study's cross-check of the model box against `OBND` (claim C1b), repeated on every
/// result so the box is never read as the base's `OBND`.
pub(crate) const OBND_NOTE: &str = "the box is the union of the bounds the model's geometry blocks store, in the model's root frame (docs/research/nif-bounds-2026-11-02.md, C1); it is not the base's OBND: over 33,684 official base objects it matched OBND within 1 cm for 23,261 (69%) and differed for the rest";

/// The `.nif` and `.mesh` entries of the Data dir's archives, read once per session and
/// Data dir (M7.R5: from the engine's index cache when no archive changed since it was
/// written, so the first call does not open every archive).
pub(super) struct MeshIndex {
    pub(super) data_dir: PathBuf,
    /// Archives holding at least one `.nif` or `.mesh`, in name order, each opened on
    /// first use.
    pub(super) archives: Vec<KeptArchive>,
    /// Normalized `.nif` entry path -> (archive index, entry index), in archive name order.
    pub(super) by_path: HashMap<String, Vec<(usize, usize)>>,
    /// Archives that did not open, with the reason.
    pub(super) unreadable: Vec<String>,
    /// Normalized `.mesh` entry path -> (archive index, entry index).
    pub(super) meshes: HashMap<String, Vec<(usize, usize)>>,
    /// Read from the index cache, not from the archives.
    pub(super) from_cache: bool,
    /// The index-cache folder this index was read from or written to (`None`: no cache);
    /// a rebuild after a stale archive writes the file there again (review H1 of M7.R5).
    pub(super) cache_dir: Option<PathBuf>,
}

/// Why [`MeshIndex::read_mesh`] returned no mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MeshMiss {
    /// No indexed archive holds the entry.
    Absent,
    /// The archive holding it no longer matches the cached index (the index is built again).
    Stale(String),
    /// The archive holding it did not open or the entry did not extract.
    Unread(String),
}

/// An archive that holds models or meshes. One read from the archives keeps the archive
/// it opened; one taken from the cache is opened on first use and checked then against
/// what the cache recorded (the cache file's names are never trusted alone).
pub(super) struct KeptArchive {
    pub(super) name: String,
    pub(super) path: PathBuf,
    /// The entry count when indexed.
    pub(super) entry_count: u32,
    /// Every indexed (entry index, normalized name, kind).
    pub(super) expect: Vec<(u32, String, u8)>,
    pub(super) archive: OnceLock<Option<Archive>>,
}

impl KeptArchive {
    /// The archive, opened once; `None` when it does not open or no longer matches what
    /// the index recorded: its entry count, any indexed entry's name and kind, or the
    /// number of `.nif`/`.mesh` entries it holds (so an entry the index omits is caught
    /// too, review M1 of M7.R5).
    pub(super) fn open(&self) -> Option<&Archive> {
        self.archive
            .get_or_init(|| {
                let (a, _) = Archive::open(&self.path).ok()?;
                let names: Vec<String> = a
                    .entries
                    .iter()
                    .map(|e| kitkraft_ba2::normalize(&e.name))
                    .collect();
                let same_count = u32::try_from(names.len()).ok() == Some(self.entry_count);
                let indexed = names.iter().filter(|n| kind_of(n).is_some()).count();
                let same_names = self.expect.iter().all(|(i, n, k)| {
                    usize::try_from(*i)
                        .ok()
                        .and_then(|i| names.get(i))
                        .is_some_and(|x| x == n && kind_of(x) == Some(*k))
                });
                (same_count && indexed == self.expect.len() && same_names).then_some(a)
            })
            .as_ref()
    }
}

/// Entry kinds in the index cache.
const KIND_NIF: u8 = 0;
const KIND_MESH: u8 = 1;

/// The kind of a normalized entry name the index keeps, if any.
fn kind_of(name: &str) -> Option<u8> {
    if name.ends_with(".nif") {
        Some(KIND_NIF)
    } else if name.ends_with(".mesh") {
        Some(KIND_MESH)
    } else {
        None
    }
}

impl MeshIndex {
    /// The index for `data_dir`: from the cache file in `cache_dir` when `read_cache` and
    /// every archive's key (name, length, modification time) matches it, else from the
    /// archives, written to the cache file afterwards (when `cache_dir` is given and every
    /// archive opened). A cache file that does not parse is ignored.
    pub(super) fn build_cached(
        data_dir: &Path,
        cache_dir: Option<&Path>,
        read_cache: bool,
        id: &str,
    ) -> Result<Self, EngineError> {
        let listing = std::fs::read_dir(data_dir).map_err(|e| {
            failed(
                id,
                format!("cannot list the Data dir {}: {e}", data_dir.display()),
            )
        })?;
        let mut paths: Vec<PathBuf> = listing
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ba2")))
            .collect();
        paths.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
        // Every archive's key; one that cannot be read turns the cache off this time.
        let keys: Option<Vec<ArchiveKey>> = paths.iter().map(|p| key_of(p)).collect();
        let file = cache_dir
            .zip(keys.as_ref())
            .map(|(dir, _)| cache_file(dir, data_dir));
        if let (true, Some(f), Some(keys)) = (read_cache, &file, &keys) {
            if let Some(mut index) = Self::from_cache_file(data_dir, &paths, keys, f) {
                index.cache_dir = cache_dir.map(Path::to_path_buf);
                return Ok(index);
            }
        }
        let mut index = MeshIndex {
            data_dir: data_dir.to_path_buf(),
            archives: Vec::new(),
            by_path: HashMap::new(),
            unreadable: Vec::new(),
            meshes: HashMap::new(),
            from_cache: false,
            cache_dir: cache_dir.map(Path::to_path_buf),
        };
        let mut stored = Stored {
            data_dir: data_dir_text(data_dir),
            keys: keys.clone().unwrap_or_default(),
            kept: Vec::new(),
        };
        for (k, path) in paths.iter().enumerate() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let archive = match Archive::open(path) {
                Ok((a, _)) => a,
                Err(e) => {
                    index.unreadable.push(format!("{name}: {e}"));
                    continue;
                }
            };
            let expect: Vec<(u32, String, u8)> = archive
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    let n = kitkraft_ba2::normalize(&e.name);
                    let kind = kind_of(&n)?;
                    Some((u32::try_from(i).ok()?, n, kind))
                })
                .collect();
            if expect.is_empty() {
                continue;
            }
            let Ok(entry_count) = u32::try_from(archive.entries.len()) else {
                index
                    .unreadable
                    .push(format!("{name}: more entries than the index records"));
                continue;
            };
            stored.kept.push(StoredArchive {
                key: k,
                entry_count,
                entries: expect.clone(),
            });
            index.push_kept(name, path.clone(), entry_count, expect, Some(archive));
        }
        if let (Some(f), true, true) = (&file, index.unreadable.is_empty(), keys.is_some()) {
            if let Some(bytes) = encode_stored(&stored) {
                write_cache_file(f, &bytes);
            }
        }
        Ok(index)
    }

    /// Adds one kept archive and its entries to the maps.
    fn push_kept(
        &mut self,
        name: String,
        path: PathBuf,
        entry_count: u32,
        expect: Vec<(u32, String, u8)>,
        archive: Option<Archive>,
    ) {
        let at = self.archives.len();
        for (i, n, kind) in &expect {
            let Ok(i) = usize::try_from(*i) else { continue };
            let map = if *kind == KIND_NIF {
                &mut self.by_path
            } else {
                &mut self.meshes
            };
            map.entry(n.clone()).or_default().push((at, i));
        }
        let lock = OnceLock::new();
        if let Some(a) = archive {
            let _ = lock.set(Some(a));
        }
        self.archives.push(KeptArchive {
            name,
            path,
            entry_count,
            expect,
            archive: lock,
        });
    }

    /// The index from the cache file `file`, when it parses and its keys are `keys`.
    fn from_cache_file(
        data_dir: &Path,
        paths: &[PathBuf],
        keys: &[ArchiveKey],
        file: &Path,
    ) -> Option<Self> {
        let len = std::fs::metadata(file).ok()?.len();
        if len > MAX_CACHE_FILE {
            return None;
        }
        let stored = parse_stored(&std::fs::read(file).ok()?)?;
        if stored.keys != keys || stored.data_dir != data_dir_text(data_dir) {
            return None;
        }
        let mut index = MeshIndex {
            data_dir: data_dir.to_path_buf(),
            archives: Vec::new(),
            by_path: HashMap::new(),
            unreadable: Vec::new(),
            meshes: HashMap::new(),
            from_cache: true,
            cache_dir: None,
        };
        for a in stored.kept {
            let path = paths.get(a.key)?.clone();
            let name = keys.get(a.key)?.name.clone();
            index.push_kept(name, path, a.entry_count, a.entries, None);
        }
        Some(index)
    }

    /// The `.mesh` entry `entry` (normalized), from the archive [`rank`] prefers, with
    /// that archive's name, or why there is none ([`MeshMiss`]).
    fn read_mesh(&self, entry: &str, cap: u64) -> Result<(String, Vec<u8>), MeshMiss> {
        let holders = self.meshes.get(entry).ok_or(MeshMiss::Absent)?;
        let (a, e) = best_holder(self, holders).ok_or(MeshMiss::Absent)?;
        let kept = self.archives.get(a).ok_or(MeshMiss::Absent)?;
        let Some(archive) = kept.open() else {
            return Err(if self.from_cache {
                MeshMiss::Stale(kept.name.clone())
            } else {
                MeshMiss::Unread(kept.name.clone())
            });
        };
        let unread = || MeshMiss::Unread(kept.name.clone());
        let mut f = File::open(&kept.path).map_err(|_| unread())?;
        let bytes = archive.extract_vec(&mut f, e, cap).map_err(|_| unread())?;
        Ok((kept.name.clone(), bytes))
    }
}

/// What identifies an archive's contents without reading it: name, length, modified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ArchiveKey {
    pub(super) name: String,
    pub(super) len: u64,
    pub(super) secs: u64,
    pub(super) nanos: u32,
}

fn key_of(path: &Path) -> Option<ArchiveKey> {
    let meta = std::fs::metadata(path).ok()?;
    let since = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    // A modification time of zero or unknown is never trusted as a key (the `.kcidx`
    // rule): the cache is not used for this Data dir (review M1 of M7.R5).
    if since.is_zero() {
        return None;
    }
    Some(ArchiveKey {
        name: path.file_name()?.to_string_lossy().into_owned(),
        len: meta.len(),
        secs: since.as_secs(),
        nanos: since.subsec_nanos(),
    })
}

/// The cache file's first bytes (format version 2: the Data dir path and a checksum).
pub(super) const CACHE_MAGIC: &[u8; 8] = b"KKMIDX02";
/// Largest cache file read.
const MAX_CACHE_FILE: u64 = 128 << 20;
/// Longest entry or archive name accepted from the cache file.
const MAX_NAME: usize = 1_024;
/// Longest Data dir path accepted from the cache file.
const MAX_PATH: usize = 4_096;
/// Most entries the cache file may hold, all archives together (review M2 of M7.R5): 2.4x
/// the official Data dir's 841,500 `.nif` and `.mesh` entries (13 of its 90 archives, 2026-11-10),
/// so a file cannot make the parser
/// hold far more memory than an honest one.
pub(super) const MAX_ENTRIES: usize = 2_000_000;

/// One kept archive as stored: its position in the key list, entry count, entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StoredArchive {
    pub(super) key: usize,
    pub(super) entry_count: u32,
    pub(super) entries: Vec<(u32, String, u8)>,
}

/// The cache file's contents: the Data dir it describes, every archive's key (so a new
/// or changed one is seen), then each kept archive's entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stored {
    pub(super) data_dir: String,
    pub(super) keys: Vec<ArchiveKey>,
    pub(super) kept: Vec<StoredArchive>,
}

/// FNV-1a (64-bit) of `bytes`, the trailer of the cache file (as `.kcidx` files end).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h = (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A bounded little-endian reader over the cache file's bytes; every read is checked.
struct CacheReader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> CacheReader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let s = self.b.get(self.at..end)?;
        self.at = end;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1)?.first().copied()
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn left(&self) -> usize {
        self.b.len().saturating_sub(self.at)
    }
    fn text(&mut self, max: usize) -> Option<String> {
        let n = usize::from(self.u16()?);
        if n > max {
            return None;
        }
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
    fn name(&mut self) -> Option<String> {
        self.text(MAX_NAME)
    }
    /// A count no larger than what the bytes left can hold at `min_item` bytes each.
    fn count(&mut self, min_item: usize) -> Option<usize> {
        let n = usize::try_from(self.u32()?).ok()?;
        (n.checked_mul(min_item)? <= self.left()).then_some(n)
    }
}

/// Parses a cache file; `None` for anything malformed (the index is then rebuilt): a bad
/// magic or checksum, a count past the bytes left, a name past its cap, a kept archive
/// that names no key, repeats one or keeps no entry, an entry index past its archive's
/// count or not above the one before (so none repeats), trailing bytes.
pub(super) fn parse_stored(file: &[u8]) -> Option<Stored> {
    let body_len = file.len().checked_sub(8)?;
    let (body, sum) = (file.get(..body_len)?, file.get(body_len..)?);
    if u64::from_le_bytes(sum.try_into().ok()?) != fnv1a(body) {
        return None;
    }
    let mut r = CacheReader { b: body, at: 0 };
    if r.take(CACHE_MAGIC.len())? != CACHE_MAGIC {
        return None;
    }
    let data_dir = r.text(MAX_PATH)?;
    // A key: name length (2), length (8), seconds (8), nanoseconds (4).
    let nkeys = r.count(22)?;
    let mut keys = Vec::with_capacity(nkeys);
    for _ in 0..nkeys {
        keys.push(ArchiveKey {
            name: r.name()?,
            len: r.u64()?,
            secs: r.u64()?,
            nanos: r.u32()?,
        });
    }
    // A kept archive: key index (4), entry count (4), entry count stored (4).
    let nkept = r.count(12)?;
    let mut kept: Vec<StoredArchive> = Vec::with_capacity(nkept);
    let mut total = 0usize;
    for _ in 0..nkept {
        let key = usize::try_from(r.u32()?).ok().filter(|k| *k < keys.len())?;
        if kept.last().is_some_and(|p| p.key >= key) {
            return None;
        }
        let entry_count = r.u32()?;
        // An entry: kind (1), index (4), name length (2), a name of at least `.nif` (4).
        let n = r.count(11)?;
        // Review M2: all archives together hold at most MAX_ENTRIES entries.
        total = total.checked_add(n).filter(|t| *t <= MAX_ENTRIES)?;
        if n == 0 {
            return None;
        }
        let mut entries: Vec<(u32, String, u8)> = Vec::with_capacity(n);
        for _ in 0..n {
            let kind = r.u8().filter(|k| *k == KIND_NIF || *k == KIND_MESH)?;
            let index = r.u32().filter(|i| *i < entry_count)?;
            if entries.last().is_some_and(|p| p.0 >= index) {
                return None;
            }
            // A name the writer would have kept: non-empty, of the kind it is stored as.
            let name = r.name().filter(|n| kind_of(n) == Some(kind))?;
            entries.push((index, name, kind));
        }
        kept.push(StoredArchive {
            key,
            entry_count,
            entries,
        });
    }
    (r.left() == 0).then_some(Stored {
        data_dir,
        keys,
        kept,
    })
}

/// The cache file's bytes, checksum last; `None` when a name or a count does not fit.
pub(super) fn encode_stored(s: &Stored) -> Option<Vec<u8>> {
    fn text(out: &mut Vec<u8>, n: &str, max: usize) -> Option<()> {
        let len = u16::try_from(n.len())
            .ok()
            .filter(|l| usize::from(*l) <= max)?;
        out.extend(len.to_le_bytes());
        out.extend(n.as_bytes());
        Some(())
    }
    let mut out = CACHE_MAGIC.to_vec();
    text(&mut out, &s.data_dir, MAX_PATH)?;
    out.extend(u32::try_from(s.keys.len()).ok()?.to_le_bytes());
    for k in &s.keys {
        text(&mut out, &k.name, MAX_NAME)?;
        out.extend(k.len.to_le_bytes());
        out.extend(k.secs.to_le_bytes());
        out.extend(k.nanos.to_le_bytes());
    }
    out.extend(u32::try_from(s.kept.len()).ok()?.to_le_bytes());
    for a in &s.kept {
        out.extend(u32::try_from(a.key).ok()?.to_le_bytes());
        out.extend(a.entry_count.to_le_bytes());
        out.extend(u32::try_from(a.entries.len()).ok()?.to_le_bytes());
        for (index, n, kind) in &a.entries {
            out.push(*kind);
            out.extend(index.to_le_bytes());
            text(&mut out, n, MAX_NAME)?;
        }
    }
    let sum = fnv1a(&out);
    out.extend(sum.to_le_bytes());
    Some(out)
}

/// The Data dir as the cache file stores and compares it (lowercased: Windows paths
/// ignore case).
fn data_dir_text(data_dir: &Path) -> String {
    data_dir.to_string_lossy().to_lowercase()
}

/// The cache file for `data_dir` in `cache_dir`: one per Data dir (its path hashed).
pub(super) fn cache_file(cache_dir: &Path, data_dir: &Path) -> PathBuf {
    let h = fnv1a(data_dir_text(data_dir).as_bytes());
    cache_dir.join(format!("mesh-index-v2-{h:016x}.bin"))
}

/// Writes the cache file through a temporary file and a rename, so a reader never sees
/// half a file; any error leaves the cache as it was (it is only an optimization).
fn write_cache_file(file: &Path, bytes: &[u8]) {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let Some(dir) = file.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let tmp = file.with_extension(format!("tmp{}-{serial}", std::process::id()));
    if std::fs::write(&tmp, bytes).is_err() || std::fs::rename(&tmp, file).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// The index-cache folder the mesh index uses, or none when `no_cache` is set.
pub(super) fn mesh_cache_dir(args: &Value) -> Option<PathBuf> {
    if args.get("no_cache").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    IndexCache::default_location().map(|c| c.dir().to_path_buf())
}

/// The holder [`rank`] prefers: (archive index, entry index).
fn best_holder(index: &MeshIndex, holders: &[(usize, usize)]) -> Option<(usize, usize)> {
    holders
        .iter()
        .enumerate()
        .max_by_key(|(order, (a, _))| {
            let name = index.archives.get(*a).map_or("", |k| k.name.as_str());
            (rank(name), *order)
        })
        .map(|(_, h)| *h)
}

/// Which archive's copy is read when several hold a model: a plugin's own archive
/// (`<Plugin> - *.ba2`, not `Starfield - *`) over the base game's, a base `*Patch*`
/// archive over the other base ones, otherwise the last by name. The game's own archive
/// order is not modelled; the result names every archive that holds the model.
pub(crate) fn rank(archive: &str) -> u8 {
    let lower = archive.to_ascii_lowercase();
    if !lower.starts_with("starfield - ") {
        2
    } else if lower.contains("patch") {
        1
    } else {
        0
    }
}

/// `meshes/<model>`, normalized as archive entry names are, from a model path as a `MODL`
/// names it (relative to `Meshes`) or one that already starts with `Meshes\`. Refused
/// before any file is opened (review M1): an empty path, a leading separator, a drive or
/// device prefix (`:` or `?` in a segment), and empty, `.` or `..` segments, so the path
/// can only name a file under `Data\Meshes` or an archive entry under `meshes/`.
#[cfg(test)]
pub(crate) fn entry_path(model: &str) -> Result<String, EngineError> {
    entry_path_in(ID, model)
}

/// [`entry_path`], its errors naming command `id`.
fn entry_path_in(id: &str, model: &str) -> Result<String, EngineError> {
    let refuse = || {
        failed(
            id,
            format!(
                "model `{model}` is not a path inside Meshes (relative, no `.` or `..` segments, no drive)"
            ),
        )
    };
    let n = kitkraft_ba2::normalize(model);
    let bad = |s: &str| s.is_empty() || s == "." || s == ".." || s.contains([':', '?']);
    if n.split('/').any(bad) {
        return Err(refuse());
    }
    Ok(if n.starts_with("meshes/") {
        n
    } else {
        format!("meshes/{n}")
    })
}

/// What a base object names as its model.
struct BaseModel {
    model: String,
    form: String,
    signature: Option<String>,
    editor_id: Option<String>,
    evidence: Evidence,
}

/// The base's `MODL` text and its evidence.
fn model_of_form(
    session: &mut Session,
    args: &Value,
    form_arg: &str,
    id: &str,
) -> Result<BaseModel, EngineError> {
    let loaded = acquire(session, args, id)?;
    let plugin = args.get("plugin").and_then(Value::as_str);
    let (form, _) = find_form(id, loaded, form_arg, plugin)?;
    let lo = &loaded.lo;
    let label = lo.form_label(form);
    let (signature, editor_id) = identity(lo, form);
    let v = lo
        .forms
        .winner(form)
        .copied()
        .ok_or_else(|| failed(id, format!("{label} has no version in the load order")))?;
    let loc = lo
        .record(v.plugin, v.record)
        .copied()
        .ok_or_else(|| failed(id, format!("{label}: version missing from the index")))?;
    let rec = RecordReader::new()
        .read(lo, v.plugin, &loc)
        .map_err(|e| failed(id, format!("{label}: {e}")))?;
    let modl = rec
        .subs
        .iter()
        .find(|s| s.signature().bytes() == *b"MODL")
        .ok_or_else(|| {
            failed(
                id,
                format!(
                    "{label} ({}) has no MODL: no model to read",
                    signature.as_deref().unwrap_or("?")
                ),
            )
        })?;
    let model = String::from_utf8_lossy(modl.data())
        .trim_end_matches('\0')
        .to_string();
    if model.is_empty() {
        return Err(failed(id, format!("{label}: its MODL is empty")));
    }
    // A compressed record's subrecords carry no file offsets: point at the record then.
    let evidence = match modl.data_offset() {
        Some(offset) => Evidence {
            file: rec.file.clone(),
            offset,
            length: modl.data().len() as u64,
            record: Some(rec.label()),
            subrecord: Some("MODL".to_string()),
        },
        None => Evidence {
            file: rec.file.clone(),
            offset: loc.offset,
            length: kitkraft_esm::HEADER_LEN as u64,
            record: Some(rec.label()),
            subrecord: None,
        },
    };
    Ok(BaseModel {
        model,
        form: label,
        signature,
        editor_id,
        evidence,
    })
}

/// What `mesh.bounds` and `mesh.geometry` share: the model found, read, and named.
struct ModelRead {
    model: String,
    base: Option<BaseModel>,
    bytes: Vec<u8>,
    /// `<archive>/<entry>` or the loose path, for evidence.
    file: String,
    source: Value,
    also_in: Vec<String>,
    limitations: Vec<String>,
    evidence: Vec<Evidence>,
}

impl ModelRead {
    /// The base's form label, signature and EditorID, when a `form` was given.
    fn form(&self) -> Option<&str> {
        self.base.as_ref().map(|b| b.form.as_str())
    }
}

/// Finds the model a `model` path or a base `form` names and reads it from the loose
/// files or the archives (errors name command `id`).
fn read_model(session: &mut Session, args: &Value, id: &str) -> Result<ModelRead, EngineError> {
    let model_arg = args.get("model").and_then(Value::as_str);
    let form_arg = args.get("form").and_then(Value::as_str);
    let mut evidence = Vec::new();
    let mut limitations = Vec::new();
    let base = match (model_arg, form_arg) {
        (Some(_), Some(_)) => return Err(failed(id, "pass `model` or `form`, not both")),
        (None, None) => {
            return Err(failed(
                id,
                "pass `model` (a path as a MODL names it) or `form` (a base object)",
            ))
        }
        (Some(_), None) => None,
        (None, Some(f)) => Some(model_of_form(session, args, f, id)?),
    };
    let model = match (&base, model_arg) {
        (Some(b), _) => b.model.clone(),
        (None, Some(m)) => m.to_string(),
        (None, None) => String::new(),
    };
    if let Some(b) = &base {
        evidence.push(b.evidence.clone());
    }

    let (data_dir, _) = data_dir_source(args)
        .ok_or_else(|| failed(id, "no Data dir: pass `data_dir` or set KITKRAFT_DATA_DIR"))?;
    let wanted = entry_path_in(id, &model)?;
    let rel: PathBuf = wanted.split('/').collect();
    // `wanted` is lowercased and the loose tree is not (`Meshes\...`): matched without
    // case, as Windows does, so a case-sensitive file system finds the file too.
    let loose = kitkraft_io::overlay::find_ci(&data_dir, &wanted);

    let cache_dir = mesh_cache_dir(args);
    if session
        .get::<MeshIndex>()
        .is_none_or(|i| i.data_dir != data_dir)
    {
        let built = MeshIndex::build_cached(&data_dir, cache_dir.as_deref(), true, id)?;
        session.insert(built);
    }
    // M7.R5: an index read from the cache whose archive for this model no longer matches
    // it is built again from the archives (once), and the call says so.
    // So is one that holds no copy of the model at all: a miss is never final on the
    // file's word (review M1 of M7.R5).
    let stale = session.get::<MeshIndex>().and_then(|index| {
        if loose.is_some() || !index.from_cache {
            return None;
        }
        let why = match index
            .by_path
            .get(&wanted)
            .and_then(|h| best_holder(index, h))
        {
            None => format!("names no archive for {wanted}"),
            Some((a, _)) => {
                let kept = index.archives.get(a)?;
                if kept.open().is_some() {
                    return None;
                }
                format!("does not match the archive {}", kept.name)
            }
        };
        Some((why, index.cache_dir.clone()))
    });
    if let Some((why, index_cache)) = stale {
        limitations.push(format!(
            "the cached mesh index {why}: the index was built again from the archives"
        ));
        // Written back where the stale index came from (review H1 of M7.R5).
        let rewrite = cache_dir.as_ref().and(index_cache);
        let built = MeshIndex::build_cached(&data_dir, rewrite.as_deref(), false, id)?;
        session.insert(built);
    }
    let index = session
        .get::<MeshIndex>()
        .ok_or_else(|| failed(id, "the archive index did not build"))?;
    if !index.unreadable.is_empty() {
        limitations.push(format!(
            "{} archive(s) of the Data dir did not open and were not searched: {}",
            index.unreadable.len(),
            index.unreadable.join("; ")
        ));
    }
    let holders: Vec<(usize, usize)> = index.by_path.get(&wanted).cloned().unwrap_or_default();
    let also_in: Vec<String> = holders
        .iter()
        .filter_map(|(a, _)| index.archives.get(*a).map(|k| k.name.clone()))
        .collect();

    let (bytes, file, source) = if let Some(loose) = loose {
        let len = std::fs::metadata(&loose).map_or(u64::MAX, |m| m.len());
        if len > MAX_MODEL_BYTES {
            return Err(failed(
                id,
                format!(
                    "{}: {len} bytes, above the {MAX_MODEL_BYTES}-byte cap",
                    loose.display()
                ),
            ));
        }
        let bytes =
            std::fs::read(&loose).map_err(|e| failed(id, format!("{}: {e}", loose.display())))?;
        let shown = rel.display().to_string();
        let source = json!({"kind": "loose", "file": shown, "archive": null});
        if !also_in.is_empty() {
            limitations.push(format!(
                "read the loose file; {} archive(s) also hold this model ({}); loose over archives is our rule (the game's order is not modelled)",
                also_in.len(),
                also_in.join(", ")
            ));
        }
        (bytes, shown, source)
    } else {
        let Some((a, e)) = best_holder(index, &holders) else {
            return Err(failed(
                id,
                format!(
                    "model `{model}` ({wanted}) is in neither the loose files nor the archives of {}",
                    data_dir.display()
                ),
            ));
        };
        let Some(kept) = index.archives.get(a) else {
            return Err(failed(id, "the archive index is inconsistent"));
        };
        let name = &kept.name;
        let Some(archive) = kept.open() else {
            return Err(failed(
                id,
                format!("{name}: the archive does not open or no longer matches its index"),
            ));
        };
        let mut f = File::open(&kept.path).map_err(|err| failed(id, format!("{name}: {err}")))?;
        let bytes = archive
            .extract_vec(&mut f, e, MAX_MODEL_BYTES)
            .map_err(|err| failed(id, format!("{name}: {err}")))?;
        let entry = archive
            .entries
            .get(e)
            .map_or_else(|| wanted.clone(), |x| x.name.clone());
        if also_in.len() > 1 {
            limitations.push(format!(
                "{} archives hold this model ({}); read {name}'s copy: a plugin's own archive over the base game's, a base Patch archive over the other base ones, ties by archive name (our rule: the game's own archive order is not modelled); every archive in the Data folder is searched, whether or not its plugin is in a load order",
                also_in.len(),
                also_in.join(", ")
            ));
        }
        let source = json!({"kind": "archive", "file": entry, "archive": name});
        (bytes, format!("{name}/{entry}"), source)
    };

    Ok(ModelRead {
        model,
        base,
        bytes,
        file,
        source,
        also_in,
        limitations,
        evidence,
    })
}

/// `mesh.bounds`.
pub(super) fn bounds(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let ModelRead {
        model,
        base,
        bytes,
        file,
        source,
        also_in,
        mut limitations,
        mut evidence,
    } = read_model(session, args, ID)?;
    let nif = Nif::parse(&bytes).map_err(|e| failed(ID, format!("{file}: {e}")))?;
    let mb = nif
        .model_bounds()
        .map_err(|e| failed(ID, format!("{file}: {e}")))?;
    let [stored, sphere_only, not_stored, unrecognised] = mb.counts();
    let total = mb.geometries.len();
    for g in mb.geometries.iter().take(MAX_EVIDENCE) {
        if matches!(g.bound, Bound::Stored { .. } | Bound::SphereOnly(_)) {
            evidence.push(Evidence {
                file: file.clone(),
                offset: g.bound_offset as u64,
                length: 40,
                record: None,
                subrecord: Some(format!("block {} BSGeometry bound", g.block)),
            });
        }
    }
    if total > MAX_EVIDENCE {
        limitations.push(format!(
            "evidence lists the first {MAX_EVIDENCE} of {total} geometry blocks"
        ));
    }
    if total == 0 {
        limitations.push("no bound in the model: it has no geometry block (study C1: 19,436 of the 120,547 official models)".to_string());
    } else if mb.aabb.is_none() && not_stored > 0 && unrecognised == 0 {
        limitations.push(format!(
            "no bound in the model: all {total} geometry block(s) are skinned, and skinned geometry stores none (study B5; 5,830 of the official models)"
        ));
    } else if not_stored > 0 {
        limitations.push(format!(
            "{not_stored} skinned geometry block(s) store no bound and are not in the box (study B5)"
        ));
    }
    if sphere_only > 0 {
        limitations.push(format!(
            "{sphere_only} geometry block(s) store only a sphere; each adds its sphere's enclosing box (study B5)"
        ));
    }
    if unrecognised > 0 {
        limitations.push(format!(
            "{unrecognised} geometry block(s) hold bound values outside the study's three patterns (no official model has them): left out of the box"
        ));
    }
    if mb.unreached_geometries > 0 || mb.revisits > 0 {
        limitations.push(format!(
            "{} geometry block(s) are not reached from the root and {} child reference(s) repeat (none in the official models): the box covers the reached ones only",
            mb.unreached_geometries, mb.revisits
        ));
    }
    if mb.box_overflow {
        limitations
            .push("the box overflows f32 (the placements are huge): no box is given".to_string());
    }
    limitations.push(OBND_NOTE.to_string());

    let bounds = mb.aabb.map(|b| json!({"min": b.min, "max": b.max}));
    let summary = match &mb.aabb {
        Some(b) => format!(
            "{model}: box {:?} to {:?} from {stored} stored and {sphere_only} sphere-only bound(s) of {total} geometry block(s)",
            b.min, b.max
        ),
        None => format!("{model}: no bound ({total} geometry block(s))"),
    };
    let complete = mb.aabb.is_some() && unrecognised == 0 && not_stored == 0;
    let (form, base_signature, base_editor_id) = match base {
        Some(b) => (Some(b.form), b.signature, b.editor_id),
        None => (None, None, None),
    };
    Ok(Outcome {
        data: json!({
            "model": model,
            "form": form,
            "base_signature": base_signature,
            "base_editor_id": base_editor_id,
            "source": source,
            "also_in": also_in,
            "bounds": bounds,
            "frame": "model root",
            "geometries": {
                "total": total,
                "stored": stored,
                "sphere_only": sphere_only,
                "not_stored": not_stored,
                "unrecognised": unrecognised,
                "unreached": mb.unreached_geometries,
            },
            "summary": summary,
        }),
        coverage: Coverage {
            status: if complete {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: vec!["geometry vertex data (.mesh files, M7.S5)".to_string()],
        },
        limitations,
        evidence,
    })
}

/// `mesh.geometry`'s id.
const GEOMETRY_ID: &str = "mesh.geometry";

/// Most vertices one `mesh.geometry` reply carries; a mesh that would pass it is skipped
/// with a limitation line (V5). The largest official model's first slots hold fewer.
pub(crate) const MAX_GEOMETRY_VERTICES: usize = 2_000_000;

/// Most indices one reply carries (review M2): with [`MAX_GEOMETRY_VERTICES`] this bounds
/// the raw buffers at 72 MB (24 bytes a vertex, 4 an index). The largest official model's
/// first slots hold 3,647,352 indices and 1,485,580 vertices (study test, 2026-11-02).
pub(crate) const MAX_GEOMETRY_INDICES: usize = 6_000_000;

/// Largest `.mesh` read, in bytes (the largest official one is under 64 MiB).
const MAX_MESH_BYTES: u64 = 256 << 20;

/// Standard base64 (RFC 4648 alphabet, `=` padding) of `bytes`.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let sym = |v: u32| char::from(ALPHABET.get((v & 63) as usize).copied().unwrap_or(b'A'));
    let mut out = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for chunk in bytes.chunks(3) {
        let b = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let n = (b(0) << 16) | (b(1) << 8) | b(2);
        out.push(sym(n >> 18));
        out.push(sym(n >> 12));
        out.push(if chunk.len() > 1 { sym(n >> 6) } else { '=' });
        out.push(if chunk.len() > 2 { sym(n) } else { '=' });
    }
    out
}

/// Applies a 3x4 row-major affine map to a point.
fn place(m: &[[f64; 4]; 3], p: [f32; 3]) -> [f32; 3] {
    let p = p.map(f64::from);
    m.map(|r| (r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + r[3]) as f32)
}

/// Turns a normal by a map's 3x3 part and renormalizes it (a zero vector stays zero).
fn turn(m: &[[f64; 4]; 3], n: [f32; 3]) -> [f32; 3] {
    let n = n.map(f64::from);
    let v = m.map(|r| r[0] * n[0] + r[1] * n[1] + r[2] * n[2]);
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 0.0 {
        v.map(|x| (x / len) as f32)
    } else {
        [0.0; 3]
    }
}

/// `mesh.geometry` (M7.S5): the model's triangles in its root frame, as base64
/// little-endian buffers (the director's geometry-encoding decision, `plan/execution-plan.md` "M6" decisions item 2 and the M7.S5 row; owner-confirmed 2026-11-01): `positions` and `normals` `f32` x, y, z
/// per vertex, `indices` `u32` per corner, three per triangle. Each reached geometry
/// block's first slot is read (claims G1 and G2 of
/// `docs/research/mesh-layout-2026-11-02.md`), placed by the block's own transform and
/// its ancestors' (claim C1 of the NIF study). Normals are grade B (claim M4).
///
/// Dispatched once the catalog entry is `Implemented` and `handlers.rs` has its row (the
/// integrator's A8 seam commit); `mesh_tests.rs` `mesh_geometry_through_the_catalog` holds
/// in both states.
pub(super) fn geometry(session: &mut Session, args: &Value) -> Result<Outcome, EngineError> {
    let started = std::time::Instant::now();
    let r = read_model(session, args, GEOMETRY_ID)?;
    let form = r.form().map(str::to_string);
    let mut limitations = r.limitations;
    let mut evidence = r.evidence;
    let nif = Nif::parse(&r.bytes).map_err(|e| failed(GEOMETRY_ID, format!("{}: {e}", r.file)))?;
    let mb = nif
        .model_bounds()
        .map_err(|e| failed(GEOMETRY_ID, format!("{}: {e}", r.file)))?;
    let no_cache = args.get("no_cache").and_then(Value::as_bool) == Some(true);
    let index = session
        .get_mut::<MeshIndex>()
        .ok_or_else(|| failed(GEOMETRY_ID, "the archive index did not build"))?;
    let mut positions: Vec<u8> = Vec::new();
    let mut normals: Vec<u8> = Vec::new();
    let mut indices: Vec<u8> = Vec::new();
    let mut vertices = 0usize;
    let mut index_total = 0usize;
    let mut parts = Vec::new();
    let mut skipped = 0usize;
    for g in &mb.geometries {
        let block = usize::try_from(g.block).unwrap_or(usize::MAX);
        // A skinned block stores no box (NIF study B5), so the mesh study's box check
        // (M2) does not show the frame of its positions: skipped, not guessed (review M1).
        if matches!(g.bound, Bound::NotStored | Bound::Unrecognised) {
            limitations.push(format!(
                "block {block} is skinned or stores no recognised bound (NIF study B5): the mesh study does not show the frame of its positions, so its mesh is skipped"
            ));
            skipped = skipped.saturating_add(1);
            continue;
        }
        let slots = match nif.slots(block) {
            Ok(s) => s.unwrap_or_default(),
            Err(e) => {
                limitations.push(format!(
                    "block {block}: its slots do not read ({e}): skipped"
                ));
                skipped = skipped.saturating_add(1);
                continue;
            }
        };
        let Some(first) = slots.into_iter().find(|s| s.slot == 0) else {
            limitations.push(format!("block {block}: no first slot: no mesh read"));
            skipped = skipped.saturating_add(1);
            continue;
        };
        let Some(entry) = first.mesh_entry() else {
            limitations.push(format!(
                "block {block}: the first slot names no mesh file (geometry kept inside the model, not decoded)"
            ));
            skipped = skipped.saturating_add(1);
            continue;
        };
        let nv = usize::try_from(first.vertex_count).unwrap_or(usize::MAX);
        let ni = usize::try_from(first.index_count).unwrap_or(usize::MAX);
        if index_total.saturating_add(ni) > MAX_GEOMETRY_INDICES {
            limitations.push(format!(
                "block {block}: {entry} skipped: its {ni} indices would pass the {MAX_GEOMETRY_INDICES}-index budget"
            ));
            skipped = skipped.saturating_add(1);
            continue;
        }
        if vertices.saturating_add(nv) > MAX_GEOMETRY_VERTICES {
            limitations.push(format!(
                "block {block}: {entry} skipped: its {nv} vertices would pass the {MAX_GEOMETRY_VERTICES}-vertex budget"
            ));
            skipped = skipped.saturating_add(1);
            continue;
        }
        let mut got = index.read_mesh(&entry, MAX_MESH_BYTES);
        // M7.R5 (review H1): an archive that no longer matches the cached index: the
        // index is built again from the archives at once (which rewrites the cache file),
        // and the read retried.
        let rebuild = match &got {
            Err(MeshMiss::Stale(archive)) => Some(format!("does not match the archive {archive}")),
            // Review M1: a mesh the cached index holds nowhere is not refused on its word.
            Err(MeshMiss::Absent) if index.from_cache => {
                Some(format!("names no archive for {entry}"))
            }
            _ => None,
        };
        if let Some(why) = rebuild {
            limitations.push(format!(
                "the cached mesh index {why}: the index was built again from the archives"
            ));
            let cache = if no_cache {
                None
            } else {
                index.cache_dir.clone()
            };
            let data_dir = index.data_dir.clone();
            *index = MeshIndex::build_cached(&data_dir, cache.as_deref(), false, GEOMETRY_ID)?;
            got = index.read_mesh(&entry, MAX_MESH_BYTES);
        }
        let (name, bytes) = match got {
            Ok(read) => read,
            Err(miss) => {
                limitations.push(match miss {
                    MeshMiss::Absent => {
                        format!("block {block}: {entry} is in no archive of the Data dir: skipped")
                    }
                    MeshMiss::Stale(a) | MeshMiss::Unread(a) => {
                        format!("block {block}: {entry} in {a} does not read: skipped")
                    }
                });
                skipped = skipped.saturating_add(1);
                continue;
            }
        };
        let mesh = match Mesh::parse(&bytes) {
            Ok(m) => m,
            Err(e) => {
                limitations.push(format!("block {block}: {name}/{entry}: {e}: skipped"));
                skipped = skipped.saturating_add(1);
                continue;
            }
        };
        if mesh.indices.len() != first.index_count as usize || mesh.vertex_count() != nv {
            limitations.push(format!(
                "block {block}: {entry}: the slot's counts ({}, {nv}) are not the mesh's ({}, {}): skipped",
                first.index_count,
                mesh.indices.len(),
                mesh.vertex_count()
            ));
            skipped = skipped.saturating_add(1);
            continue;
        }
        let base = u32::try_from(vertices).unwrap_or(u32::MAX);
        for p in mesh.positions() {
            for v in place(&g.to_root, p) {
                positions.extend_from_slice(&v.to_le_bytes());
            }
        }
        for n in mesh.normals() {
            for v in turn(&g.to_root, n) {
                normals.extend_from_slice(&v.to_le_bytes());
            }
        }
        for i in &mesh.indices {
            indices.extend_from_slice(&base.saturating_add(u32::from(*i)).to_le_bytes());
        }
        evidence.push(Evidence {
            file: format!("{name}/{entry}"),
            offset: mesh.offsets.positions as u64,
            length: (nv as u64).saturating_mul(6),
            record: None,
            subrecord: Some("positions".to_string()),
        });
        parts.push(json!({
            "block": g.block,
            "mesh": entry,
            "archive": name,
            "vertex_offset": vertices,
            "vertex_count": nv,
            "index_count": mesh.indices.len(),
        }));
        vertices = vertices.saturating_add(nv);
        index_total = index_total.saturating_add(mesh.indices.len());
    }
    if mb.geometries.is_empty() {
        limitations.push("the model has no geometry block: no triangles".to_string());
    }
    if mb.unreached_geometries > 0 || mb.revisits > 0 {
        limitations.push(format!(
            "{} geometry block(s) are not reached from the root and {} child reference(s) repeat (none in the official models): their meshes are not read",
            mb.unreached_geometries, mb.revisits
        ));
    }
    if skipped > 0 {
        limitations.push(format!(
            "{skipped} geometry block(s) skipped, each named above"
        ));
    }
    limitations.push("normals are grade B (docs/research/mesh-layout-2026-11-02.md, M4: 99.86% unit length and 98.8% facing their triangles over the official meshes, not proven per vertex)".to_string());
    limitations.push("only each geometry block's first slot is read; the other three slots name further meshes of the block whose use is not shown".to_string());
    let triangles = indices.len() / 12;
    let envelope_bytes = positions
        .len()
        .saturating_add(normals.len())
        .saturating_add(indices.len());
    let summary = format!(
        "{}: {vertices} vertices, {triangles} triangles from {} mesh(es), {skipped} skipped; {} ms",
        r.model,
        parts.len(),
        started.elapsed().as_millis()
    );
    Ok(Outcome {
        data: json!({
            "model": r.model,
            "form": form,
            "source": r.source,
            "frame": "model root",
            "encoding": "base64 little-endian: positions and normals f32 x, y, z per vertex; indices u32, three per triangle",
            "vertex_count": vertices,
            "index_count": indices.len() / 4,
            "raw_bytes": envelope_bytes,
            "meshes": parts,
            "positions": base64(&positions),
            "normals": base64(&normals),
            "indices": base64(&indices),
            "summary": summary,
        }),
        coverage: Coverage {
            status: if skipped == 0
                && !mb.geometries.is_empty()
                && mb.unreached_geometries == 0
                && mb.revisits == 0
            {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            decoded_subrecords: None,
            opaque_subrecords: None,
            not_examined: vec!["the mesh sections and entries the study names by place".to_string()],
        },
        limitations,
        evidence,
    })
}
