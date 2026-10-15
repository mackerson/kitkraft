//! Running a `where` query over a load order, a page at a time (M2.D10/M2.D11).
//!
//! Candidates are the versions of forms of one record type: the winning version of each
//! form (`Scope::Winner`) or every version (`Scope::All`), ordered by the load-order index of
//! the plugin holding the version, then by FormID as stored. A page evaluates candidates
//! from the cursor's position until `limit` match; the cursor binds the load order and the
//! query, so a cursor from another load order or query is refused.

use std::collections::BTreeSet;

use crate::hash::Fnv;
use crate::load_order::LoadOrder;
use crate::overrides::Version;
use crate::query::{decode_cursor, encode_cursor, Query, Tri};
use crate::record::RecordReader;
use crate::strings::StringTables;
use crate::subject::VersionSubject;

/// Default and largest page size.
pub const DEFAULT_LIMIT: usize = 50;
/// Largest page size.
pub const MAX_LIMIT: usize = 1000;
/// Unknown records listed per page.
pub const UNKNOWN_LISTED: usize = 20;

/// Which versions are candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The winning version of each form.
    Winner,
    /// Every version.
    All,
}

impl Scope {
    /// `winner` or `all`.
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Winner => "winner",
            Scope::All => "all",
        }
    }
}

/// One page of results.
#[derive(Debug, Clone, Default)]
pub struct Page {
    /// Matching versions, in order.
    pub matched: Vec<Version>,
    /// The first unknown versions with their reasons.
    pub unknown: Vec<(Version, Vec<String>)>,
    /// Unknown versions on this page.
    pub unknown_count: u64,
    /// Candidates evaluated on this page.
    pub scanned: u64,
    /// Candidates in total.
    pub candidates: usize,
    /// Cursor for the next page, `None` at the end.
    pub next_cursor: Option<String>,
    /// Every distinct reason something was unknown.
    pub reasons: BTreeSet<String>,
}

/// The candidates for `signature` under `scope`, in result order.
pub fn candidates(lo: &LoadOrder, signature: [u8; 4], scope: Scope) -> Vec<Version> {
    let mut out: Vec<Version> = Vec::new();
    let sig_of = |v: &Version| lo.record(v.plugin, v.record).map(|r| r.signature);
    for run in lo.forms.runs() {
        match scope {
            Scope::Winner => {
                if let Some(w) = run.last() {
                    if sig_of(w) == Some(signature) {
                        out.push(*w);
                    }
                }
            }
            Scope::All => out.extend(run.iter().filter(|v| sig_of(v) == Some(signature))),
        }
    }
    let raw = |v: &Version| lo.record(v.plugin, v.record).map_or(0, |r| r.form_id);
    out.sort_by_key(|v| (v.plugin, raw(v), v.record));
    out
}

/// Fingerprint of a load order and a query, for cursors.
pub fn fingerprint(lo: &LoadOrder, signature: [u8; 4], query: Option<&Query>, scope: Scope) -> u64 {
    let mut h = Fnv::new();
    h.write_u64(lo.fingerprint());
    h.write(&signature);
    h.write(query.map_or("", |q| q.text.as_str()).as_bytes());
    h.write(scope.as_str().as_bytes());
    h.finish()
}

/// What one page asks for.
#[derive(Debug, Clone, Copy)]
pub struct PageRequest<'a> {
    /// Record type.
    pub signature: [u8; 4],
    /// Predicate; `None` matches every candidate.
    pub query: Option<&'a Query>,
    /// Which versions.
    pub scope: Scope,
    /// Most matches on the page (clamped to 1..=[`MAX_LIMIT`]).
    pub limit: usize,
    /// Cursor from the previous page.
    pub cursor: Option<&'a str>,
}

/// Evaluate one page; localized lstrings resolve through `strings` when given.
pub fn run_page(
    lo: &LoadOrder,
    reader: &mut RecordReader,
    request: &PageRequest<'_>,
    strings: Option<&mut StringTables>,
) -> Result<Page, String> {
    run_page_with(
        lo,
        reader,
        request,
        strings,
        kitkraft_schema::builtin().ok(),
    )
}

