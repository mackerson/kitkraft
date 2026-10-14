//! Record-level diff of two plugins (M2.D12).
//!
//! Records are matched by identity, not by raw FormID: each file's FormIDs are resolved
//! through its own masters, by master *name*, so two files with differently ordered masters
//! still match. The file's own records match the other file's own records (a plugin and its
//! edited copy). Resolution needs every master's scale (`form` module docs): masters are
//! looked up next to each file and in the extra directories given. When a master cannot be
//! found and both files list the same masters in the same order with the same own scale,
//! raw FormIDs are compared directly (the encodings are then identical); otherwise the diff
//! is refused with the master's name.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use kitcraft_esm::{DecodeLimits, HEADER_LEN};

use crate::compare::{classify, OverrideClass};
use crate::error::{DocError, Result};
use crate::flags::Scale;
use crate::form::{FormRef, MasterMap};
use crate::index::{file_name_of, read_header, PluginIndex, RecordLoc};
use crate::record::{read_record_at, LoadedRecord};

/// A record's identity across the two files.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    /// Defined by the file itself (either file).
    Own(u32),
    /// Defined by a master, named lower-case.
    Master(String, u32),
    /// Raw FormID (only when masters could not be read and the encodings are identical).
    Raw(u32),
}

impl Key {
    /// `Plugin.esm:XXXXXX`, `<self>:XXXXXX` or `raw:XXXXXXXX`.
    pub fn label(&self, own_name: &str) -> String {
        match self {
            Key::Own(o) => format!("{own_name}:{o:06X}"),
            Key::Master(m, o) => format!("{m}:{o:06X}"),
            Key::Raw(r) => format!("raw:{r:08X}"),
        }
    }
}

/// One side of a changed record.
#[derive(Debug, Clone)]
pub struct Side {
    /// Where the record is.
    pub loc: RecordLoc,
    /// The record (read only for changed records).
    pub record: LoadedRecord,
}

/// A record present in both files with different content.
#[derive(Debug, Clone)]
pub struct Changed {
    /// Identity.
    pub key: Key,
    /// In `a`.
    pub a: Side,
    /// In `b`.
    pub b: Side,
    /// What differs.
    pub class: OverrideClass,
}

/// The diff.
#[derive(Debug, Clone)]
pub struct PluginDiff {
    /// File `a`.
    pub a: PluginIndex,
    /// File `b`.
    pub b: PluginIndex,
    /// `name` (resolved by master name) or `raw`.
    pub keyed_by: &'static str,
    /// Only in `b`.
    pub added: Vec<(Key, RecordLoc)>,
    /// Only in `a`.
    pub removed: Vec<(Key, RecordLoc)>,
    /// In both, different.
    pub changed: Vec<Changed>,
    /// In both, byte-identical bodies and equal flags.
    pub unchanged: u64,
    /// Records whose FormID did not resolve (counted per side), with the first error.
    pub unresolved: (u64, u64, Option<String>),
}

/// Find `name` in `dirs`, ASCII case-insensitively.
fn find_file(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    for dir in dirs {
        let direct = dir.join(name);
        if direct.is_file() {
            return Some(direct);
        }
        let Ok(read) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in read.flatten() {
            if e.file_name().to_string_lossy().eq_ignore_ascii_case(name) && e.path().is_file() {
                return Some(e.path());
            }
        }
    }
    None
}

/// Name table index 0 is the file itself; masters follow.
fn key_map(
    index: &PluginIndex,
    dirs: &[PathBuf],
) -> std::result::Result<(MasterMap, Vec<String>), String> {
    let own = index
        .header
        .flags
        .scale()
        .ok_or_else(|| format!("{} sets both scale flags", index.file_name))?;
    let mut names = vec![String::new()];
    let mut masters = Vec::new();
    for m in &index.header.masters {
        let path = find_file(dirs, m)
            .ok_or_else(|| format!("master `{m}` of {} not found", index.file_name))?;
        let scale = read_header(&path)
            .map_err(|e| e.to_string())?
            .flags
            .scale()
            .ok_or_else(|| format!("master `{m}` sets both scale flags"))?;
        let i = u16::try_from(names.len()).map_err(|_| "too many masters".to_string())?;
        names.push(m.to_ascii_lowercase());
        masters.push((i, scale));
    }
    Ok((MasterMap::new(&index.file_name, 0, own, &masters), names))
}

