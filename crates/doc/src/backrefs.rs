//! Reverse link index (M4.D1): which records of a plugin name a given raw FormID in a typed
//! member, and where each placed reference sits.
//!
//! One [`PluginBackrefs`] per plugin, built by streaming the plugin once: every record whose
//! type has a definition with a `formid` member is read (inflated when compressed) and
//! decoded by the generated decoder (`kitkraft_esm::generated::records::Record`), and every
//! value its `form_ids()` visitor yields is entered as `(raw value, record index)`. The
//! null value `00000000` and "none" (`FFFFFFFF`) are not entered. Raw values are kept as
//! stored: resolving them needs the load order (`crate::form`), so a query turns the target
//! form into the raw values each plugin would store for it ([`raw_candidates`]).
//!
//! Subrecords the generated decoder keeps raw (`extra`: unplaced or undecodable payloads,
//! `any_until` component blocks) and records of types with no definition are not scanned;
//! [`SigStats`] counts them per record type so a result can say what was not searched.
//!
//! Placed references: for every record inside a cell children group (types 6, 8, 9;
//! `kitkraft_esm::cells`), the enclosing cell's and world's stored FormIDs and the group
//! type (8 persistent, 9 temporary) are kept as ranges of record indices.
//!
//! Cache: like the record index (`crate::cache`, M2.D5): one file per plugin path in the
//! same directory (`<hash>.kclnk`), used only when its checksum, format version, schema
//! fingerprint ([`schema_fingerprint`]: the builtin definitions decide what is typed),
//! path, size, modification time, content sample and record count all match; otherwise it
//! is rebuilt, never trusted. Layout (little-endian): magic `KCLNK\0\r\n`, `u32` format
//! version, `u64` schema fingerprint, `u32` path length + path, `u64` size, `u64` mtime
//! (ns), `u64` content sample, `u32` record count, `u64` entry count + 8 bytes per entry
//! (raw, record) sorted, `u64` range count + 16 bytes per range (first record, end record,
//! cell raw, world raw) + 1 byte group type each, `u32` stats count + per type (signature,
//! five `u64`), `u64` FNV-1a checksum of everything before it.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use kitkraft_esm::bridge::{has_definition, Ctx, RawSub};
use kitkraft_esm::cells::group_type;
use kitkraft_esm::generated::records::Record as Generated;
use kitkraft_esm::index::{index_file, read_record_at, IndexOptions};
use kitkraft_esm::{DecodeLimits, Reader, Signature};
use kitkraft_schema::{FieldType, Member, Schema};

use crate::cache::content_sample;
use crate::error::{DocError, Result};
use crate::flags::Scale;
use crate::form::{GlobalForm, RawRef};
use crate::hash::Fnv;
use crate::index::file_stamp;
use crate::load_order::LoadOrder;

/// Bump when the layout or the meaning of an entry changes.
pub const FORMAT_VERSION: u32 = 1;

const MAGIC: &[u8; 8] = b"KCLNK\0\r\n";

/// Record flag 0x20.
const DELETED: u32 = 0x20;

/// Bytes read ahead when streaming a plugin.
const WINDOW: usize = 8 << 20;

/// FNV-1a over the builtin definitions (file names and text): what is typed decides what
/// an index holds.
pub fn schema_fingerprint() -> u64 {
    let mut h = Fnv::new();
    for (name, text) in kitkraft_schema::BUILTIN_DEFINITIONS {
        h.write(name.as_bytes());
        h.write(&[0]);
        h.write(text.as_bytes());
        h.write(&[0]);
    }
    h.finish()
}

/// What one record type of a plugin contributed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SigStats {
    /// Records of the type.
    pub records: u64,
    /// Records scanned (the type's definition has `formid` members).
    pub scanned: u64,
    /// Records not scanned because the type has no definition.
    pub no_definition: u64,
    /// Scanned records with subrecords the generated decoder kept raw (not searched).
    pub with_raw_subrecords: u64,
    /// Deleted records (flag 0x20): their payload is not the form's data; not scanned.
    pub deleted: u64,
}

