//! A synthetic script built byte by byte, its disassembly, and hostile input.

use crate::text::{python_g, python_repr};
use crate::{disassemble, Error, Pex, Value};

/// Little-endian byte builder.
#[derive(Default)]
struct B(Vec<u8>);
impl B {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn ws(&mut self, s: &str) -> &mut Self {
        let bytes: Vec<u8> = s.chars().map(|c| c as u8).collect();
        self.u16(bytes.len() as u16);
        self.0.extend(bytes);
        self
    }
    fn ident(&mut self, i: u16) -> &mut Self {
        self.u8(1).u16(i)
    }
    fn string(&mut self, i: u16) -> &mut Self {
        self.u8(2).u16(i)
    }
    fn int(&mut self, v: i32) -> &mut Self {
        self.u8(3);
        self.0.extend(v.to_le_bytes());
        self
    }
    fn float(&mut self, v: f32) -> &mut Self {
        self.u8(4);
        self.0.extend(v.to_le_bytes());
        self
    }
}

const STRINGS: [&str; 18] = [
    "",                // 0
    "MyScript",        // 1
    "ObjectReference", // 2
    "Int",             // 3
    "Float",           // 4
    "count",           // 5
    "ratio",           // 6
    "Speed",           // 7
    "OnLoad",          // 8
    "None",            // 9
    "self",            // 10
    "Activate",        // 11
    "it's \"x\"\n",    // 12
    "::temp0",         // 13
    "Busy",            // 14
    "Point",           // 15
    "x",               // 16
    "Value",           // 17
];

fn function(b: &mut B, params: &[(u16, u16)], code: impl FnOnce(&mut B) -> u16) {
    b.u16(9).u16(0).u32(0).u8(0); // return type None, doc, user flags, flags
    b.u16(params.len() as u16);
    for (n, t) in params {
        b.u16(*n).u16(*t);
    }
    b.u16(1).u16(13).u16(3); // one local ::temp0 : Int
    let mut body = B::default();
    let n = code(&mut body);
    b.u16(n);
    b.0.extend(body.0);
}

fn script() -> Vec<u8> {
    let mut b = B::default();
    b.u32(0xFA57_C0DE).u8(3).u8(12).u16(2).u64(1_700_000_000);
    b.ws("MyScript.psc").ws("user").ws("machine");
    b.u16(STRINGS.len() as u16);
    for s in STRINGS {
        b.ws(s);
    }
    b.u8(0); // no debug info
    b.u16(1).u16(1).u8(0); // one user flag
    b.u16(1); // one object
    let mut o = B::default();
    o.u16(2).u16(0).u8(0).u32(0).u16(0); // parent, doc, const, user flags, auto state
                                         // One struct Point { Int x = 3 }.
    o.u16(1).u16(15).u16(1);
    o.u16(16).u16(3).u32(0).int(3).u8(0).u16(0);
    // Variables: count = -7, ratio = 0.1.
    o.u16(2);
    o.u16(5).u16(3).u32(0).int(-7).u8(0);
    o.u16(6).u16(4).u32(0).float(0.1).u8(0);
    o.u16(1).u16(14); // one guard
                      // Properties: Speed (get + set), Value (auto, backed by count).
    o.u16(2);
    o.u16(7).u16(4).u16(0).u32(0).u8(3);
    function(&mut o, &[], |c| {
        c.u8(26).ident(6); // return ratio
        1
    });
    function(&mut o, &[(17, 4)], |c| {
        c.u8(13).ident(6).ident(17); // assign ratio Value
        1
    });
    o.u16(17).u16(3).u16(0).u32(0).u8(7).u16(5);
    // States: default with OnLoad; "Busy" with Activate.
    o.u16(2);
    o.u16(0).u16(1).u16(8);
    function(&mut o, &[], |c| {
        // callmethod Activate self ::temp0 2 "it's..." 1.5e7
        c.u8(23)
            .ident(11)
            .ident(10)
            .ident(13)
            .int(2)
            .string(12)
            .float(1.5e7);
        c.u8(48).int(1).ident(14); // lock_guards Busy
        c.u8(1).ident(13).ident(13).int(1); // iadd
        c.u8(0); // nop
        c.u8(26).u8(0); // return None
        5
    });
    o.u16(14).u16(1).u16(11);
    function(&mut o, &[(10, 2), (5, 3)], |c| {
        c.u8(15).ident(13).u8(5).u8(1).u8(5).u8(0); // cmp_eq ::temp0 True False
        1
    });
    b.u16(1); // object name MyScript
    b.u32(o.0.len() as u32 + 4);
    b.0.extend(o.0);
    b.0
}