/// `as_master`: when set, the file's own records are keyed as that master's (the other
/// file names this one as a master, or both files have the same name).
fn keys(
    index: &PluginIndex,
    map: Option<&(MasterMap, Vec<String>)>,
    as_master: Option<&str>,
) -> (Vec<Option<Key>>, u64, Option<String>) {
    let mut bad = 0u64;
    let mut first = None;
    let keys = index
        .records
        .iter()
        .map(|r| match map {
            None => Some(Key::Raw(r.form_id)),
            Some((m, names)) => match m.resolve(r.form_id) {
                Ok(g) if g.plugin == 0 => Some(match as_master {
                    Some(name) => Key::Master(name.to_string(), g.object_id),
                    None => Key::Own(g.object_id),
                }),
                Ok(g) => names
                    .get(usize::from(g.plugin))
                    .map(|n| Key::Master(n.clone(), g.object_id)),
                Err(e) => {
                    bad = bad.saturating_add(1);
                    first.get_or_insert_with(|| e.to_string());
                    None
                }
            },
        })
        .collect();
    (keys, bad, first)
}

fn raw_bytes(file: &mut File, name: &str, loc: &RecordLoc) -> Result<Vec<u8>> {
    let io = |e| DocError::Io {
        path: name.into(),
        source: e,
    };
    file.seek(SeekFrom::Start(loc.offset)).map_err(io)?;
    let len = u64::from(loc.data_size).saturating_add(HEADER_LEN as u64);
    let mut buf = Vec::new();
    file.take(len).read_to_end(&mut buf).map_err(io)?;
    Ok(buf)
}