/// Where a placed reference is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// The cell's FormID as stored in this plugin.
    pub cell_raw: u32,
    /// The world's FormID as stored (`None` for an interior cell).
    pub world_raw: Option<u32>,
    /// The enclosing group type: 8 persistent, 9 temporary, 6 the cell children group
    /// itself.
    pub group_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Range {
    first: u32,
    end: u32,
    cell: u32,
    world: u32,
    group_type: u8,
}

/// The reverse index of one plugin.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PluginBackrefs {
    /// Records in the plugin (`TES4` excluded), as indexed.
    pub record_count: u32,
    /// `(raw value, record index)`, sorted.
    entries: Vec<(u32, u32)>,
    ranges: Vec<Range>,
    /// Per record type.
    pub stats: BTreeMap<[u8; 4], SigStats>,
}

/// Whether a type holds a `formid` anywhere.
fn type_has_form(ty: &FieldType, depth: usize) -> bool {
    if depth > 64 {
        return true;
    }
    let d = depth.saturating_add(1);
    match ty {
        FieldType::FormId { .. } => true,
        FieldType::Array { element, .. } => type_has_form(element, d),
        FieldType::Struct { fields } => fields.iter().any(|f| type_has_form(&f.ty, d)),
        FieldType::Ref { ty, .. } => type_has_form(ty, d),
        FieldType::Union { variants, .. } => variants.iter().any(|v| type_has_form(&v.ty, d)),
        _ => false,
    }
}

fn members_have_form(members: &[Member], depth: usize) -> bool {
    members.iter().any(|m| match m {
        Member::Subrecord(s) => type_has_form(&s.ty, depth),
        Member::Group(g) => depth < 64 && members_have_form(&g.members, depth.saturating_add(1)),
        Member::AnyUntil(_) => false,
    })
}

fn members_have_any_until(members: &[Member], depth: usize) -> bool {
    members.iter().any(|m| match m {
        Member::AnyUntil(_) => true,
        Member::Group(g) => {
            depth < 64 && members_have_any_until(&g.members, depth.saturating_add(1))
        }
        Member::Subrecord(_) => false,
    })
}

/// Whether the definition of `sig` has an `any_until` member (raw bodies).
fn type_has_any_until(schema: &Schema, sig: [u8; 4]) -> bool {
    kitkraft_schema::Signature::from_bytes(sig)
        .and_then(|s| schema.record(s))
        .is_some_and(|def| members_have_any_until(&def.members, 0))
}

/// Whether records of `sig` can hold typed FormIDs (their definition has a `formid`).
pub fn type_has_links(schema: &Schema, sig: [u8; 4]) -> bool {
    kitkraft_schema::Signature::from_bytes(sig)
        .and_then(|s| schema.record(s))
        .is_some_and(|def| members_have_form(&def.members, 0))
}

/// A read-ahead window over a file that is read mostly in order (`read_record_at` seeks
/// to the end and to each record; both stay cheap here).
struct Window {
    file: File,
    len: u64,
    buf: Vec<u8>,
    start: u64,
    pos: u64,
}

impl Window {
    fn new(file: File) -> std::io::Result<Window> {
        let len = file.metadata()?.len();
        Ok(Window {
            file,
            len,
            buf: Vec::new(),
            start: 0,
            pos: 0,
        })
    }

    fn end(&self) -> u64 {
        self.start.saturating_add(self.buf.len() as u64)
    }
}

impl Read for Window {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if out.is_empty() || self.pos >= self.len {
            return Ok(0);
        }
        if self.pos < self.start || self.pos >= self.end() {
            if out.len() >= WINDOW {
                self.file.seek(SeekFrom::Start(self.pos))?;
                let n = self.file.read(out)?;
                self.pos = self.pos.saturating_add(n as u64);
                return Ok(n);
            }
            self.file.seek(SeekFrom::Start(self.pos))?;
            self.buf.clear();
            let want = usize::try_from(self.len.saturating_sub(self.pos))
                .unwrap_or(WINDOW)
                .min(WINDOW);
            self.buf.resize(want, 0);
            let mut filled = 0usize;
            while filled < want {
                let slot = self.buf.get_mut(filled..).unwrap_or_default();
                let n = self.file.read(slot)?;
                if n == 0 {
                    break;
                }
                filled = filled.saturating_add(n);
            }
            self.buf.truncate(filled);
            self.start = self.pos;
            if filled == 0 {
                return Ok(0);
            }
        }
        let at = usize::try_from(self.pos.saturating_sub(self.start)).unwrap_or(usize::MAX);
        let avail = self.buf.get(at..).unwrap_or_default();
        let n = avail.len().min(out.len());
        if let (Some(dst), Some(src)) = (out.get_mut(..n), avail.get(..n)) {
            dst.copy_from_slice(src);
        }
        self.pos = self.pos.saturating_add(n as u64);
        Ok(n)
    }
}

