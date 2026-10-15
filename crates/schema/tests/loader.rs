//! Loader and validator tests (M1.8).

// Helpers outside `#[test]` fns panic to fail the calling test; this is test code.
#![allow(clippy::panic, clippy::unwrap_used)]

use std::path::Path;

use kitcraft_schema::{
    load_dir, load_sources, parse_definition, ArrayCount, Cardinality, CommonBody, Confidence,
    Decider, Definition, ErrorKind, FieldType, Member, Scalar, Schema, SchemaError, Signature,
    XEDIT_FILES, XEDIT_REVISION,
};

const FIXTURE: &str = include_str!("fixtures/ZZZZ.toml");

fn sig(s: &str) -> Signature {
    Signature::parse(s).unwrap()
}

/// A minimal valid record file with `body` appended.
fn record(body: &str) -> String {
    format!("signature = \"TEST\"\nname = \"Test\"\nsource = \"corpus: unit test\"\nconfidence = \"corpus\"\n{body}")
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
    assert_eq!(rec.signature, sig("ZZZZ"));
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
                    targets: vec![sig("KYWD")]
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
    let text = "signature = \"HDR0\"\nname = \"Hdr\"\nkind = \"header\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n\
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
            assert_eq!(line, 8);
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
        Err(SchemaError::Duplicate { key, first, .. }) => {
            assert_eq!(key, "signature TEST");
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
        let text = format!(
            "signature = \"TEST\"\nname = \"Test\"\nsource = \"{prefix}\"\n\
             evidence = [\"corpus: c\"]\nconfidence = \"corpus\"\n"
        );
        parse_definition("TEST.toml", &text).unwrap();
    }
    // A Skyrim container fact may say `confidence = "corpus"` or better (Amendment 2).
    let skyrim = "signature = \"TEST\"\nname = \"Test\"\nsource = \"uesp-skyrim: x\"\n\
                  evidence = [\"corpus: walk\"]\nconfidence = \"corpus\"\n";
    parse_definition("TEST.toml", skyrim).unwrap();
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
        ErrorKind::AnyUntilUnclosed(sig("BEND"))
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
        .records
        .keys()
        .map(|s| s.as_str().to_owned())
        .collect();
    assert_eq!(sigs, ["TEST", "ZZZZ"]);
    assert!(matches!(load_dir(&dir), Err(SchemaError::Io { .. })));
}

// ---------------------------------------------------------------------------------------
// Round 2: version probes, shared definitions, new field keys, headers, sources.

#[test]
fn size_by_version_is_checked_at_every_gate_boundary() {
    // Fields gated inside the entry's range: the old min/max probe passed this.
    let gated = "field = [{ name = \"A\", type = \"u32\" }, \
                 { name = \"B\", type = \"u32\", since = 50, until = 150 }]\n";
    assert_eq!(
        invalid(&format!(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\n\
             size_by_version = [{{ min = 0, max = 200, size = 4 }}]\n{gated}"
        )),
        ErrorKind::SizeConflict {
            declared: 4,
            computed: 8
        }
    );
    // An unbounded entry is probed across the whole range too.
    assert!(matches!(
        invalid(&format!(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\n\
             size_by_version = [{{ size = 4 }}]\n{gated}"
        )),
        ErrorKind::SizeConflict { .. }
    ));
    // The exact split is accepted.
    parse_definition(
        "TEST.toml",
        &record(&format!(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\n\
             size_by_version = [{{ max = 49, size = 4 }}, {{ min = 50, max = 150, size = 8 }}, \
             {{ min = 151, size = 4 }}]\n{gated}"
        )),
    )
    .unwrap();
}

#[test]
fn overlapping_size_by_version_ranges_are_rejected() {
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"opaque\"\n\
             size_by_version = [{ max = 100, size = 4 }, { min = 100, size = 8 }]\n"
        ),
        ErrorKind::OverlappingVersions {
            first: (None, Some(100)),
            second: (Some(100), None)
        }
    );
    assert!(matches!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"opaque\"\n\
             size_by_version = [{ size = 4 }, { min = 7, max = 9, size = 8 }]\n"
        ),
        ErrorKind::OverlappingVersions { .. }
    ));
}