/// Diff `a` against `b`. `extra_dirs` are searched for masters after each file's own
/// directory. `only` restricts the diff to one form (`Plugin.esm:XXXXXX`; naming `a` or
/// `b` means that file's own record).
pub fn diff_plugins(
    a: &Path,
    b: &Path,
    extra_dirs: &[PathBuf],
    only: Option<&FormRef>,
) -> Result<PluginDiff> {
    let ia = PluginIndex::build(a)?;
    let ib = PluginIndex::build(b)?;
    let dirs = |p: &Path| -> Vec<PathBuf> {
        p.parent()
            .map(Path::to_path_buf)
            .into_iter()
            .chain(extra_dirs.iter().cloned())
            .collect()
    };
    let (ma, mb) = (key_map(&ia, &dirs(a)), key_map(&ib, &dirs(b)));
    let same_encoding = ia.header.masters.len() == ib.header.masters.len()
        && ia
            .header
            .masters
            .iter()
            .zip(&ib.header.masters)
            .all(|(x, y)| x.eq_ignore_ascii_case(y))
        && ia.header.flags.scale() == ib.header.flags.scale()
        && ia.header.flags.scale().is_some();
    let (maps, keyed_by) = match (ma, mb) {
        (Ok(x), Ok(y)) => ((Some(x), Some(y)), "name"),
        (Err(_), _) | (_, Err(_)) if same_encoding => ((None, None), "raw"),
        (Err(e), _) | (_, Err(e)) => {
            return Err(DocError::Invalid(format!(
                "{e}; the two files' masters differ, so their FormIDs cannot be matched without every master's scale (pass the directory that holds it)"
            )))
        }
    };
    // A file's own records are the other file's master records when the other lists it as
    // a master; two files with the same name are versions of one plugin. Otherwise the two
    // files' own records are matched with each other (an edited copy under another name).
    let (la, lb) = (
        ia.file_name.to_ascii_lowercase(),
        ib.file_name.to_ascii_lowercase(),
    );
    let lists = |idx: &PluginIndex, name: &str| {
        idx.header
            .masters
            .iter()
            .any(|m| m.eq_ignore_ascii_case(name))
    };
    let a_as = (la == lb || lists(&ib, &la)).then_some(la.as_str());
    let b_as = (la == lb || lists(&ia, &lb)).then_some(lb.as_str());
    let (ka, bad_a, err_a) = keys(&ia, maps.0.as_ref(), a_as);
    let (kb, bad_b, err_b) = keys(&ib, maps.1.as_ref(), b_as);
    let mut ia_sorted: Vec<(Key, usize)> = ka
        .into_iter()
        .enumerate()
        .filter_map(|(i, k)| Some((k?, i)))
        .collect();
    let mut ib_sorted: Vec<(Key, usize)> = kb
        .into_iter()
        .enumerate()
        .filter_map(|(i, k)| Some((k?, i)))
        .collect();
    if let Some(f) = only {
        let name = f.plugin.to_ascii_lowercase();
        let k = if name == la {
            a_as.map_or(Key::Own(f.object_id), |n| {
                Key::Master(n.to_string(), f.object_id)
            })
        } else if name == lb {
            b_as.map_or(Key::Own(f.object_id), |n| {
                Key::Master(n.to_string(), f.object_id)
            })
        } else {
            Key::Master(name, f.object_id)
        };
        let k = &k;
        ia_sorted.retain(|(x, _)| x == k);
        ib_sorted.retain(|(x, _)| x == k);
    }
    ia_sorted.sort();
    ib_sorted.sort();

    let mut fa = File::open(a).map_err(|e| DocError::io(a, e))?;
    let mut fb = File::open(b).map_err(|e| DocError::io(b, e))?;
    let (na, nb) = (file_name_of(a), file_name_of(b));
    let limits = DecodeLimits::default();
    let mut diff = PluginDiff {
        keyed_by,
        added: Vec::new(),
        removed: Vec::new(),
        changed: Vec::new(),
        unchanged: 0,
        unresolved: (bad_a, bad_b, err_a.or(err_b)),
        a: ia.clone(),
        b: ib.clone(),
    };
    let (mut i, mut j) = (0usize, 0usize);
    loop {
        let (x, y) = (ia_sorted.get(i), ib_sorted.get(j));
        match (x, y) {
            (None, None) => break,
            (Some((k, r)), None) => {
                if let Some(loc) = ia.records.get(*r) {
                    diff.removed.push((k.clone(), *loc));
                }
                i = i.saturating_add(1);
            }
            (None, Some((k, r))) => {
                if let Some(loc) = ib.records.get(*r) {
                    diff.added.push((k.clone(), *loc));
                }
                j = j.saturating_add(1);
            }
            (Some((k1, r1)), Some((k2, r2))) => match k1.cmp(k2) {
                std::cmp::Ordering::Less => {
                    if let Some(loc) = ia.records.get(*r1) {
                        diff.removed.push((k1.clone(), *loc));
                    }
                    i = i.saturating_add(1);
                }
                std::cmp::Ordering::Greater => {
                    if let Some(loc) = ib.records.get(*r2) {
                        diff.added.push((k2.clone(), *loc));
                    }
                    j = j.saturating_add(1);
                }
                std::cmp::Ordering::Equal => {
                    i = i.saturating_add(1);
                    j = j.saturating_add(1);
                    let (Some(la), Some(lb)) = (ia.records.get(*r1), ib.records.get(*r2)) else {
                        continue;
                    };
                    let (ra, rb) = (raw_bytes(&mut fa, &na, la)?, raw_bytes(&mut fb, &nb, lb)?);
                    let body = |r: &[u8]| r.get(HEADER_LEN..).map(<[u8]>::to_vec);
                    if la.signature == lb.signature
                        && la.flags == lb.flags
                        && body(&ra) == body(&rb)
                    {
                        diff.unchanged = diff.unchanged.saturating_add(1);
                        continue;
                    }
                    let a_rec = read_record_at(
                        &mut fa,
                        &na,
                        la.offset,
                        Some((la.signature(), la.form_id)),
                        &limits,
                    )?;
                    let b_rec = read_record_at(
                        &mut fb,
                        &nb,
                        lb.offset,
                        Some((lb.signature(), lb.form_id)),
                        &limits,
                    )?;
                    let class = classify(&a_rec, &b_rec);
                    if class == OverrideClass::IdenticalToPrevious {
                        diff.unchanged = diff.unchanged.saturating_add(1);
                        continue;
                    }
                    diff.changed.push(Changed {
                        key: k1.clone(),
                        a: Side {
                            loc: *la,
                            record: a_rec,
                        },
                        b: Side {
                            loc: *lb,
                            record: b_rec,
                        },
                        class,
                    });
                }
            },
        }
    }
    Ok(diff)
}

