//! Where a patch's records come from: plugins in load order and the winning version of a
//! form among them (M2.E4, M2.E5).
//!
//! This is the explicit-list form `set` and `build` use until lane D's load order is in the
//! session: the caller names the plugins in load order (`plugins`), each is indexed
//! (`kitcraft_esm::index`), and a form is found by:
//!
//! - `Plugin.esm:00ABCDEF`: the object ID of a form owned by `Plugin.esm`; in every plugin
//!   that has `Plugin.esm` as itself or as a master, the stored FormID is that plugin's
//!   index for it in the top byte (file-relative FormIDs, `plan/execution-plan.md`
//!   "Format facts"); the winner is the last plugin in the list holding a record with it.
//! - an EditorID (ASCII case-insensitive): the first plugin whose record has it names the
//!   form, which is then resolved as above (so a later override wins).
//!
//! Small and medium plugins (TES4 bits 0x100, 0x400) address their masters by scale class
//! (lane D, `docs/load-order.md`); resolving across them is refused here rather than
//! guessed.

use std::fs::File;
use std::path::{Path, PathBuf};

use kitcraft_esm::index::{index_file, read_indexed, IndexOptions, PluginIndex, RecordLoc};
use kitcraft_esm::tes4::{Tes4, FLAG_LOCALIZED, FLAG_MEDIUM, FLAG_SMALL};
use kitcraft_esm::{DecodeLimits, GroupHeader, Record};

use super::fields::split_formref;

/// TES4 facts of a plugin a patch copies from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePlugin {
    /// File name (`Starfield.esm`).
    pub name: String,
    /// Path on disk.
    pub path: PathBuf,
    /// Master file names in order.
    pub masters: Vec<String>,
    /// Raw TES4 flags.
    pub flags: u32,
}

impl SourcePlugin {
    /// Whether the localized flag (0x80) is set.
    pub fn localized(&self) -> bool {
        self.flags & FLAG_LOCALIZED != 0
    }

    /// Whether a small or medium bit is set (FormIDs then address masters by scale class).
    pub fn scaled(&self) -> bool {
        self.flags & (FLAG_SMALL | FLAG_MEDIUM) != 0
    }

    /// The facts in `index`'s header, for the plugin at `path`.
    pub fn from_index(path: &Path, index: &PluginIndex) -> SourcePlugin {
        let tes4 = Tes4::new(&index.header);
        SourcePlugin {
            name: file_name(path),
            path: path.to_path_buf(),
            masters: tes4.masters().iter().map(|m| m.name_lossy()).collect(),
            flags: tes4.flags(),
        }
    }

    /// The master-list index this plugin uses for `owner` (its masters, then itself), or
    /// `None` when `owner` is neither.
    pub fn index_of(&self, owner: &str) -> Option<usize> {
        if owner.eq_ignore_ascii_case(&self.name) {
            return Some(self.masters.len());
        }
        self.masters
            .iter()
            .position(|m| m.eq_ignore_ascii_case(owner))
    }

    /// The plugin that owns a FormID stored in this plugin (its top byte names a master,
    /// or this plugin when it equals the master count).
    pub fn owner_of(&self, form_id: u32) -> Option<&str> {
        let index = usize::try_from(form_id >> 24).ok()?;
        match index.cmp(&self.masters.len()) {
            std::cmp::Ordering::Less => self.masters.get(index).map(String::as_str),
            std::cmp::Ordering::Equal => Some(self.name.as_str()),
            std::cmp::Ordering::Greater => None,
        }
    }
}

/// The file name of `path` as shown in results.
pub fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// One indexed plugin.
#[derive(Debug)]
pub struct Indexed {
    /// Its header facts.
    pub plugin: SourcePlugin,
    /// Its record index.
    pub index: PluginIndex,
}

/// Indexes the plugins at `paths`, in order.
pub fn index_plugins(paths: &[PathBuf]) -> Result<Vec<Indexed>, String> {
    paths
        .iter()
        .map(|path| {
            let index = index_file(path, &IndexOptions::default())
                .map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(Indexed {
                plugin: SourcePlugin::from_index(path, &index),
                index,
            })
        })
        .collect()
}

