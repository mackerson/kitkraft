//! Whether the winning version of one form is a safe source to copy (M3.D1, lane E's round 2
//! request): a load order with problems somewhere is not by itself a reason to distrust a
//! winner those problems cannot touch.
//!
//! A problem *blocks* the winner when it can change which version wins or what the winner's
//! FormIDs mean:
//!
//! - the form has no version, or its winning version is deleted (record flag `0x20`);
//! - a plugin holding a version of the form loads before one of its masters (the order of
//!   the versions, and so the winner, is in doubt);
//! - a plugin loading after the winner may hold a version that the override table could not
//!   see: it failed to load (its records are unknown), or some of its records do not resolve
//!   (a missing or unloaded master, both scale bits, an index past every class) and one of
//!   them could mean this form. A record could mean it only when the plugin is the defining
//!   plugin or names it as a master (an override needs its master), and the record's stored
//!   object ID equals the form's under some class width (24, 16 or 12 bits).
//!
//! Everything else (a failed plugin loading before the winner, an unrelated ordering error,
//! an active plugin with no file, which the game does not load either) is *unrelated*: worth
//! reporting, never a reason to refuse. The winner's own masters are always loaded: a version
//! exists only when its plugin's FormIDs resolve, which needs every master's scale.

use crate::flags::RECORD_FLAG_DELETED;
use crate::form::GlobalForm;
use crate::load_order::{LoadOrder, LoadedPlugin};
use crate::overrides::Version;

/// The verdict for one form ([`LoadOrder::source_check`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceCheck {
    /// The winning version, when the form has one.
    pub winner: Option<Version>,
    /// Why the winner cannot be trusted (empty when it can).
    pub blocking: Vec<String>,
    /// Problems in the load order that do not affect this form.
    pub unrelated: Vec<String>,
}

impl SourceCheck {
    /// Whether the winner can be copied: it exists and nothing blocks it.
    pub fn is_safe(&self) -> bool {
        self.winner.is_some() && self.blocking.is_empty()
    }
}

/// Whether the stored object ID of `raw` equals `object_id` under some class width.
fn may_mean(raw: u32, object_id: u32) -> bool {
    [0x00FF_FFFF, 0xFFFF, 0xFFF]
        .iter()
        .any(|mask| raw & mask == object_id)
}

/// Whether `p` names `defining` as a master (ASCII case-insensitive) or is it.
fn may_override(p: &LoadedPlugin, defining: &str) -> bool {
    p.name.eq_ignore_ascii_case(defining)
        || p.masters
            .iter()
            .any(|m| m.name.eq_ignore_ascii_case(defining))
}

