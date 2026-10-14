//! Loader and validator tests (M1.8).

// Helpers outside `#[test]` fns panic to fail the calling test; this is test code.
#![allow(clippy::panic)]

use kitcraft_schema::{
    load_dir, load_sources, parse_definition, ArrayCount, Cardinality, Definition, ErrorKind,
    FieldType, Member, Scalar, SchemaError, Signature,
};

const FIXTURE: &str = include_str!("fixtures/ZZZZ.toml");

/// A minimal valid record file with `body` appended.
fn record(body: &str) -> String {
    format!("signature = \"TEST\"\nname = \"Test\"\nsource = \"corpus: unit test\"\n{body}")
}

fn kind_of(result: Result<Definition, SchemaError>) -> ErrorKind {
    match result {
        Err(SchemaError::Invalid { kind, .. }) => kind,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

fn invalid(body: &str) -> ErrorKind {
    kind_of(parse_definition("TEST.toml", &record(body)))
}

#[test]
fn fixture_loads_with_every_feature() {
    let def = parse_definition("ZZZZ.toml", FIXTURE).unwrap();
    let Definition::Record(rec) = def else {
        panic!("expected a record");
    };
    assert_eq!(rec.signature, Signature(*b"ZZZZ"));
    assert_eq!(rec.flags.len(), 2);
    let names: Vec<&str> = rec.members.iter().map(Member::name).collect();
    assert_eq!(
        names,
        [
            "EditorId",
            "Name",
            "Data",
            "Versioned",
            "Keywords",
            "Entry",
            "Block",
            "Tail",
            "BigValue",
            "Skipped"
        ]
    );
    let Member::Subrecord(data) = &rec.members[2] else {
        panic!("DATA is a subrecord");
    };
    assert_eq!(data.size, Some(39));
    assert_eq!(data.ty.fixed_size(None), Some(39));
    let Member::Subrecord(vers) = &rec.members[3] else {
        panic!("VERS is a subrecord");
    };
    assert_eq!(vers.ty.fixed_size(Some(99)), Some(4));
    assert_eq!(vers.ty.fixed_size(Some(100)), Some(12));
    let Member::Subrecord(kwda) = &rec.members[4] else {
        panic!("KWDA is a subrecord");
    };
    match &kwda.ty {
        FieldType::Array { element, count } => {
            assert_eq!(*count, ArrayCount::Rest);
            assert_eq!(
                **element,
                FieldType::FormId {
                    targets: vec![Signature(*b"KYWD")]
                }
            );
        }
        other => panic!("unexpected {other:?}"),
    }
    let Member::Group(block) = &rec.members[6] else {
        panic!("Block is a group");
    };
    assert_eq!(block.cardinality, Cardinality::Repeated);
    assert!(matches!(block.members[1], Member::AnyUntil(_)));
}

#[test]
fn header_definition_loads_and_checks_its_size() {
    let text = "signature = \"HDR0\"\nname = \"Hdr\"\nkind = \"header\"\nsource = \"corpus: t\"\n\
                size = 8\nfield = [{ name = \"Type\", type = \"sig\" }, { name = \"Size\", type = \"u32\" }]\n";
    let def = parse_definition("HDR0.toml", text).unwrap();
    let Definition::Header(h) = def else {
        panic!("expected a header");
    };
    assert_eq!(h.size, 8);
    assert_eq!(h.fields[1].ty, FieldType::Scalar(Scalar::U32));

    let bad = text.replace("size = 8", "size = 9");
    assert_eq!(
        kind_of(parse_definition("HDR0.toml", &bad)),
        ErrorKind::SizeConflict {
            declared: 9,
            computed: 8
        }
    );
}

#[test]
fn malformed_toml_reports_line_and_column() {
    let text = record("[[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = zstring\n");
    match parse_definition("TEST.toml", &text) {
        Err(SchemaError::Toml { line, column, .. }) => {
            assert_eq!(line, 7);
            assert!(column > 1);
        }
        other => panic!("expected a TOML error, got {other:?}"),
    }
}

#[test]
fn unknown_keys_are_rejected() {
    let text =
        record("[[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = \"zstring\"\nsizee = 3\n");
    assert!(matches!(
        parse_definition("TEST.toml", &text),
        Err(SchemaError::Toml { .. })
    ));
}

#[test]
fn missing_top_level_keys_are_errors() {
    let text = "signature = \"TEST\"\nname = \"Test\"\n";
    assert!(matches!(
        parse_definition("TEST.toml", text),
        Err(SchemaError::Toml { .. })
    ));
}

#[test]
fn duplicate_signatures_across_files() {
    let a = record("");
    let b = record("");
    match load_sources([("a/TEST.toml", a.as_str()), ("b/TEST.toml", b.as_str())]) {
        Err(SchemaError::Duplicate {
            signature, first, ..
        }) => {
            assert_eq!(signature, Signature(*b"TEST"));
            assert_eq!(first, "a/TEST.toml");
        }
        other => panic!("expected a duplicate error, got {other:?}"),
    }
}

#[test]
fn unknown_field_types() {
    assert_eq!(
        invalid("[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"u24\"\n"),
        ErrorKind::UnknownType("u24".into())
    );
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\n\
             field = [{ name = \"A\", type = \"float\" }]\n"
        ),
        ErrorKind::UnknownType("float".into())
    );
}

#[test]
fn size_conflicts() {
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\nsize = 6\n\
             field = [{ name = \"A\", type = \"u32\" }, { name = \"B\", type = \"u8\" }]\n"
        ),
        ErrorKind::SizeConflict {
            declared: 6,
            computed: 5
        }
    );
    assert_eq!(
        invalid("[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"u16\"\nsize = 4\n"),
        ErrorKind::SizeConflict {
            declared: 4,
            computed: 2
        }
    );
    assert_eq!(
        invalid("[[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = \"zstring\"\nsize = 4\n"),
        ErrorKind::SizeOfVariable { declared: 4 }
    );
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\n\
             size_by_version = [{ min = 10, size = 8 }]\n\
             field = [{ name = \"A\", type = \"u32\" }, { name = \"B\", type = \"u32\", since = 11 }]\n"
        ),
        ErrorKind::SizeConflict {
            declared: 8,
            computed: 4
        }
    );
}

#[test]
fn signature_and_name_rules() {
    assert_eq!(
        invalid("[[member]]\nsig = \"EDIDX\"\nname = \"EditorId\"\ntype = \"zstring\"\n"),
        ErrorKind::InvalidSignature("EDIDX".into())
    );
    assert_eq!(
        invalid("[[member]]\nsig = \"EDID\"\nname = \"editor_id\"\ntype = \"zstring\"\n"),
        ErrorKind::InvalidName("editor_id".into())
    );
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"EDID\"\nname = \"Name\"\ntype = \"zstring\"\n\
             [[member]]\nsig = \"FULL\"\nname = \"Name\"\ntype = \"lstring\"\n"
        ),
        ErrorKind::DuplicateName("Name".into())
    );
    assert_eq!(
        invalid("[[member]]\nsig = \"EDID\"\nname = \"Extra\"\ntype = \"zstring\"\n"),
        ErrorKind::ReservedName("Extra".into())
    );
    assert!(matches!(
        kind_of(parse_definition("WEAP.toml", &record(""))),
        ErrorKind::FileName { .. }
    ));
    let bad_source = "signature = \"TEST\"\nname = \"Test\"\nsource = \"somewhere\"\n";
    assert_eq!(
        kind_of(parse_definition("TEST.toml", bad_source)),
        ErrorKind::BadSource("somewhere".into())
    );
}