/// [`run_page`], decoding field paths through `schema` (the builtin definitions in
/// [`run_page`]).
pub fn run_page_with(
    lo: &LoadOrder,
    reader: &mut RecordReader,
    request: &PageRequest<'_>,
    mut strings: Option<&mut StringTables>,
    schema: Option<&kitkraft_schema::Schema>,
) -> Result<Page, String> {
    let PageRequest {
        signature,
        query,
        scope,
        limit,
        cursor,
    } = *request;
    let limit = limit.clamp(1, MAX_LIMIT);
    let fp = fingerprint(lo, signature, query, scope);
    let start = match cursor {
        Some(c) => decode_cursor(c, fp)?,
        None => 0,
    };
    let all = candidates(lo, signature, scope);
    let mut page = Page {
        candidates: all.len(),
        ..Page::default()
    };
    let mut pos = start;
    while let Some(v) = all.get(pos) {
        pos = pos.saturating_add(1);
        page.scanned = page.scanned.saturating_add(1);
        let verdict = match query {
            None => Tri::True,
            Some(q) => {
                let mut reasons = Vec::new();
                let mut subject = VersionSubject::new(lo, reader, *v)
                    .with_strings(strings.as_deref_mut())
                    .with_schema(schema);
                let t = q.eval(&mut subject, &mut reasons);
                if t == Tri::Unknown {
                    page.unknown_count = page.unknown_count.saturating_add(1);
                    page.reasons.extend(reasons.iter().cloned());
                    if page.unknown.len() < UNKNOWN_LISTED {
                        page.unknown.push((*v, reasons));
                    }
                }
                t
            }
        };
        if verdict == Tri::True {
            page.matched.push(*v);
            if page.matched.len() >= limit {
                break;
            }
        }
    }
    if pos < all.len() {
        page.next_cursor = Some(encode_cursor(fp, pos));
    }
    Ok(page)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::load_order::{LoadOptions, PluginSpec};
    use crate::testdata::{self, rec};

    fn lo() -> LoadOrder {
        let dir = testdata::temp_dir("search");
        let base = testdata::plugin_with(
            0x1,
            &[],
            &(0..30u32)
                .map(|i| {
                    rec(
                        b"WEAP",
                        0x800 + i,
                        &[
                            ("EDID", format!("Gun{i}\0").into_bytes()),
                            ("FULL", format!("Rifle {i}\0").into_bytes()),
                        ],
                    )
                })
                .chain(std::iter::once(rec(
                    b"KYWD",
                    0x900,
                    &[("EDID", b"Kw\0".to_vec())],
                )))
                .collect::<Vec<_>>(),
        );
        let patch = testdata::plugin_with(
            0x1,
            &["Base.esm"],
            &[rec(
                b"WEAP",
                0x805,
                &[
                    ("EDID", b"Gun5\0".to_vec()),
                    ("FULL", b"Renamed\0".to_vec()),
                ],
            )],
        );
        let localized = testdata::plugin_with(
            0x81,
            &["Base.esm"],
            &[rec(b"WEAP", 0x806, &[("FULL", vec![1, 0, 0, 0])])],
        );
        let mut specs = Vec::new();
        for (name, bytes) in [
            ("Base.esm", base),
            ("Patch.esm", patch),
            ("Loc.esm", localized),
        ] {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            specs.push(PluginSpec {
                name: name.into(),
                path,
            });
        }
        LoadOrder::load(specs, &LoadOptions::default()).unwrap()
    }

    fn page(
        lo: &LoadOrder,
        reader: &mut RecordReader,
        sig: &[u8; 4],
        q: Option<&Query>,
        scope: Scope,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<Page, String> {
        let request = PageRequest {
            signature: *sig,
            query: q,
            scope,
            limit,
            cursor,
        };
        run_page(lo, reader, &request, None)
    }

    #[test]
    fn pages_cover_the_same_set_as_one_page() {
        let lo = lo();
        let mut reader = RecordReader::new();
        let q = Query::parse(r#"FULL ~ "rifle""#).unwrap();
        let all = page(
            &lo,
            &mut reader,
            b"WEAP",
            Some(&q),
            Scope::Winner,
            1000,
            None,
        )
        .unwrap();
        // 30 weapons; Gun5 renamed by Patch, Gun6 localized in Loc (unknown).
        assert_eq!(all.candidates, 30);
        assert_eq!(all.matched.len(), 28);
        assert_eq!(all.unknown_count, 1);
        assert!(all
            .reasons
            .iter()
            .next()
            .unwrap()
            .contains("string ID 0x00000001"));
        assert!(all.next_cursor.is_none());
        let mut paged = Vec::new();
        let mut cursor = None;
        loop {
            let p = page(
                &lo,
                &mut reader,
                b"WEAP",
                Some(&q),
                Scope::Winner,
                7,
                cursor.as_deref(),
            )
            .unwrap();
            paged.extend(p.matched);
            match p.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        assert_eq!(paged, all.matched);
        // A cursor for another query is refused.
        let other = Query::parse(r#"FULL ~ "x""#).unwrap();
        let c = page(&lo, &mut reader, b"WEAP", Some(&q), Scope::Winner, 7, None)
            .unwrap()
            .next_cursor
            .unwrap();
        assert!(page(
            &lo,
            &mut reader,
            b"WEAP",
            Some(&other),
            Scope::Winner,
            7,
            Some(&c)
        )
        .is_err());
    }

    #[test]
    fn localized_strings_resolve_through_tables() {
        use crate::strings::{DirSource, StringTables};
        use kitkraft_strings::{Entry, Kind, Table};
        let lo = lo();
        let dir = lo.plugins[2].path.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(dir.join("Strings")).unwrap();
        let table = Table::new(Kind::Strings, vec![Entry::new(1, "Localized Rifle")]);
        std::fs::write(
            dir.join("Strings").join("loc_en.strings"),
            table.to_bytes().unwrap(),
        )
        .unwrap();
        let mut st = StringTables::new("en", Box::new(DirSource { data_dir: dir }));
        let mut reader = RecordReader::new();
        let q = Query::parse(r#"FULL ~ "localized""#).unwrap();
        let request = PageRequest {
            signature: *b"WEAP",
            query: Some(&q),
            scope: Scope::Winner,
            limit: 50,
            cursor: None,
        };
        let p = run_page(&lo, &mut reader, &request, Some(&mut st)).unwrap();
        assert_eq!(p.matched.len(), 1);
        assert_eq!(p.unknown_count, 0);
        assert_eq!(p.matched[0].plugin, 2);
    }

    #[test]
    fn scope_all_and_order() {
        let lo = lo();
        let mut reader = RecordReader::new();
        let q = Query::parse(r#"@edid == "Gun5""#).unwrap();
        let winner = page(&lo, &mut reader, b"WEAP", Some(&q), Scope::Winner, 50, None).unwrap();
        assert_eq!(
            winner.matched.iter().map(|v| v.plugin).collect::<Vec<_>>(),
            [1]
        );
        let all = page(&lo, &mut reader, b"WEAP", Some(&q), Scope::All, 50, None).unwrap();
        assert_eq!(
            all.matched.iter().map(|v| v.plugin).collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(all.candidates, 32);
        let none = page(&lo, &mut reader, b"KYWD", None, Scope::Winner, 50, None).unwrap();
        assert_eq!(none.matched.len(), 1);
    }

    /// Review round 2: a deleted winner's payload is never evaluated silently; index paths
    /// still work (`@flags & 0x20` finds it).
    #[test]
    fn deleted_winners_are_not_evaluated() {
        use crate::testdata::rec_flags;
        let dir = testdata::temp_dir("search-deleted");
        let base = testdata::plugin_with(
            0x1,
            &[],
            &[rec(
                b"WEAP",
                0x800,
                &[("EDID", b"Gun\0".to_vec()), ("FULL", b"Rifle\0".to_vec())],
            )],
        );
        // The patch deletes the weapon but leaves stale subrecords behind.
        let patch = testdata::plugin_with(
            0x1,
            &["Base.esm"],
            &[rec_flags(
                b"WEAP",
                0x20,
                0x800,
                &[("EDID", b"Gun\0".to_vec()), ("FULL", b"Rifle\0".to_vec())],
            )],
        );
        let mut specs = Vec::new();
        for (name, bytes) in [("Base.esm", base), ("Patch.esm", patch)] {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            specs.push(PluginSpec {
                name: name.into(),
                path,
            });
        }
        let lo = LoadOrder::load(specs, &LoadOptions::default()).unwrap();
        let mut reader = RecordReader::new();
        let q = Query::parse(r#"FULL ~ "rifle""#).unwrap();
        let p = page(&lo, &mut reader, b"WEAP", Some(&q), Scope::Winner, 50, None).unwrap();
        assert!(p.matched.is_empty());
        assert_eq!(p.unknown_count, 1);
        assert!(
            p.reasons.iter().any(|r| r.contains("deleted")),
            "{:?}",
            p.reasons
        );
        let has = Query::parse("has FULL").unwrap();
        let p = page(
            &lo,
            &mut reader,
            b"WEAP",
            Some(&has),
            Scope::Winner,
            50,
            None,
        )
        .unwrap();
        assert_eq!(p.unknown_count, 1);
        // The base version (not deleted) still evaluates under scope all.
        let p = page(&lo, &mut reader, b"WEAP", Some(&q), Scope::All, 50, None).unwrap();
        assert_eq!(p.matched.iter().map(|v| v.plugin).collect::<Vec<_>>(), [0]);
        let flags = Query::parse("@flags & 0x20").unwrap();
        let p = page(
            &lo,
            &mut reader,
            b"WEAP",
            Some(&flags),
            Scope::Winner,
            50,
            None,
        )
        .unwrap();
        assert_eq!(p.matched.len(), 1);
    }

    /// Review round 2: `@flags` of a version whose record is missing from the index is
    /// unknown, not 0.
    #[test]
    fn flags_without_an_index_entry_are_unknown() {
        use crate::query::{Lookup, Path, Subject};
        let lo = lo();
        let mut reader = RecordReader::new();
        let mut v = lo
            .forms
            .winner(lo.forms.runs().next().unwrap()[0].form())
            .copied()
            .unwrap();
        v.record = 999_999;
        let mut subject = crate::subject::VersionSubject::new(&lo, &mut reader, v);
        assert!(matches!(subject.lookup(&Path::Flags), Lookup::Unknown(_)));
    }
}