impl LoadOrder {
    /// Whether the winning version of `form` is a safe source (module docs).
    /// `missing_active` names active plugins that have no file (the profile reader knows
    /// them; the load order never saw them): they are always unrelated, since the game does
    /// not load them either, and a plugin that needs one as a master is judged by the
    /// unresolved-records rule.
    pub fn source_check(&self, form: GlobalForm, missing_active: &[String]) -> SourceCheck {
        let versions = self.forms.versions(form);
        let winner = versions.last().copied();
        let mut check = SourceCheck {
            winner,
            ..SourceCheck::default()
        };
        let label = self.form_label(form);
        let Some(w) = winner else {
            check
                .blocking
                .push(format!("{label} has no version in the load order"));
            return check;
        };
        if self
            .record(w.plugin, w.record)
            .is_some_and(|l| l.flags & RECORD_FLAG_DELETED != 0)
        {
            let name = self.plugin(w.plugin).map_or("?", |p| p.name.as_str());
            check.blocking.push(format!(
                "the winning version of {label} (in {name}) is deleted (record flag 0x20)"
            ));
        }
        let holders: Vec<u16> = versions.iter().map(|v| v.plugin).collect();
        let defining = self
            .plugin(form.plugin)
            .map_or_else(String::new, |p| p.name.clone());
        for (i, p) in self.plugins.iter().enumerate() {
            let Ok(index) = u16::try_from(i) else { break };
            let late = p.late_masters(index);
            if !late.is_empty() {
                let text = format!("{} loads before its master(s) {}", p.name, late.join(", "));
                if holders.contains(&index) {
                    check.blocking.push(format!(
                        "{text}, and it holds a version of {label}, so which version wins is in doubt"
                    ));
                } else {
                    check.unrelated.push(text);
                }
            }
            let after = index > w.plugin;
            if let Some(e) = &p.error {
                let text = format!("{} failed to load ({e})", p.name);
                if after {
                    check.blocking.push(format!(
                        "{text}; it loads after the winner, and its records are unknown, so it \
                         may override {label}"
                    ));
                } else {
                    check.unrelated.push(text);
                }
                continue;
            }
            let unresolved = self.forms.unresolved_count(index);
            if unresolved == 0 {
                continue;
            }
            let why = match &p.map {
                Err(e) => e.to_string(),
                Ok(_) => "FormIDs past every master class".to_string(),
            };
            let candidates = if after && may_override(p, &defining) {
                p.records()
                    .iter()
                    .filter(|r| {
                        may_mean(r.form_id, form.object_id)
                            && match &p.map {
                                Err(_) => true,
                                Ok(m) => m.resolve(r.form_id).is_err(),
                            }
                    })
                    .count()
            } else {
                0
            };
            if candidates > 0 {
                check.blocking.push(format!(
                    "{} loads after the winner and has {candidates} unresolved record(s) that \
                     may be versions of {label} ({why})",
                    p.name
                ));
            } else {
                check.unrelated.push(format!(
                    "{} has {unresolved} unresolved record(s) ({why}); none can be a later \
                     version of {label}",
                    p.name
                ));
            }
        }
        if !missing_active.is_empty() {
            check.unrelated.push(format!(
                "active plugins with no file (the game does not load them either): {}",
                missing_active.join(", ")
            ));
        }
        check
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::load_order::{LoadOptions, PluginSpec};
    use crate::testdata::{self, rec, rec_flags};

    fn load(tag: &str, plugins: &[(&str, Vec<u8>)]) -> LoadOrder {
        let dir = testdata::temp_dir(tag);
        let specs = plugins
            .iter()
            .map(|(name, bytes)| {
                let path = dir.join(name);
                std::fs::write(&path, bytes).unwrap();
                PluginSpec {
                    name: (*name).to_string(),
                    path,
                }
            })
            .collect();
        LoadOrder::load(specs, &LoadOptions::default()).unwrap()
    }

    fn form(plugin: u16, object_id: u32) -> GlobalForm {
        GlobalForm { plugin, object_id }
    }

    #[test]
    fn unrelated_problems_do_not_block_and_related_ones_do() {
        let base = testdata::plugin_with(
            0x1,
            &[],
            &[rec(b"WEAP", 0x800, &[]), rec(b"WEAP", 0x801, &[])],
        );
        // Overrides 800; names a master that is not in the load order, so none of its
        // records resolve, and one of them may be 801.
        let orphan = testdata::plugin_with(
            0x1,
            &["Base.esm", "Missing.esm"],
            &[rec(b"WEAP", 0x801, &[])],
        );
        // Loads before its master, holds a version of 800 only.
        let early = testdata::plugin_with(0x1, &["Late.esm"], &[rec(b"KYWD", 0x900, &[])]);
        let late = testdata::plugin_with(0x1, &[], &[rec(b"KYWD", 0x900, &[])]);
        let patch = testdata::plugin_with(0x1, &["Base.esm"], &[rec(b"WEAP", 0x800, &[])]);
        let lo = load(
            "source-check",
            &[
                ("Base.esm", base),
                ("Early.esm", early),
                ("Late.esm", late),
                ("Patch.esm", patch),
                ("Orphan.esm", orphan),
                ("Broken.esm", b"GRUPxxxx".to_vec()),
            ],
        );
        let missing = vec!["Gone.esm".to_string()];
        // Base:800 wins in Patch; Orphan has one record whose object ID is 801, not 800, and
        // Broken loads after it.
        let c = lo.source_check(form(0, 0x800), &missing);
        assert_eq!(c.winner.unwrap().plugin, 3);
        assert!(!c.is_safe());
        assert_eq!(c.blocking.len(), 1, "{c:?}");
        assert!(c.blocking[0].contains("Broken.esm failed to load"));
        assert!(c
            .unrelated
            .iter()
            .any(|u| u.contains("Early.esm loads before its master(s) Late.esm")));
        assert!(c
            .unrelated
            .iter()
            .any(|u| u.contains("Orphan.esm has 1 unresolved") && u.contains("none can be")));
        assert!(c.unrelated.iter().any(|u| u.contains("Gone.esm")));
        // Base:801: Orphan may override it.
        let c = lo.source_check(form(0, 0x801), &[]);
        assert!(c
            .blocking
            .iter()
            .any(|b| b.contains("Orphan.esm loads after the winner and has 1 unresolved")));
        // Early:900 (resolves to Late.esm): its holder loads before its master.
        let late_form = lo.resolve(1, 0x900).unwrap();
        let c = lo.source_check(late_form, &[]);
        assert!(c
            .blocking
            .iter()
            .any(|b| b.contains("Early.esm loads before") && b.contains("in doubt")));
        // No version at all.
        let c = lo.source_check(form(0, 0x999), &[]);
        assert!(c.winner.is_none() && !c.is_safe());
    }

    #[test]
    fn a_clean_load_order_is_safe_and_a_deleted_winner_is_not() {
        let base = testdata::plugin_with(
            0x1,
            &[],
            &[rec(b"WEAP", 0x800, &[]), rec(b"WEAP", 0x801, &[])],
        );
        let patch = testdata::plugin_with(
            0x1,
            &["Base.esm"],
            &[
                rec(b"WEAP", 0x800, &[]),
                rec_flags(b"WEAP", 0x20, 0x801, &[]),
            ],
        );
        // A broken plugin loading before the winner cannot win.
        let lo = load(
            "source-clean",
            &[
                ("Broken.esm", b"GRUPxxxx".to_vec()),
                ("Base.esm", base),
                ("Patch.esm", patch),
            ],
        );
        let c = lo.source_check(form(1, 0x800), &[]);
        assert!(c.is_safe(), "{c:?}");
        assert!(c.unrelated.iter().any(|u| u.contains("Broken.esm failed")));
        let c = lo.source_check(form(1, 0x801), &[]);
        assert!(!c.is_safe());
        assert!(c.blocking[0].contains("deleted"));
    }
}
