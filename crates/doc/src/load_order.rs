//! The load order: plugins in order, their headers and indexes, master links, FormID
//! resolution and the override table.
//!
//! Every plugin handed to [`LoadOrder::load`] is treated as active (choosing which plugins
//! are active is the profile reader's job, `kitkraft-io`). A plugin that fails to parse
//! stays in the list with its error and no records, so one broken file does not hide the
//! rest; FormIDs of plugins that name it as a master cannot be resolved.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::cache::{CacheUse, IndexCache};
use crate::error::{DocError, Result};
use crate::flags::Scale;
use crate::form::{runtime_slots, GlobalForm, MasterMap, ResolveError, RuntimeSlot};
use crate::hash::Fnv;
use crate::index::{HeaderInfo, PluginIndex, RecordLoc};
use crate::overrides::FormTable;

/// Most plugins in one load order (indices are `u16`).
pub const MAX_PLUGINS: usize = 0xFFFF;

/// One plugin to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSpec {
    /// File name as listed (used for master matching, case-insensitively).
    pub name: String,
    /// Where the file is.
    pub path: PathBuf,
}

/// Settings for [`LoadOrder::load`].
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// Index cache; `None` builds every index from the file.
    pub cache: Option<IndexCache>,
    /// Worker threads for indexing (0 = available parallelism, at most 4).
    pub threads: usize,
}

/// A master as named in a plugin's header, and where it is in the load order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterLink {
    /// Name as stored in `MAST`.
    pub name: String,
    /// Load-order index of the plugin with that name (ASCII case-insensitive).
    pub plugin: Option<u16>,
}

/// One loaded plugin.
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    /// File name as listed.
    pub name: String,
    /// Path read.
    pub path: PathBuf,
    /// The index, `None` when the plugin failed to load (see `error`).
    pub index: Option<Arc<PluginIndex>>,
    /// Why it failed to load.
    pub error: Option<String>,
    /// How the index was obtained.
    pub cache: CacheUse,
    /// Masters with their load-order positions.
    pub masters: Vec<MasterLink>,
    /// FormID resolution for this file, or why it is impossible.
    pub map: std::result::Result<MasterMap, ResolveError>,
    /// Runtime slot (an assumption; `docs/load-order.md`, "Runtime IDs").
    pub runtime_slot: Option<RuntimeSlot>,
}

impl LoadedPlugin {
    /// Header facts, when loaded.
    pub fn header(&self) -> Option<&HeaderInfo> {
        self.index.as_deref().map(|i| &i.header)
    }

    /// Scale from the header flags (`None` when not loaded or both scale bits are set).
    pub fn scale(&self) -> Option<Scale> {
        self.header().and_then(|h| h.flags.scale())
    }

    /// Masters not in the load order.
    pub fn missing_masters(&self) -> Vec<&str> {
        self.masters
            .iter()
            .filter(|m| m.plugin.is_none())
            .map(|m| m.name.as_str())
            .collect()
    }

    /// Masters that load after this plugin.
    pub fn late_masters(&self, own: u16) -> Vec<&str> {
        self.masters
            .iter()
            .filter(|m| m.plugin.is_some_and(|p| p > own))
            .map(|m| m.name.as_str())
            .collect()
    }

    /// Records (empty when not loaded).
    pub fn records(&self) -> &[RecordLoc] {
        self.index.as_deref().map_or(&[], |i| &i.records)
    }
}

/// The resolved load order.
#[derive(Debug, Clone)]
pub struct LoadOrder {
    /// Plugins in load order.
    pub plugins: Vec<LoadedPlugin>,
    /// Every version of every form.
    pub forms: FormTable,
    /// Time spent indexing (cache reads included).
    pub index_ms: u64,
    /// Time spent building the override table.
    pub table_ms: u64,
}

