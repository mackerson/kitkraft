//! Lossless decode of the `REFL` subrecord, Bethesda's reflection serialization.
//! `crates/doc/tests/refl_corpus.rs` walks every one of the official files through this
//! module.
//!
//! The payload is a run of chunks: a 4-byte ASCII tag, a u32 little-endian size, then
//! exactly that many bytes. `decode` keeps each chunk's raw payload, so `encode`
//! reproduces the input byte for byte whatever the record type is; `lmsw_entries` reads
//! the entries of the `LMSW` payloads on top of that.
//!
//! Layout, verified on every official file (29897 `REFL`s, 17410 of them `LMSW`):
//!
//! - `BETH`: two u32: a version (4 in every payload) and the total number of chunks in
//!   the payload.
//! - `STRT`: NUL-terminated names; the other chunks name them by byte offset into this
//!   table.
//! - `TYPE`: a u32, the number of `CLAS` chunks.
//! - `CLAS`: a class: a u32 `STRT` offset of the class name, a u32 of flags (0 except in
//!   a handful of payloads), a u32 whose high u16 is the field count and whose low u16
//!   is 0, 4, 8 or 12, then the fields, each a u32 name offset, a u32 type (a
//!   `0xFFFFFFxx` built-in, or a `STRT` offset of another class), and two u16 that
//!   record the field's offset and size in bytes within the class.
//! - `OBJT`: a u32, 0 in every payload.
//! - `LIST`: a list: a u32 kind naming a class (by its `STRT` offset) or a built-in
//!   type (`0xFFFFFFxx`), a u32 item count, then the items.
//!
//! The `LMSW` payloads (`BGSLayeredMaterialSwap`) show exactly five list kinds:
//!
//! - the `0xFFFFFF01` lists, empty in every payload;
//! - the lists named by the `BGSLayeredMaterialSwap::Entry` class, the swaps: each item
//!   is a u16 length and NUL-terminated key material path, an 8-byte
//!   `KeyMaterialPathForm` (a u32 tag `0xFFFFFF0D` and a u32 form reference, 0 where the
//!   payload shows none), then a u16 length and NUL-terminated override material path;
//! - the `0xFFFFFF05` lists, each item a u32 tag `0xFFFFFF0D` and a u32 material form
//!   reference;
//! - in 18 payloads, a pair of lists: one named by the `BSBind::Snapshot::Entry` class,
//!   each item a u32 property class id followed by that property's float values, and
//!   one `0xFFFFFF02` list whose items are the property names, one per value item.
//!
//! 2586 `LMSW` payloads hold no swaps at all: 2567 of them hold only material form
//! references, and 19 hold only empty lists and yield no entries.

/// One chunk of a `REFL` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// The 4-byte tag.
    pub tag: [u8; 4],
    /// The chunk's bytes, as stored.
    pub payload: Vec<u8>,
}

/// A field of a `CLAS` class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// `STRT` offset of the field name.
    pub name_offset: u32,
    /// The field name, resolved from the string table.
    pub name: String,
    /// A `0xFFFFFFxx` built-in, or a `STRT` offset of another class.
    pub ty: u32,
    /// The field's offset in bytes within the class.
    pub a: u16,
    /// The field's size in bytes within the class.
    pub b: u16,
}

/// A `CLAS` class, in payload order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// `STRT` offset of the class name.
    pub name_offset: u32,
    /// The class name, resolved from the string table.
    pub name: String,
    /// The class header's flags (0 except in a handful of official payloads).
    pub flags: u32,
    /// The class header's third u32: the field count in the high u16, 0, 4, 8 or 12 in
    /// the low u16.
    pub field_words: u32,
    /// The fields, in payload order.
    pub fields: Vec<Field>,
}

