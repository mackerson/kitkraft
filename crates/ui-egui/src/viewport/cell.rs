//! A `cell` envelope's references as viewport markers (M7.R2, M7.R3), without egui: the
//! view-model the Render Window tab keeps, and the base-type categories that color the
//! markers (from the theme's palette tokens) and give a reference without bounds its
//! shape. Everything here comes from the envelope the engine returned; nothing reads a
//! plugin.

use glam::Vec3;
use serde_json::Value;

use super::scene::{Marker, Shape};
use crate::theme::{Palette, Rgb};

/// A reference's base record type, grouped for the legend: each group has its own
/// color token and, for references drawn without bounds, its own shape (so the type does
/// not show by color alone).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Static,
    Container,
    Activator,
    Actor,
    Item,
    Door,
    Light,
    Other,
}

impl Category {
    /// Legend order.
    pub const ALL: [Category; 8] = [
        Category::Static,
        Category::Container,
        Category::Activator,
        Category::Actor,
        Category::Item,
        Category::Door,
        Category::Light,
        Category::Other,
    ];

    /// The group of a base record type (`base_signature`); an actor reference (`ACHR`)
    /// is an actor whatever its base.
    pub fn of(base_signature: Option<&str>, ref_signature: Option<&str>) -> Self {
        if ref_signature == Some("ACHR") {
            return Category::Actor;
        }
        match base_signature.unwrap_or("") {
            "STAT" | "SCOL" | "MSTT" | "PKIN" | "BNDS" | "GBFM" | "TXST" | "TREE" | "GRAS" => {
                Category::Static
            }
            "CONT" => Category::Container,
            "ACTI" | "TERM" | "FURN" | "TACT" => Category::Activator,
            "NPC_" | "LVLN" => Category::Actor,
            "MISC" | "WEAP" | "AMMO" | "ARMO" | "ALCH" | "BOOK" | "KEYM" | "LVLI" | "FLOR"
            | "INGR" | "NOTE" | "OMOD" | "RSPJ" => Category::Item,
            "DOOR" => Category::Door,
            "LIGH" => Category::Light,
            _ => Category::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Static => "static",
            Category::Container => "container",
            Category::Activator => "activator, furniture",
            Category::Actor => "actor",
            Category::Item => "item",
            Category::Door => "door",
            Category::Light => "light",
            Category::Other => "other (markers, sounds, decals, ...)",
        }
    }

    /// The base types the group holds, for the legend's hover text.
    pub fn types(self) -> &'static str {
        match self {
            Category::Static => "STAT SCOL MSTT PKIN BNDS GBFM TXST TREE GRAS",
            Category::Container => "CONT",
            Category::Activator => "ACTI TERM FURN TACT",
            Category::Actor => "NPC_ LVLN, and every ACHR",
            Category::Item => "MISC WEAP AMMO ARMO ALCH BOOK KEYM LVLI FLOR INGR NOTE OMOD RSPJ",
            Category::Door => "DOOR",
            Category::Light => "LIGH",
            Category::Other => "every other type, and a reference without a base",
        }
    }

    /// The shape of a reference of this group drawn without bounds.
    pub fn shape(self) -> Shape {
        match self {
            Category::Static | Category::Container => Shape::Box,
            Category::Activator | Category::Actor | Category::Item => Shape::Diamond,
            Category::Door | Category::Light | Category::Other => Shape::Pyramid,
        }
    }

    /// The palette token the group is drawn in (`docs/design/workbench-visual-system.md`).
    pub fn token(self) -> &'static str {
        match self {
            Category::Static => "status.not_run",
            Category::Container => "status.warn",
            Category::Activator => "accent",
            Category::Actor => "status.fail",
            Category::Item => "status.inconclusive",
            Category::Door => "status.pass",
            Category::Light => "status.unknown",
            Category::Other => "text.strong",
        }
    }

    /// The group's color in `palette` ([`Category::token`]).
    pub fn color(self, palette: &Palette) -> Rgb {
        let s = &palette.status;
        match self {
            Category::Static => s.not_run,
            Category::Container => s.warn,
            Category::Activator => palette.accent,
            Category::Actor => s.fail,
            Category::Item => s.inconclusive,
            Category::Door => s.pass,
            Category::Light => s.unknown,
            Category::Other => palette.text.strong,
        }
    }
}