impl Seek for Window {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let invalid = || std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek");
        self.pos = match to {
            SeekFrom::Start(n) => n,
            SeekFrom::End(d) => self.len.checked_add_signed(d).ok_or_else(invalid)?,
            SeekFrom::Current(d) => self.pos.checked_add_signed(d).ok_or_else(invalid)?,
        };
        Ok(self.pos)
    }
}

impl PluginBackrefs {
    /// Build the index of the plugin at `path`. `localized` is the plugin's localized flag
    /// (it decides how lstrings decode, not FormIDs, but the decoder needs it).
    pub fn build(path: &Path, schema: &Schema) -> Result<PluginBackrefs> {
        let name = crate::index::file_name_of(path);
        let esm = index_file(path, &IndexOptions::default())
            .map_err(|e| DocError::esm(name.clone(), e))?;
        let localized = crate::index::header_info(&esm.header).flags.localized();
        let limits = DecodeLimits::default();
        let file = File::open(path).map_err(|e| DocError::io(path, e))?;
        let mut window = Window::new(file).map_err(|e| DocError::io(path, e))?;
        let mut out = PluginBackrefs::default();
        let mut has_links: BTreeMap<[u8; 4], bool> = BTreeMap::new();
        let mut has_any_until: BTreeMap<[u8; 4], bool> = BTreeMap::new();
        let mut current: Option<(u32, Range)> = None;
        let mut index: u32 = 0;
        for loc in esm.records.iter().filter(|r| r.offset != 0) {
            let sig = loc.signature.0;
            let stats = out.stats.entry(sig).or_default();
            stats.records = stats.records.saturating_add(1);
            // Placement: the innermost group, when it is a cell children group.
            let group = loc.group;
            let same = matches!((&current, group), (Some((g, _)), Some(x)) if *g == x);
            if !same {
                if let Some((_, r)) = current.take() {
                    out.ranges.push(r);
                }
                if let Some(g) = group {
                    let path = esm.group_path(Some(g));
                    if let Some(inner) = path.last() {
                        let t = inner.header.group_type;
                        if t == group_type::CELL_PERSISTENT
                            || t == group_type::CELL_TEMPORARY
                            || t == group_type::CELL_CHILDREN
                        {
                            let world = path
                                .iter()
                                .rev()
                                .find(|p| p.header.group_type == group_type::WORLD_CHILDREN)
                                .map_or(0, |p| p.header.label_u32());
                            current = Some((
                                g,
                                Range {
                                    first: index,
                                    end: index,
                                    cell: inner.header.label_u32(),
                                    world,
                                    group_type: u8::try_from(t).unwrap_or(0),
                                },
                            ));
                        }
                    }
                }
            }
            if let Some((_, r)) = current.as_mut() {
                r.end = index.saturating_add(1);
            }
            let record = index;
            index = index.saturating_add(1);
            if !has_definition(Signature(sig)) {
                stats.no_definition = stats.no_definition.saturating_add(1);
                continue;
            }
            if !*has_links
                .entry(sig)
                .or_insert_with(|| type_has_links(schema, sig))
            {
                continue;
            }
            if loc.flags & DELETED != 0 {
                stats.deleted = stats.deleted.saturating_add(1);
                continue;
            }
            let rec = read_record_at(&mut window, loc.offset, Some(&limits))
                .map_err(|e| DocError::esm(name.clone(), e))?;
            let Some(subs) = rec.subrecords() else {
                continue;
            };
            let raw: Vec<RawSub<'_>> = subs.iter().map(RawSub::from).collect();
            let ctx = Ctx {
                form_version: loc.form_version,
                localized,
            };
            let Some(typed) = Generated::decode(sig, &raw, ctx) else {
                stats.no_definition = stats.no_definition.saturating_add(1);
                continue;
            };
            stats.scanned = stats.scanned.saturating_add(1);
            // Raw subrecords: what the decoder could not place or decode (`extra`), and
            // the bodies of `any_until` members (component blocks), found by placing the
            // record's signatures in its definition (only for types that have one).
            let raw_body = *has_any_until
                .entry(sig)
                .or_insert_with(|| type_has_any_until(schema, sig))
                && {
                    let sigs: Vec<[u8; 4]> = subs.iter().map(|s| s.signature().0).collect();
                    crate::typed::placements(schema, sig, &sigs).is_some_and(|p| {
                        p.iter()
                            .any(|x| matches!(x, crate::typed::Placement::Raw { .. }))
                    })
                };
            if !typed.extra().is_empty() || raw_body {
                stats.with_raw_subrecords = stats.with_raw_subrecords.saturating_add(1);
            }
            for (_, f) in typed.form_ids() {
                if f.0 != 0 && f.0 != crate::links::NONE_FORM_ID {
                    out.entries.push((f.0, record));
                }
            }
        }
        if let Some((_, r)) = current.take() {
            out.ranges.push(r);
        }
        out.record_count = index;
        out.entries.sort_unstable();
        out.entries.dedup();
        Ok(out)
    }

