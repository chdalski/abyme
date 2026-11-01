//! Tests of zones, modes, and statement kinds.

use rstest::rstest;
use serde::Deserialize;
use serde_json::json;

use super::*;

/// A request shape like an operation's: an internally tagged enum buffers
/// its fields before it reads them, which takes another way through serde
/// than a file does.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Tagged {
    Set { zone: Zone },
}

fn zone_in_request(zone: &serde_json::Value) -> Result<Zone, serde_json::Error> {
    let Tagged::Set { zone } = serde_json::from_value(json!({"op": "set", "zone": zone}))?;
    Ok(zone)
}

#[test]
fn reads_and_writes_every_mode_and_kind_by_its_name() {
    for mode in Mode::ALL {
        assert_eq!(
            serde_json::from_value::<Mode>(json!(mode.as_str())).ok(),
            Some(mode)
        );
        assert_eq!(serde_json::to_value(mode).ok(), Some(json!(mode.as_str())));
    }
    for kind in StatementKind::ALL {
        assert_eq!(
            serde_json::from_value::<StatementKind>(json!(kind.as_str())).ok(),
            Some(kind)
        );
        assert_eq!(serde_json::to_value(kind).ok(), Some(json!(kind.as_str())));
    }
}

#[test]
fn reads_a_zone_from_a_file_and_from_a_request() {
    let expected = Zone {
        mode: Mode::Mirrored,
        statement: StatementKind::Knowledge,
    };
    let zone = json!({"mode": "mirrored", "statement": "knowledge"});

    assert_eq!(
        serde_json::from_value::<Zone>(zone.clone()).ok(),
        Some(expected)
    );
    assert_eq!(zone_in_request(&zone).ok(), Some(expected));
}

#[rstest]
#[case::synced(json!({"mode": "synced", "statement": "current"}))]
#[case::locked(json!({"mode": "locked", "statement": "current"}))]
#[case::historic(json!({"mode": "free", "statement": "historic"}))]
#[case::capitals(json!({"mode": "free", "statement": "Current"}))]
#[case::trailing_space(json!({"mode": "free", "statement": "current "}))]
#[case::empty_name(json!({"mode": "", "statement": "current"}))]
#[case::mode_as_an_object(json!({"mode": {"free": null}, "statement": "current"}))]
#[case::mode_as_an_object_with_content(json!({"mode": {"free": {}}, "statement": "current"}))]
#[case::kind_as_an_object(json!({"mode": "free", "statement": {"idea": null}}))]
#[case::mode_as_a_number(json!({"mode": 0, "statement": "current"}))]
#[case::null_mode(json!({"mode": null, "statement": "current"}))]
#[case::no_mode(json!({"statement": "current"}))]
#[case::no_kind(json!({"mode": "free"}))]
#[case::empty(json!({}))]
#[case::unknown_field(json!({"mode": "free", "statement": "current", "preset": "current"}))]
#[case::preset_name(json!("current"))]
#[case::as_an_array(json!(["free", "current"]))]
#[case::as_a_boolean(json!(true))]
#[case::as_a_number(json!(1))]
fn rejects_any_other_zone_in_a_file_and_in_a_request(#[case] zone: serde_json::Value) {
    assert!(
        serde_json::from_value::<Zone>(zone.clone()).is_err(),
        "{zone}"
    );
    assert!(zone_in_request(&zone).is_err(), "{zone}");
}

#[test]
fn rejects_a_field_given_twice_in_a_zone() {
    let text = r#"{"mode": "free", "mode": "mirrored", "statement": "idea"}"#;

    assert!(serde_json::from_str::<Zone>(text).is_err());
}

#[test]
fn names_the_kinds_but_never_repeats_an_unknown_name() {
    let error = serde_json::from_value::<StatementKind>(json!("x\n\u{1b}[31m"))
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();

    assert!(
        error.contains("expected one of: current, decision, idea, knowledge, question, target"),
        "{error}"
    );
    assert!(
        !error.contains('\n') && !error.contains('\u{1b}'),
        "{error:?}"
    );
}

#[test]
fn a_statement_kind_is_found_by_exactly_its_name() {
    for kind in StatementKind::ALL {
        assert_eq!(StatementKind::from_name(kind.as_str()), Some(kind));
    }
}

#[rstest::rstest]
#[case::not_built_yet("historic")]
#[case::capitalized("Current")]
#[case::empty("")]
#[case::with_a_space("current ")]
fn the_file_and_a_request_refuse_the_same_names(#[case] name: &str) {
    assert_eq!(StatementKind::from_name(name), None);
    assert!(serde_json::from_value::<StatementKind>(serde_json::json!(name)).is_err());
}