/// An sRGB token as the viewport's 0..1 RGBA.
pub fn rgba(c: Rgb) -> [f32; 4] {
    [
        f32::from(c.0) / 255.0,
        f32::from(c.1) / 255.0,
        f32::from(c.2) / 255.0,
        1.0,
    ]
}

/// One placed reference, as the `cell` command returned it.
#[derive(Debug, Clone, PartialEq)]
pub struct CellRef {
    /// Formref (`Starfield.esm:0A6F06`).
    pub form: String,
    pub signature: Option<String>,
    pub editor_id: Option<String>,
    pub base: Option<String>,
    pub base_signature: Option<String>,
    pub base_editor_id: Option<String>,
    /// `None` when the reference has no readable `DATA`.
    pub position: Option<Vec3>,
    /// Radians, as stored (zero when absent).
    pub rotation: Vec3,
    /// `XSCL`; `None` when absent (the game uses 1).
    pub scale: Option<f32>,
    /// The base's `OBND` corners (`with_bounds`), in the base's own space.
    pub bounds: Option<(Vec3, Vec3)>,
    pub persistent: bool,
    pub plugin: String,
}

impl CellRef {
    pub fn category(&self) -> Category {
        Category::of(self.base_signature.as_deref(), self.signature.as_deref())
    }

    /// The marker it is drawn as.
    pub fn marker(&self, palette: &Palette) -> Option<Marker> {
        let cat = self.category();
        Some(Marker::placed(
            self.position?,
            self.rotation,
            self.scale,
            self.bounds,
            cat.shape(),
            rgba(cat.color(palette)),
        ))
    }
}

/// Three numbers of a JSON array as a vector (`None` unless all three are finite numbers).
fn vec3(v: Option<&Value>) -> Option<Vec3> {
    let a = v?.as_array()?;
    let n = |i: usize| a.get(i).and_then(Value::as_f64).map(|f| f as f32);
    let out = Vec3::new(n(0)?, n(1)?, n(2)?);
    out.is_finite().then_some(out)
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// One page of a `cell` envelope in `cell` mode.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CellPage {
    /// The cell's formref, EditorID and world (formref), as the engine named them.
    pub cell: String,
    pub cell_editor_id: Option<String>,
    pub world: Option<String>,
    /// References in the cell (all pages).
    pub total: usize,
    pub next_cursor: Option<String>,
    pub refs: Vec<CellRef>,
    pub limitations: Vec<String>,
    /// The summary line.
    pub summary: String,
}

