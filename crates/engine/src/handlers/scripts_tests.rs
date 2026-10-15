//! Script attachment (M4.E9) through `execute`: `set`, `new`, `place` and a build spec
//! attach a script whose `.pex` sits in a loose `Scripts` folder next to the target;
//! unknown scripts and properties are refused; `validate` reads the attachment back
//! (`papyrus.missing-script`, `papyrus.property-unresolved`).

#![allow(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use kitkraft_esm::index::{index_bytes, read_indexed, IndexOptions};
use kitkraft_esm::{DecodeLimits, Signature};
use serde_json::{json, Value};

use super::place_tests::world_master;
use crate::edit::patch_tests::dir;
use crate::edit::vmad;
use crate::execute;

/// A compiled script `name extends parent` with auto properties `(name, type)` and no
/// functions (the layout `kitkraft-pex` reads; see `papyrus::tests::tiny_pex`).
pub(crate) fn pex_with(name: &str, parent: &str, props: &[(&str, &str)]) -> Vec<u8> {
    let mut b = Vec::new();
    let ws = |b: &mut Vec<u8>, s: &str| {
        b.extend((s.len() as u16).to_le_bytes());
        b.extend(s.as_bytes());
    };
    b.extend(0xFA57_C0DEu32.to_le_bytes());
    b.extend([3u8, 12]);
    b.extend(2u16.to_le_bytes());
    b.extend(0u64.to_le_bytes());
    ws(&mut b, &format!("{name}.psc"));
    ws(&mut b, "u");
    ws(&mut b, "m");
    // Strings: "", name, parent, then each property's name, type and backing variable.
    let mut strings: Vec<String> = vec![String::new(), name.to_string(), parent.to_string()];
    for (n, t) in props {
        strings.push((*n).to_string());
        strings.push((*t).to_string());
        strings.push(format!("::{n}_var"));
    }
    b.extend((strings.len() as u16).to_le_bytes());
    for s in &strings {
        ws(&mut b, s);
    }
    b.push(0); // no debug info
    b.extend(0u16.to_le_bytes()); // user flags
    b.extend(1u16.to_le_bytes()); // objects
    let mut o = Vec::new();
    for v in [2u16, 0] {
        o.extend(v.to_le_bytes()); // parent, doc
    }
    o.push(0);
    o.extend(0u32.to_le_bytes());
    o.extend(0u16.to_le_bytes()); // auto state
    for _ in 0..3 {
        o.extend(0u16.to_le_bytes()); // structs, variables, guards
    }
    o.extend((props.len() as u16).to_le_bytes());
    for i in 0..props.len() {
        let base = 3 + 3 * i as u16;
        o.extend(base.to_le_bytes()); // name
        o.extend((base + 1).to_le_bytes()); // type
        o.extend(0u16.to_le_bytes()); // doc
        o.extend(0u32.to_le_bytes()); // user flags
        o.push(7); // read, write, auto
        o.extend((base + 2).to_le_bytes()); // backing variable
    }
    o.extend(0u16.to_le_bytes()); // states
    b.extend(1u16.to_le_bytes()); // object name
    b.extend((o.len() as u32 + 4).to_le_bytes());
    b.extend(o);
    b
}

/// `World.esm` in `<d>/Data`, and `KK:Hello extends ObjectReference` in `<d>/Scripts`
/// (next to the patches the tests write into `d`).
fn setup(name: &str) -> (PathBuf, PathBuf) {
    let d = dir(name);
    let data = world_master(&d);
    let scripts = d.join("Scripts").join("KK");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::write(
        scripts.join("Hello.pex"),
        pex_with(
            "KK:Hello",
            "ObjectReference",
            &[
                ("Count", "Int"),
                ("Target", "ObjectReference"),
                ("Scale", "Float"),
                ("Names", "String[]"),
            ],
        ),
    )
    .unwrap();
    (d, data)
}

fn load_args(data: &Path) -> Value {
    json!({"plugins": ["World.esm"], "data_dir": data.display().to_string(), "no_cache": true})
}