/// One entry of an `LMSW` reflection.
#[derive(Debug, Clone, PartialEq)]
pub enum SwapEntry {
    /// A swap from the `Entries` list: the key material path mapped to the override
    /// material path, with the key material's form reference as stored (0 where the
    /// payload shows none).
    Swap {
        /// The key material path.
        key: String,
        /// The form reference of the key material.
        key_material_path_form: u32,
        /// The override material path.
        r#override: String,
    },
    /// A material form reference from the `SwapsCollection` list.
    Ref(u32),
    /// A material property from the snapshot lists: its name, the id of its property
    /// class, and its float values, in payload order.
    Property {
        /// The property name.
        name: String,
        /// The property class id.
        class_id: u32,
        /// The property's float values.
        values: Vec<f32>,
    },
}

/// A decoded `REFL` payload: every chunk, plus the string table and classes read off it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refl {
    /// The chunks, in payload order.
    pub chunks: Vec<Chunk>,
    /// (offset, name) pairs of the string table, in table order.
    pub strings: Vec<(u32, String)>,
    /// The `CLAS` classes, in payload order.
    pub classes: Vec<Class>,
}

/// Decodes a `REFL` payload into its chunks, string table and classes.
pub fn decode(data: &[u8]) -> Result<Refl, String> {
    let mut c = Cursor { data, pos: 0 };
    let mut chunks = Vec::new();
    while c.remaining() > 0 {
        let tag = c.tag4()?;
        let size = c.u32()?;
        let size = usize::try_from(size).map_err(|_| "chunk size does not fit")?;
        chunks.push(Chunk {
            tag,
            payload: c.take(size)?.to_vec(),
        });
    }
    let strings = match chunks.iter().find(|k| k.tag == *b"STRT") {
        Some(k) => parse_strings(&k.payload)?,
        None => Vec::new(),
    };
    let classes = chunks
        .iter()
        .filter(|k| k.tag == *b"CLAS")
        .map(|k| parse_class(&k.payload, &strings))
        .collect::<Result<Vec<Class>, String>>()?;
    Ok(Refl {
        chunks,
        strings,
        classes,
    })
}

/// Re-encodes a decoded payload: the chunks byte for byte.
pub fn encode(r: &Refl) -> Vec<u8> {
    let mut out = Vec::new();
    for k in &r.chunks {
        out.extend_from_slice(&k.tag);
        // A decoded payload never has a chunk this large; saturate so `encode` stays
        // total for hand-built values.
        let len = u32::try_from(k.payload.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&k.payload);
    }
    out
}

