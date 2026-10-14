//! Synthetic fixtures for the `.psc` declaration reader. Every script here is ours; no
//! official source text is in the repository (ADR 0006 Amendment 1).

use super::{has_flag, parse, CallableKind, PscError, MAX_NESTING, MAX_SOURCE_BYTES};

const BASE: &str = r#"ScriptName Kc:Base extends ScriptObject Native Hidden
{Doc for the script}

Import Utility
CustomEvent OnThingHappened
Guard StateGuard ProtectsFunctionLogic

; Returns the answer.
; Two lines.
Int Function Answer(Int aiBase = -1, String asName = "a, b", Float afScale = 1.5) Native Global
Bool Function IsReady() Native

Event OnLoad()
EndEvent

Event OnHit(ObjectReference akTarget, Form akSource, Var[] akArgs)
  ; handler body is skipped, even with Function words inside
  if akTarget
    Debug.Trace("Function Fake() EndEvent")
  endif
EndEvent

ObjectReference[] Function Nearby(Float afRadius) \
    Native DebugOnly
"#;

#[test]
fn header_imports_custom_events_guards() {
    let s = parse(BASE.as_bytes()).unwrap();
    assert_eq!(s.name, "Kc:Base");
    assert_eq!(s.parent.as_deref(), Some("ScriptObject"));
    assert!(has_flag(&s.flags, "native") && has_flag(&s.flags, "HIDDEN"));
    assert_eq!(s.span.line, 1);
    assert_eq!(s.doc.as_deref(), Some("Doc for the script"));
    assert_eq!(s.imports[0].name, "Utility");
    assert_eq!(s.custom_events[0].name, "OnThingHappened");
    assert_eq!(s.custom_events[0].span.line, 5);
    assert_eq!(s.guards[0].name, "StateGuard");
    assert_eq!(s.guards[0].flags[0].name, "ProtectsFunctionLogic");
    assert!(s.diagnostics.is_empty(), "{:?}", s.diagnostics);
}

#[test]
fn functions_with_params_defaults_flags_and_comments() {
    let s = parse(BASE.as_bytes()).unwrap();
    let names: Vec<&str> = s.functions.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Answer", "IsReady", "Nearby"]);
    let answer = &s.functions[0];
    assert_eq!(answer.kind, CallableKind::Function);
    assert_eq!(answer.return_type.as_ref().unwrap().to_string(), "Int");
    assert_eq!(answer.span.line, 10);
    assert_eq!(answer.params.len(), 3);
    assert_eq!(answer.params[0].ty.name, "Int");
    assert_eq!(answer.params[0].name, "aiBase");
    assert_eq!(answer.params[0].default.as_deref(), Some("-1"));
    assert_eq!(answer.params[1].default.as_deref(), Some("\"a, b\""));
    assert_eq!(answer.params[2].default.as_deref(), Some("1.5"));
    assert!(has_flag(&answer.flags, "native") && has_flag(&answer.flags, "global"));
    assert_eq!(
        answer.comment.as_deref(),
        Some("Returns the answer.\nTwo lines.")
    );
    // The span covers the statement as written.
    let text = &BASE[answer.span.offset as usize..][..answer.span.len as usize];
    assert!(text.starts_with("Int Function Answer(") && text.ends_with("Native Global"));

    // A continued line is one statement; array return type.
    let nearby = &s.functions[2];
    assert_eq!(
        nearby.return_type.as_ref().unwrap().to_string(),
        "ObjectReference[]"
    );
    assert!(nearby.return_type.as_ref().unwrap().array);
    assert!(has_flag(&nearby.flags, "DebugOnly"));
    assert_eq!(nearby.span.line, 23);
    assert!(s.functions[1].comment.is_none());
}

#[test]
fn events_bodies_are_skipped() {
    let s = parse(BASE.as_bytes()).unwrap();
    let names: Vec<&str> = s.events.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["OnLoad", "OnHit"]);
    let hit = &s.events[1];
    assert_eq!(hit.kind, CallableKind::Event);
    assert!(hit.return_type.is_none());
    let types: Vec<String> = hit.params.iter().map(|p| p.ty.to_string()).collect();
    assert_eq!(types, ["ObjectReference", "Form", "Var[]"]);
    assert_eq!(hit.span.line, 16);
}