    /// Records naming `raw` (each once), in index order.
    pub fn records_naming(&self, raw: u32) -> Vec<u32> {
        let start = self.entries.partition_point(|(r, _)| *r < raw);
        self.entries
            .get(start..)
            .unwrap_or_default()
            .iter()
            .take_while(|(r, _)| *r == raw)
            .map(|(_, rec)| *rec)
            .collect()
    }

    /// Entries whose raw value is in `lo..=hi`.
    pub fn entries_in(&self, lo: u32, hi: u32) -> &[(u32, u32)] {
        let start = self.entries.partition_point(|(r, _)| *r < lo);
        let end = self.entries.partition_point(|(r, _)| *r <= hi);
        self.entries.get(start..end.max(start)).unwrap_or_default()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Where record `record` sits, when it is inside a cell children group.
    pub fn placement(&self, record: u32) -> Option<Placement> {
        let i = self.ranges.partition_point(|r| r.end <= record);
        let r = self.ranges.get(i)?;
        (r.first <= record && record < r.end).then_some(Placement {
            cell_raw: r.cell,
            world_raw: (r.world != 0).then_some(r.world),
            group_type: i32::from(r.group_type),
        })
    }

    fn encode(&self, path_key: &str, stamp: (u64, u64), sample: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            self.entries
                .len()
                .saturating_mul(8)
                .saturating_add(self.ranges.len().saturating_mul(17))
                .saturating_add(4096),
        );
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&schema_fingerprint().to_le_bytes());
        out.extend_from_slice(&u32::try_from(path_key.len()).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(path_key.as_bytes());
        out.extend_from_slice(&stamp.0.to_le_bytes());
        out.extend_from_slice(&stamp.1.to_le_bytes());
        out.extend_from_slice(&sample.to_le_bytes());
        out.extend_from_slice(&self.record_count.to_le_bytes());
        out.extend_from_slice(&(self.entries.len() as u64).to_le_bytes());
        for (raw, rec) in &self.entries {
            out.extend_from_slice(&raw.to_le_bytes());
            out.extend_from_slice(&rec.to_le_bytes());
        }
        out.extend_from_slice(&(self.ranges.len() as u64).to_le_bytes());
        for r in &self.ranges {
            for v in [r.first, r.end, r.cell, r.world] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.push(r.group_type);
        }
        out.extend_from_slice(&u32::try_from(self.stats.len()).unwrap_or(0).to_le_bytes());
        for (sig, s) in &self.stats {
            out.extend_from_slice(sig);
            for v in [
                s.records,
                s.scanned,
                s.no_definition,
                s.with_raw_subrecords,
                s.deleted,
            ] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        let mut h = Fnv::new();
        h.write(&out);
        out.extend_from_slice(&h.finish().to_le_bytes());
        out
    }

    fn decode(bytes: &[u8], path_key: &str, stamp: (u64, u64), sample: u64) -> Option<Self> {
        let body_len = bytes.len().checked_sub(8)?;
        let (body, sum) = bytes.split_at(body_len);
        let mut h = Fnv::new();
        h.write(body);
        if u64::from_le_bytes(sum.try_into().ok()?) != h.finish() {
            return None;
        }
        let mut r = Reader::new(body, 0);
        if r.take(8).ok()? != MAGIC
            || r.u32().ok()? != FORMAT_VERSION
            || r.u64().ok()? != schema_fingerprint()
        {
            return None;
        }
        let n = usize::try_from(r.u32().ok()?).ok()?;
        if r.take(n).ok()? != path_key.as_bytes()
            || r.u64().ok()? != stamp.0
            || r.u64().ok()? != stamp.1
            || r.u64().ok()? != sample
        {
            return None;
        }
        let record_count = r.u32().ok()?;
        let count = usize::try_from(r.u64().ok()?).ok()?;
        if count > r.remaining() / 8 {
            return None;
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push((r.u32().ok()?, r.u32().ok()?));
        }
        let count = usize::try_from(r.u64().ok()?).ok()?;
        if count > r.remaining() / 17 {
            return None;
        }
        let mut ranges = Vec::with_capacity(count);
        for _ in 0..count {
            ranges.push(Range {
                first: r.u32().ok()?,
                end: r.u32().ok()?,
                cell: r.u32().ok()?,
                world: r.u32().ok()?,
                group_type: r.u8().ok()?,
            });
        }
        let count = r.u32().ok()?;
        let mut stats = BTreeMap::new();
        for _ in 0..count {
            let sig = r.array::<4>("signature").ok()?;
            let s = SigStats {
                records: r.u64().ok()?,
                scanned: r.u64().ok()?,
                no_definition: r.u64().ok()?,
                with_raw_subrecords: r.u64().ok()?,
                deleted: r.u64().ok()?,
            };
            stats.insert(sig, s);
        }
        if !r.is_empty() {
            return None;
        }
        Some(PluginBackrefs {
            record_count,
            entries,
            ranges,
            stats,
        })
    }
}

/// How a plugin's reverse index was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Obtained {
    /// Read from the cache.
    Hit,
    /// Built and stored.
    Built,
    /// Built, not stored (no cache, or the store failed).
    Uncached,
}

