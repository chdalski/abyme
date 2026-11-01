//! The check of the UI's TypeScript types: `ui/src/bindings.ts` must hold
//! what ts-rs generates from the Rust types the UI receives. The test only
//! reads the file; when it fails, it prints the text the file must hold.

use std::fs;
use std::path::Path;

use abyme_model::{Mode, RelationKind};
use ts_rs::{Config, TS};

use crate::boards::BoardEntry;
use crate::projection::{BoardView, Placement, Relation, Shown};

/// Where the UI's generated types lie, relative to this crate.
const BINDINGS: &str = "ui/src/bindings.ts";

/// The first lines of the generated file.
const BINDINGS_HEADER: &str = "\
// Generated from the Rust types of abyme-canvas with ts-rs; do not edit. When the test
// `committed_bindings_match_the_rust_types` fails, replace this file with the text it expects.
";

/// The exported declaration of `T`.
fn exported<T: TS>(config: &Config) -> String {
    format!("export {}\n", T::decl(config))
}

/// The UI's types as the Rust types declare them.
fn bindings() -> String {
    let config = Config::new();
    [
        BINDINGS_HEADER.to_owned(),
        exported::<BoardEntry>(&config),
        exported::<BoardView>(&config),
        exported::<Mode>(&config),
        exported::<Placement>(&config),
        exported::<Relation>(&config),
        exported::<RelationKind>(&config),
        exported::<Shown>(&config),
    ]
    .join("\n")
}

#[test]
fn committed_bindings_match_the_rust_types() {
    let committed =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(BINDINGS)).unwrap();
    let expected = bindings();

    assert!(
        committed == expected,
        "{BINDINGS} is stale; the Rust types expect this text:\n{expected}"
    );
}

#[test]
fn ids_cross_as_strings() {
    let bindings = bindings();

    assert!(bindings.contains("id: string"));
    assert!(bindings.contains("element: string"));
}
