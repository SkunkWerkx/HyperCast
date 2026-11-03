//! src/fold_table.rs, the simple case folding the text doors use for `enum`, is generated
//! from unicode/CaseFolding-17.0.0.txt (the Unicode Character Database file, unmodified):
//! the rows with status `C` or `S`, in code point order. This regenerates it and fails when
//! the committed file differs. `HYPERCAST_BLESS=1` writes it.
//!
//! Compiled out on wasm, which has no checkout to read.
#![cfg(not(target_family = "wasm"))]

use std::fmt::Write;
use std::path::Path;

const SOURCE: &str = "unicode/CaseFolding-17.0.0.txt";
const TABLE: &str = "src/fold_table.rs";

fn generate(source: &str) -> String {
    let mut rows: Vec<(u32, u32)> = Vec::new();
    for line in source.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(';').map(str::trim).collect();
        assert_eq!(fields.len(), 4, "unexpected row: {line}");
        if fields[1] != "C" && fields[1] != "S" {
            continue;
        }
        let from = u32::from_str_radix(fields[0], 16).unwrap();
        let to = u32::from_str_radix(fields[2], 16).unwrap();
        assert!(!fields[2].contains(' '), "a C or S row maps to one code point: {line}");
        rows.push((from, to));
    }
    assert!(rows.windows(2).all(|w| w[0].0 < w[1].0), "rows must be strictly ascending");
    let mut out = String::new();
    out.push_str(
        "//! Simple case folding (Unicode 17.0.0, `CaseFolding.txt` statuses `C` and `S`), as\n\
         //! `(code point, folded code point)` rows in code point order, for the binary search\n\
         //! in `text::fold`. Turkic (`T`) and full (`F`) foldings are not here.\n\
         //!\n\
         //! Generated, not typed, from `rust/unicode/CaseFolding-17.0.0.txt`. Regenerate with\n\
         //! `HYPERCAST_BLESS=1 cargo test --test fold_table` in `rust/`; the test fails when\n\
         //! this file differs from what it generates.\n\n",
    );
    writeln!(out, "pub(crate) static FOLD: [(u32, u32); {}] = [", rows.len()).unwrap();
    for (from, to) in rows {
        writeln!(out, "    (0x{from:04X}, 0x{to:04X}),").unwrap();
    }
    out.push_str("];\n");
    out
}

#[test]
fn fold_table_is_current() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(crate_dir.join(SOURCE)).unwrap();
    let generated = generate(&source);
    let table = crate_dir.join(TABLE);
    if std::env::var_os("HYPERCAST_BLESS").is_some() {
        std::fs::write(&table, &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&table).unwrap_or_default();
    assert!(
        committed == generated,
        "{} is not what {SOURCE} generates; run `HYPERCAST_BLESS=1 cargo test --test fold_table` in rust/",
        table.display()
    );
}