/// The cache directory for reverse indexes (the record index cache's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackrefCache {
    dir: PathBuf,
}

fn path_key(path: &Path) -> String {
    std::path::absolute(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

impl BackrefCache {
    /// Entries in `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> BackrefCache {
        BackrefCache { dir: dir.into() }
    }

    /// Entry file for a plugin path.
    pub fn entry_path(&self, plugin: &Path) -> PathBuf {
        let mut h = Fnv::new();
        h.write(path_key(plugin).as_bytes());
        self.dir.join(format!("{:016x}.kclnk", h.finish()))
    }

    /// A valid cached index of `plugin` with `records` records, if there is one.
    pub fn load(&self, plugin: &Path, records: u32) -> Option<PluginBackrefs> {
        let stamp = file_stamp(plugin).ok()?;
        if stamp.1 == 0 {
            return None;
        }
        let bytes = std::fs::read(self.entry_path(plugin)).ok()?;
        let sample = content_sample(plugin).ok()?;
        PluginBackrefs::decode(&bytes, &path_key(plugin), stamp, sample)
            .filter(|b| b.record_count == records)
    }

    /// Store `index` for `plugin` (atomically: temp file, then rename).
    pub fn store(&self, plugin: &Path, index: &PluginBackrefs) -> Result<()> {
        let stamp = file_stamp(plugin)?;
        if stamp.1 == 0 {
            return Err(DocError::Invalid(format!(
                "{}: modification time unknown or 0; not cached",
                plugin.display()
            )));
        }
        let sample = content_sample(plugin)?;
        std::fs::create_dir_all(&self.dir).map_err(|e| DocError::io(&self.dir, e))?;
        let target = self.entry_path(plugin);
        let tmp = target.with_extension(format!("tmp{}", std::process::id()));
        let bytes = index.encode(&path_key(plugin), stamp, sample);
        let written =
            File::create(&tmp).and_then(|mut f| f.write_all(&bytes).and_then(|()| f.sync_all()));
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(DocError::io(&tmp, e));
        }
        std::fs::rename(&tmp, &target).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            DocError::io(&target, e)
        })
    }
}

