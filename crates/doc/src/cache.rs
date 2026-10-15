//! On-disk cache of [`PluginIndex`]es.
//!
//! Location: `%LOCALAPPDATA%\kitkraft\cache` (override with `KITKRAFT_CACHE_DIR`, or pass a
//! directory). Never inside the game Data dir or an MO2 profile. One file per plugin path,
//! named by a hash of the path.
//!
//! **Staleness rule.** An entry is used only when all of these hold, else it is ignored and
//! rebuilt, never trusted:
//!
//! 1. its checksum verifies, and its [`FORMAT_VERSION`] and esm index format version match;
//! 2. its stored absolute path equals the plugin's;
//! 3. the plugin's current size and modification time (nanoseconds since the Unix epoch, as
//!    the file system reports it) equal the stored ones;
//! 4. a content sample equals the stored one: FNV-1a over the file's size and its first and
//!    last [`SAMPLE_BYTES`] (the TES4 header, with masters and flags, is in the first part).
//!
//! A file whose modification time is unknown or 0 is never cached (an edit could keep it).
//! An edit that keeps size and mtime and touches only bytes outside both sampled ends is
//! not detected; tools that write plugins update the mtime.
//!
//! Layout (little-endian): magic `KCIDX\0\r\n`, `u32` format version, `u32` esm index
//! format version, `u32` path length +
//! path (UTF-8), `u64` size, `u64` mtime (ns), `u64` content sample, `u32` flags, `u16` form version, `u8` HEDR
//! present + `f32`/`u32`/`u32`, `u32` master count + (`u32` length + name) each, `u64` edid
//! errors, `u64` record count + 32 bytes per record, `u64` EDID arena length + bytes, `u64`
//! FNV-1a checksum of everything before it.

use std::io::Write;
use std::path::{Path, PathBuf};

use kitkraft_esm::Reader;

use crate::error::{DocError, Result};
use crate::flags::PluginFlags;
use crate::hash::Fnv;
use crate::index::{file_stamp, HeaderInfo, HedrInfo, PluginIndex, RecordLoc};

/// Bump when this layout changes. Entries also store the esm index format
/// (`kitkraft_esm::index::INDEX_FORMAT_VERSION`); either changing rebuilds them.
pub const FORMAT_VERSION: u32 = 3;

/// Bytes sampled at each end of a plugin for the staleness check.
pub const SAMPLE_BYTES: u64 = 64 * 1024;

const MAGIC: &[u8; 8] = b"KCIDX\0\r\n";
const RECORD_BYTES: usize = 32;

/// A temp file of another process unchanged this long is a leftover, not a write in
/// progress (a store writes and renames one entry in well under a second).
const STALE_TEMP_AGE: std::time::Duration = std::time::Duration::from_secs(120);

/// FNV-1a over the file's size, its first [`SAMPLE_BYTES`] and its last [`SAMPLE_BYTES`].
pub fn content_sample(path: &Path) -> Result<u64> {
    use std::io::{Read, Seek, SeekFrom};
    let io = |e| DocError::io(path, e);
    let mut file = std::fs::File::open(path).map_err(io)?;
    let len = file.metadata().map_err(io)?.len();
    let mut h = Fnv::new();
    h.write_u64(len);
    let mut buf = Vec::new();
    (&mut file)
        .take(SAMPLE_BYTES)
        .read_to_end(&mut buf)
        .map_err(io)?;
    h.write(&buf);
    buf.clear();
    file.seek(SeekFrom::Start(len.saturating_sub(SAMPLE_BYTES)))
        .map_err(io)?;
    file.take(SAMPLE_BYTES).read_to_end(&mut buf).map_err(io)?;
    h.write(&buf);
    Ok(h.finish())
}

/// Where cache entries live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexCache {
    dir: PathBuf,
}

/// How an index was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheUse {
    /// Read from a valid cache entry.
    Hit,
    /// Built, and the entry written (or rewritten).
    Built,
    /// Built; no cache was configured or writing it failed.
    Uncached,
}

impl IndexCache {
    /// A cache in `dir` (created on first write).
    pub fn new(dir: impl Into<PathBuf>) -> IndexCache {
        IndexCache { dir: dir.into() }
    }