/// Scale of a plugin file from its header (`None` for both scale bits).
pub fn scale_of(path: &Path) -> Result<Option<Scale>> {
    Ok(read_header(path)?.flags.scale())
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::compare::Change;
    use crate::testdata::{self, rec};

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn same_file_is_empty_and_one_edit_is_exact() {
        let dir = testdata::temp_dir("diff");
        let other = testdata::temp_dir("diff-b");
        let base = testdata::plugin_with(0x1, &[], &[rec(b"WEAP", 0x800, &[])]);
        write(&dir, "Base.esm", &base);
        write(&other, "Base.esm", &base);
        let records = |data: u8| {
            vec![
                rec(
                    b"WEAP",
                    0x800,
                    &[("EDID", b"A\0".to_vec()), ("DATA", vec![1, 2, 3])],
                ),
                rec(
                    b"WEAP",
                    0x0100_0801,
                    &[("EDID", b"B\0".to_vec()), ("DATA", vec![data, 2, 3])],
                ),
                testdata::compressed(b"WEAP", 0x0100_0802, &[("EDID", b"C\0".to_vec())]),
            ]
        };
        let a = write(
            &dir,
            "Mod.esm",
            &testdata::plugin_with(0x1, &["Base.esm"], &records(1)),
        );
        let b = write(
            &other,
            "Mod.esm",
            &testdata::plugin_with(0x1, &["Base.esm"], &records(9)),
        );
        let same = diff_plugins(&a, &a, &[], None).unwrap();
        assert_eq!(same.keyed_by, "name");
        assert!(same.added.is_empty() && same.removed.is_empty() && same.changed.is_empty());
        assert_eq!(same.unchanged, 3);
        let d = diff_plugins(&a, &b, &[], None).unwrap();
        assert_eq!(d.changed.len(), 1);
        let c = &d.changed[0];
        assert_eq!(c.key, Key::Master("mod.esm".into(), 0x801));
        let OverrideClass::Differs { subrecords, .. } = &c.class else {
            panic!()
        };
        assert_eq!(subrecords.len(), 1);
        assert_eq!(subrecords[0].signature, "DATA");
        assert_eq!(subrecords[0].change, Change::Changed);
        let bytes_a = std::fs::read(&a).unwrap();
        let at = subrecords[0].a.unwrap().file_offset.unwrap() as usize;
        assert_eq!(bytes_a[at], 1);
        let bytes_b = std::fs::read(&b).unwrap();
        let at = subrecords[0].b.unwrap().file_offset.unwrap() as usize;
        assert_eq!(bytes_b[at], 9);
        assert_eq!(d.unchanged, 2);
    }

    #[test]
    fn differently_ordered_masters_match_by_name() {
        let dir = testdata::temp_dir("diff-order");
        write(&dir, "X.esm", &testdata::plugin_with(0x1, &[], &[]));
        write(&dir, "Y.esm", &testdata::plugin_with(0x101, &[], &[]));
        write(&dir, "Z.esm", &testdata::plugin_with(0x1, &[], &[]));
        let rec_x = |id: u32| rec(b"WEAP", id, &[("DATA", vec![1])]);
        // a: masters X (full 0), Y (small 0), Z (full 1); b: Z (full 0), X (full 1), Y.
        let a = write(
            &dir,
            "A.esm",
            &testdata::plugin_with(
                0x1,
                &["X.esm", "Y.esm", "Z.esm"],
                &[
                    rec_x(0x0000_0800),
                    rec_x(0xFE00_0801),
                    rec_x(0x0100_0802),
                    rec_x(0x0200_0803),
                ],
            ),
        );
        let b = write(
            &dir,
            "B.esm",
            &testdata::plugin_with(
                0x1,
                &["Z.esm", "X.esm", "Y.esm"],
                &[
                    rec_x(0x0100_0800),
                    rec_x(0xFE00_0801),
                    rec_x(0x0000_0802),
                    rec_x(0x0200_0803),
                    rec_x(0x0200_0804),
                ],
            ),
        );
        let d = diff_plugins(&a, &b, &[], None).unwrap();
        assert!(d.removed.is_empty(), "{:?}", d.removed);
        assert_eq!(d.added.len(), 1);
        assert_eq!(d.added[0].0, Key::Own(0x804));
        assert!(d.changed.is_empty());
        assert_eq!(d.unchanged, 4);
        let only =
            diff_plugins(&a, &b, &[], Some(&FormRef::parse("X.esm:000800").unwrap())).unwrap();
        assert_eq!(only.unchanged, 1);
        assert!(only.added.is_empty());
    }

    #[test]
    fn missing_master_with_different_lists_is_refused_but_identical_lists_key_raw() {
        let dir = testdata::temp_dir("diff-missing");
        let a = write(
            &dir,
            "A.esm",
            &testdata::plugin_with(0x1, &["Gone.esm"], &[rec(b"WEAP", 0x800, &[])]),
        );
        let b = write(
            &dir,
            "B.esm",
            &testdata::plugin_with(
                0x1,
                &["Gone.esm"],
                &[rec(b"WEAP", 0x800, &[("DATA", vec![1])])],
            ),
        );
        let c = write(
            &dir,
            "C.esm",
            &testdata::plugin_with(0x1, &["Other.esm"], &[]),
        );
        let d = diff_plugins(&a, &b, &[], None).unwrap();
        assert_eq!(d.keyed_by, "raw");
        assert_eq!(d.changed.len(), 1);
        let err = diff_plugins(&a, &c, &[], None).unwrap_err();
        assert!(err.to_string().contains("Gone.esm"), "{err}");
    }
}