/// Reads a `cell` envelope. A reply in another mode (a list of worlds or cells) gives an
/// empty page with its summary.
pub fn parse_page(env: &Value) -> CellPage {
    let data = env.get("data").unwrap_or(&Value::Null);
    let refs = if data.get("mode").and_then(Value::as_str) == Some("cell") {
        data.get("items")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(parse_ref).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    CellPage {
        cell: text(data, "cell").unwrap_or_default(),
        cell_editor_id: text(data, "cell_editor_id"),
        world: text(data, "world"),
        total: data
            .get("total")
            .and_then(Value::as_u64)
            .and_then(|t| usize::try_from(t).ok())
            .unwrap_or(0),
        next_cursor: text(data, "next_cursor"),
        refs,
        limitations: env
            .get("limitations")
            .and_then(Value::as_array)
            .map(|l| {
                l.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        summary: text(data, "summary").unwrap_or_default(),
    }
}

fn parse_ref(item: &Value) -> Option<CellRef> {
    let bounds = item.get("bounds").and_then(|b| {
        let (lo, hi) = (vec3(b.get("min"))?, vec3(b.get("max"))?);
        Some((lo.min(hi), lo.max(hi)))
    });
    Some(CellRef {
        form: text(item, "form")?,
        signature: text(item, "signature"),
        editor_id: text(item, "editor_id"),
        base: item.get("base").and_then(|b| text(b, "form")),
        base_signature: text(item, "base_signature"),
        base_editor_id: text(item, "base_editor_id"),
        position: vec3(item.get("position")),
        rotation: vec3(item.get("rotation")).unwrap_or(Vec3::ZERO),
        scale: item
            .get("scale")
            .and_then(Value::as_f64)
            .map(|s| s as f32)
            .filter(|s| s.is_finite()),
        bounds,
        persistent: item.get("persistent").and_then(Value::as_bool) == Some(true),
        plugin: text(item, "plugin").unwrap_or_default(),
    })
}

/// The markers of `refs`, at most `cap` of them, in order, with the index of the
/// reference each one stands for; references without a position are skipped.
pub fn markers(refs: &[CellRef], palette: &Palette, cap: usize) -> (Vec<Marker>, Vec<usize>) {
    let mut out = Vec::new();
    let mut index = Vec::new();
    for (i, r) in refs.iter().enumerate() {
        if out.len() >= cap {
            break;
        }
        if let Some(m) = r.marker(palette) {
            out.push(m);
            index.push(i);
        }
    }
    (out, index)
}

/// How many references fall in each category, in legend order (empty ones left out).
pub fn counts(refs: &[CellRef]) -> Vec<(Category, usize)> {
    Category::ALL
        .iter()
        .map(|c| (*c, refs.iter().filter(|r| r.category() == *c).count()))
        .filter(|(_, n)| *n > 0)
        .collect()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::theme::tokens::workbench;
    use serde_json::json;

    fn envelope() -> Value {
        json!({
            "command": "cell",
            "data": {
                "mode": "cell", "cell": "Starfield.esm:21966F",
                "cell_editor_id": "CityNewAtlantisApartmentTEMPLATE", "world": null,
                "total": 3, "next_cursor": "abc", "summary": "3 reference(s)",
                "items": [
                    {"form": "Starfield.esm:0A6F06", "signature": "REFR", "editor_id": null,
                     "base": {"kind": "formref", "form": "Starfield.esm:00375C", "raw": "0000375C"},
                     "base_signature": "STAT", "base_editor_id": "LP_Lights_Vol_REF",
                     "position": [4.0, 0.5, 4.5], "rotation": [0.0, 0.0, 1.5], "scale": null,
                     "bounds": {"min": [-0.25, -0.3, 0.0], "max": [0.25, 0.25, 0.2]},
                     "persistent": false, "plugin": "Starfield.esm"},
                    {"form": "Starfield.esm:0A6F07", "signature": "REFR",
                     "base": {"kind": "formref", "form": "Starfield.esm:001234"},
                     "base_signature": "LIGH", "position": [1.0, 2.0, 3.0],
                     "rotation": null, "scale": 2.0, "bounds": null},
                    {"form": "Starfield.esm:0A6F08", "signature": "ACHR",
                     "base": null, "base_signature": null, "position": null,
                     "rotation": [0.0, 0.0, 0.0], "scale": null}
                ]
            },
            "limitations": ["one", "two"]
        })
    }

    #[test]
    fn a_page_reads_every_reference_and_its_bounds() {
        let p = parse_page(&envelope());
        assert_eq!(p.cell, "Starfield.esm:21966F");
        assert_eq!(
            p.cell_editor_id.as_deref(),
            Some("CityNewAtlantisApartmentTEMPLATE")
        );
        assert_eq!((p.total, p.next_cursor.as_deref()), (3, Some("abc")));
        assert_eq!(p.limitations, ["one", "two"]);
        assert_eq!(p.refs.len(), 3);
        let a = &p.refs[0];
        assert_eq!(a.base.as_deref(), Some("Starfield.esm:00375C"));
        assert_eq!(
            a.bounds,
            Some((Vec3::new(-0.25, -0.3, 0.0), Vec3::new(0.25, 0.25, 0.2)))
        );
        assert_eq!(a.rotation, Vec3::new(0.0, 0.0, 1.5));
        assert_eq!(a.scale, None);
        assert_eq!(a.category(), Category::Static);
        let b = &p.refs[1];
        assert_eq!(
            (b.bounds, b.scale, b.rotation),
            (None, Some(2.0), Vec3::ZERO)
        );
        assert_eq!(b.category(), Category::Light);
        let c = &p.refs[2];
        assert_eq!((c.position, c.category()), (None, Category::Actor));
        // Other modes give no references.
        let worlds = parse_page(&json!({"data": {"mode": "worlds", "items": [{"form": "x"}]}}));
        assert!(worlds.refs.is_empty());
        assert_eq!(parse_page(&Value::Null), CellPage::default());
    }

    #[test]
    fn markers_follow_the_references_up_to_the_cap() {
        let p = parse_page(&envelope());
        let pal = *workbench().palette(true);
        let (m, index) = markers(&p.refs, &pal, 10);
        // The third has no position.
        assert_eq!((m.len(), index.as_slice()), (2, &[0, 1][..]));
        assert_eq!(m[0].shape, Shape::Box);
        assert_eq!(m[0].position, Vec3::new(4.0, 0.5, 4.5));
        assert_eq!(m[0].color, rgba(pal.status.not_run));
        // A light without bounds: its group's shape, at the unit size, scaled.
        assert_eq!(m[1].shape, Shape::Pyramid);
        assert_eq!(m[1].scale, 2.0);
        assert_eq!(m[1].color, rgba(pal.status.unknown));
        let (capped, index) = markers(&p.refs, &pal, 1);
        assert_eq!((capped.len(), index.as_slice()), (1, &[0][..]));
        assert_eq!(
            counts(&p.refs),
            [
                (Category::Static, 1),
                (Category::Actor, 1),
                (Category::Light, 1)
            ]
        );
    }

    #[test]
    fn every_group_has_its_own_color_and_shape_pair() {
        for preset in crate::theme::Preset::ALL {
            for dark in [true, false] {
                let theme = preset.theme();
                let pal = theme.palette(dark);
                // Shape and color together tell every group apart in every preset; the
                // color alone does in all but the legacy Classic, whose palette repeats.
                let key = |c: &Category| {
                    let rgb = c.color(pal);
                    (c.shape(), rgb.0, rgb.1, rgb.2)
                };
                let mut pairs: Vec<_> = Category::ALL.iter().map(key).collect();
                pairs.sort_unstable();
                pairs.dedup();
                assert_eq!(pairs.len(), Category::ALL.len(), "{preset:?} dark {dark}");
                if !preset.legacy() {
                    let mut colors: Vec<_> = pairs.iter().map(|p| (p.1, p.2, p.3)).collect();
                    colors.sort_unstable();
                    colors.dedup();
                    assert_eq!(colors.len(), Category::ALL.len(), "{preset:?} dark {dark}");
                }
            }
        }
        let mut pairs: Vec<(Shape, &str)> = Category::ALL
            .iter()
            .map(|c| (c.shape(), c.token()))
            .collect();
        pairs.sort_unstable();
        pairs.dedup();
        assert_eq!(pairs.len(), Category::ALL.len());
        // Shapes are shared only within groups of distinct colors: three shapes in all.
        let mut shapes: Vec<Shape> = Category::ALL.iter().map(|c| c.shape()).collect();
        shapes.dedup();
        assert!(shapes.len() >= 3);
        for c in Category::ALL {
            assert!(c.types().len() > 2 && !c.label().is_empty());
        }
        assert_eq!(Category::of(Some("STAT"), Some("ACHR")), Category::Actor);
        assert_eq!(Category::of(None, Some("REFR")), Category::Other);
    }
}