/// The entries of an `LMSW` payload: each swap's key and override material paths with
/// the key material's form reference, the material form references, and the named
/// property values where the payload shows them, in payload order. Payloads without the
/// `BGSLayeredMaterialSwap::Entry` class hold no swaps; an error when a list does not
/// parse.
pub fn lmsw_entries(r: &Refl) -> Result<Vec<SwapEntry>, String> {
    let entry_kind = r
        .classes
        .iter()
        .find(|c| c.name == "BGSLayeredMaterialSwap::Entry")
        .map(|c| c.name_offset);
    let snapshot_kind = r
        .classes
        .iter()
        .find(|c| c.name == "BSBind::Snapshot::Entry")
        .map(|c| c.name_offset);
    let mut out = Vec::new();
    let mut properties: Vec<(u32, Vec<f32>)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for k in r.chunks.iter().filter(|k| k.tag == *b"LIST") {
        let mut c = Cursor {
            data: &k.payload,
            pos: 0,
        };
        let kind = c.u32()?;
        let n = c.count(1)?;
        match kind {
            0xFFFFFF01 => {
                if n != 0 {
                    return Err(format!(
                        "LIST kind 0xFFFFFF01 holds {n} items; the official payloads hold 0"
                    ));
                }
            }
            0xFFFFFF05 => {
                for _ in 0..n {
                    let tag = c.u32()?;
                    if tag != 0xFFFFFF0D {
                        return Err(format!(
                            "LIST kind 0xFFFFFF05 item tag is 0x{tag:08X}, not 0xFFFFFF0D"
                        ));
                    }
                    out.push(SwapEntry::Ref(c.u32()?));
                }
            }
            k if entry_kind == Some(k) => {
                for _ in 0..n {
                    let key = path("key", c.text()?)?;
                    let tag = c.u32()?;
                    if tag != 0xFFFFFF0D {
                        return Err(format!(
                            "entry KeyMaterialPathForm tag is 0x{tag:08X}, not 0xFFFFFF0D"
                        ));
                    }
                    let key_material_path_form = c.u32()?;
                    let override_ = path("override", c.text()?)?;
                    out.push(SwapEntry::Swap {
                        key,
                        key_material_path_form,
                        r#override: override_,
                    });
                }
            }
            k if snapshot_kind == Some(k) => {
                // Items: a u32 property class id plus whole float values, all items
                // the same size.
                let rest = c.remaining();
                let mut values_per_item = 0;
                if n != 0 {
                    let Some(item) = rest.checked_div(n) else {
                        return Err("LIST count overflow".to_string());
                    };
                    if !rest.is_multiple_of(n) || item < 4 || !item.is_multiple_of(4) {
                        return Err(format!(
                            "LIST kind 0x{k:08X} items are not whole property values"
                        ));
                    }
                    let Some(minus_four) = item.checked_sub(4) else {
                        return Err(format!(
                            "LIST kind 0x{k:08X} items are not whole property values"
                        ));
                    };
                    values_per_item = minus_four / 4;
                }
                for _ in 0..n {
                    let class_id = c.u32()?;
                    let mut values = Vec::with_capacity(values_per_item);
                    for _ in 0..values_per_item {
                        values.push(f32::from_bits(c.u32()?));
                    }
                    properties.push((class_id, values));
                }
            }
            0xFFFFFF02 => {
                for _ in 0..n {
                    names.push(path("property", c.text()?)?);
                }
            }
            k => return Err(format!("LIST names unknown kind 0x{k:08X}")),
        }
        if c.remaining() != 0 {
            return Err("LIST has trailing bytes after its items".into());
        }
    }
    if properties.len() != names.len() {
        return Err(format!(
            "the payload holds {} property values but {} property names",
            properties.len(),
            names.len()
        ));
    }
    for ((class_id, values), name) in properties.into_iter().zip(names) {
        out.push(SwapEntry::Property {
            name,
            class_id,
            values,
        });
    }
    Ok(out)
}

fn parse_strings(payload: &[u8]) -> Result<Vec<(u32, String)>, String> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off < payload.len() {
        let rest = payload.get(off..).ok_or_else(|| {
            format!(
                "STRT offset {off} runs past the {}-byte table",
                payload.len()
            )
        })?;
        let Some(i) = rest.iter().position(|&b| b == 0) else {
            return Err(format!("STRT at {off} is not NUL-terminated"));
        };
        let end = off
            .checked_add(i)
            .ok_or_else(|| "STRT offset overflow".to_string())?;
        let slice = payload.get(off..end).ok_or_else(|| {
            format!(
                "STRT slice {off}..{end} runs past the {}-byte table",
                payload.len()
            )
        })?;
        let name =
            std::str::from_utf8(slice).map_err(|_| format!("STRT name at {off} is not UTF-8"))?;
        out.push((
            u32::try_from(off).map_err(|_| "STRT offset does not fit")?,
            name.to_string(),
        ));
        off = end
            .checked_add(1)
            .ok_or_else(|| "STRT offset overflow".to_string())?;
    }
    Ok(out)
}

fn parse_class(payload: &[u8], strings: &[(u32, String)]) -> Result<Class, String> {
    let mut c = Cursor {
        data: payload,
        pos: 0,
    };
    let name_offset = c.u32()?;
    let flags = c.u32()?;
    let field_words = c.u32()?;
    let fields = {
        let rest = c.remaining();
        if !rest.is_multiple_of(12) {
            return Err(format!(
                "CLAS of {rest} bytes after the header is not whole fields"
            ));
        }
        let mut v = Vec::with_capacity(rest / 12);
        for _ in 0..rest / 12 {
            let name_offset = c.u32()?;
            let ty = c.u32()?;
            let a = c.u16()?;
            let b = c.u16()?;
            v.push(Field {
                name_offset,
                name: name_of(strings, name_offset)?,
                ty,
                a,
                b,
            });
        }
        v
    };
    Ok(Class {
        name_offset,
        name: name_of(strings, name_offset)?,
        flags,
        field_words,
        fields,
    })
}