    /// `KITKRAFT_CACHE_DIR`, else `%LOCALAPPDATA%\kitkraft\cache`, else none.
    /// `KITCRAFT_CACHE_DIR` (the pre-rename spelling) is still read after
    /// `KITKRAFT_CACHE_DIR`; deprecated, kept for one release.
    ///
    /// The first call in a process also removes stale temp files a killed process left
    /// there ([`IndexCache::sweep_stale_temps`]).
    pub fn default_location() -> Option<IndexCache> {
        static SWEPT: std::sync::Once = std::sync::Once::new();
        let named = std::env::var_os("KITKRAFT_CACHE_DIR")
            .filter(|d| !d.is_empty())
            .or_else(|| std::env::var_os("KITCRAFT_CACHE_DIR").filter(|d| !d.is_empty()));
        let cache = match named {
            Some(dir) => IndexCache::new(PathBuf::from(dir)),
            None => {
                let local = std::env::var_os("LOCALAPPDATA").filter(|d| !d.is_empty())?;
                IndexCache::new(PathBuf::from(local).join("kitkraft").join("cache"))
            }
        };
        SWEPT.call_once(|| {
            let _ = cache.sweep_stale_temps(STALE_TEMP_AGE);
        });
        Some(cache)
    }

    /// Removes `<entry hash>.tmp<pid>` files ([`IndexCache::store`] writes one, then renames
    /// it) that another process left and that have not changed for `min_age`: a process
    /// killed mid-store (a window closed during a long `load`) never renames or removes
    /// its temp file. This process's own temp files and recent ones (another process may
    /// be writing them right now) are kept. Returns how many were removed.
    pub fn sweep_stale_temps(&self, min_age: std::time::Duration) -> usize {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return 0;
        };
        let own = format!("tmp{}", std::process::id());
        let now = std::time::SystemTime::now();
        let mut removed = 0usize;
        for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            let is_temp = ext
                .strip_prefix("tmp")
                .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()));
            // `<16 hex digits>.tmp<pid>`: an entry's name with its extension replaced.
            let is_entry = path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit()));
            if !is_temp || !is_entry || ext == own {
                continue;
            }
            let old = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| now.duration_since(t).ok())
                .is_some_and(|age| age >= min_age);
            if old && std::fs::remove_file(&path).is_ok() {
                removed = removed.saturating_add(1);
            }
        }
        removed
    }

    /// The directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Entry file for a plugin path.
    pub fn entry_path(&self, plugin: &Path) -> PathBuf {
        let key = path_key(plugin);
        let mut h = Fnv::new();
        h.write(key.as_bytes());
        self.dir.join(format!("{:016x}.kcidx", h.finish()))
    }

    /// A valid cached index for `plugin`, if there is one.
    pub fn load(&self, plugin: &Path) -> Option<PluginIndex> {
        let stamp = file_stamp(plugin).ok()?;
        if stamp.1 == 0 {
            return None;
        }
        let bytes = std::fs::read(self.entry_path(plugin)).ok()?;
        let sample = content_sample(plugin).ok()?;
        decode(&bytes, plugin, stamp, sample).ok()
    }

    /// Write `index` as the entry for its path (atomically: temp file, then rename). A
    /// plugin whose modification time is unknown or 0 is refused (module docs).
    pub fn store(&self, index: &PluginIndex) -> Result<()> {
        if index.mtime_ns == 0 {
            return Err(DocError::Invalid(format!(
                "{}: modification time unknown or 0; not cached",
                index.path.display()
            )));
        }
        let sample = content_sample(&index.path)?;
        std::fs::create_dir_all(&self.dir).map_err(|e| DocError::io(&self.dir, e))?;
        let target = self.entry_path(&index.path);
        let tmp = target.with_extension(format!("tmp{}", std::process::id()));
        let bytes = encode(index, sample);
        let written = std::fs::File::create(&tmp)
            .and_then(|mut f| f.write_all(&bytes).and_then(|()| f.sync_all()));
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(DocError::io(&tmp, e));
        }
        std::fs::rename(&tmp, &target).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            DocError::io(&target, e)
        })
    }

    /// The cached index of `plugin`, or a fresh one (then stored).
    pub fn get_or_build(&self, plugin: &Path) -> Result<(PluginIndex, CacheUse)> {
        if let Some(index) = self.load(plugin) {
            return Ok((index, CacheUse::Hit));
        }
        let index = PluginIndex::build(plugin)?;
        let used = match self.store(&index) {
            Ok(()) => CacheUse::Built,
            Err(_) => CacheUse::Uncached,
        };
        Ok((index, used))
    }
}