fn with(mut base: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

/// The VMAD of the record `form_id` in the plugin at `path`.
fn vmad_of(path: &Path, form_id: u32) -> vmad::Vmad {
    let bytes = std::fs::read(path).unwrap();
    let index = index_bytes(&bytes, &IndexOptions::default()).unwrap();
    let loc = index.records.iter().find(|r| r.form_id == form_id).unwrap();
    let record = read_indexed(
        &mut std::io::Cursor::new(&bytes),
        loc,
        Some(&DecodeLimits::default()),
    )
    .unwrap();
    let subs = record.subrecords().unwrap();
    let s = subs
        .iter()
        .find(|s| s.signature() == Signature(*b"VMAD"))
        .unwrap();
    vmad::decode(s.data()).unwrap()
}

fn hello(properties: Value) -> Value {
    json!([{"name": "KK:Hello", "properties": properties}])
}

#[test]
fn set_attaches_a_script_typed_by_its_declaration() {
    let (d, data) = setup("scripts-set");
    let into = d.join("Set.esp");
    let r = execute(
        "set",
        with(
            load_args(&data),
            json!({
                "form": "KC_Crate", "into": into.display().to_string(), "fields": {},
                "scripts": hello(json!({"Count": 3, "Target": "KC_Npc", "Scale": 1.5, "Names": ["a", "b"]})),
            }),
        ),
    )
    .unwrap();
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]
    );
    let s = &r["data"]["scripts"][0];
    assert!(s["found"].as_str().unwrap().ends_with("Hello.pex"), "{s:#}");
    assert_eq!(s["properties"][0]["type"], json!("Int"));
    assert_eq!(s["properties"][0]["declared_in"], json!("KK:Hello"));
    // The parent is not found as a .pex (and no base game sources are next to Data).
    assert!(r["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("extends `ObjectReference`")));
    let fields = r["data"]["records"][0]["fields"].as_array().unwrap();
    assert!(
        fields.iter().any(|f| f["path"] == json!("VMAD")),
        "{fields:?}"
    );
    let v = vmad_of(&into, 0x0000_0800);
    assert_eq!((v.version, v.object_format), (6, 2));
    assert_eq!(v.scripts.len(), 1);
    let p = |name: &str| {
        v.scripts[0]
            .properties
            .iter()
            .find(|p| p.name == name.as_bytes())
            .map(|p| (p.ty, p.value.clone()))
            .unwrap()
    };
    assert_eq!(p("Count"), (3, vmad::Value::Int(3)));
    assert_eq!(
        p("Target"),
        (
            1,
            vmad::Value::Object(vmad::Object {
                form_id: 0x0000_0801,
                alias: -1,
                unused: 0
            })
        )
    );
    assert_eq!(p("Scale"), (4, vmad::Value::Float(1.5f32.to_bits())));
    assert_eq!(
        p("Names"),
        (12, vmad::Value::Strings(vec![b"a".to_vec(), b"b".to_vec()]))
    );
    // Validate reads it back: the script is found, the properties declared.
    let v = execute(
        "validate",
        with(
            load_args(&data),
            json!({"plugin": into.display().to_string()}),
        ),
    )
    .unwrap();
    assert_eq!(v["data"]["passed"], json!(true), "{:#}", v["data"]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn unknown_scripts_properties_and_values_are_refused() {
    let (d, data) = setup("scripts-refused");
    let into = d.join("Refused.esp");
    let set = |scripts: Value| -> String {
        execute(
            "set",
            with(
                load_args(&data),
                json!({"form": "KC_Crate", "into": into.display().to_string(), "fields": {}, "scripts": scripts}),
            ),
        )
        .unwrap_err()
        .to_string()
    };
    let e = set(json!([{"name": "KK:Nope"}]));
    assert!(
        e.contains("papyrus.missing-script") && e.contains("KK:Nope"),
        "{e}"
    );
    let e = set(hello(json!({"Nothing": 1})));
    assert!(
        e.contains("papyrus.property-unresolved") && e.contains("scripts[0].properties.Nothing"),
        "{e}"
    );
    let e = set(hello(json!({"Count": "three"})));
    assert!(e.contains("does not fit the declared type Int"), "{e}");
    let e = set(hello(json!({"Count": 3.5})));
    assert!(e.contains("not a 32-bit integer"), "{e}");
    let e = set(hello(json!({"Target": "KC_Missing"})));
    assert!(e.contains("Target"), "{e}");
    let e = set(json!([{"name": "KK:Hello", "extra": 1}]));
    assert!(e.contains("/scripts/0/extra"), "{e}");
    // Nothing to set at all.
    let e = execute(
        "set",
        with(
            load_args(&data),
            json!({"form": "KC_Crate", "into": into.display().to_string(), "fields": {}}),
        ),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("no fields to set"), "{e}");
    assert!(!into.exists());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn new_and_place_attach_scripts_and_a_spec_names_new_forms() {
    let (d, data) = setup("scripts-new-place");
    let into = d.join("New.esp");
    let r = execute(
        "new",
        with(
            load_args(&data),
            json!({"type": "STAT", "edid": "KK_Scripted", "into": into.display().to_string(),
                   "masters": ["World.esm"], "scripts": hello(json!({"Count": 1}))}),
        ),
    )
    .unwrap();
    assert_eq!(
        r["data"]["validate"]["passed"],
        json!(true),
        "{:#}",
        r["data"]
    );
    assert_eq!(
        vmad_of(&into, 0x0100_1000).scripts[0].name_text(),
        "KK:Hello"
    );
    let placed = d.join("Place.esp");
    let r = execute(
        "place",
        with(
            load_args(&data),
            json!({"base": "KC_Crate", "cell": "TestInterior", "position": [1.0, 2.0, 3.0],
                   "into": placed.display().to_string(), "scripts": hello(json!({"Target": null}))}),
        ),
    )
    .unwrap();
    let form_id =
        u32::from_str_radix(r["data"]["placed"][0]["form_id"].as_str().unwrap(), 16).unwrap();
    let v = vmad_of(&placed, form_id);
    assert_eq!(
        v.scripts[0].properties[0].value,
        vmad::Value::Object(vmad::Object {
            form_id: 0,
            alias: -1,
            unused: 0
        })
    );
    // A spec: a [[place]] whose script points at a [[clone]] of the same build.
    let spec = d.join("s.toml");
    std::fs::write(
        &spec,
        format!(
            "version = 3\n[load_order]\nplugins = [\"World.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n\
[[clone]]\nname = \"crate\"\nform = \"KC_Crate\"\nedid = \"KK_CrateClone\"\n\n\
[[place]]\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\nposition = [0, 0, 0]\n\
scripts = [{{ name = \"KK:Hello\", properties = {{ Target = \"new:crate\", Count = 2 }} }}]\n",
            data.display()
        ),
    )
    .unwrap();
    let out = d.join("Spec.esp");
    let r = execute(
        "build",
        json!({"spec": spec.display().to_string(), "out": out.display().to_string()}),
    )
    .unwrap();
    assert_eq!(r["data"]["passed"], json!(true), "{:#}", r["data"]);
    assert_eq!(r["data"]["scripts"][0]["entry"], json!("place[0]"));
    let form_id =
        u32::from_str_radix(r["data"]["placed"][0]["form_id"].as_str().unwrap(), 16).unwrap();
    let v = vmad_of(&out, form_id);
    let target = v.scripts[0]
        .properties
        .iter()
        .find(|p| p.name == b"Target")
        .unwrap();
    // The clone is the build's first new form.
    assert_eq!(
        target.value,
        vmad::Value::Object(vmad::Object {
            form_id: 0x0100_1000,
            alias: -1,
            unused: 0
        })
    );
    // A spec name no entry has is a spec error naming the key.
    std::fs::write(
        &spec,
        format!(
            "version = 3\n[load_order]\nplugins = [\"World.esm\"]\ndata_dir = '{}'\nno_cache = true\n\n\
[[place]]\nbase = \"KC_Crate\"\ncell = \"TestInterior\"\nposition = [0, 0, 0]\n\
scripts = [{{ name = \"KK:Hello\", properties = {{ Target = \"new:nope\" }} }}]\n",
            data.display()
        ),
    )
    .unwrap();
    let e = execute(
        "build",
        json!({"spec": spec.display().to_string(), "out": d.join("E.esp").display().to_string()}),
    )
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("place[0].scripts") && e.contains("unknown name `nope`"),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// The two rules on synthetic fixtures: a plugin whose attached script's `.pex` is gone
/// (`papyrus.missing-script`), and one whose script no longer declares a stored property
/// or declares it with another type (`papyrus.property-unresolved`).
#[test]
fn validate_rules_on_attached_scripts() {
    let (d, data) = setup("scripts-validate");
    let into = d.join("Attached.esp");
    execute(
        "set",
        with(
            load_args(&data),
            json!({"form": "KC_Crate", "into": into.display().to_string(), "fields": {},
                   "scripts": hello(json!({"Count": 3, "Scale": 2.0}))}),
        ),
    )
    .unwrap();
    let validate = || -> Value {
        execute(
            "validate",
            with(
                load_args(&data),
                json!({"plugin": into.display().to_string()}),
            ),
        )
        .unwrap()
    };
    let rules = |v: &Value| -> Vec<(String, String)> {
        v["data"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                (
                    f["rule"].as_str().unwrap().to_string(),
                    f["message"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    assert_eq!(validate()["data"]["passed"], json!(true));
    // The script now declares Count as a Float and no longer declares Scale.
    let pex = d.join("Scripts").join("KK").join("Hello.pex");
    std::fs::write(&pex, pex_with("KK:Hello", "", &[("Count", "Float")])).unwrap();
    let v = validate();
    assert_eq!(v["data"]["passed"], json!(false));
    let found = rules(&v);
    assert!(found.iter().any(|(r, m)| r == "papyrus.property-unresolved"
        && m.contains("KK:Hello.Count")
        && m.contains("Float")));
    assert!(found
        .iter()
        .any(|(r, m)| r == "papyrus.property-unresolved" && m.contains("KK:Hello.Scale")));
    // The script is gone.
    std::fs::remove_file(&pex).unwrap();
    let v = validate();
    let found = rules(&v);
    assert!(
        found
            .iter()
            .any(|(r, m)| r == "papyrus.missing-script" && m.contains("scripts/kk/hello.pex")),
        "{found:?}"
    );
    assert!(v["data"]["findings"][0]["record"]
        .as_str()
        .unwrap()
        .starts_with("STAT:"));
    let _ = std::fs::remove_dir_all(&d);
}

/// An override that carries its master's attachment unchanged gets a warning, not an
/// error, when the master's script is gone (the official files carry such stale
/// attachments; a patch of those records must still write).
#[test]
fn an_inherited_stale_attachment_is_a_warning() {
    let (d, data) = setup("scripts-inherited");
    let scripts = data.join("Scripts").join("KK");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::copy(
        d.join("Scripts").join("KK").join("Hello.pex"),
        scripts.join("Hello.pex"),
    )
    .unwrap();
    // Base.esm (a master next to World.esm) attaches KK:Hello to KC_Crate.
    let base = data.join("Base.esm");
    execute(
        "set",
        with(
            load_args(&data),
            json!({"form": "KC_Crate", "into": base.display().to_string(), "fields": {},
                   "scripts": hello(json!({"Count": 1}))}),
        ),
    )
    .unwrap();
    // The script is gone everywhere; Over.esp overrides the record without touching it.
    std::fs::remove_file(scripts.join("Hello.pex")).unwrap();
    std::fs::remove_file(d.join("Scripts").join("KK").join("Hello.pex")).unwrap();
    let over = d.join("Over.esp");
    let two = json!({"plugins": ["World.esm", "Base.esm"], "data_dir": data.display().to_string(), "no_cache": true});
    let r = execute(
        "set",
        with(
            two.clone(),
            json!({"form": "KC_Crate", "into": over.display().to_string(),
                   "fields": {"EDID": "KC_CrateOver"}}),
        ),
    )
    .unwrap();
    let findings = r["data"]["validate"]["findings"].as_array().unwrap();
    let f = findings
        .iter()
        .find(|f| f["rule"] == json!("papyrus.missing-script"))
        .unwrap();
    assert_eq!(f["severity"], json!("warning"), "{f:#}");
    assert!(f["message"]
        .as_str()
        .unwrap()
        .contains("carried unchanged from Base.esm"));
    // The master itself, whose own record attaches it, has the error.
    let v = execute(
        "validate",
        with(two, json!({"plugin": base.display().to_string()})),
    )
    .unwrap();
    assert_eq!(v["data"]["passed"], json!(false));
    let _ = std::fs::remove_dir_all(&d);
}