#[test]
fn source_forms_follow_adr_0005() {
    for prefix in [
        "uesp-skyrim: x",
        "corpus: x",
        "ck-diff: obs-1",
        "ck-wiki: Weapon",
    ] {
        let text = format!("signature = \"TEST\"\nname = \"Test\"\nsource = \"{prefix}\"\n");
        parse_definition("TEST.toml", &text).unwrap();
    }
    let old_form = "signature = \"TEST\"\nname = \"Test\"\nsource = \"UESP: x\"\n";
    assert_eq!(
        kind_of(parse_definition("TEST.toml", old_form)),
        ErrorKind::BadSource("UESP: x".into())
    );
    assert_eq!(
        invalid("evidence = [\"corpus: ok\", \"forum post\"]\n"),
        ErrorKind::BadSource("forum post".into())
    );
}

#[test]
fn layout_rules() {
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\n\
             field = [{ name = \"S\", type = \"zstring\" }, { name = \"B\", type = \"u8\" }]\n"
        ),
        ErrorKind::VariableNotLast("S".into())
    );
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"array\"\nelement = \"zstring\"\ncount = 2\n"
        ),
        ErrorKind::BadElement
    );
    assert_eq!(
        invalid("[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"array\"\nelement = \"u8\"\ncount = 0\n"),
        ErrorKind::BadCount("0".into())
    );
    assert!(matches!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"u32\"\ntargets = [\"KYWD\"]\n"
        ),
        ErrorKind::UnexpectedKey { key: "targets", .. }
    ));
    assert_eq!(
        invalid("[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"u32\"\ncard = \"many\"\n"),
        ErrorKind::UnknownCardinality("many".into())
    );
    assert_eq!(
        invalid("[[member]]\nname = \"Data\"\ntype = \"u32\"\n"),
        ErrorKind::MemberKind
    );
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\n\
             field = [{ name = \"A\", type = \"u32\", since = 5, until = 4 }]\n"
        ),
        ErrorKind::BadVersionRange {
            min: Some(5),
            max: Some(4)
        }
    );
}