#[test]
fn if_remaining_and_variable_fields_must_be_last() {
    let field = |f: &str| {
        invalid(&format!(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\nfield = [{f}]\n"
        ))
    };
    assert_eq!(
        field(
            "{ name = \"T\", type = \"u32\", if_remaining = true }, { name = \"U\", type = \"u8\" }"
        ),
        ErrorKind::VariableNotLast("T".into())
    );
    // Length-prefixed strings and count_from arrays delimit themselves: not last is fine.
    parse_definition(
        "TEST.toml",
        &record(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\nfield = [\
             { name = \"N\", type = \"u16\" }, \
             { name = \"L\", type = \"array\", element = \"u32\", count_from = \"N\" }, \
             { name = \"W\", type = \"wstring\" }, { name = \"B\", type = \"bstring\" }, \
             { name = \"T\", type = \"u8\", if_remaining = true }]\n",
        ),
    )
    .unwrap();
    // A struct ending in if_remaining reads to the end: it cannot be an array element.
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"array\"\ncount = 2\n\
             element = \"struct\"\nfield = [{ name = \"T\", type = \"u8\", if_remaining = true }]\n"
        ),
        ErrorKind::BadElement
    );
    // ... and the payload is variable, so a fixed `size` is wrong.
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\nsize = 1\n\
             field = [{ name = \"T\", type = \"u8\", if_remaining = true }]\n"
        ),
        ErrorKind::SizeOfVariable { declared: 1 }
    );
}

#[test]
fn count_from_rules() {
    let field = |f: &str| {
        invalid(&format!(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\nfield = [{f}]\n"
        ))
    };
    let arr = "{ name = \"L\", type = \"array\", element = \"u8\", count_from = \"N\" }";
    // Unknown, later, float, or gated sibling.
    assert_eq!(field(arr), ErrorKind::BadCountFrom("N".into()));
    assert_eq!(
        field(&format!("{arr}, {{ name = \"N\", type = \"u8\" }}")),
        ErrorKind::BadCountFrom("N".into())
    );
    assert_eq!(
        field(&format!("{{ name = \"N\", type = \"f32\" }}, {arr}")),
        ErrorKind::BadCountFrom("N".into())
    );
    assert_eq!(
        field(&format!(
            "{{ name = \"N\", type = \"u8\", since = 3 }}, {arr}"
        )),
        ErrorKind::BadCountFrom("N".into())
    );
    // Not with `count`, and not on a whole-subrecord array (no siblings).
    assert!(matches!(
        field(
            "{ name = \"N\", type = \"u8\" }, \
             { name = \"L\", type = \"array\", element = \"u8\", count = 2, count_from = \"N\" }"
        ),
        ErrorKind::UnexpectedKey {
            key: "count_from",
            ..
        }
    ));
    assert!(matches!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"array\"\nelement = \"u8\"\n\
             count_from = \"N\"\n"
        ),
        ErrorKind::UnexpectedKey {
            key: "count_from",
            ..
        }
    ));
}

const POINT: &str = "name = \"Point\"\nkind = \"type\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\ntype = \"struct\"\n\
                     field = [{ name = \"X\", type = \"i16\" }, { name = \"Y\", type = \"i16\" }]\n";
const BLOCK: &str =
    "name = \"Block\"\nkind = \"group\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n\
                     [[member]]\nsig = \"BEGN\"\nname = \"Begin\"\ntype = \"empty\"\n\
                     [[member]]\nany_until = \"BEND\"\nname = \"Body\"\n\
                     [[member]]\nsig = \"BEND\"\nname = \"End\"\ntype = \"empty\"\n";

fn with_common(body: &str) -> Result<Schema, SchemaError> {
    let text = record(body);
    load_sources([
        ("common/Point.toml", POINT),
        ("TEST.toml", text.as_str()),
        ("common/Block.toml", BLOCK),
    ])
}