/// The path as stored in, and compared with, an entry: absolute when it can be made so.
fn path_key(path: &Path) -> String {
    std::path::absolute(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Names and paths are far below 4 GiB; a longer one would make the entry undecodable,
/// which only costs a rebuild.
fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, u32::try_from(s.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(s.as_bytes());
}

fn encode(index: &PluginIndex, sample: u64) -> Vec<u8> {
    let records_len = index.records.len().saturating_mul(RECORD_BYTES);
    let mut out = Vec::with_capacity(
        records_len
            .saturating_add(index.edid_arena().len())
            .saturating_add(1024),
    );
    out.extend_from_slice(MAGIC);
    put_u32(&mut out, FORMAT_VERSION);
    put_u32(&mut out, kitkraft_esm::index::INDEX_FORMAT_VERSION);
    put_str(&mut out, &path_key(&index.path));
    put_u64(&mut out, index.size);
    put_u64(&mut out, index.mtime_ns);
    put_u64(&mut out, sample);
    let h = &index.header;
    put_u32(&mut out, h.flags.0);
    out.extend_from_slice(&h.form_version.to_le_bytes());
    match h.hedr {
        Some(hedr) => {
            out.push(1);
            out.extend_from_slice(&hedr.version.to_le_bytes());
            put_u32(&mut out, hedr.record_count);
            put_u32(&mut out, hedr.next_object_id);
        }
        None => {
            out.push(0);
            out.extend_from_slice(&[0u8; 12]);
        }
    }
    put_u32(&mut out, u32::try_from(h.masters.len()).unwrap_or(0));
    for m in h
        .masters
        .iter()
        .take(usize::try_from(u32::MAX).unwrap_or(0))
    {
        put_str(&mut out, m);
    }
    put_u64(&mut out, index.edid_errors);
    put_u64(&mut out, index.records.len() as u64);
    for r in &index.records {
        out.extend_from_slice(&r.signature);
        put_u32(&mut out, r.form_id);
        put_u32(&mut out, r.flags);
        put_u64(&mut out, r.offset);
        put_u32(&mut out, r.data_size);
        put_u32(&mut out, r.edid_start);
        put_u32(&mut out, r.edid_len);
    }
    put_u64(&mut out, index.edid_arena().len() as u64);
    out.extend_from_slice(index.edid_arena().as_bytes());
    let mut h = Fnv::new();
    h.write(&out);
    put_u64(&mut out, h.finish());
    out
}

fn bad(what: &str) -> DocError {
    DocError::Invalid(format!("index cache entry: {what}"))
}

fn decode(bytes: &[u8], plugin: &Path, stamp: (u64, u64), sample: u64) -> Result<PluginIndex> {
    let body_len = bytes.len().checked_sub(8).ok_or_else(|| bad("too short"))?;
    let (body, sum) = bytes.split_at(body_len);
    let mut h = Fnv::new();
    h.write(body);
    let stored = u64::from_le_bytes(sum.try_into().map_err(|_| bad("checksum"))?);
    if stored != h.finish() {
        return Err(bad("checksum mismatch"));
    }
    let e = |_| bad("truncated");
    let mut r = Reader::new(body, 0);
    if r.take(8).map_err(e)? != MAGIC {
        return Err(bad("magic"));
    }
    if r.u32().map_err(e)? != FORMAT_VERSION {
        return Err(bad("format version"));
    }
    if r.u32().map_err(e)? != kitkraft_esm::index::INDEX_FORMAT_VERSION {
        return Err(bad("esm index format version"));
    }
    let path = take_str(&mut r)?;
    if path != path_key(plugin) {
        return Err(bad("path"));
    }
    let size = r.u64().map_err(e)?;
    let mtime_ns = r.u64().map_err(e)?;
    if (size, mtime_ns) != stamp || mtime_ns == 0 {
        return Err(bad("stale"));
    }
    if r.u64().map_err(e)? != sample {
        return Err(bad("content sample differs"));
    }
    let flags = PluginFlags(r.u32().map_err(e)?);
    let form_version = r.u16().map_err(e)?;
    let has_hedr = r.u8().map_err(e)?;
    let version = r.f32().map_err(e)?;
    let record_count = r.u32().map_err(e)?;
    let next_object_id = r.u32().map_err(e)?;
    let hedr = (has_hedr == 1).then_some(HedrInfo {
        version,
        record_count,
        next_object_id,
    });
    let master_count = r.u32().map_err(e)?;
    let mut masters = Vec::new();
    for _ in 0..master_count {
        masters.push(take_str(&mut r)?);
    }
    let edid_errors = r.u64().map_err(e)?;
    let count = usize::try_from(r.u64().map_err(e)?).map_err(|_| bad("count"))?;
    let needed = count
        .checked_mul(RECORD_BYTES)
        .ok_or_else(|| bad("count"))?;
    if needed > r.remaining() {
        return Err(bad("record count past the end"));
    }
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(RecordLoc {
            signature: r.array::<4>("signature").map_err(e)?,
            form_id: r.u32().map_err(e)?,
            flags: r.u32().map_err(e)?,
            offset: r.u64().map_err(e)?,
            data_size: r.u32().map_err(e)?,
            edid_start: r.u32().map_err(e)?,
            edid_len: r.u32().map_err(e)?,
        });
    }
    let arena_len = usize::try_from(r.u64().map_err(e)?).map_err(|_| bad("arena"))?;
    let arena = r.take(arena_len).map_err(e)?;
    let edids = String::from_utf8(arena.to_vec()).map_err(|_| bad("arena utf-8"))?;
    if !r.is_empty() {
        return Err(bad("trailing bytes"));
    }
    let header = HeaderInfo {
        flags,
        form_version,
        hedr,
        masters,
    };
    let index = PluginIndex::from_parts(
        crate::index::file_name_of(plugin),
        plugin.to_path_buf(),
        stamp,
        header,
        records,
        edids,
        edid_errors,
    );
    // Every EDID range must lie inside the arena on a char boundary.
    if index
        .records
        .iter()
        .any(|loc| loc.edid_start != u32::MAX && index.edid(loc).is_none())
    {
        return Err(bad("EDID range"));
    }
    Ok(index)
}

fn take_str(r: &mut Reader<'_>) -> Result<String> {
    let len = r.u32().map_err(|_| bad("truncated"))?;
    let bytes = r
        .take(usize::try_from(len).map_err(|_| bad("length"))?)
        .map_err(|_| bad("truncated"))?;
    String::from_utf8(bytes.to_vec()).map_err(|_| bad("utf-8"))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::testdata;

    fn sample() -> (PathBuf, IndexCache) {
        let bytes = testdata::plugin_with(
            0x181,
            &["Starfield.esm"],
            &[
                testdata::rec(b"WEAP", 0xFE00_0800, &[("EDID", b"Gun\0".to_vec())]),
                testdata::rec(b"KYWD", 0x0000_0123, &[]),
            ],
        );
        let path = testdata::temp_file("cache.esm", &bytes);
        let cache = IndexCache::new(testdata::temp_dir("cache"));
        (path, cache)
    }

    #[test]
    fn store_then_load_roundtrips() {
        let (path, cache) = sample();
        let (built, used) = cache.get_or_build(&path).unwrap();
        assert_eq!(used, CacheUse::Built);
        let (again, used) = cache.get_or_build(&path).unwrap();
        assert_eq!(used, CacheUse::Hit);
        assert_eq!(again, built);
        assert_eq!(again.edid(&again.records[0]), Some("Gun"));
    }

    #[test]
    fn changed_size_or_mtime_invalidates() {
        let (path, cache) = sample();
        cache.get_or_build(&path).unwrap();
        // Same size, new mtime.
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        file.set_modified(later).unwrap();
        drop(file);
        assert_eq!(cache.get_or_build(&path).unwrap().1, CacheUse::Built);
        assert_eq!(cache.get_or_build(&path).unwrap().1, CacheUse::Hit);
        // New size.
        let mut bytes = std::fs::read(&path).unwrap();
        let extra = testdata::group(b"GLOB", &testdata::rec(b"GLOB", 0x0000_0999, &[]));
        bytes.extend(extra);
        std::fs::write(&path, &bytes).unwrap();
        let (index, used) = cache.get_or_build(&path).unwrap();
        assert_eq!(used, CacheUse::Built);
        assert_eq!(index.records.len(), 3);
    }

    #[test]
    fn corrupt_entries_are_rebuilt_not_trusted() {
        let (path, cache) = sample();
        cache.get_or_build(&path).unwrap();
        let entry = cache.entry_path(&path);
        let good = std::fs::read(&entry).unwrap();
        let stamp = file_stamp(&path).unwrap();
        let sample = content_sample(&path).unwrap();
        assert!(decode(&good, &path, stamp, sample).is_ok());
        // Every truncation and a flip in every byte position are rejected.
        for cut in 0..good.len() {
            assert!(
                decode(&good[..cut], &path, stamp, sample).is_err(),
                "cut {cut}"
            );
        }
        for i in 0..good.len() {
            let mut b = good.clone();
            b[i] ^= 0x41;
            assert!(decode(&b, &path, stamp, sample).is_err(), "flip {i}");
        }
        std::fs::write(&entry, b"garbage").unwrap();
        assert_eq!(cache.get_or_build(&path).unwrap().1, CacheUse::Built);
    }

    #[test]
    fn hostile_counts_with_valid_checksum_do_not_allocate() {
        let (path, cache) = sample();
        let (index, _) = cache.get_or_build(&path).unwrap();
        let sample = content_sample(&path).unwrap();
        let mut bytes = encode(&index, sample);
        // Find the record count (u64 = 2) and replace it with a huge value; re-sign.
        let at = bytes
            .windows(8)
            .position(|w| w == 2u64.to_le_bytes())
            .unwrap();
        bytes[at..at + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        let n = bytes.len() - 8;
        let mut h = Fnv::new();
        h.write(&bytes[..n]);
        let sum = h.finish().to_le_bytes();
        bytes[n..].copy_from_slice(&sum);
        assert!(decode(&bytes, &path, file_stamp(&path).unwrap(), sample).is_err());
    }

    /// Review round 2: an edit that keeps size and mtime is caught by the content sample.
    #[test]
    fn same_size_and_mtime_with_new_content_invalidates() {
        let (path, cache) = sample();
        cache.get_or_build(&path).unwrap();
        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        let at = bytes.windows(4).position(|w| w == b"Gun\0").unwrap();
        bytes[at + 2] = b'm';
        std::fs::write(&path, &bytes).unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(mtime).unwrap();
        drop(file);
        let (index, used) = cache.get_or_build(&path).unwrap();
        assert_eq!(used, CacheUse::Built);
        assert_eq!(index.edid(&index.records[0]), Some("Gum"));
    }

    /// Review round 2: a plugin with mtime 0 is never cached (nor read from the cache).
    #[test]
    fn mtime_zero_is_never_cached() {
        let (path, cache) = sample();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(std::time::UNIX_EPOCH).unwrap();
        drop(file);
        assert_eq!(file_stamp(&path).unwrap().1, 0);
        assert_eq!(cache.get_or_build(&path).unwrap().1, CacheUse::Uncached);
        assert!(!cache.entry_path(&path).exists());
        assert_eq!(cache.get_or_build(&path).unwrap().1, CacheUse::Uncached);
        let (index, _) = cache.get_or_build(&path).unwrap();
        assert!(cache.store(&index).is_err());
    }

    /// A process killed mid-store leaves `<entry hash>.tmp<pid>`; the next start removes it
    /// once it is old, and keeps recent ones, this process's own, and other files.
    #[test]
    fn stale_temp_files_of_a_killed_store_are_swept() {
        let (path, cache) = sample();
        cache.get_or_build(&path).unwrap();
        let entry = cache.entry_path(&path);
        let temp = |ext: &str| {
            let p = entry.with_extension(ext);
            std::fs::write(&p, b"partial").unwrap();
            p
        };
        let age = |p: &Path, secs: u64| {
            let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
            f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(secs))
                .unwrap();
        };
        let killed = temp("tmp4242424");
        age(&killed, 3600);
        let busy = temp("tmp4242425");
        let own = temp(&format!("tmp{}", std::process::id()));
        age(&own, 3600);
        let other = cache.dir().join("notes.tmp1");
        std::fs::write(&other, b"x").unwrap();
        age(&other, 3600);

        assert_eq!(cache.sweep_stale_temps(STALE_TEMP_AGE), 1);
        assert!(!killed.exists());
        assert!(busy.exists() && own.exists() && other.exists() && entry.exists());
        assert_eq!(cache.sweep_stale_temps(STALE_TEMP_AGE), 0);
        assert_eq!(
            IndexCache::new(cache.dir().join("none")).sweep_stale_temps(STALE_TEMP_AGE),
            0
        );
    }
}
