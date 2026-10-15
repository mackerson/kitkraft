//! Compiles the codegen goldens (`tests/golden/*.rs`) as a non-test target.
//!
//! Integration tests are built with `cfg(test)`, where `clippy.toml` relaxes `unwrap`,
//! `panic` and indexing. Generated code lands in a library crate (`crates/esm`), so it has
//! to pass the workspace lints without those allowances; `cargo clippy --all-targets`
//! checks this example under exactly those rules. Only `dead_code` is allowed, because a
//! binary does not use every generated item the way a library exports them.
//!
//! Run it to decode a small synthetic TES4 body: `cargo run -p kitkraft-schema --example
//! generated`.

#[allow(dead_code)]
mod fixture {
    include!("../tests/golden/fixture.rs");
}

#[allow(dead_code)]
mod seed {
    include!("../tests/golden/seed.rs");
}

fn main() {
    let hedr = [0u8; 12];
    let subs = [
        seed::RawSub {
            signature: *b"HEDR",
            data: &hedr,
            oversized: false,
        },
        seed::RawSub {
            signature: *b"CNAM",
            data: b"example\0",
            oversized: false,
        },
    ];
    let record = seed::Record::decode(*b"TES4", &subs, seed::Ctx::default());
    match record {
        Some(record) => {
            let same = record.encode().len() == subs.len();
            println!(
                "decoded {}; re-encodes to the same subrecords: {same}",
                String::from_utf8_lossy(&record.signature())
            );
        }
        None => println!("TES4 is not defined"),
    }
    let empty = fixture::Record::decode(*b"EMPT", &[], fixture::Ctx::default());
    println!("synthetic EMPT record: {}", empty.is_some());
}