fn invalid_with_common(body: &str) -> ErrorKind {
    match with_common(body) {
        Err(SchemaError::Invalid { kind, .. }) => kind,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[test]
fn shared_definitions_expand_into_records() {
    let schema = with_common(
        "[[member]]\nsig = \"PNTA\"\nname = \"Anchor\"\ntype = \"ref:Point\"\nsize = 4\n\
         [[member]]\nsig = \"PNTS\"\nname = \"Points\"\ntype = \"array\"\nelement = \"ref:Point\"\ncount = \"rest\"\n\
         [[member]]\ngroup_ref = \"Block\"\ncard = \"repeated\"\n\
         [[member]]\ngroup_ref = \"Block\"\nname = \"Second\"\ncard = \"optional\"\n",
    )
    .unwrap();
    assert_eq!(schema.common.len(), 2);
    assert!(matches!(
        schema.common["Block"].body,
        CommonBody::Group(ref m) if m.len() == 3
    ));
    let rec = schema.record(sig("TEST")).unwrap();
    let Member::Subrecord(anchor) = &rec.members[0] else {
        panic!("subrecord");
    };
    assert!(matches!(&anchor.ty, FieldType::Ref { name, .. } if name == "Point"));
    assert_eq!(anchor.ty.fixed_size(None), Some(4));
    let Member::Group(first) = &rec.members[2] else {
        panic!("group");
    };
    assert_eq!(
        (first.name.as_str(), first.shared.as_deref()),
        ("Block", Some("Block"))
    );
    assert_eq!(first.cardinality, Cardinality::Repeated);
    let Member::Group(second) = &rec.members[3] else {
        panic!("group");
    };
    assert_eq!(second.name, "Second");
    // Model helpers for coverage.
    let leaves: Vec<String> = rec.leaf_subrecords().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        leaves,
        [
            "Anchor",
            "Points",
            "Block.Begin",
            "Block.End",
            "Second.Begin",
            "Second.End"
        ]
    );
    let known = schema.known_signatures(sig("TEST")).unwrap();
    assert_eq!(known.len(), 4);
    assert!(known.contains(&sig("BEND")));
    assert!(schema.known_signatures(sig("NONE")).is_none());
}

#[test]
fn shared_definition_errors() {
    assert_eq!(
        invalid_with_common("[[member]]\nsig = \"DATA\"\nname = \"D\"\ntype = \"ref:Nope\"\n"),
        ErrorKind::UnknownRef("Nope".into())
    );
    assert_eq!(
        invalid_with_common("[[member]]\nsig = \"DATA\"\nname = \"D\"\ntype = \"ref:Block\"\n"),
        ErrorKind::RefKind {
            name: "Block".into(),
            expected: "type"
        }
    );
    assert_eq!(
        invalid_with_common("[[member]]\ngroup_ref = \"Point\"\n"),
        ErrorKind::RefKind {
            name: "Point".into(),
            expected: "group"
        }
    );
    assert!(matches!(
        invalid_with_common(
            "[[member]]\nsig = \"DATA\"\nname = \"D\"\ntype = \"ref:Point\"\nbase = \"u8\"\n"
        ),
        ErrorKind::UnexpectedKey { key: "base", .. }
    ));
    // Standalone parsing has no shared definitions.
    assert_eq!(
        invalid("[[member]]\ngroup_ref = \"Block\"\n"),
        ErrorKind::UnknownRef("Block".into())
    );
    // Cycles between shared types.
    let a = "name = \"A\"\nkind = \"type\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\ntype = \"struct\"\n\
             field = [{ name = \"B\", type = \"ref:B\" }]\n";
    let b = "name = \"B\"\nkind = \"type\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\ntype = \"ref:A\"\n";
    assert!(matches!(
        load_sources([("common/A.toml", a), ("common/B.toml", b)]),
        Err(SchemaError::Invalid {
            kind: ErrorKind::RefCycle(_),
            ..
        })
    ));
    // Duplicate shared names; file name must match the name.
    assert!(matches!(
        load_sources([("common/Point.toml", POINT), ("x/Point.toml", POINT)]),
        Err(SchemaError::Duplicate { .. })
    ));
    assert!(matches!(
        load_sources([("common/Pt.toml", POINT)]),
        Err(SchemaError::Invalid {
            kind: ErrorKind::FileName { .. },
            ..
        })
    ));
}

#[test]
fn header_without_signature() {
    let text = "name = \"RecordHeader\"\nkind = \"header\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\nsize = 8\n\
                field = [{ name = \"Signature\", type = \"sig\" }, { name = \"Size\", type = \"u32\" }]\n";
    let def = parse_definition("RECORD.toml", text).unwrap();
    let Definition::Header(h) = def else {
        panic!("header");
    };
    assert_eq!(h.signature, None);
    assert!(matches!(
        kind_of(parse_definition("Record.toml", text)),
        ErrorKind::FileName { .. }
    ));
    // A record still needs its signature.
    let rec = "name = \"Test\"\nsource = \"corpus: t\"\nconfidence = \"corpus\"\n";
    assert_eq!(
        kind_of(parse_definition("TEST.toml", rec)),
        ErrorKind::MissingKey("signature")
    );
}

#[test]
fn uesp_starfield_sources_and_the_forbidden_ctda_page() {
    let ok = "signature = \"TEST\"\nname = \"Test\"\n\
              source = \"uesp-starfield: https://starfieldwiki.net/wiki/Starfield_Mod:Mod_File_Format/Raw_Data\"\n\
              evidence = [\"corpus: c\"]\nconfidence = \"corpus\"\n";
    parse_definition("TEST.toml", ok).unwrap();
    let ctda = ok.replace("Raw_Data", "CTDA");
    assert!(matches!(
        kind_of(parse_definition("TEST.toml", &ctda)),
        ErrorKind::ForbiddenSource(_)
    ));
}

const PIN: &str = "9fb016884bec138ea6c7b872cec831537d464c3e";

/// A record file citing `source`, with `rest` appended.
fn cited(source: &str, rest: &str) -> String {
    format!("signature = \"TEST\"\nname = \"Test\"\nsource = \"{source}\"\n{rest}")
}

fn xedit(lines: &str) -> String {
    format!("xedit: wbDefinitionsSF1.pas@{PIN}#{lines}")
}

#[test]
fn xedit_sources_name_the_pinned_revision_and_a_line_range() {
    let ok = |source: &str, rest: &str| {
        parse_definition("TEST.toml", &cited(source, rest)).unwrap();
    };
    let reason = |source: &str| match kind_of(parse_definition(
        "TEST.toml",
        &cited(source, "confidence = \"reference\"\n"),
    )) {
        ErrorKind::BadXeditSource { citation, reason } => {
            assert_eq!(citation, source);
            reason
        }
        other => panic!("{source}: expected BadXeditSource, got {other:?}"),
    };

    ok(&xedit("L18594-L18833"), "confidence = \"reference\"\n");
    ok(&xedit("L7-L7"), "confidence = \"reference\"\n");
    // The owner approval of 2026-10-14 adds the common helper file, at the same pin.
    ok(
        &format!("xedit: wbDefinitionsCommon.pas@{PIN}#L8935-L8951"),
        "confidence = \"reference\"\n",
    );
    assert_eq!(XEDIT_REVISION, PIN);
    assert_eq!(
        XEDIT_FILES,
        ["wbDefinitionsSF1.pas", "wbDefinitionsCommon.pas"]
    );
    // Helper ranges go in `evidence`, same form.
    ok(
        "corpus: WEAP records of the official ESMs",
        &format!(
            "evidence = [\"{}\"]\nconfidence = \"corpus\"\n",
            xedit("L1-L2")
        ),
    );

    let pinned = |r: &str| r.contains("pins");
    let hex = |r: &str| r.contains("40-character");
    let range = |r: &str| r.contains("line range") || r.contains("line numbers");
    assert!(reason("xedit: wbDefinitionsSF1.pas").contains("`@<sha>`"));
    assert!(reason(&format!("xedit: wbDefinitionsSF1.pas@{PIN}")).contains("`#L"));
    assert!(reason(&format!("xedit: wbDefinitionsFO4.pas@{PIN}#L1-L2")).contains("file"));
    assert!(reason(&format!("xedit: wbDefinitionsSignatures.pas@{PIN}#L1-L2")).contains("file"));
    assert!(pinned(reason(
        "xedit: wbDefinitionsCommon.pas@4440381a3cff2174d173489085fcb8c9987ac71a#L1-L2"
    )));
    assert!(reason(&format!("xedit: Core/wbDefinitionsSF1.pas@{PIN}#L1-L2")).contains("file"));
    assert!(reason("xedit: wbDefinitionsSF1.pas@dev-4.1.6#L1-L2").contains("40-character"));
    assert!(hex(reason(&format!(
        "xedit: wbDefinitionsSF1.pas@{}#L1-L2",
        PIN.to_uppercase()
    ))));
    assert!(hex(reason(&format!(
        "xedit: wbDefinitionsSF1.pas@{}#L1-L2",
        PIN.get(..39).unwrap()
    ))));
    assert!(pinned(reason(
        "xedit: wbDefinitionsSF1.pas@4440381a3cff2174d173489085fcb8c9987ac71a#L1-L2"
    )));
    for lines in [
        "L5", "5-9", "L5-9", "L0-L3", "L03-L9", "L9-L5", "L-L5", "L+5-L9", "L5-L", "L1-L2x",
    ] {
        let r = reason(&xedit(lines));
        assert!(range(r), "{lines}: {r}");
    }
    // The prefix needs its space, like every other form.
    assert!(matches!(
        kind_of(parse_definition(
            "TEST.toml",
            &cited(
                &format!("xedit:wbDefinitionsSF1.pas@{PIN}#L1-L2"),
                "confidence = \"reference\"\n"
            )
        )),
        ErrorKind::BadSource(_)
    ));
}

#[test]
fn confidence_is_required_and_backed_by_citations() {
    let check =
        |source: &str, rest: &str| kind_of(parse_definition("TEST.toml", &cited(source, rest)));
    let x = xedit("L1-L2");
    let missing = ErrorKind::MissingConfidence("on every definition file");

    // Missing: an error on every file (M2.B1), whatever it cites.
    assert_eq!(check(&x, ""), missing);
    assert_eq!(
        check("corpus: c", &format!("evidence = [\"{x}\"]\n")),
        missing
    );
    assert_eq!(check("corpus: c", ""), missing);
    assert_eq!(
        check("uesp-skyrim: x", "evidence = [\"corpus: walk\"]\n"),
        missing
    );
    parse_definition(
        "TEST.toml",
        &cited("corpus: c", "confidence = \"corpus\"\n"),
    )
    .unwrap();
    assert_eq!(
        check("corpus: c", "confidence = \"high\"\n"),
        ErrorKind::UnknownConfidence("high".into())
    );
    assert_eq!(
        check(&x, "confidence = \"Reference\"\n"),
        ErrorKind::UnknownConfidence("Reference".into())
    );

    // Each level needs the citations that back it.
    let unsupported = |kind: ErrorKind, level: &str| match kind {
        ErrorKind::UnsupportedConfidence { confidence, .. } => assert_eq!(confidence, level),
        other => panic!("expected UnsupportedConfidence for {level}, got {other:?}"),
    };
    unsupported(
        check("corpus: c", "confidence = \"reference\"\n"),
        "reference",
    );
    unsupported(check(&x, "confidence = \"corpus\"\n"), "corpus");
    unsupported(check(&x, "confidence = \"ck-diff\"\n"), "ck-diff");
    unsupported(
        check(
            &x,
            "evidence = [\"corpus: c\"]\nconfidence = \"verified\"\n",
        ),
        "verified",
    );
    unsupported(
        check(
            "uesp-skyrim: x",
            &format!("evidence = [\"{x}\"]\nconfidence = \"reference\"\n"),
        ),
        "reference",
    );
    unsupported(
        check("uesp-skyrim: x", "confidence = \"corpus\"\n"),
        "corpus",
    );

    for rest in [
        "evidence = [\"corpus: c\"]\nconfidence = \"corpus\"\n",
        "evidence = [\"ck-diff: obs-1\"]\nconfidence = \"ck-diff\"\n",
        "evidence = [\"corpus: c\", \"ck-diff: obs-1\"]\nconfidence = \"verified\"\n",
    ] {
        parse_definition("TEST.toml", &cited(&x, rest)).unwrap();
    }
    parse_definition(
        "TEST.toml",
        &cited(
            "uesp-skyrim: x",
            "evidence = [\"corpus: c\", \"ck-diff: o\"]\nconfidence = \"verified\"\n",
        ),
    )
    .unwrap();
}

/// M2.B1: members and fields carry their own provenance, or inherit their parent's; the
/// model stores the effective value on every node.
#[test]
fn members_and_fields_carry_provenance() {
    let x = xedit("L10-L20");
    let text = format!(
        "signature = \"TEST\"\nname = \"Test\"\nsource = \"{x}\"\nconfidence = \"reference\"\n\
         [[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = \"zstring\"\n\
         evidence = [\"corpus: every EDID ends in NUL\"]\nconfidence = \"corpus\"\n\
         [[member]]\nsig = \"DATA\"\nname = \"Data\"\ntype = \"struct\"\n\
         field = [\n\
           {{ name = \"A\", type = \"u32\" }},\n\
           {{ name = \"B\", type = \"f32\", source = \"ck-diff: obs-7\", confidence = \"ck-diff\" }},\n\
         ]\n\
         [[member]]\ngroup = \"Pair\"\ncard = \"optional\"\n\
         source = \"corpus: pair scan\"\nconfidence = \"corpus\"\n\
         [[member.member]]\nsig = \"XNAM\"\nname = \"X\"\ntype = \"u8\"\n"
    );
    let Definition::Record(rec) = parse_definition("TEST.toml", &text).unwrap() else {
        panic!("record");
    };
    assert_eq!(rec.provenance.confidence, Confidence::Reference);
    assert_eq!(rec.provenance.source, x);
    let Member::Subrecord(edid) = &rec.members[0] else {
        panic!("EDID");
    };
    // Inherited source, own evidence added, own level.
    assert_eq!(edid.provenance.source, x);
    assert_eq!(edid.provenance.evidence, ["corpus: every EDID ends in NUL"]);
    assert_eq!(edid.provenance.confidence, Confidence::Corpus);
    let Member::Subrecord(data) = &rec.members[1] else {
        panic!("DATA");
    };
    assert_eq!(data.provenance, rec.provenance);
    let FieldType::Struct { fields } = &data.ty else {
        panic!("struct");
    };
    assert_eq!(fields[0].provenance, rec.provenance);
    assert_eq!(fields[1].provenance.source, "ck-diff: obs-7");
    assert!(fields[1].provenance.evidence.is_empty());
    assert_eq!(fields[1].provenance.confidence, Confidence::CkDiff);
    let Member::Group(pair) = &rec.members[2] else {
        panic!("group");
    };
    assert_eq!(pair.provenance.source, "corpus: pair scan");
    let Member::Subrecord(xnam) = &pair.members[0] else {
        panic!("XNAM");
    };
    assert_eq!(xnam.provenance, pair.provenance);
    let levels: Vec<Confidence> = rec
        .leaf_subrecords()
        .iter()
        .map(|(_, s)| s.provenance.confidence)
        .collect();
    assert_eq!(
        levels,
        [
            Confidence::Corpus,
            Confidence::Reference,
            Confidence::Corpus
        ]
    );

    // Errors: an own source needs an own confidence; levels are checked against the
    // effective citations; values and citation forms are checked on members too.
    let member = |keys: &str| {
        invalid(&format!(
            "[[member]]\nsig = \"EDID\"\nname = \"E\"\ntype = \"zstring\"\n{keys}"
        ))
    };
    assert_eq!(
        member("source = \"corpus: c\"\n"),
        ErrorKind::MissingConfidence("on a member or field with its own `source`")
    );
    assert!(matches!(
        member("confidence = \"reference\"\n"),
        ErrorKind::UnsupportedConfidence { .. }
    ));
    assert!(matches!(
        member("source = \"corpus: c\"\nconfidence = \"ck-diff\"\n"),
        ErrorKind::UnsupportedConfidence { .. }
    ));
    assert_eq!(
        member("confidence = \"sure\"\n"),
        ErrorKind::UnknownConfidence("sure".into())
    );
    assert_eq!(
        member("evidence = [\"a hunch\"]\n"),
        ErrorKind::BadSource("a hunch".into())
    );
    assert!(matches!(
        invalid("[[member]]\nsig = \"DATA\"\nname = \"D\"\ntype = \"struct\"\n\
                 field = [{ name = \"A\", type = \"u8\", source = \"xedit: nope\", confidence = \"reference\" }]\n"),
        ErrorKind::BadXeditSource { .. }
    ));
    // Own evidence on top of the file's citations can raise the level.
    parse_definition(
        "TEST.toml",
        &record(
            "[[member]]\nsig = \"EDID\"\nname = \"E\"\ntype = \"zstring\"\n\
             evidence = [\"ck-diff: obs-2\"]\nconfidence = \"verified\"\n",
        ),
    )
    .unwrap();
}

/// Shared definitions keep their own provenance wherever they are expanded.
#[test]
fn shared_members_keep_the_shared_provenance() {
    let block = "name = \"Block\"\nkind = \"group\"\nsource = \"corpus: block scan\"\n\
                 confidence = \"corpus\"\n\
                 [[member]]\nsig = \"BNAM\"\nname = \"B\"\ntype = \"u8\"\n";
    let rec = format!(
        "signature = \"TEST\"\nname = \"Test\"\nsource = \"{}\"\nconfidence = \"reference\"\n\
         [[member]]\ngroup_ref = \"Block\"\n",
        xedit("L1-L9")
    );
    let schema = load_sources([("common/Block.toml", block), ("TEST.toml", rec.as_str())]).unwrap();
    let test = schema.record(sig("TEST")).unwrap();
    let Member::Group(g) = &test.members[0] else {
        panic!("group");
    };
    assert_eq!(g.provenance.confidence, Confidence::Reference);
    let Member::Subrecord(b) = &g.members[0] else {
        panic!("BNAM");
    };
    assert_eq!(b.provenance.source, "corpus: block scan");
    assert_eq!(b.provenance.confidence, Confidence::Corpus);
    assert_eq!(
        schema.common["Block"]
            .provenance
            .citations()
            .collect::<Vec<_>>(),
        ["corpus: block scan"]
    );
}

/// M2.B9: `type = "union"` (layout by EditorID prefix) on whole subrecord members only.
#[test]
fn union_rules() {
    let edid = "[[member]]\nsig = \"EDID\"\nname = \"EditorId\"\ntype = \"zstring\"\n";
    let union = |keys: &str| {
        format!("{edid}[[member]]\nsig = \"DATA\"\nname = \"Value\"\ntype = \"union\"\n{keys}")
    };
    let ok = "decider = \"edid-prefix\"\nvariants = [\n\
              { when = \"f\", name = \"Float\", type = \"f32\" },\n\
              { when = \"s\", name = \"Text\", type = \"lstring\" },\n\
              { when = \"r\", name = \"Ref\", type = \"formid\", targets = [\"KYWD\"] },\n]\n";
    let Definition::Record(rec) = parse_definition("TEST.toml", &record(&union(ok))).unwrap()
    else {
        panic!("record");
    };
    let Member::Subrecord(data) = &rec.members[1] else {
        panic!("DATA");
    };
    let FieldType::Union { decider, variants } = &data.ty else {
        panic!("union");
    };
    assert_eq!(*decider, Decider::EditorIdPrefix);
    assert_eq!(decider.as_str(), "edid-prefix");
    let whens: Vec<u8> = variants.iter().map(|v| v.when).collect();
    assert_eq!(whens, b"fsr");
    // Variable (lstring) variant: no fixed size, and a declared size is rejected.
    assert_eq!(data.ty.fixed_size(None), None);
    assert!(matches!(
        invalid(&union(&format!("size = 4\n{ok}"))),
        ErrorKind::SizeOfVariable { declared: 4 }
    ));
    // Same-size variants have a fixed size.
    let fixed = "decider = \"edid-prefix\"\nsize = 4\nvariants = [\n\
                 { when = \"f\", name = \"Float\", type = \"f32\" },\n\
                 { when = \"i\", name = \"Int\", type = \"i32\" },\n]\n";
    parse_definition("TEST.toml", &record(&union(fixed))).unwrap();

    let v = |entries: &str| format!("decider = \"edid-prefix\"\nvariants = [{entries}]\n");
    assert_eq!(
        invalid(&union(
            "variants = [{ when = \"f\", name = \"F\", type = \"f32\" }]\n"
        )),
        ErrorKind::MissingKey("decider")
    );
    assert_eq!(
        invalid(&union("decider = \"edid-prefix\"\n")),
        ErrorKind::MissingKey("variants")
    );
    assert_eq!(
        invalid(&union("decider = \"by-color\"\nvariants = []\n")),
        ErrorKind::UnknownDecider("by-color".into())
    );
    assert_eq!(invalid(&union(&v(""))), ErrorKind::Empty);
    assert_eq!(
        invalid(&union(&v(
            "{ when = \"ff\", name = \"F\", type = \"f32\" }"
        ))),
        ErrorKind::BadWhen("ff".into())
    );
    assert_eq!(
        invalid(&union(&v("{ when = \"?\", name = \"F\", type = \"f32\" }"))),
        ErrorKind::BadWhen("?".into())
    );
    assert_eq!(
        invalid(&union(&v(
            "{ when = \"f\", name = \"A\", type = \"f32\" }, { when = \"f\", name = \"B\", type = \"u32\" }"
        ))),
        ErrorKind::DuplicateValue(i128::from(b'f'))
    );
    assert_eq!(
        invalid(&union(&v(
            "{ when = \"f\", name = \"A\", type = \"f32\" }, { when = \"g\", name = \"A\", type = \"u32\" }"
        ))),
        ErrorKind::DuplicateName("A".into())
    );
    assert!(matches!(
        invalid(&union(&v(
            "{ when = \"f\", name = \"A\", type = \"struct\" }"
        ))),
        ErrorKind::UnexpectedKey { key: "type", .. }
    ));
    assert!(matches!(
        invalid(&union(&format!("targets = [\"KYWD\"]\n{ok}"))),
        ErrorKind::UnexpectedKey { key: "targets", .. }
    ));
    // Only on whole subrecords, and only in a record that has an EDID member.
    assert_eq!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"D\"\ntype = \"struct\"\n\
             field = [{ name = \"A\", type = \"union\" }]\n"
        ),
        ErrorKind::UnionPlacement
    );
    assert_eq!(
        invalid(&format!(
            "[[member]]\nsig = \"DATA\"\nname = \"Value\"\ntype = \"union\"\n{ok}"
        )),
        ErrorKind::UnionWithoutEditorId
    );
    assert!(matches!(
        invalid(
            "[[member]]\nsig = \"DATA\"\nname = \"D\"\ntype = \"u32\"\ndecider = \"edid-prefix\"\n"
        ),
        ErrorKind::UnexpectedKey { key: "decider", .. }
    ));
}