/// A form's identity independent of any one plugin: the owning plugin's file name and the
/// object ID (low 24 bits).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalForm {
    /// File name of the plugin that owns (created) the form.
    pub owner: String,
    /// Object ID.
    pub object_id: u32,
}

impl GlobalForm {
    /// `Plugin.esm:00ABCDEF`.
    pub fn formref(&self) -> String {
        format!("{}:{:08X}", self.owner, self.object_id)
    }
}

/// The version of a form a patch copies, and where it sits.
#[derive(Debug)]
pub struct Winner<'p> {
    /// Index of the winning plugin in the list.
    pub plugin: usize,
    /// The record's index entry in that plugin.
    pub loc: &'p RecordLoc,
    /// The form's identity.
    pub form: GlobalForm,
    /// Every plugin (by list index) that has a version, in load order.
    pub versions: Vec<usize>,
}

/// Finds the winning version of `form` (formref or EditorID) among `plugins` (load order).
pub fn find_winner<'p>(plugins: &'p [Indexed], form: &str) -> Result<Winner<'p>, String> {
    let global = match split_formref(form) {
        Some((owner, id)) => {
            if id > 0x00FF_FFFF {
                return Err(format!(
                    "`{form}`: a formref's object ID has at most 24 bits (Plugin.esm:00ABCDEF)"
                ));
            }
            GlobalForm {
                owner: owner.to_string(),
                object_id: id,
            }
        }
        None => by_editor_id(plugins, form)?,
    };
    let mut versions: Vec<(usize, &RecordLoc)> = Vec::new();
    for (i, p) in plugins.iter().enumerate() {
        let Some(index) = p.plugin.index_of(&global.owner) else {
            continue;
        };
        if p.plugin.scaled() {
            return Err(format!(
                "{} is a small or medium plugin: its FormIDs address masters by scale class, \
                 which needs lane D's load-order resolution (not available to this command yet)",
                p.plugin.name
            ));
        }
        let Some(raw) = u32::try_from(index)
            .ok()
            .filter(|i| *i < 0xFD)
            .map(|i| (i << 24) | global.object_id)
        else {
            continue;
        };
        if let Some(loc) = p.index.records.iter().skip(1).find(|r| r.form_id == raw) {
            versions.push((i, loc));
        }
    }
    let Some(&(plugin, loc)) = versions.last() else {
        return Err(format!(
            "no plugin in the load order has a record for {}",
            global.formref()
        ));
    };
    Ok(Winner {
        plugin,
        loc,
        form: global,
        versions: versions.iter().map(|(i, _)| *i).collect(),
    })
}

fn by_editor_id(plugins: &[Indexed], editor_id: &str) -> Result<GlobalForm, String> {
    for p in plugins {
        let hit = p.index.records.iter().skip(1).find(|r| {
            r.edid
                .as_deref()
                .is_some_and(|e| e.eq_ignore_ascii_case(editor_id.as_bytes()))
        });
        if let Some(loc) = hit {
            let owner = p.plugin.owner_of(loc.form_id).ok_or_else(|| {
                format!(
                    "{}: record {:08X} has a FormID beyond its master list",
                    p.plugin.name, loc.form_id
                )
            })?;
            return Ok(GlobalForm {
                owner: owner.to_string(),
                object_id: loc.form_id & 0x00FF_FFFF,
            });
        }
    }
    Err(format!(
        "`{editor_id}` is neither a formref (Plugin.esm:00ABCDEF) nor an EditorID in the load order"
    ))
}

/// The record at `loc` of the plugin at `path`, decoded, and the group path to place a
/// copy of it.
pub fn read_copy(
    path: &Path,
    index: &PluginIndex,
    loc: &RecordLoc,
    limits: &DecodeLimits,
) -> Result<(Record<'static>, Vec<GroupHeader>), String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let record = read_indexed(&mut file, loc, Some(limits))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((record, index.group_headers(loc)))
}

/// Group types whose label is the FormID of the record that owns the group (world
/// children 1, cell children 6, topic children 7; UESP, Skyrim_Mod:Mod_File_Format,
/// "Groups"; quest children 10, lane A's corpus inventory, M4.A1). A copy of a record
/// under one of these needs that record in the target too (lane A's `upsert_styled`
/// refuses an ownerless children group).
pub const PARENT_LABELLED: [i32; 4] = [1, 6, 7, 10];