#[test]
fn enum_and_flag_rules() {
    let field = |f: &str| {
        invalid(&format!(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\nfield = [{f}]\n"
        ))
    };
    assert_eq!(
        field("{ name = \"K\", type = \"enum\", base = \"u8\", values = [[\"A\", 256]] }"),
        ErrorKind::ValueOutOfRange {
            value: 256,
            base: "u8"
        }
    );
    assert_eq!(
        field(
            "{ name = \"K\", type = \"enum\", base = \"u8\", values = [[\"A\", 1], [\"B\", 1]] }"
        ),
        ErrorKind::DuplicateValue(1)
    );
    assert_eq!(
        field("{ name = \"K\", type = \"enum\", base = \"u8\", values = [[\"Other\", 1]] }"),
        ErrorKind::ReservedName("Other".into())
    );
    assert_eq!(
        field("{ name = \"K\", type = \"enum\", base = \"f32\", values = [[\"A\", 1]] }"),
        ErrorKind::BadBase {
            base: "f32",
            what: "enum values"
        }
    );
    assert_eq!(
        field("{ name = \"F\", type = \"flags\", base = \"u8\", bits = [[\"A\", 0]] }"),
        ErrorKind::ZeroFlag("A".into())
    );
    assert!(matches!(
        field("{ name = \"F\", type = \"flags\", base = \"i8\", bits = [[\"A\", 1]] }"),
        ErrorKind::BadBase { base: "i8", .. }
    ));
    assert_eq!(
        field("{ name = \"K\", type = \"enum\", base = \"u8\" }"),
        ErrorKind::MissingKey("values")
    );
}

#[test]
fn group_rules() {
    assert_eq!(
        invalid(
            "[[member]]\ngroup = \"Block\"\n[[member.member]]\nany_until = \"BEND\"\nname = \"Body\"\n\
             [[member.member]]\nsig = \"BEND\"\nname = \"End\"\ntype = \"empty\"\n"
        ),
        ErrorKind::GroupStart
    );
    assert_eq!(
        invalid(
            "[[member]]\ngroup = \"Block\"\n[[member.member]]\nsig = \"BEGN\"\nname = \"Begin\"\ntype = \"empty\"\n\
             [[member.member]]\nany_until = \"BEND\"\nname = \"Body\"\n"
        ),
        ErrorKind::AnyUntilUnclosed(Signature(*b"BEND"))
    );
    assert_eq!(
        invalid("[[member]]\ngroup = \"Block\"\nmember = []\n"),
        ErrorKind::Empty
    );
    assert_eq!(
        invalid("[[member]]\ngroup = \"Block\"\n"),
        ErrorKind::MissingKey("member")
    );
}

#[test]
fn nesting_is_bounded() {
    let mut body = String::new();
    let mut prefix = String::from("member");
    for depth in 0..12 {
        body.push_str(&format!("[[{prefix}]]\ngroup = \"G{depth}\"\n"));
        prefix.push_str(".member");
    }
    body.push_str(&format!(
        "[[{prefix}]]\nsig = \"LEAF\"\nname = \"Leaf\"\ntype = \"u8\"\n"
    ));
    assert!(matches!(invalid(&body), ErrorKind::TooDeep(_)));
}

#[test]
fn load_dir_reads_toml_files_in_order() {
    let dir = std::env::temp_dir().join(format!("kitcraft-schema-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ZZZZ.toml"), FIXTURE).unwrap();
    std::fs::write(dir.join("TEST.toml"), record("")).unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    let schema = load_dir(&dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let sigs: Vec<String> = schema
        .definitions
        .keys()
        .map(|s| s.as_str().to_owned())
        .collect();
    assert_eq!(sigs, ["TEST", "ZZZZ"]);
    assert!(matches!(load_dir(&dir), Err(SchemaError::Io { .. })));
}