#[test]
fn load_dir_reads_common_definitions() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let schema = load_dir(&dir).unwrap();
    let names: Vec<&str> = schema.common.keys().map(String::as_str).collect();
    assert_eq!(names, ["Link", "Marker", "Point"]);
    let sigs: Vec<&str> = schema.records.keys().map(Signature::as_str).collect();
    assert_eq!(sigs, ["EMPT", "UNOR", "UUUU", "VVVV", "YYYY", "ZZZZ"]);
    assert!(schema.records[&sig("UNOR")].unordered);
    assert!(!schema.records[&sig("ZZZZ")].unordered);
}

/// M4.BX1: `unordered = true` marks a record whose members may occur in any order. Only
/// records take it; an unordered record may not have a top-level `any_until` (it would
/// accept every subrecord out of place), and `DiskOrder` is a reserved member name.
#[test]
fn unordered_records() {
    let one = "[[member]]\nsig = \"DATA\"\nname = \"D\"\ntype = \"u32\"\n";
    let def = parse_definition("TEST.toml", &record(&format!("unordered = true\n{one}"))).unwrap();
    let Definition::Record(rec) = def else {
        panic!("expected a record");
    };
    assert!(rec.unordered);
    let any = "[[member]]\nany_until = \"DATA\"\nname = \"Body\"\n";
    assert_eq!(
        invalid(&format!("unordered = true\n{any}{one}")),
        ErrorKind::UnorderedAnyUntil
    );
    assert_eq!(
        invalid("[[member]]\nsig = \"DATA\"\nname = \"DiskOrder\"\ntype = \"u32\"\n"),
        ErrorKind::ReservedName("DiskOrder".into())
    );
    let header = "name = \"Hdr\"\nkind = \"header\"\nsource = \"corpus: t\"\n\
                  confidence = \"corpus\"\nunordered = true\nsize = 4\n\
                  field = [{ name = \"Size\", type = \"u32\" }]\n";
    assert!(matches!(
        kind_of(parse_definition("HDR.toml", header)),
        ErrorKind::UnexpectedKey {
            key: "unordered",
            ..
        }
    ));
    let shared = "name = \"Pt\"\nkind = \"type\"\nsource = \"corpus: t\"\n\
                  confidence = \"corpus\"\nunordered = true\ntype = \"u32\"\n";
    assert!(matches!(
        kind_of(parse_definition("common/Pt.toml", shared)),
        ErrorKind::UnexpectedKey {
            key: "unordered",
            ..
        }
    ));
}

#[test]
fn signatures_from_bytes_enforce_printable_ascii() {
    assert_eq!(Signature::from_bytes(*b"WEAP"), Some(sig("WEAP")));
    assert_eq!(Signature::from_bytes([b'W', 0, b'A', b'P']), None);
    assert_eq!(Signature::from_bytes([0xFF; 4]), None);
    assert_eq!(sig("NPC_").as_bytes(), *b"NPC_");
    assert_eq!(sig("NPC_").to_string(), "NPC_");
}