/// The records that own the groups on `loc`'s path (outermost first), each with its own
/// group path: what [`kitcraft_esm::Plugin::upsert`] needs placed first (lane A's
/// CELL/WRLD parent rule).
pub fn parents_of<'i>(
    index: &'i PluginIndex,
    loc: &RecordLoc,
) -> Vec<(&'i RecordLoc, Vec<GroupHeader>)> {
    let path = index.group_path(loc.group);
    let mut out = Vec::new();
    for g in path {
        if !PARENT_LABELLED.contains(&g.header.group_type) {
            continue;
        }
        let owner = u32::from_le_bytes(g.header.label);
        let parent = index
            .records
            .iter()
            .skip(1)
            .find(|r| r.form_id == owner && r.group == g.parent && r.offset < g.offset);
        if let Some(parent) = parent {
            out.push((parent, index.group_headers(parent)));
        }
    }
    out
}

/// Where a record sits in its file: the groups around it, outermost first, and for each
/// group of a [`PARENT_LABELLED`] type the offset of the record that owns it (the record
/// right before the group whose FormID is its label), when there is one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Placement {
    /// Group headers, outermost first.
    pub path: Vec<GroupHeader>,
    /// `(depth in path, offset of the owning record)`.
    pub owners: Vec<(usize, u64)>,
}

/// Finds the [`Placement`] of the record whose header is at `offset` by reading group and
/// record headers only (24 bytes each, UESP's container layout; lane A's parser checks
/// the same sizes): siblings are skipped by their size fields, and only the groups that
/// contain `offset` are entered. Bounded by the file length and a depth of 64.
pub fn placement_at(path: &Path, offset: u64) -> Result<Placement, String> {
    use std::io::{Read, Seek, SeekFrom};
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let mut file = File::open(path).map_err(|e| err(&e))?;
    let len = file.metadata().map_err(|e| err(&e))?.len();
    let mut head = |at: u64| -> Result<(Signature4, u64, [u8; 24]), String> {
        let mut buf = [0u8; 24];
        file.seek(SeekFrom::Start(at)).map_err(|e| err(&e))?;
        file.read_exact(&mut buf).map_err(|e| err(&e))?;
        let size = u64::from(u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]));
        Ok(([buf[0], buf[1], buf[2], buf[3]], size, buf))
    };
    let mut out = Placement::default();
    // The TES4 record first, then the top level.
    let (_, tes4_size, _) = head(0)?;
    let mut at = tes4_size.saturating_add(24);
    let mut end = len;
    let mut last_record: Option<(u64, u32)> = None;
    while at < end {
        if out.path.len() > 64 {
            return Err(err(&"groups nest deeper than 64"));
        }
        let (sig, size, buf) = head(at)?;
        if at == offset {
            if &sig == b"GRUP" {
                return Err(err(&format!("offset {offset} holds a group, not a record")));
            }
            return Ok(out);
        }
        if &sig == b"GRUP" {
            let group_end = at.saturating_add(size);
            if size < 24 || group_end > end {
                return Err(err(&format!("group at {at} has a bad size {size}")));
            }
            if (at..group_end).contains(&offset) {
                let header = GroupHeader {
                    label: [buf[8], buf[9], buf[10], buf[11]],
                    group_type: i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
                    timestamp: u16::from_le_bytes([buf[16], buf[17]]),
                    version_control: u16::from_le_bytes([buf[18], buf[19]]),
                    unknown: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
                };
                if PARENT_LABELLED.contains(&header.group_type) {
                    let label = u32::from_le_bytes(header.label);
                    if let Some((owner_at, _)) = last_record.filter(|(_, id)| *id == label) {
                        out.owners.push((out.path.len(), owner_at));
                    }
                }
                out.path.push(header);
                end = group_end;
                at = at.saturating_add(24);
                last_record = None;
                continue;
            }
            at = group_end;
        } else {
            let form_id = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
            last_record = Some((at, form_id));
            at = at.saturating_add(24).saturating_add(size);
        }
    }
    Err(err(&format!("no record starts at offset {offset}")))
}

type Signature4 = [u8; 4];