fn name_of(strings: &[(u32, String)], off: u32) -> Result<String, String> {
    strings
        .iter()
        .find(|(o, _)| *o == off)
        .map(|(_, n)| n.clone())
        .ok_or_else(|| format!("STRT offset {off} names no entry in the table"))
}

/// A u16 length plus that many bytes: the last must be a NUL, and the rest must be
/// UTF-8; the NUL is dropped.
fn path(what: &str, mut b: Vec<u8>) -> Result<String, String> {
    let Some(last) = b.pop() else {
        return Err(format!("LIST {what} is empty"));
    };
    if last != 0 {
        return Err(format!("LIST {what} is not NUL-terminated"));
    }
    std::str::from_utf8(&b)
        .map(|s| s.to_string())
        .map_err(|_| format!("LIST {what} is not UTF-8"))
}

struct Cursor<'d> {
    data: &'d [u8],
    pos: usize,
}

impl<'d> Cursor<'d> {
    fn take(&mut self, n: usize) -> Result<&'d [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| format!("offset overflow at {}", self.pos))?;
        let s = self.data.get(self.pos..end).ok_or_else(|| {
            format!(
                "REFL ends at {} (needed {n} bytes at {})",
                self.data.len(),
                self.pos
            )
        })?;
        self.pos = end;
        Ok(s)
    }
    fn tag4(&mut self) -> Result<[u8; 4], String> {
        let b = self.take(4)?;
        Ok(b.try_into().map_err(|_| "tag")?)
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes(b.try_into().map_err(|_| "u16")?))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes(b.try_into().map_err(|_| "u32")?))
    }
    /// A u16 length plus that many bytes, as stored.
    fn text(&mut self) -> Result<Vec<u8>, String> {
        let n = self.u16()?;
        Ok(self.take(usize::from(n))?.to_vec())
    }
    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
    /// A u32 count of items at least `min` bytes each, capped by the bytes remaining.
    fn count(&mut self, min: usize) -> Result<usize, String> {
        let at = self.pos;
        let n = usize::try_from(self.u32()?).map_err(|_| "count")?;
        let Some(need) = n.checked_mul(min) else {
            return Err(format!("LIST count {n} at {at} overflows"));
        };
        if need > self.remaining() {
            return Err(format!(
                "LIST count {n} at {at} exceeds the {} bytes left",
                self.remaining()
            ));
        }
        Ok(n)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(tag);
        v.extend((payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    fn u32(v: u32) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    fn u16(v: u16) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    /// The string table and offsets of the names the LMSW payloads show.
    fn names() -> (Vec<u8>, Vec<u32>) {
        let list = [
            "BGSLayeredMaterialSwap",
            "Keywords",
            "Entries",
            "SwapsCollection",
            "BGSLayeredMaterialSwap::Entry",
            "Key",
            "KeyMaterialPathForm",
            "OverrideMaterialPath",
            "BSBind::Snapshot",
            "Properties",
            "BSBind::Snapshot::Entry",
        ];
        let mut strt = Vec::new();
        let mut offs = Vec::new();
        for n in list {
            offs.push(strt.len() as u32);
            strt.extend_from_slice(n.as_bytes());
            strt.push(0);
        }
        (strt, offs)
    }

    fn field(off: u32, ty: u32, a: u16, b: u16) -> Vec<u8> {
        let mut v = u32(off);
        v.extend(u32(ty));
        v.extend(u16(a));
        v.extend(u16(b));
        v
    }

    /// A synthetic LMSW payload in the observed shape: root, snapshot and entry
    /// classes, an empty list, the swaps, a reference list, another empty list.
    fn lmsw_payload() -> Vec<u8> {
        let (strt, o) = names();
        let root = u32(o[0]);
        let mut root = root;
        root.extend(u32(0));
        root.extend(u32(3 * 0x10000));
        root.extend(field(o[1], 0xFFFFFF03, 1, 2));
        root.extend(field(o[2], 0xFFFFFF03, 1, 2));
        root.extend(field(o[3], 0xFFFFFF03, 1, 2));
        let snap = u32(o[8]);
        let mut snap = snap;
        snap.extend(u32(0));
        snap.extend(u32(0x10000));
        snap.extend(field(o[2], 0xFFFFFF03, 8, 400));
        let entry = u32(o[4]);
        let mut entry = entry;
        entry.extend(u32(0));
        entry.extend(u32(4 * 0x10000));
        entry.extend(field(o[5], 0xFFFFFF02, 0, 8));
        entry.extend(field(o[6], 0xFFFFFF05, 8, 8));
        entry.extend(field(o[7], 0xFFFFFF02, 16, 8));
        entry.extend(field(o[9], o[8], 24, 408));
        let key = b"A\\B.mat\0".to_vec();
        let override_ = b"C\\D.mat\0".to_vec();
        let key2 = b"E\\F.mat\0".to_vec();
        let override2 = b"G\\H.mat\0".to_vec();
        let mut swaps = u32(o[4]);
        swaps.extend(u32(2));
        for (k, v) in [
            (key.as_slice(), override_.as_slice()),
            (key2.as_slice(), override2.as_slice()),
        ] {
            swaps.extend(u16(k.len() as u16));
            swaps.extend(k);
            swaps.extend(u32(0xFFFFFF0D));
            swaps.extend(u32(0));
            swaps.extend(u16(v.len() as u16));
            swaps.extend(v);
        }
        let mut refs = u32(0xFFFFFF05);
        refs.extend(u32(1));
        refs.extend(u32(0xFFFFFF0D));
        refs.extend(u32(0x1234_5678));
        let mut out = chunk(b"BETH", &[4, 0, 0, 0, 11, 0, 0, 0]);
        out.extend(chunk(b"STRT", &strt));
        out.extend(chunk(b"TYPE", &u32(3)));
        out.extend(chunk(b"CLAS", &root));
        out.extend(chunk(b"CLAS", &snap));
        out.extend(chunk(b"CLAS", &entry));
        out.extend(chunk(b"OBJT", &u32(0)));
        out.extend(chunk(b"LIST", &[0x01, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]));
        out.extend(chunk(b"LIST", &swaps));
        out.extend(chunk(b"LIST", &refs));
        out.extend(chunk(b"LIST", &[0x01, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]));
        out
    }

    /// Like `lmsw_payload`, but with the snapshot pair: one swap with a nonzero key
    /// material reference, one property value item and its name.
    fn lmsw_payload_with_property() -> Vec<u8> {
        let (strt, o) = names();
        let root = u32(o[0]);
        let mut root = root;
        root.extend(u32(0));
        root.extend(u32(3 * 0x10000));
        root.extend(field(o[1], 0xFFFFFF03, 312, 16));
        root.extend(field(o[2], 0xFFFFFF03, 328, 16));
        root.extend(field(o[3], 0xFFFFFF03, 344, 16));
        let snap = u32(o[8]);
        let mut snap = snap;
        snap.extend(u32(0));
        snap.extend(u32(0x10000));
        snap.extend(field(o[2], 0xFFFFFF03, 8, 400));
        let snap_entry = u32(o[10]);
        let mut snap_entry = snap_entry;
        snap_entry.extend(u32(0));
        snap_entry.extend(u32(0));
        let entry = u32(o[4]);
        let mut entry = entry;
        entry.extend(u32(0));
        entry.extend(u32(4 * 0x10000));
        entry.extend(field(o[5], 0xFFFFFF02, 0, 8));
        entry.extend(field(o[6], 0xFFFFFF05, 8, 8));
        entry.extend(field(o[7], 0xFFFFFF02, 16, 8));
        entry.extend(field(o[9], o[8], 24, 408));
        let key = b"A\\B.mat\0".to_vec();
        let override_ = b"C\\D.mat\0".to_vec();
        let mut swaps = u32(o[4]);
        swaps.extend(u32(1));
        swaps.extend(u16(key.len() as u16));
        swaps.extend(&key);
        swaps.extend(u32(0xFFFFFF0D));
        swaps.extend(u32(0x001D_EDA3));
        swaps.extend(u16(override_.len() as u16));
        swaps.extend(&override_);
        let mut props = u32(o[10]);
        props.extend(u32(1));
        props.extend(u32(219));
        props.extend(u32(0x3F80_0000)); // 1.0f
        props.extend(u32(0x3F80_0000)); // 1.0f
        let mut prop_names = u32(0xFFFFFF02);
        prop_names.extend(u32(1));
        let name = b"UVScale\0";
        prop_names.extend(u16(name.len() as u16));
        prop_names.extend(name);
        let mut out = chunk(b"BETH", &[4, 0, 0, 0, 12, 0, 0, 0]);
        out.extend(chunk(b"STRT", &strt));
        out.extend(chunk(b"TYPE", &u32(4)));
        out.extend(chunk(b"CLAS", &root));
        out.extend(chunk(b"CLAS", &snap));
        out.extend(chunk(b"CLAS", &snap_entry));
        out.extend(chunk(b"CLAS", &entry));
        out.extend(chunk(b"OBJT", &u32(0)));
        out.extend(chunk(b"LIST", &[0x01, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]));
        out.extend(chunk(b"LIST", &swaps));
        out.extend(chunk(b"LIST", &props));
        out.extend(chunk(b"LIST", &prop_names));
        out.extend(chunk(b"LIST", &[0x01, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]));
        out
    }

    #[test]
    fn decode_encode_roundtrips_the_synthetic_payload() {
        let p = lmsw_payload();
        let r = decode(&p).unwrap();
        assert_eq!(encode(&r), p);
        assert_eq!(r.chunks.len(), 11);
        assert_eq!(r.strings.len(), 11);
        assert_eq!(r.classes.len(), 3);
        assert_eq!(r.classes[0].name, "BGSLayeredMaterialSwap");
        assert_eq!(r.classes[0].fields.len(), 3);
        assert_eq!(r.classes[2].name, "BGSLayeredMaterialSwap::Entry");
        assert_eq!(r.classes[2].fields[3].name, "Properties");
        assert_eq!(r.classes[2].fields[3].ty, o_at(&r, "BSBind::Snapshot"));
    }

    fn o_at(r: &Refl, name: &str) -> u32 {
        r.strings.iter().find(|(_, n)| n == name).unwrap().0
    }

    #[test]
    fn lmsw_entries_read_the_swaps_and_the_references() {
        let r = decode(&lmsw_payload()).unwrap();
        let e = lmsw_entries(&r).unwrap();
        assert_eq!(e.len(), 3);
        assert_eq!(
            e[0],
            SwapEntry::Swap {
                key: "A\\B.mat".into(),
                key_material_path_form: 0,
                r#override: "C\\D.mat".into(),
            }
        );
        assert_eq!(
            e[1],
            SwapEntry::Swap {
                key: "E\\F.mat".into(),
                key_material_path_form: 0,
                r#override: "G\\H.mat".into(),
            }
        );
        assert_eq!(e[2], SwapEntry::Ref(0x1234_5678));
    }

    #[test]
    fn lmsw_entries_read_the_named_property_values() {
        let r = decode(&lmsw_payload_with_property()).unwrap();
        let e = lmsw_entries(&r).unwrap();
        assert_eq!(
            e,
            [
                SwapEntry::Swap {
                    key: "A\\B.mat".into(),
                    key_material_path_form: 0x001D_EDA3,
                    r#override: "C\\D.mat".into(),
                },
                SwapEntry::Property {
                    name: "UVScale".into(),
                    class_id: 219,
                    values: [1.0, 1.0].to_vec(),
                },
            ]
        );
    }

    #[test]
    fn a_property_value_without_a_name_is_an_error() {
        let p = lmsw_payload_with_property();
        let r = decode(&p).unwrap();
        // Drop the 0xFFFFFF02 list from the decoded chunks.
        let mut r = r;
        r.chunks.retain(|k| {
            k.tag != *b"LIST" || k.payload.get(0..4) != Some(&[0x02, 0xFF, 0xFF, 0xFF])
        });
        assert!(lmsw_entries(&r).is_err());
    }

    #[test]
    fn a_swaps_list_with_a_bad_item_tag_is_an_error() {
        let p = lmsw_payload();
        let r = decode(&p).unwrap();
        let mut r = r;
        // Corrupt the first 0xFFFFFF0D tag in the list chunks: the swaps list's entry
        // tag (the reference list also holds one, but comes later).
        let mut found = false;
        for k in r.chunks.iter_mut().filter(|k| k.tag == *b"LIST") {
            let Some(off) = k
                .payload
                .windows(4)
                .position(|w| w == 0xFFFFFF0Du32.to_le_bytes())
            else {
                continue;
            };
            k.payload[off..off + 4].copy_from_slice(&0u32.to_le_bytes());
            found = true;
            break;
        }
        assert!(found);
        assert!(lmsw_entries(&r).is_err());
    }

    #[test]
    fn truncated_input_is_an_error() {
        let p = lmsw_payload();
        for cut in [1, 7, 8, 9, p.len() - 1, p.len() - 4] {
            assert!(decode(&p[..cut]).is_err(), "cut at {cut} should fail");
        }
    }

    #[test]
    fn a_chunk_oversized_against_the_input_is_an_error() {
        let mut p = lmsw_payload();
        // Bloat the STRT chunk's declared size (bytes 20..24) past the payload.
        p[20] = 0xFF;
        p[21] = 0xFF;
        assert!(decode(&p).is_err());
    }

    /// The shape of the `LMSW`s without the `Entry` class: a payload holding only the
    /// material reference list.
    fn lmsw_payload_refs_only() -> Vec<u8> {
        let mut strt = Vec::new();
        let mut o = Vec::new();
        for n in [
            "BGSLayeredMaterialSwap",
            "Keywords",
            "Entries",
            "SwapsCollection",
        ] {
            o.push(strt.len() as u32);
            strt.extend_from_slice(n.as_bytes());
            strt.push(0);
        }
        let mut root = u32(o[0]);
        root.extend(u32(0));
        root.extend(u32(3 * 0x10000));
        root.extend(field(o[1], 0xFFFFFF03, 312, 16));
        root.extend(field(o[2], 0xFFFFFF03, 328, 16));
        root.extend(field(o[3], 0xFFFFFF03, 344, 16));
        let mut refs = u32(0xFFFFFF05);
        refs.extend(u32(1));
        refs.extend(u32(0xFFFFFF0D));
        refs.extend(u32(0x0000_08B4));
        let mut out = chunk(b"BETH", &[4, 0, 0, 0, 7, 0, 0, 0]);
        out.extend(chunk(b"STRT", &strt));
        out.extend(chunk(b"TYPE", &u32(1)));
        out.extend(chunk(b"CLAS", &root));
        out.extend(chunk(b"OBJT", &u32(0)));
        out.extend(chunk(b"LIST", &[0x01, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]));
        out.extend(chunk(b"LIST", &refs));
        out
    }

    #[test]
    fn a_payload_without_the_entry_class_yields_its_references() {
        let r = decode(&lmsw_payload_refs_only()).unwrap();
        assert_eq!(lmsw_entries(&r).unwrap(), [SwapEntry::Ref(0x0000_08B4)]);
    }

    #[test]
    fn a_swaps_list_without_the_entry_class_is_an_error() {
        let r = decode(&lmsw_payload()).unwrap();
        // Drop the Entry class: its swaps list then names a class the payload does not
        // declare.
        let mut r = r;
        r.chunks
            .retain(|k| !(k.tag == *b"CLAS" && k.payload.get(0..4) == Some(&[56, 0, 0, 0])));
        let r = decode(&encode(&r)).unwrap();
        assert!(lmsw_entries(&r).is_err());
    }
}