const EXPECTED: &str = "MyScript.psc v3.12 gid2
OBJECT MyScript extends ObjectReference
  VAR Int count = -7
  VAR Float ratio = 0.1
  PROP Float Speed
  FUNC Speed.get()
      0 return ratio
  FUNC Speed.set(Value)
      0 assign ratio Value
  PROP Int Value
  FUNC OnLoad()
      0 callmethod Activate self ::temp0 'it\\'s \"x\"\\n' 1.5e+07
      1 lock_guards Busy
      2 iadd ::temp0 ::temp0 1
      3 nop\x20
      4 return None
  FUNC Busy.Activate(self, count)
      0 cmp_eq ::temp0 True False
";

#[test]
fn synthetic_script_disassembles_like_pexdis() {
    let bytes = script();
    let pex = Pex::parse(&bytes).unwrap();
    assert_eq!(disassemble(&pex), EXPECTED);
    let object = &pex.objects[0];
    assert_eq!(object.structs.len(), 1);
    assert_eq!(object.structs[0].members[0].value, Value::Int(3));
    assert_eq!(object.guards, [14]);
    assert_eq!(object.properties[1].auto_var, Some(5));
    assert_eq!(
        object.size as usize,
        bytes.len() - object.offset as usize - 2
    );
    let call = &object.states[0].functions[0].function.instructions[0];
    assert_eq!(call.args.len(), 3);
    assert_eq!(call.varargs.as_ref().map(Vec::len), Some(2));
    assert_eq!(pex.user_flags[0].name, 1);
    assert_eq!(pex.string(999), "");
}

#[test]
fn hostile_input_gives_typed_errors() {
    let good = script();
    for len in 0..good.len() {
        assert!(Pex::parse(&good[..len]).is_err(), "len {len}");
    }
    for i in 0..good.len() {
        for mask in [0x01u8, 0x80, 0xFF] {
            let mut bad = good.clone();
            bad[i] ^= mask;
            let _ = Pex::parse(&bad).map(|p| disassemble(&p));
        }
    }
    let mut trailing = good.clone();
    trailing.push(0);
    assert!(matches!(
        Pex::parse(&trailing),
        Err(Error::TrailingBytes { count: 1, .. })
    ));
    let mut magic = good.clone();
    magic[0] = 0;
    assert!(matches!(Pex::parse(&magic), Err(Error::BadMagic { .. })));
    // A string index past the table.
    let mut bad = good.clone();
    let at = bad.len() - 1 - 5; // the cmp_eq first operand's index (ident 13)
    assert_eq!(bad[at - 1], 1);
    bad[at] = 200;
    assert!(matches!(
        Pex::parse(&bad),
        Err(Error::BadString { index: 200, .. })
    ));
}

#[test]
fn python_float_and_string_formats() {
    for (v, want) in [
        (0.5, "0.5"),
        (1.0, "1"),
        (100_000.0, "100000"),
        (999_999.0, "999999"),
        (999_999.5, "1e+06"),
        (1_000_000.0, "1e+06"),
        (0.0001, "0.0001"),
        (0.000_01, "1e-05"),
        (123_456_789.0, "1.23457e+08"),
        (-2.5, "-2.5"),
        (f64::from(0.1f32), "0.1"),
        (f64::from(2.718_281_7_f32), "2.71828"),
        (0.0, "0"),
        (-0.0, "-0"),
        (1e100, "1e+100"),
        (f64::INFINITY, "inf"),
        (f64::NEG_INFINITY, "-inf"),
        (f64::NAN, "nan"),
    ] {
        assert_eq!(python_g(v), want, "{v}");
    }
    for (s, want) in [
        ("abc", "'abc'"),
        ("a'b", "\"a'b\""),
        ("a\"b", "'a\"b'"),
        ("a'\"b", "'a\\'\"b'"),
        ("\n\t\r\\", "'\\n\\t\\r\\\\'"),
        ("\u{1}\u{7f}\u{a0}\u{ad}", "'\\x01\\x7f\\xa0\\xad'"),
        ("\u{e9}\u{a1}", "'\u{e9}\u{a1}'"),
    ] {
        assert_eq!(python_repr(s), want, "{s:?}");
    }
}
