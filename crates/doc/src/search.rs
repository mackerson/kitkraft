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

/// Evaluate one page.
pub fn run_page(
    lo: &LoadOrder,
    reader: &mut RecordReader,
    signature: [u8; 4],
    query: Option<&Query>,
    scope: Scope,
    limit: usize,
    cursor: Option<&str>,
) -> Result<Page, String> {
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
                let mut subject = VersionSubject::new(lo, reader, *v);
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

    #[test]
    fn pages_cover_the_same_set_as_one_page() {
        let lo = lo();
        let mut reader = RecordReader::new();
        let q = Query::parse(r#"FULL ~ "rifle""#).unwrap();
        let all = run_page(
            &lo,
            &mut reader,
            *b"WEAP",
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
            let page = run_page(
                &lo,
                &mut reader,
                *b"WEAP",
                Some(&q),
                Scope::Winner,
                7,
                cursor.as_deref(),
            )
            .unwrap();
            paged.extend(page.matched);
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        assert_eq!(paged, all.matched);
        // A cursor for another query is refused.
        let other = Query::parse(r#"FULL ~ "x""#).unwrap();
        let c = run_page(&lo, &mut reader, *b"WEAP", Some(&q), Scope::Winner, 7, None)
            .unwrap()
            .next_cursor
            .unwrap();
        assert!(run_page(
            &lo,
            &mut reader,
            *b"WEAP",
            Some(&other),
            Scope::Winner,
            7,
            Some(&c)
        )
        .is_err());
    }

    #[test]
    fn scope_all_and_order() {
        let lo = lo();
        let mut reader = RecordReader::new();
        let q = Query::parse(r#"@edid == "Gun5""#).unwrap();
        let winner = run_page(
            &lo,
            &mut reader,
            *b"WEAP",
            Some(&q),
            Scope::Winner,
            50,
            None,
        )
        .unwrap();
        assert_eq!(
            winner.matched.iter().map(|v| v.plugin).collect::<Vec<_>>(),
            [1]
        );
        let all = run_page(&lo, &mut reader, *b"WEAP", Some(&q), Scope::All, 50, None).unwrap();
        assert_eq!(
            all.matched.iter().map(|v| v.plugin).collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(all.candidates, 32);
        let none = run_page(&lo, &mut reader, *b"KYWD", None, Scope::Winner, 50, None).unwrap();
        assert_eq!(none.matched.len(), 1);
    }
}