const USER: &str = r#"Scriptname Kc_Door extends ObjectReference Const

Group Required_Properties CollapsedOnRef
    Quest Property MyQuest Auto Const Mandatory
    {The quest}
    Int Property Stage = 10 Auto Const
    Keyword Property Lock Auto Const RequiresGuard (StateGuard)
EndGroup

Float Property Speed Hidden
    Float Function Get()
        return 1.0
    EndFunction
    Function Set(Float value)
    EndFunction
EndProperty

Struct Entry
    Form Item
    Int Count = 1
    String Label = "x" Hidden
EndStruct

Int counter = 0
bool done

Event ObjectReference.OnLoad(ObjectReference akSender)
EndEvent

Auto State Waiting
    Event OnActivate(ObjectReference akActionRef)
        GoToState("Busy")
    EndEvent
EndState

State Busy
    Event OnActivate(ObjectReference akActionRef)
    EndEvent
EndState
"#;

#[test]
fn properties_groups_structs_variables_states_remote_events() {
    let s = parse(USER.as_bytes()).unwrap();
    assert!(s.diagnostics.is_empty(), "{:?}", s.diagnostics);
    assert_eq!(s.groups[0].name, "Required_Properties");
    assert_eq!(s.groups[0].flags[0].name, "CollapsedOnRef");

    let props: Vec<(&str, Option<&str>, bool)> = s
        .properties
        .iter()
        .map(|p| (p.name.as_str(), p.group.as_deref(), p.full))
        .collect();
    assert_eq!(
        props,
        [
            ("MyQuest", Some("Required_Properties"), false),
            ("Stage", Some("Required_Properties"), false),
            ("Lock", Some("Required_Properties"), false),
            ("Speed", None, true),
        ]
    );
    assert_eq!(s.properties[0].doc.as_deref(), Some("The quest"));
    assert!(has_flag(&s.properties[0].flags, "mandatory"));
    assert_eq!(s.properties[1].default.as_deref(), Some("10"));
    let guard = s.properties[2]
        .flags
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case("requiresguard"))
        .unwrap();
    assert_eq!(guard.arg.as_deref(), Some("StateGuard"));
    // Get/Set are not script functions.
    assert!(s.functions.is_empty());

    assert_eq!(s.structs[0].name, "Entry");
    let members: Vec<(&str, Option<&str>)> = s.structs[0]
        .members
        .iter()
        .map(|m| (m.name.as_str(), m.default.as_deref()))
        .collect();
    assert_eq!(
        members,
        [
            ("Item", None),
            ("Count", Some("1")),
            ("Label", Some("\"x\""))
        ]
    );
    let vars: Vec<&str> = s.variables.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(vars, ["counter", "done"]);

    let events: Vec<(Option<&str>, &str, Option<&str>)> = s
        .events
        .iter()
        .map(|e| (e.remote.as_deref(), e.name.as_str(), e.state.as_deref()))
        .collect();
    assert_eq!(
        events,
        [
            (Some("ObjectReference"), "OnLoad", None),
            (None, "OnActivate", Some("Waiting")),
            (None, "OnActivate", Some("Busy")),
        ]
    );
    assert_eq!(s.states.len(), 2);
    assert!(has_flag(&s.states[0].flags, "auto"));
    assert!(!has_flag(&s.states[1].flags, "auto"));
}

#[test]
fn comments_of_every_kind_and_case_insensitive_keywords() {
    let src = "scriptNAME Kc_C   ; trailing\n\
               ;/ a block\n comment over lines\n with Function words /;\n\
               FUNCTION Foo() NATIVE ; trailing\n\
               { a doc\n over two lines }\n\
               endFunction\n";
    // `endFunction` after a native function has nothing to close.
    let err = parse(src.as_bytes()).unwrap_err();
    assert_eq!(
        err,
        PscError::UnexpectedEnd {
            line: 8,
            keyword: "endFunction".into()
        }
    );
    let ok = src.replace("endFunction\n", "");
    let s = parse(ok.as_bytes()).unwrap();
    assert_eq!(s.functions[0].name, "Foo");
    assert_eq!(s.functions[0].span.line, 5);
    assert_eq!(
        s.functions[0].doc.as_deref(),
        Some("a doc\n over two lines")
    );
    // A block comment ending right above the function is its comment too.
    assert_eq!(
        s.functions[0].comment.as_deref(),
        Some("a block\n comment over lines\n with Function words")
    );
}