/// One plugin's reverse index and how it was obtained, or why it could not be built.
type Built = std::result::Result<(PluginBackrefs, Obtained), String>;

/// The reverse indexes of a load order, by load-order index (`None` where the plugin did
/// not load or its index failed to build: the reason is in `errors`).
#[derive(Debug, Default)]
pub struct Backrefs {
    /// Per plugin.
    pub plugins: Vec<Option<PluginBackrefs>>,
    /// How each was obtained.
    pub obtained: Vec<Option<Obtained>>,
    /// Plugins whose index could not be built, with the reason.
    pub errors: Vec<(u16, String)>,
    /// Build time (cache reads included), milliseconds.
    pub elapsed_ms: u64,
}

impl Backrefs {
    /// The reverse index of every loaded plugin of `lo`, through `cache` when given, built
    /// on up to `threads` threads (0: available parallelism, at most 4).
    pub fn build(
        lo: &LoadOrder,
        schema: &Schema,
        cache: Option<&BackrefCache>,
        threads: usize,
    ) -> Backrefs {
        let started = std::time::Instant::now();
        let n = lo.plugins.len();
        let results: Mutex<Vec<Option<Built>>> = Mutex::new(vec![None; n]);
        let next = AtomicUsize::new(0);
        let workers = if threads == 0 {
            std::thread::available_parallelism()
                .map_or(1, std::num::NonZeroUsize::get)
                .min(4)
        } else {
            threads
        }
        .clamp(1, n.max(1));
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(p) = lo.plugins.get(i) else { break };
                    let Some(index) = p.index.as_deref() else {
                        continue;
                    };
                    let records = u32::try_from(index.records.len()).unwrap_or(u32::MAX);
                    let result = match cache.and_then(|c| c.load(&p.path, records)) {
                        Some(b) => Ok((b, Obtained::Hit)),
                        None => match PluginBackrefs::build(&p.path, schema) {
                            Ok(b) if b.record_count != records => Err(format!(
                                "{} has {} records now, {records} when the load order was read: reload it",
                                p.name, b.record_count
                            )),
                            Ok(b) => {
                                let how = match cache.map(|c| c.store(&p.path, &b)) {
                                    Some(Ok(())) => Obtained::Built,
                                    _ => Obtained::Uncached,
                                };
                                Ok((b, how))
                            }
                            Err(e) => Err(e.to_string()),
                        },
                    };
                    if let Ok(mut all) = results.lock() {
                        if let Some(slot) = all.get_mut(i) {
                            *slot = Some(result);
                        }
                    }
                });
            }
        });
        let mut out = Backrefs::default();
        for (i, r) in results
            .into_inner()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            match r {
                Some(Ok((b, how))) => {
                    out.plugins.push(Some(b));
                    out.obtained.push(Some(how));
                }
                Some(Err(e)) => {
                    out.plugins.push(None);
                    out.obtained.push(None);
                    out.errors.push((u16::try_from(i).unwrap_or(u16::MAX), e));
                }
                None => {
                    out.plugins.push(None);
                    out.obtained.push(None);
                }
            }
        }
        out.elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        out
    }

    /// Every `(plugin, record)` whose typed FormID members name `target`, in load order,
    /// then index order; and the plugins that could not be searched, with why.
    pub fn referrers(&self, lo: &LoadOrder, target: GlobalForm) -> (Vec<(u16, u32)>, Vec<String>) {
        let mut hits = Vec::new();
        let mut skipped = Vec::new();
        for (i, p) in lo.plugins.iter().enumerate() {
            let Ok(plugin) = u16::try_from(i) else { break };
            let Some(index) = self.plugins.get(i).and_then(Option::as_ref) else {
                if p.index.is_some() {
                    skipped.push(format!("{}: no reverse index", p.name));
                }
                continue;
            };
            let map = match &p.map {
                Ok(m) => m,
                Err(e) => {
                    if !index.is_empty() {
                        skipped.push(format!("{}: {e}", p.name));
                    }
                    continue;
                }
            };
            let mut records: Vec<u32> = Vec::new();
            for raw in raw_candidates(map, target) {
                records.extend(index.records_naming(raw));
            }
            // `full-style-self` with bits above the scale (`Basis::FullStyleSelfMasked`):
            // every full-class value at the file's own index that masks to the object ID.
            if target.plugin == plugin && map.own_scale != Scale::Full {
                if let Some(n) = u32::try_from(map.full.len()).ok().filter(|n| *n < 0xFD) {
                    let lo_raw = n << 24;
                    let hi_raw = lo_raw | 0x00FF_FFFF;
                    let mask = map.own_scale.max_object_id();
                    for (raw, rec) in index.entries_in(lo_raw, hi_raw) {
                        if raw & 0x00FF_FFFF & mask == target.object_id
                            && raw & 0x00FF_FFFF != target.object_id
                        {
                            records.push(*rec);
                        }
                    }
                }
            }
            records.sort_unstable();
            records.dedup();
            hits.extend(records.into_iter().map(|r| (plugin, r)));
        }
        (hits, skipped)
    }

    /// Per record type over every plugin: what was scanned and what was not.
    pub fn totals(&self) -> BTreeMap<[u8; 4], SigStats> {
        let mut out: BTreeMap<[u8; 4], SigStats> = BTreeMap::new();
        for p in self.plugins.iter().flatten() {
            for (sig, s) in &p.stats {
                let t = out.entry(*sig).or_default();
                t.records = t.records.saturating_add(s.records);
                t.scanned = t.scanned.saturating_add(s.scanned);
                t.no_definition = t.no_definition.saturating_add(s.no_definition);
                t.with_raw_subrecords = t.with_raw_subrecords.saturating_add(s.with_raw_subrecords);
                t.deleted = t.deleted.saturating_add(s.deleted);
            }
        }
        out
    }
}

