//! The override table: every version of every form, in load order; the last one wins.
//!
//! One sorted vector of `(form, plugin, record)` entries; the versions of a form are a
//! contiguous run found by binary search. 16 bytes per record, so `Starfield.esm`'s 3.8 M
//! records cost about 60 MB.

use crate::form::{GlobalForm, ResolveError};
use crate::load_order::LoadedPlugin;

/// One version of a form: which plugin and which record of its index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// [`GlobalForm::key`] of the form.
    pub key: u64,
    /// Load-order index of the plugin holding this version.
    pub plugin: u16,
    /// Position in that plugin's index.
    pub record: u32,
}

impl Version {
    /// The form.
    pub fn form(&self) -> GlobalForm {
        GlobalForm::from_key(self.key)
    }
}

/// A record whose FormID could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    /// Plugin.
    pub plugin: u16,
    /// Record position.
    pub record: u32,
    /// Why.
    pub error: ResolveError,
}

/// Unresolved records kept with their error, per plugin; the rest are only counted.
const UNRESOLVED_KEPT: usize = 20;

/// Every version of every form.
#[derive(Debug, Clone, Default)]
pub struct FormTable {
    entries: Vec<Version>,
    unresolved_counts: Vec<u64>,
    unresolved: Vec<Unresolved>,
    full_style_self: Vec<u64>,
}

impl FormTable {
    /// Resolve every record of every plugin and sort.
    pub fn build(plugins: &[LoadedPlugin]) -> FormTable {
        let total: usize = plugins.iter().map(|p| p.records().len()).sum();
        let mut entries = Vec::with_capacity(total);
        let mut unresolved_counts = vec![0u64; plugins.len()];
        let mut unresolved = Vec::new();
        let mut full_style_self = vec![0u64; plugins.len()];
        for (i, p) in plugins.iter().enumerate() {
            let Ok(plugin) = u16::try_from(i) else { break };
            let mut kept = 0usize;
            for (r, loc) in p.records().iter().enumerate() {
                let Ok(record) = u32::try_from(r) else { break };
                let result = p
                    .map
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|m| m.resolve_detailed(loc.form_id));
                match result {
                    Ok((form, assumed)) => {
                        if assumed {
                            if let Some(c) = full_style_self.get_mut(i) {
                                *c = c.saturating_add(1);
                            }
                        }
                        entries.push(Version {
                            key: form.key(),
                            plugin,
                            record,
                        });
                    }
                    Err(error) => {
                        if let Some(c) = unresolved_counts.get_mut(i) {
                            *c = c.saturating_add(1);
                        }
                        if kept < UNRESOLVED_KEPT {
                            kept = kept.saturating_add(1);
                            unresolved.push(Unresolved {
                                plugin,
                                record,
                                error,
                            });
                        }
                    }
                }
            }
        }
        entries.sort_unstable_by_key(|v| (v.key, v.plugin, v.record));
        FormTable {
            entries,
            unresolved_counts,
            unresolved,
            full_style_self,
        }
    }

    /// Every version of `form`, in load order (empty when no plugin has it).
    pub fn versions(&self, form: GlobalForm) -> &[Version] {
        let key = form.key();
        let start = self.entries.partition_point(|v| v.key < key);
        let end = self.entries.partition_point(|v| v.key <= key);
        self.entries.get(start..end).unwrap_or(&[])
    }

    /// The winning (last) version of `form`.
    pub fn winner(&self, form: GlobalForm) -> Option<&Version> {
        self.versions(form).last()
    }

    /// Every form's versions, by form key.
    pub fn runs(&self) -> impl Iterator<Item = &[Version]> {
        self.entries.chunk_by(|a, b| a.key == b.key)
    }

    /// Number of resolved records.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no record resolved.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Records of plugin `plugin` whose FormID did not resolve.
    pub fn unresolved_count(&self, plugin: u16) -> u64 {
        self.unresolved_counts
            .get(usize::from(plugin))
            .copied()
            .unwrap_or(0)
    }

    /// Records of plugin `plugin` resolved by the `full-style-self` assumption (its own
    /// records stored in the full class although the plugin is small or medium).
    pub fn full_style_self_count(&self, plugin: u16) -> u64 {
        self.full_style_self
            .get(usize::from(plugin))
            .copied()
            .unwrap_or(0)
    }

    /// The first unresolved records of each plugin, with their errors.
    pub fn unresolved(&self) -> &[Unresolved] {
        &self.unresolved
    }
}