impl LoadOrder {
    /// Index every plugin (in parallel, through the cache), link masters, and build the
    /// override table.
    pub fn load(specs: Vec<PluginSpec>, options: &LoadOptions) -> Result<LoadOrder> {
        if specs.len() > MAX_PLUGINS {
            return Err(DocError::Invalid(format!(
                "{} plugins: at most {MAX_PLUGINS} are supported",
                specs.len()
            )));
        }
        let started = Instant::now();
        let loaded = index_all(&specs, options);
        let index_ms = elapsed_ms(started);

        let names: Vec<String> = specs.iter().map(|s| s.name.to_ascii_lowercase()).collect();
        let find = |name: &str| -> Option<u16> {
            let lower = name.to_ascii_lowercase();
            names
                .iter()
                .position(|n| *n == lower)
                .and_then(|i| u16::try_from(i).ok())
        };
        let mut plugins: Vec<LoadedPlugin> = specs
            .into_iter()
            .zip(loaded)
            .map(|(spec, result)| {
                let (index, error, cache) = match result {
                    Ok((index, cache)) => (Some(Arc::new(index)), None, cache),
                    Err(e) => (None, Some(e.to_string()), CacheUse::Uncached),
                };
                let masters = index
                    .as_deref()
                    .map(|i| {
                        i.header
                            .masters
                            .iter()
                            .map(|m| MasterLink {
                                name: m.clone(),
                                plugin: find(m),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                LoadedPlugin {
                    name: spec.name,
                    path: spec.path,
                    index,
                    error,
                    cache,
                    masters,
                    map: Err(ResolveError::BadScale {
                        plugin: String::new(),
                    }),
                    runtime_slot: None,
                }
            })
            .collect();

        let scales: Vec<Option<Scale>> = plugins.iter().map(LoadedPlugin::scale).collect();
        let states: Vec<MasterState> = plugins.iter().map(MasterState::of).collect();
        for (i, p) in plugins.iter_mut().enumerate() {
            let own = u16::try_from(i).unwrap_or(u16::MAX);
            p.map = master_map(p, own, &states);
        }
        let slots = runtime_slots(
            &scales
                .iter()
                .map(|s| (true, *s))
                .collect::<Vec<(bool, Option<Scale>)>>(),
        );
        for (p, slot) in plugins.iter_mut().zip(slots) {
            p.runtime_slot = slot;
        }

        let started = Instant::now();
        let forms = FormTable::build(&plugins);
        Ok(LoadOrder {
            plugins,
            forms,
            index_ms,
            table_ms: elapsed_ms(started),
        })
    }

    /// Load-order index of the plugin named `name` (ASCII case-insensitive).
    pub fn find(&self, name: &str) -> Option<u16> {
        self.plugins
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
            .and_then(|i| u16::try_from(i).ok())
    }

    /// The plugin at `index`.
    pub fn plugin(&self, index: u16) -> Option<&LoadedPlugin> {
        self.plugins.get(usize::from(index))
    }

    /// Resolve a raw FormID read from plugin `index`.
    pub fn resolve(&self, index: u16, raw: u32) -> std::result::Result<GlobalForm, ResolveError> {
        match self.plugin(index) {
            Some(p) => p.map.clone().and_then(|m| m.resolve(raw)),
            None => Err(ResolveError::PluginNotLoaded {
                plugin: format!("#{index}"),
                reason: "no plugin at this load-order index".into(),
            }),
        }
    }

    /// The raw FormID plugin `index` stores for `form`.
    pub fn to_raw(&self, index: u16, form: GlobalForm) -> Option<u32> {
        self.plugin(index)?.map.as_ref().ok()?.to_raw(form)
    }

    /// Runtime FormID of `form` under the runtime-slot assumption.
    pub fn runtime_id(&self, form: GlobalForm) -> Option<u32> {
        self.plugin(form.plugin)?
            .runtime_slot?
            .runtime_id(form.object_id)
    }

    /// The form whose runtime ID is `runtime` (under the same assumption).
    pub fn from_runtime_id(&self, runtime: u32) -> Option<GlobalForm> {
        let r = crate::form::RawRef::decode(runtime);
        let slot = match r.class {
            Scale::Full => RuntimeSlot::Full(u8::try_from(r.index).ok()?),
            Scale::Medium => RuntimeSlot::Medium(u8::try_from(r.index).ok()?),
            Scale::Small => RuntimeSlot::Small(r.index),
        };
        let plugin = self
            .plugins
            .iter()
            .position(|p| p.runtime_slot == Some(slot))?;
        Some(GlobalForm {
            plugin: u16::try_from(plugin).ok()?,
            object_id: r.object_id,
        })
    }

    /// The record of `plugin` at position `record` in its index.
    pub fn record(&self, plugin: u16, record: u32) -> Option<&RecordLoc> {
        self.plugin(plugin)?
            .records()
            .get(usize::try_from(record).ok()?)
    }

    /// EditorID of a record.
    pub fn edid(&self, plugin: u16, loc: &RecordLoc) -> Option<&str> {
        self.plugin(plugin)?.index.as_deref()?.edid(loc)
    }

    /// `Plugin.esm:XXXXXX` for a form.
    pub fn form_label(&self, form: GlobalForm) -> String {
        let name = self.plugin(form.plugin).map_or("?", |p| p.name.as_str());
        format!("{name}:{:06X}", form.object_id)
    }

    /// Whether a raw FormID written by plugin `a` means the same form when read in plugin
    /// `b`, for every form `a` can reference (its masters and itself). When it does, payload
    /// bytes of the two versions can be compared as stored.
    pub fn encodings_compatible(&self, a: u16, b: u16) -> bool {
        let (Some(pa), Some(pb)) = (self.plugin(a), self.plugin(b)) else {
            return false;
        };
        let (Ok(ma), Ok(mb)) = (pa.map.as_ref(), pb.map.as_ref()) else {
            return false;
        };
        let referenced = pa
            .masters
            .iter()
            .filter_map(|m| m.plugin)
            .chain(std::iter::once(a));
        for p in referenced {
            let g = GlobalForm {
                plugin: p,
                object_id: 0,
            };
            match (ma.to_raw(g), mb.to_raw(g)) {
                (Some(x), Some(y)) if x == y => {}
                _ => return false,
            }
        }
        true
    }

    /// Fingerprint of the plugin list (names, sizes, mtimes), for paging cursors.
    pub fn fingerprint(&self) -> u64 {
        let mut h = Fnv::new();
        for p in &self.plugins {
            h.write(p.name.to_ascii_lowercase().as_bytes());
            h.write(&[0]);
            let (size, mtime) = p.index.as_deref().map_or((0, 0), |i| (i.size, i.mtime_ns));
            h.write_u64(size);
            h.write_u64(mtime);
        }
        h.finish()
    }
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// What a plugin offers to the plugins that name it as a master.
enum MasterState {
    /// Loaded, with a scale.
    Scale(Scale),
    /// Loaded, but both scale bits are set.
    BadScale,
    /// Its file failed to load.
    Failed(String),
}

impl MasterState {
    fn of(p: &LoadedPlugin) -> MasterState {
        match (p.index.as_deref(), p.scale()) {
            (Some(_), Some(s)) => MasterState::Scale(s),
            (Some(_), None) => MasterState::BadScale,
            (None, _) => {
                MasterState::Failed(p.error.clone().unwrap_or_else(|| "no index".to_string()))
            }
        }
    }
}

fn master_map(
    p: &LoadedPlugin,
    own: u16,
    states: &[MasterState],
) -> std::result::Result<MasterMap, ResolveError> {
    if p.index.is_none() {
        return Err(ResolveError::PluginNotLoaded {
            plugin: p.name.clone(),
            reason: p.error.clone().unwrap_or_else(|| "no index".to_string()),
        });
    }
    let Some(own_scale) = p.scale() else {
        return Err(ResolveError::BadScale {
            plugin: p.name.clone(),
        });
    };
    let mut masters = Vec::with_capacity(p.masters.len());
    for m in &p.masters {
        let missing = || ResolveError::MissingMaster {
            plugin: p.name.clone(),
            master: m.name.clone(),
        };
        let index = m.plugin.ok_or_else(missing)?;
        let scale = match states.get(usize::from(index)) {
            Some(MasterState::Scale(s)) => *s,
            Some(MasterState::BadScale) => {
                return Err(ResolveError::BadScale {
                    plugin: m.name.clone(),
                })
            }
            Some(MasterState::Failed(reason)) => {
                return Err(ResolveError::MasterNotLoaded {
                    plugin: p.name.clone(),
                    master: m.name.clone(),
                    reason: reason.clone(),
                })
            }
            None => return Err(missing()),
        };
        masters.push((index, scale));
    }
    Ok(MasterMap::new(&p.name, own, own_scale, &masters))
}

type Indexed = Result<(PluginIndex, CacheUse)>;

fn index_one(path: &Path, cache: Option<&IndexCache>) -> Indexed {
    match cache {
        Some(c) => c.get_or_build(path),
        None => PluginIndex::build(path).map(|i| (i, CacheUse::Uncached)),
    }
}

/// Index every plugin with a small worker pool, largest files first.
fn index_all(specs: &[PluginSpec], options: &LoadOptions) -> Vec<Indexed> {
    let threads = match options.threads {
        0 => std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .min(4),
        n => n,
    }
    .max(1);
    let mut order: Vec<usize> = (0..specs.len()).collect();
    let size = |i: &usize| {
        specs
            .get(*i)
            .and_then(|s| std::fs::metadata(&s.path).ok())
            .map_or(0, |m| m.len())
    };
    order.sort_by_key(|i| std::cmp::Reverse(size(i)));
    let results: Mutex<Vec<Option<Indexed>>> = Mutex::new((0..specs.len()).map(|_| None).collect());
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let n = next.fetch_add(1, Ordering::SeqCst);
                let Some(&i) = order.get(n) else { break };
                let Some(spec) = specs.get(i) else { break };
                let result = index_one(&spec.path, options.cache.as_ref());
                if let Ok(mut slots) = results.lock() {
                    if let Some(slot) = slots.get_mut(i) {
                        *slot = Some(result);
                    }
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .zip(specs)
        .map(|(r, spec)| {
            r.unwrap_or_else(|| {
                Err(DocError::Invalid(format!(
                    "{}: indexing did not complete",
                    spec.name
                )))
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::testdata::{self, rec};

    pub(crate) fn spec(dir: &Path, name: &str, bytes: &[u8]) -> PluginSpec {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        PluginSpec {
            name: name.to_string(),
            path,
        }
    }

    #[test]
    fn masters_resolve_case_insensitively_and_missing_ones_are_errors() {
        let dir = testdata::temp_dir("lo");
        let base = testdata::plugin_with(0x1, &[], &[rec(b"WEAP", 0x0000_0800, &[])]);
        let small = testdata::plugin_with(0x101, &[], &[rec(b"KYWD", 0xFE00_0801, &[])]);
        let child = testdata::plugin_with(
            0x1,
            &["BASE.ESM", "Small.esm"],
            &[
                rec(b"WEAP", 0x0000_0800, &[]),
                rec(b"KYWD", 0xFE00_0801, &[]),
                rec(b"GLOB", 0x0100_0900, &[]),
            ],
        );
        let orphan = testdata::plugin_with(0x1, &["Nowhere.esm"], &[rec(b"WEAP", 0x800, &[])]);
        let lo = LoadOrder::load(
            vec![
                spec(&dir, "Base.esm", &base),
                spec(&dir, "Small.esm", &small),
                spec(&dir, "Child.esm", &child),
                spec(&dir, "Orphan.esm", &orphan),
            ],
            &LoadOptions::default(),
        )
        .unwrap();
        let child_p = &lo.plugins[2];
        assert_eq!(child_p.masters[0].plugin, Some(0));
        assert_eq!(child_p.masters[1].plugin, Some(1));
        assert_eq!(
            lo.resolve(2, 0xFE00_0801),
            Ok(GlobalForm {
                plugin: 1,
                object_id: 0x801
            })
        );
        assert_eq!(lo.resolve(2, 0x0100_0900).unwrap().plugin, 2);
        assert_eq!(lo.plugins[3].missing_masters(), ["Nowhere.esm"]);
        assert!(matches!(
            lo.resolve(3, 0x800),
            Err(ResolveError::MissingMaster { .. })
        ));
        // Overrides: WEAP 800 of Base has two versions, the winner is Child's.
        let g = GlobalForm {
            plugin: 0,
            object_id: 0x800,
        };
        let versions = lo.forms.versions(g);
        assert_eq!(
            versions.iter().map(|v| v.plugin).collect::<Vec<_>>(),
            [0, 2]
        );
        assert_eq!(lo.forms.winner(g).unwrap().plugin, 2);
        assert_eq!(lo.forms.unresolved_count(3), 1);
        // Runtime slots: Base 00, Small FE:000, Child 01, Orphan 02.
        assert_eq!(lo.runtime_id(g), Some(0x0000_0800));
        assert_eq!(
            lo.runtime_id(GlobalForm {
                plugin: 1,
                object_id: 0x801
            }),
            Some(0xFE00_0801)
        );
        assert_eq!(lo.from_runtime_id(0x0100_0900).unwrap().plugin, 2);
        assert_eq!(lo.form_label(g), "Base.esm:000800");
    }

    #[test]
    fn small_plugin_own_records_in_the_full_class_resolve_by_assumption() {
        let dir = testdata::temp_dir("lo-fullstyle");
        let base = testdata::plugin_with(0x1, &[], &[rec(b"WEAP", 0x800, &[])]);
        // A small plugin with one full master storing its own records as 01xxxxxx.
        let small = testdata::plugin_with(
            0x101,
            &["Base.esm"],
            &[
                rec(b"KYWD", 0x0100_0801, &[]),
                rec(b"KYWD", 0xFE00_0802, &[]),
            ],
        );
        let lo = LoadOrder::load(
            vec![
                spec(&dir, "Base.esm", &base),
                spec(&dir, "Small.esm", &small),
            ],
            &LoadOptions::default(),
        )
        .unwrap();
        assert_eq!(lo.forms.unresolved_count(1), 0);
        assert_eq!(lo.forms.full_style_self_count(1), 1);
        assert_eq!(lo.resolve(1, 0x0100_0801).unwrap().plugin, 1);
        assert_eq!(lo.resolve(1, 0xFE00_0802).unwrap().plugin, 1);
        // Index 2 is past the full masters and the file itself.
        assert!(lo.resolve(1, 0x0200_0801).is_err());
    }

    /// Ortham's `02051808` -> `FD011808` shape in a medium plugin: `01012345` is its own
    /// object `2345` (the `01` above the medium width dropped), said per version; a masked
    /// ID landing on another record of the same plugin is reported as a duplicate.
    #[test]
    fn medium_plugin_full_style_ids_are_masked_and_collisions_reported() {
        let dir = testdata::temp_dir("lo-masked");
        let base = testdata::plugin_with(0x1, &[], &[rec(b"WEAP", 0x800, &[])]);
        let medium = testdata::plugin_with(
            0x401,
            &["Base.esm"],
            &[
                rec(b"KYWD", 0x0101_2345, &[]),
                rec(b"KYWD", 0x0100_0900, &[]),
                rec(b"KYWD", 0xFD00_0A00, &[]),
                rec(b"KYWD", 0x0105_0A00, &[]),
            ],
        );
        let lo = LoadOrder::load(
            vec![
                spec(&dir, "Base.esm", &base),
                spec(&dir, "Medium.esm", &medium),
            ],
            &LoadOptions::default(),
        )
        .unwrap();
        assert_eq!(
            lo.resolve(1, 0x0101_2345),
            Ok(GlobalForm {
                plugin: 1,
                object_id: 0x2345
            })
        );
        let g = GlobalForm {
            plugin: 1,
            object_id: 0x2345,
        };
        let v = lo.forms.versions(g);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].basis, crate::form::Basis::FullStyleSelfMasked);
        let unmasked = lo.forms.versions(GlobalForm {
            plugin: 1,
            object_id: 0x900,
        });
        assert_eq!(unmasked[0].basis, crate::form::Basis::FullStyleSelf);
        assert_eq!(lo.forms.full_style_self_count(1), 3);
        assert_eq!(lo.forms.masked_count(1), 2);
        // FD000A00 and 01050A00 both mean Medium.esm:000A00.
        assert_eq!(lo.forms.duplicate_count(1), 1);
        let d = lo.forms.duplicates()[0];
        assert_eq!((d.plugin, d.form.object_id, d.masked), (1, 0xA00, true));
        assert_eq!((d.first, d.second), (2, 3));
        assert_eq!(lo.forms.duplicate_count(0), 0);
    }

    #[test]
    fn master_after_dependent_is_reported_and_still_resolves() {
        let dir = testdata::temp_dir("lo-late");
        let base = testdata::plugin_with(0x1, &[], &[rec(b"WEAP", 0x800, &[])]);
        let child = testdata::plugin_with(0x1, &["Base.esm"], &[rec(b"WEAP", 0x800, &[])]);
        let lo = LoadOrder::load(
            vec![
                spec(&dir, "Child.esm", &child),
                spec(&dir, "Base.esm", &base),
            ],
            &LoadOptions::default(),
        )
        .unwrap();
        assert_eq!(lo.plugins[0].late_masters(0), ["Base.esm"]);
        assert_eq!(lo.resolve(0, 0x800).unwrap().plugin, 1);
        // Versions are in load order: Child (0) first, then Base (1) wins.
        let g = GlobalForm {
            plugin: 1,
            object_id: 0x800,
        };
        assert_eq!(lo.forms.winner(g).unwrap().plugin, 1);
    }

    #[test]
    fn broken_plugin_stays_listed_with_its_error() {
        let dir = testdata::temp_dir("lo-broken");
        let base = testdata::plugin_with(0x1, &[], &[rec(b"WEAP", 0x800, &[])]);
        let lo = LoadOrder::load(
            vec![
                spec(&dir, "Base.esm", &base),
                spec(&dir, "Bad.esm", b"GRUPxxxx"),
                spec(
                    &dir,
                    "Child.esm",
                    &testdata::plugin_with(0x1, &["Bad.esm"], &[]),
                ),
            ],
            &LoadOptions {
                cache: Some(IndexCache::new(testdata::temp_dir("lo-broken-cache"))),
                threads: 2,
            },
        )
        .unwrap();
        assert!(lo.plugins[1].error.is_some());
        assert!(lo.plugins[1].index.is_none());
        // The broken plugin is not loaded; its dependent's master is not loaded (neither is
        // a missing master nor a bad scale).
        assert!(
            matches!(&lo.plugins[1].map, Err(ResolveError::PluginNotLoaded { plugin, .. }) if plugin == "Bad.esm"),
            "{:?}",
            lo.plugins[1].map
        );
        assert!(
            matches!(&lo.plugins[2].map, Err(ResolveError::MasterNotLoaded { plugin, master, reason })
                if plugin == "Child.esm" && master == "Bad.esm" && !reason.is_empty()),
            "{:?}",
            lo.plugins[2].map
        );
        assert!(matches!(
            lo.resolve(9, 0x800),
            Err(ResolveError::PluginNotLoaded { .. })
        ));
        assert_eq!(lo.plugins[0].cache, CacheUse::Built);
    }

    #[test]
    fn both_scale_bits_refuse_resolution() {
        let dir = testdata::temp_dir("lo-scale");
        let weird = testdata::plugin_with(0x501, &[], &[rec(b"WEAP", 0x800, &[])]);
        let child = testdata::plugin_with(0x1, &["Weird.esm"], &[rec(b"WEAP", 0x800, &[])]);
        let lo = LoadOrder::load(
            vec![
                spec(&dir, "Weird.esm", &weird),
                spec(&dir, "Child.esm", &child),
            ],
            &LoadOptions::default(),
        )
        .unwrap();
        assert!(matches!(
            lo.resolve(0, 0x800),
            Err(ResolveError::BadScale { .. })
        ));
        assert!(matches!(
            lo.resolve(1, 0x800),
            Err(ResolveError::BadScale { plugin }) if plugin == "Weird.esm"
        ));
        assert_eq!(lo.plugins[0].runtime_slot, None);
        assert_eq!(lo.plugins[1].runtime_slot, Some(RuntimeSlot::Full(0)));
    }
}