/// The raw values plugin `map` would store for `target`: its encoding through the master
/// list, and for its own forms in a small or medium plugin also the `full-style-self`
/// spelling (`crate::form`).
pub fn raw_candidates(map: &crate::form::MasterMap, target: GlobalForm) -> Vec<u32> {
    let mut out: Vec<u32> = map.to_raw(target).into_iter().collect();
    if target.plugin == map.own_index && map.own_scale != Scale::Full {
        if let Some(raw) = u16::try_from(map.full.len()).ok().and_then(|index| {
            RawRef {
                class: Scale::Full,
                index,
                object_id: target.object_id,
            }
            .encode()
        }) {
            if !out.contains(&raw) {
                out.push(raw);
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::load_order::{LoadOptions, PluginSpec};
    use crate::testdata::{plugin_with, raw_record, rec, temp_dir};

    fn kwda(ids: &[u32]) -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("EDID", b"W\0".to_vec()),
            ("KSIZ", (ids.len() as u32).to_le_bytes().to_vec()),
            ("KWDA", ids.iter().flat_map(|i| i.to_le_bytes()).collect()),
        ]
    }

    /// A group of `group_type` labelled `label`.
    fn grup(label: u32, group_type: i32, contents: &[u8]) -> Vec<u8> {
        let mut out = b"GRUP".to_vec();
        out.extend_from_slice(&u32::try_from(contents.len() + 24).unwrap().to_le_bytes());
        out.extend_from_slice(&label.to_le_bytes());
        out.extend_from_slice(&group_type.to_le_bytes());
        out.extend_from_slice(&[0u8; 8]);
        out.extend_from_slice(contents);
        out
    }

    /// Base.esm: a keyword, two weapons naming it, a STAT; an interior cell with one
    /// temporary REFR of the STAT.
    fn fixture(tag: &str) -> (PathBuf, LoadOrder) {
        let dir = temp_dir(tag);
        let mut bytes = plugin_with(
            0x1,
            &[],
            &[
                rec(b"KYWD", 0x800, &[("EDID", b"Kw\0".to_vec())]),
                rec(b"WEAP", 0x801, &kwda(&[0x800, 0x800, 0])),
                rec(b"WEAP", 0x802, &kwda(&[0x800])),
                rec(b"STAT", 0x803, &[("EDID", b"Rock\0".to_vec())]),
            ],
        );
        let cell = raw_record(b"CELL", 0, 0x804, &crate::testdata::sub("EDID", b"Room\0"));
        let refr = raw_record(
            b"REFR",
            0,
            0x805,
            &crate::testdata::sub("NAME", &0x803u32.to_le_bytes()),
        );
        let children = grup(0x804, 6, &grup(0x804, 9, &refr));
        let sub_block = grup(0, 3, &[cell, children].concat());
        let block = grup(4, 2, &sub_block);
        let mut top = b"GRUP".to_vec();
        top.extend_from_slice(&u32::try_from(block.len() + 24).unwrap().to_le_bytes());
        top.extend_from_slice(b"CELL");
        top.extend_from_slice(&0i32.to_le_bytes());
        top.extend_from_slice(&[0u8; 8]);
        top.extend_from_slice(&block);
        bytes.extend(top);
        let path = dir.join("Base.esm");
        std::fs::write(&path, bytes).unwrap();
        let lo = LoadOrder::load(
            vec![PluginSpec {
                name: "Base.esm".into(),
                path: path.clone(),
            }],
            &LoadOptions::default(),
        )
        .unwrap();
        (dir, lo)
    }

    #[test]
    fn index_finds_typed_referrers_and_placements() {
        let (dir, lo) = fixture("backrefs");
        let schema = kitkraft_schema::builtin().unwrap();
        let path = dir.join("Base.esm");
        let b = PluginBackrefs::build(&path, schema).unwrap();
        assert_eq!(b.record_count, 6);
        // Both weapons name the keyword (the first twice, entered once); the null is not.
        assert_eq!(b.records_naming(0x800), vec![1, 2]);
        assert!(b.records_naming(0).is_empty());
        // The REFR names the STAT and sits in the interior cell's temporary children.
        assert_eq!(b.records_naming(0x803), vec![5]);
        assert_eq!(
            b.placement(5),
            Some(Placement {
                cell_raw: 0x804,
                world_raw: None,
                group_type: 9
            })
        );
        assert_eq!(b.placement(4), None);
        assert_eq!(b.stats[b"WEAP"].scanned, 2);
        // Round trip through the cache, and staleness by record count.
        let cache = BackrefCache::new(dir.join("cache"));
        cache.store(&path, &b).unwrap();
        assert_eq!(cache.load(&path, 6), Some(b.clone()));
        assert_eq!(cache.load(&path, 7), None);
        // A corrupt entry is rebuilt, never trusted.
        let entry = cache.entry_path(&path);
        let mut bytes = std::fs::read(&entry).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        std::fs::write(&entry, bytes).unwrap();
        assert_eq!(cache.load(&path, 6), None);
        // Through the load order.
        let all = Backrefs::build(&lo, schema, Some(&cache), 1);
        assert_eq!(all.obtained, vec![Some(Obtained::Built)]);
        let (hits, skipped) = all.referrers(
            &lo,
            GlobalForm {
                plugin: 0,
                object_id: 0x800,
            },
        );
        assert_eq!(hits, vec![(0, 1), (0, 2)]);
        assert!(skipped.is_empty());
        let again = Backrefs::build(&lo, schema, Some(&cache), 0);
        assert_eq!(again.obtained, vec![Some(Obtained::Hit)]);
    }

    #[test]
    fn full_style_self_spellings_are_candidates() {
        let map = crate::form::MasterMap::new("Small.esm", 3, Scale::Small, &[(0, Scale::Full)]);
        let own = GlobalForm {
            plugin: 3,
            object_id: 0x801,
        };
        // FE000801 by class, 01000801 by `full-style-self`.
        assert_eq!(raw_candidates(&map, own), vec![0xFE00_0801, 0x0100_0801]);
        let master = GlobalForm {
            plugin: 0,
            object_id: 0x801,
        };
        assert_eq!(raw_candidates(&map, master), vec![0x0000_0801]);
    }
}