#[test]
fn unknown_statements_are_diagnostics_not_errors() {
    let src = "Scriptname Kc_D\nWhatever = 5\nInt Function F(Int) Native\n+ junk\n";
    let s = parse(src.as_bytes()).unwrap();
    let lines: Vec<u32> = s.diagnostics.iter().map(|d| d.line).collect();
    assert_eq!(lines, [2, 3, 4]);
    assert!(s.diagnostics[1].message.contains("unreadable parameters"));
    // A function with unreadable parameters still has its body skipped.
    let src = "Scriptname Kc_E\nFunction F(Int = ) \n  x = 1\nEndFunction\nInt v\n";
    let s = parse(src.as_bytes()).unwrap();
    assert_eq!(s.variables[0].name, "v");
    assert_eq!(s.diagnostics.len(), 1);
}

#[test]
fn hostile_inputs_are_typed_errors() {
    let cases: [(&str, PscError); 9] = [
        ("", PscError::MissingScriptName),
        ("; only a comment\n", PscError::MissingScriptName),
        (
            "Scriptname A\n;/ never closed\n",
            PscError::UnterminatedComment { line: 2 },
        ),
        (
            "Scriptname A\n{ never closed\n",
            PscError::UnterminatedDoc { line: 2 },
        ),
        (
            "Scriptname A\nString s = \"open\n",
            PscError::UnterminatedString { line: 2 },
        ),
        (
            "Scriptname A\nEndEvent\n",
            PscError::UnexpectedEnd {
                line: 2,
                keyword: "EndEvent".into(),
            },
        ),
        (
            "Scriptname A\nFunction F()\n",
            PscError::Unclosed {
                line: 2,
                kind: "function",
                name: "F".into(),
                expected: "EndFunction",
                found: String::new(),
            },
        ),
        (
            "Scriptname A\nEvent E()\nEndFunction\n",
            PscError::Unclosed {
                line: 2,
                kind: "event",
                name: "E".into(),
                expected: "EndEvent",
                found: " before `endfunction` at line 3".into(),
            },
        ),
        (
            "Scriptname A\nGroup G\nEndStruct\n",
            PscError::Unclosed {
                line: 2,
                kind: "group",
                name: "G".into(),
                expected: "EndGroup",
                found: " before `endstruct` at line 3".into(),
            },
        ),
    ];
    for (src, want) in cases {
        assert_eq!(parse(src.as_bytes()).unwrap_err(), want, "{src:?}");
    }
    let deep = format!("Scriptname A\n{}", "Group G\n".repeat(MAX_NESTING + 1));
    assert_eq!(
        parse(deep.as_bytes()).unwrap_err(),
        PscError::TooDeep {
            line: MAX_NESTING as u32 + 2
        }
    );
    let big = vec![b' '; MAX_SOURCE_BYTES + 1];
    assert!(matches!(
        parse(&big).unwrap_err(),
        PscError::TooLarge { .. }
    ));
}

/// Arbitrary bytes never panic: every prefix of the fixtures, and byte noise.
#[test]
fn no_panics_on_truncated_or_noisy_input() {
    for src in [BASE, USER] {
        let bytes = src.as_bytes();
        for cut in 0..bytes.len() {
            let _ = parse(&bytes[..cut]);
        }
    }
    const ALPHABET: &[u8] = b"Scriptname;/{}()[]=,.\\\"\n -1FunctionEndEventProperty";
    let mut state: u32 = 0x1234_5678;
    for _ in 0..2_000 {
        let mut noise = Vec::with_capacity(64);
        for _ in 0..64 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            noise.push(ALPHABET[state as usize % ALPHABET.len()]);
        }
        let _ = parse(&noise);
    }
    // Invalid UTF-8 is replaced, not an error.
    let mut bad = b"Scriptname A\n; caf".to_vec();
    bad.push(0xE9);
    bad.extend(b"\nInt x\n");
    assert_eq!(parse(&bad).unwrap().variables[0].name, "x");
}
