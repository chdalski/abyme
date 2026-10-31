//! The operations a request carries, as data.
//!
//! An AI sends them over MCP and the canvas sends them over IPC, so they are
//! untrusted input: every operation is read only from a JSON object, unknown
//! fields are rejected, and references and keys pass the same checks as
//! everything else.

use std::fmt;

use abyme_model::{BoardId, ElementId, ObjectOnly, RelationKind, Zone, shown};
use serde::{Deserialize, Serialize};

/// The longest local key, in characters.
pub const MAX_KEY_LEN: usize = 64;

/// The largest distance of a moved element's position from the origin,
/// in each direction: a position near the `i32` limit would leave later
/// placements on that board no room below it.
pub const MAX_COORDINATE: i32 = 1_000_000;

/// How many characters of a rejected key an error shows.
const SHOWN_LIMIT: usize = 80;

/// One change in a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum Operation {
    /// Creates an element. Its ID is derived from the name.
    CreateElement {
        /// The key by which later operations of the request refer to it.
        key: LocalKey,
        /// The semantic type, such as `c4/container`.
        type_ref: String,
        /// The name shown on the board.
        name: String,
        /// A one-line summary.
        #[serde(default)]
        summary: Option<String>,
        /// The description as Markdown.
        #[serde(default)]
        body: String,
        /// The path relative to the project root that the element stands
        /// for, such as `crates/payment-service`.
        #[serde(default)]
        anchor: Option<String>,
    },
    /// Creates a board. Its ID is derived from the title.
    CreateBoard {
        /// The key by which later operations of the request refer to it.
        key: LocalKey,
        /// The title shown for the board.
        title: String,
        /// The element whose inside the board is; an element owns at most
        /// one board.
        #[serde(default)]
        owner: Option<Ref<ElementId>>,
        /// A one-line summary of what the board shows.
        #[serde(default)]
        summary: Option<String>,
        /// The board's mode and the statement kind of its content.
        #[serde(default)]
        zone: Option<Zone>,
    },
    /// Changes the fields it names of a board; the others stay as they are.
    /// The board keeps its ID and its placements, also when its title
    /// changes.
    UpdateBoard {
        /// The board to change.
        board: Ref<BoardId>,
        /// The new title.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        title: Option<String>,
        /// The new one-line summary; `null` removes it.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "Option<String>"))]
        summary: Option<Option<String>>,
        /// The element whose inside the board becomes; `null` makes the
        /// board the inside of no element.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "Option<Ref<ElementId>>"))]
        owner: Option<Option<Ref<ElementId>>>,
        /// The board's new mode and statement kind; `null` makes the board
        /// no zone.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "Option<Zone>"))]
        zone: Option<Option<Zone>>,
    },
    /// Gives an element its home on a board.
    PlaceElement {
        /// The element to place.
        element: Ref<ElementId>,
        /// The board to place it on.
        board: Ref<BoardId>,
    },
    /// Moves an element on a board where it lies: sets the position of its
    /// placement there and keeps its size. A pinned element moves only when
    /// the operation says what becomes of the pin.
    MoveElement {
        /// The element to move.
        element: Ref<ElementId>,
        /// The board it lies on.
        board: Ref<BoardId>,
        /// The new left edge, from -1,000,000 to 1,000,000.
        #[cfg_attr(
            feature = "json-schema",
            schemars(range(min = -MAX_COORDINATE, max = MAX_COORDINATE))
        )]
        x: i32,
        /// The new top edge, from -1,000,000 to 1,000,000.
        #[cfg_attr(
            feature = "json-schema",
            schemars(range(min = -MAX_COORDINATE, max = MAX_COORDINATE))
        )]
        y: i32,
        /// `true` moves and pins the element, `false` moves and releases it.
        /// Without it, the pin stays as it is and a pinned element is refused.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "bool"))]
        pinned: Option<bool>,
    },
    /// Changes the fields it names of an element; the others stay as they
    /// are. The element keeps its ID, also when its name changes, and loses
    /// its verification stamp.
    UpdateElement {
        /// The element to change.
        element: Ref<ElementId>,
        /// The new name.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        name: Option<String>,
        /// The new semantic type, such as `c4/container`.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        type_ref: Option<String>,
        /// The new one-line summary; `null` removes it.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "Option<String>"))]
        summary: Option<Option<String>>,
        /// The new description as Markdown.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        body: Option<String>,
        /// The new anchor, a path relative to the project root; `null`
        /// removes it.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "json-schema", schemars(with = "Option<String>"))]
        anchor: Option<Option<String>>,
    },
    /// Deletes an element: its file, its placement on every board, and its
    /// own relations. It is rejected while other elements still relate to it.
    DeleteElement {
        /// The element to delete; also an ID that a board places although
        /// its file is missing.
        element: Ref<ElementId>,
    },
    /// Adds a relation from one element to another; the element it goes out
    /// from loses its verification stamp.
    CreateRelation {
        /// The element the relation goes out from.
        from: Ref<ElementId>,
        /// What the relation says.
        kind: RelationKind,
        /// The element the relation points to.
        to: Ref<ElementId>,
        /// A short label, such as `REST`.
        #[serde(default)]
        label: Option<String>,
    },
    /// Removes a relation from the element it goes out from, which loses its
    /// verification stamp.
    DeleteRelation {
        /// The element the relation goes out from.
        from: Ref<ElementId>,
        /// The relation's kind.
        kind: RelationKind,
        /// The element the relation points to, also one whose file is gone.
        to: Ref<ElementId>,
    },
}

/// A reference to an element or a board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum Ref<T> {
    /// One that an earlier operation of the same request creates.
    New(LocalKey),
    /// One that exists already, by its ID.
    Existing(T),
}

/// The operations of one request, each read only from a JSON object.
///
/// Read requests through this type, never as bare [`Operation`]s: the
/// derived reader of an internally tagged enum also accepts the array form.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "json-schema",
    schemars(
        description = "The operations of one request, applied in order: all of them or none."
    )
)]
pub struct Operations(Vec<ObjectOnly<Operation>>);

/// A name for something a request creates, valid only within that request:
/// 1 to [`MAX_KEY_LEN`] ASCII letters, digits, hyphens, and underscores.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LocalKey(String);

/// A value that is not a valid local key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "{shown} is not a valid key: use 1 to {MAX_KEY_LEN} ASCII letters, digits, hyphens, and underscores"
)]
pub struct LocalKeyError {
    shown: String,
}

/// Reads a field that is present: `None` stands only for a field the
/// request leaves out. So `null` is rejected where the field's own type
/// rejects it, and a summary or an anchor of `null` reads as `Some(None)`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

impl Operations {
    /// The operations, in order.
    #[must_use]
    pub fn into_vec(self) -> Vec<Operation> {
        self.0.into_iter().map(ObjectOnly::into_inner).collect()
    }
}

impl LocalKey {
    /// The key as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for LocalKey {
    type Error = LocalKeyError;

    fn try_from(value: String) -> Result<Self, LocalKeyError> {
        let allowed = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_';
        if (1..=MAX_KEY_LEN).contains(&value.len()) && value.bytes().all(allowed) {
            Ok(Self(value))
        } else {
            Err(LocalKeyError {
                shown: shown(&value, SHOWN_LIMIT),
            })
        }
    }
}

impl From<LocalKey> for String {
    fn from(key: LocalKey) -> Self {
        key.0
    }
}

impl fmt::Display for LocalKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(feature = "json-schema")]
impl schemars::JsonSchema for LocalKey {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("local_key")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^[A-Za-z0-9_-]+$",
            "minLength": 1,
            "maxLength": MAX_KEY_LEN,
            "description": "A name for something this request creates, valid only within the request",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use abyme_model::{Mode, StatementKind};
    use rstest::rstest;

    fn read(json: &str) -> Result<Vec<Operation>, serde_json::Error> {
        serde_json::from_str::<Operations>(json).map(Operations::into_vec)
    }

    #[test]
    fn reads_the_documented_shape() {
        let json = r#"[
            {"op": "create_board", "key": "b", "title": "Container Level"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payment Service"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "b"}},
            {"op": "place_element", "element": {"existing": "orders-db"}, "board": {"new": "b"}}
        ]"#;

        let operations = read(json).unwrap();

        assert_eq!(operations.len(), 4);
        assert_eq!(
            operations.get(1),
            Some(&Operation::CreateElement {
                key: LocalKey::try_from("pay".to_owned()).unwrap(),
                type_ref: "c4/container".to_owned(),
                name: "Payment Service".to_owned(),
                summary: None,
                body: String::new(),
                anchor: None,
            })
        );
    }

    #[test]
    fn reads_a_created_element_with_or_without_an_anchor() {
        let cases = [
            ("", None),
            (r#", "anchor": null"#, None),
            (r#", "anchor": "crates/pay""#, Some("crates/pay")),
        ];

        for (field, expected) in cases {
            let json = format!(
                r#"[{{"op": "create_element", "key": "k", "type_ref": "t", "name": "n"{field}}}]"#
            );
            let operations = read(&json).unwrap();

            match operations.as_slice() {
                [Operation::CreateElement { anchor, .. }] => {
                    assert_eq!(anchor.as_deref(), expected, "{field}");
                }
                other => panic!("expected one creation, got {other:?}"),
            }
        }
    }

    #[test]
    fn reads_a_move() {
        let json = r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"new": "b"}, "x": -40, "y": 1000000}]"#;

        let operations = read(json).unwrap();

        assert_eq!(
            operations,
            vec![Operation::MoveElement {
                element: Ref::Existing("orders-db".parse().unwrap()),
                board: Ref::New(LocalKey::try_from("b".to_owned()).unwrap()),
                x: -40,
                y: 1_000_000,
                pinned: None,
            }]
        );
    }

    #[test]
    fn reads_relation_operations() {
        let json = r#"[
            {"op": "create_relation", "from": {"new": "pay"}, "kind": "depends_on", "to": {"existing": "orders-db"}, "label": "SQL"},
            {"op": "create_relation", "from": {"new": "pay"}, "kind": "uses", "to": {"existing": "orders-db"}},
            {"op": "delete_relation", "from": {"existing": "web"}, "kind": "see_also", "to": {"existing": "gone"}}
        ]"#;

        let operations = read(json).unwrap();

        let pay = || Ref::New(LocalKey::try_from("pay".to_owned()).unwrap());
        let existing = |id: &str| Ref::Existing(id.parse().unwrap());
        assert_eq!(
            operations,
            vec![
                Operation::CreateRelation {
                    from: pay(),
                    kind: RelationKind::DependsOn,
                    to: existing("orders-db"),
                    label: Some("SQL".to_owned()),
                },
                Operation::CreateRelation {
                    from: pay(),
                    kind: RelationKind::Uses,
                    to: existing("orders-db"),
                    label: None,
                },
                Operation::DeleteRelation {
                    from: existing("web"),
                    kind: RelationKind::SeeAlso,
                    to: existing("gone"),
                },
            ]
        );
    }

    #[test]
    fn reads_an_update_that_keeps_removes_or_replaces_the_summary() {
        let cases = [
            (r#""name": "N""#, None),
            (r#""summary": null"#, Some(None)),
            (r#""summary": "S""#, Some(Some("S"))),
        ];

        for (fields, expected) in cases {
            let json = format!(
                r#"[{{"op": "update_element", "element": {{"existing": "a"}}, {fields}}}]"#
            );
            let operations = read(&json).unwrap();

            match operations.as_slice() {
                [Operation::UpdateElement { summary, .. }] => {
                    assert_eq!(summary.as_ref().map(Option::as_deref), expected, "{fields}");
                }
                other => panic!("expected one update, got {other:?}"),
            }
        }
    }

    #[test]
    fn reads_an_update_that_keeps_removes_or_replaces_the_anchor() {
        let cases = [
            (r#""name": "N""#, None),
            (r#""anchor": null"#, Some(None)),
            (r#""anchor": "crates/pay""#, Some(Some("crates/pay"))),
        ];

        for (fields, expected) in cases {
            let json = format!(
                r#"[{{"op": "update_element", "element": {{"existing": "a"}}, {fields}}}]"#
            );
            let operations = read(&json).unwrap();

            match operations.as_slice() {
                [Operation::UpdateElement { anchor, .. }] => {
                    assert_eq!(anchor.as_ref().map(Option::as_deref), expected, "{fields}");
                }
                other => panic!("expected one update, got {other:?}"),
            }
        }
    }

    #[test]
    fn reads_a_deletion() {
        let json = r#"[{"op": "delete_element", "element": {"existing": "orders-db"}}]"#;

        assert_eq!(
            read(json).unwrap(),
            vec![Operation::DeleteElement {
                element: Ref::Existing("orders-db".parse().unwrap()),
            }]
        );
    }

    #[test]
    fn reads_an_update_of_every_field() {
        let json = r#"[{"op": "update_element", "element": {"new": "pay"}, "name": "Payments",
            "type_ref": "c4/component", "summary": "S", "body": "B", "anchor": "crates/pay"}]"#;

        assert_eq!(
            read(json).unwrap(),
            vec![Operation::UpdateElement {
                element: Ref::New(LocalKey::try_from("pay".to_owned()).unwrap()),
                name: Some("Payments".to_owned()),
                type_ref: Some("c4/component".to_owned()),
                summary: Some(Some("S".to_owned())),
                body: Some("B".to_owned()),
                anchor: Some(Some("crates/pay".to_owned())),
            }]
        );
    }

    #[test]
    fn reads_a_board_with_an_owner_and_a_summary() {
        let json = r#"[{"op": "create_board", "key": "b", "title": "Inside",
            "owner": {"existing": "shop"}, "summary": "S"}]"#;

        assert_eq!(
            read(json).unwrap(),
            vec![Operation::CreateBoard {
                key: LocalKey::try_from("b".to_owned()).unwrap(),
                title: "Inside".to_owned(),
                owner: Some(Ref::Existing("shop".parse().unwrap())),
                summary: Some("S".to_owned()),
                zone: None,
            }]
        );
    }

    /// An update of the board `main` with the given fields.
    fn board_update(json_fields: &str) -> String {
        format!(r#"[{{"op": "update_board", "board": {{"existing": "main"}}{json_fields}}}]"#)
    }

    const MIRRORED_CURRENT: Zone = Zone {
        mode: Mode::Mirrored,
        statement: StatementKind::Current,
    };

    #[rstest]
    #[case::with_a_zone(
        r#", "zone": {"mode": "mirrored", "statement": "current"}"#,
        Some(MIRRORED_CURRENT)
    )]
    #[case::without_a_zone("", None)]
    #[case::with_a_null_zone(r#", "zone": null"#, None)]
    fn reads_a_board_with_and_without_a_zone(
        #[case] zone_field: &str,
        #[case] expected: Option<Zone>,
    ) {
        let json = format!(r#"[{{"op": "create_board", "key": "b", "title": "T"{zone_field}}}]"#);

        assert_eq!(
            read(&json).unwrap(),
            vec![Operation::CreateBoard {
                key: LocalKey::try_from("b".to_owned()).unwrap(),
                title: "T".to_owned(),
                owner: None,
                summary: None,
                zone: expected,
            }]
        );
    }

    #[test]
    fn reads_an_update_of_every_field_of_a_board() {
        let json = board_update(
            r#", "title": "T", "summary": "S", "owner": {"new": "e"},
            "zone": {"mode": "mirrored", "statement": "current"}"#,
        );

        assert_eq!(
            read(&json).unwrap(),
            vec![Operation::UpdateBoard {
                board: Ref::Existing("main".parse().unwrap()),
                title: Some("T".to_owned()),
                summary: Some(Some("S".to_owned())),
                owner: Some(Some(Ref::New(LocalKey::try_from("e".to_owned()).unwrap()))),
                zone: Some(Some(MIRRORED_CURRENT)),
            }]
        );
    }

    #[test]
    fn reads_null_as_removing_a_board_s_summary_owner_and_zone() {
        let json = board_update(r#", "summary": null, "owner": null, "zone": null"#);

        assert_eq!(
            read(&json).unwrap(),
            vec![Operation::UpdateBoard {
                board: Ref::Existing("main".parse().unwrap()),
                title: None,
                summary: Some(None),
                owner: Some(None),
                zone: Some(None),
            }]
        );
    }

    #[test]
    fn reads_an_update_of_a_board_that_names_no_field() {
        assert_eq!(
            read(&board_update("")).unwrap(),
            vec![Operation::UpdateBoard {
                board: Ref::Existing("main".parse().unwrap()),
                title: None,
                summary: None,
                owner: None,
                zone: None,
            }]
        );
    }

    #[rstest]
    #[case::synced(r#"{"mode": "synced", "statement": "current"}"#)]
    #[case::locked(r#"{"mode": "locked", "statement": "current"}"#)]
    #[case::historic(r#"{"mode": "free", "statement": "historic"}"#)]
    #[case::capitals(r#"{"mode": "free", "statement": "Current"}"#)]
    #[case::mode_as_an_object(r#"{"mode": {"free": null}, "statement": "current"}"#)]
    #[case::mode_as_an_object_with_content(r#"{"mode": {"free": {}}, "statement": "current"}"#)]
    #[case::kind_as_an_object(r#"{"mode": "free", "statement": {"idea": null}}"#)]
    #[case::null_mode(r#"{"mode": null, "statement": "current"}"#)]
    #[case::no_mode(r#"{"statement": "current"}"#)]
    #[case::no_kind(r#"{"mode": "free"}"#)]
    #[case::empty("{}")]
    #[case::unknown_field(r#"{"mode": "free", "statement": "current", "preset": "current"}"#)]
    #[case::mode_twice(r#"{"mode": "free", "mode": "mirrored", "statement": "idea"}"#)]
    #[case::preset_name(r#""current""#)]
    #[case::as_an_array(r#"["free", "current"]"#)]
    #[case::as_a_boolean("true")]
    fn rejects_any_other_zone_when_creating_or_updating_a_board(#[case] zone: &str) {
        let create =
            format!(r#"[{{"op": "create_board", "key": "b", "title": "T", "zone": {zone}}}]"#);
        let update = board_update(&format!(r#", "zone": {zone}"#));

        assert!(read(&create).is_err(), "{create}");
        assert!(read(&update).is_err(), "{update}");
    }

    #[rstest]
    #[case::summary_null(
        r#"[{"op": "create_element", "key": "k", "type_ref": "t", "name": "n", "summary": null}]"#
    )]
    #[case::op_last(r#"[{"key": "k", "title": "t", "op": "create_board"}]"#)]
    fn accepts_variants_of_the_shape(#[case] json: &str) {
        assert!(read(json).is_ok());
    }

    #[rstest]
    #[case::create_board_as_array(r#"[["create_board", "k", "t"]]"#)]
    #[case::create_element_as_array(r#"[["create_element", "k", "t", "n", null, ""]]"#)]
    #[case::place_element_as_array(r#"[["place_element", {"new": "a"}, {"new": "b"}]]"#)]
    #[case::pinned_null(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 0, "y": 0, "pinned": null}]"#
    )]
    #[case::pinned_string(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 0, "y": 0, "pinned": "true"}]"#
    )]
    #[case::pinned_number(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 0, "y": 0, "pinned": 1}]"#
    )]
    #[case::pinned_twice(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 0, "y": 0, "pinned": true, "pinned": false}]"#
    )]
    #[case::move_element_as_array(r#"[["move_element", {"new": "a"}, {"new": "b"}, 0, 0]]"#)]
    #[case::move_x_as_fraction(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 1.5, "y": 0}]"#
    )]
    #[case::move_x_as_exponent(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 1e3, "y": 0}]"#
    )]
    #[case::move_x_as_text(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": "1", "y": 0}]"#
    )]
    #[case::move_x_null(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": null, "y": 0}]"#
    )]
    #[case::move_without_x(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "y": 0}]"#
    )]
    #[case::move_x_twice(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 1, "x": 2, "y": 0}]"#
    )]
    #[case::move_x_beyond_i32(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 2147483648, "y": 0}]"#
    )]
    #[case::move_y_beyond_i32(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 0, "y": -2147483649}]"#
    )]
    #[case::move_with_a_size(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 0, "y": 0, "width": 9}]"#
    )]
    #[case::unknown_relation_kind(
        r#"[{"op": "create_relation", "from": {"new": "a"}, "kind": "contains", "to": {"new": "b"}}]"#
    )]
    #[case::capitalized_relation_kind(
        r#"[{"op": "create_relation", "from": {"new": "a"}, "kind": "Uses", "to": {"new": "b"}}]"#
    )]
    #[case::relation_without_a_kind(
        r#"[{"op": "create_relation", "from": {"new": "a"}, "to": {"new": "b"}}]"#
    )]
    #[case::deletion_with_a_label(
        r#"[{"op": "delete_relation", "from": {"new": "a"}, "kind": "uses", "to": {"new": "b"}, "label": "l"}]"#
    )]
    #[case::relation_label_as_number(
        r#"[{"op": "create_relation", "from": {"new": "a"}, "kind": "uses", "to": {"new": "b"}, "label": 1}]"#
    )]
    #[case::update_name_null(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "name": null}]"#
    )]
    #[case::update_type_null(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "type_ref": null}]"#
    )]
    #[case::update_body_null(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "body": null}]"#
    )]
    #[case::update_summary_as_number(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "summary": 1}]"#
    )]
    #[case::update_summary_as_object(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "summary": {}}]"#
    )]
    #[case::update_summary_as_array(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "summary": ["s"]}]"#
    )]
    #[case::update_summary_twice(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "summary": null, "summary": "x"}]"#
    )]
    #[case::update_anchor_as_number(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "anchor": 5}]"#
    )]
    #[case::update_anchor_as_object(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "anchor": {}}]"#
    )]
    #[case::update_anchor_as_array(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "anchor": []}]"#
    )]
    #[case::create_anchor_as_number(
        r#"[{"op": "create_element", "key": "k", "type_ref": "t", "name": "n", "anchor": 5}]"#
    )]
    #[case::create_with_a_commit(
        r#"[{"op": "create_element", "key": "k", "type_ref": "t", "name": "n", "verified": "0000000000000000000000000000000000000000"}]"#
    )]
    #[case::create_with_a_verifier(
        r#"[{"op": "create_element", "key": "k", "type_ref": "t", "name": "n", "verified_by": "person"}]"#
    )]
    #[case::update_with_a_commit(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "verified": "0000000000000000000000000000000000000000"}]"#
    )]
    #[case::update_with_a_verifier(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "verified_by": "person"}]"#
    )]
    #[case::move_with_an_anchor(
        r#"[{"op": "move_element", "element": {"new": "a"}, "board": {"new": "b"}, "x": 0, "y": 0, "anchor": "x"}]"#
    )]
    #[case::update_name_twice(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "name": "x", "name": "y"}]"#
    )]
    #[case::update_of_relations(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "relations": []}]"#
    )]
    #[case::update_of_the_id(
        r#"[{"op": "update_element", "element": {"existing": "a"}, "id": "b"}]"#
    )]
    #[case::update_without_an_element(r#"[{"op": "update_element", "name": "x"}]"#)]
    #[case::update_as_array(r#"[["update_element", {"existing": "a"}, "x"]]"#)]
    #[case::deletion_with_a_cascade(
        r#"[{"op": "delete_element", "element": {"existing": "a"}, "cascade": true}]"#
    )]
    #[case::deletion_without_an_element(r#"[{"op": "delete_element"}]"#)]
    #[case::deletion_of_an_invalid_id(
        r#"[{"op": "delete_element", "element": {"existing": "../x"}}]"#
    )]
    #[case::update_of_an_invalid_id(
        r#"[{"op": "update_element", "element": {"existing": "../x"}, "name": "x"}]"#
    )]
    #[case::unknown_field(
        r#"[{"op": "create_board", "key": "k", "title": "t", "unexpected": {}}]"#
    )]
    #[case::duplicate_field(r#"[{"op": "create_board", "key": "k", "key": "j", "title": "t"}]"#)]
    #[case::duplicate_op(
        r#"[{"op": "create_board", "op": "create_board", "key": "k", "title": "t"}]"#
    )]
    #[case::missing_op(r#"[{"key": "k", "title": "t"}]"#)]
    #[case::op_as_number(r#"[{"op": 1, "key": "k", "title": "t"}]"#)]
    #[case::unknown_op(r#"[{"op": "delete_board", "key": "k"}]"#)]
    #[case::board_update_with_a_null_title(
        r#"[{"op": "update_board", "board": {"existing": "main"}, "title": null}]"#
    )]
    #[case::board_update_with_an_unknown_field(
        r#"[{"op": "update_board", "board": {"existing": "main"}, "unexpected": {}}]"#
    )]
    #[case::board_update_of_the_id(
        r#"[{"op": "update_board", "board": {"existing": "main"}, "id": "other"}]"#
    )]
    #[case::board_update_of_placements(
        r#"[{"op": "update_board", "board": {"existing": "main"}, "placements": []}]"#
    )]
    #[case::board_update_without_a_board(r#"[{"op": "update_board", "title": "T"}]"#)]
    #[case::board_update_as_array(r#"[["update_board", {"existing": "main"}, "T"]]"#)]
    #[case::board_owner_of_an_invalid_id(
        r#"[{"op": "update_board", "board": {"existing": "main"}, "owner": {"existing": "../x"}}]"#
    )]
    #[case::new_board_owner_of_an_invalid_id(
        r#"[{"op": "create_board", "key": "k", "title": "t", "owner": {"existing": "../x"}}]"#
    )]
    #[case::board_summary_as_number(
        r#"[{"op": "create_board", "key": "k", "title": "t", "summary": 1}]"#
    )]
    #[case::op_in_other_case(r#"[{"op": "CreateBoard", "key": "k", "title": "t"}]"#)]
    #[case::body_null(
        r#"[{"op": "create_element", "key": "k", "type_ref": "t", "name": "n", "body": null}]"#
    )]
    #[case::ref_with_two_keys(
        r#"[{"op": "place_element", "element": {"new": "a", "existing": "b"}, "board": {"new": "c"}}]"#
    )]
    #[case::ref_as_string(r#"[{"op": "place_element", "element": "a", "board": {"new": "c"}}]"#)]
    #[case::ref_as_array(
        r#"[{"op": "place_element", "element": ["new", "a"], "board": {"new": "c"}}]"#
    )]
    #[case::existing_path_traversal(
        r#"[{"op": "place_element", "element": {"existing": "../x"}, "board": {"new": "c"}}]"#
    )]
    #[case::existing_uppercase(
        r#"[{"op": "place_element", "element": {"existing": "A"}, "board": {"new": "c"}}]"#
    )]
    #[case::existing_sixty_five_characters(&format!(
        r#"[{{"op": "place_element", "element": {{"existing": "{}"}}, "board": {{"new": "c"}}}}]"#,
        "a".repeat(65)
    ))]
    fn rejects_anything_but_the_documented_shape(#[case] json: &str) {
        assert!(read(json).is_err(), "{json}");
    }

    #[rstest]
    #[case::empty("")]
    #[case::sixty_five_characters(&"k".repeat(65))]
    #[case::line_break("a\nb")]
    #[case::terminal_escape("\u{1b}[31m")]
    #[case::right_to_left_override("\u{202e}")]
    #[case::space("a b")]
    #[case::dot("a.b")]
    fn rejects_invalid_keys(#[case] value: &str) {
        let error = LocalKey::try_from(value.to_owned()).unwrap_err();

        let message = error.to_string();
        assert!(!message.chars().any(char::is_control), "{message:?}");
        assert!(!message.contains('\u{202e}'), "{message:?}");
    }

    #[rstest]
    #[case::letters_and_digits("pay2")]
    #[case::hyphen_and_underscore("a-b_C")]
    #[case::sixty_four_characters(&"k".repeat(64))]
    fn accepts_valid_keys(#[case] value: &str) {
        assert_eq!(
            LocalKey::try_from(value.to_owned()).unwrap().as_str(),
            value
        );
    }

    #[test]
    fn error_cuts_a_long_rejected_reference() {
        let json = format!(
            r#"[{{"op": "place_element", "element": {{"existing": "{}\u0007"}}, "board": {{"new": "c"}}}}]"#,
            "x".repeat(10_000)
        );

        let message = read(&json).unwrap_err().to_string();

        assert!(message.len() < 500, "{} bytes", message.len());
        assert!(!message.chars().any(char::is_control), "{message:?}");
    }

    #[cfg(feature = "json-schema")]
    #[test]
    fn schema_forbids_unknown_fields_in_every_operation_and_reference() {
        let schema = serde_json::to_value(schemars::schema_for!(Operations)).unwrap();

        let definitions = schema
            .pointer("/$defs")
            .and_then(serde_json::Value::as_object)
            .unwrap();
        // The relation kinds are variants too, but strings, not objects.
        let variants: Vec<&serde_json::Value> = definitions
            .values()
            .filter_map(|definition| definition.pointer("/oneOf")?.as_array())
            .flatten()
            .filter(|variant| variant.pointer("/type") == Some(&serde_json::json!("object")))
            .collect();
        assert_eq!(variants.len(), 9 + 2 + 2, "{schema}");
        for variant in variants {
            assert_eq!(
                variant.pointer("/additionalProperties"),
                Some(&serde_json::Value::Bool(false)),
                "{variant}"
            );
        }
    }

    #[cfg(feature = "json-schema")]
    #[test]
    fn schema_lets_only_the_summary_and_the_anchor_of_an_update_be_null() {
        let schema = serde_json::to_value(schemars::schema_for!(Operations)).unwrap();

        let update = schema
            .pointer("/$defs")
            .and_then(serde_json::Value::as_object)
            .unwrap()
            .values()
            .filter_map(|definition| definition.pointer("/oneOf")?.as_array())
            .flatten()
            .find(|variant| {
                variant.pointer("/properties/op/const")
                    == Some(&serde_json::json!("update_element"))
            })
            .unwrap();
        assert_eq!(
            update.pointer("/required"),
            Some(&serde_json::json!(["op", "element"])),
            "{update}"
        );
        for field in ["name", "type_ref", "body"] {
            assert_eq!(
                update.pointer(&format!("/properties/{field}/type")),
                Some(&serde_json::json!("string")),
                "{update}"
            );
        }
        for field in ["summary", "anchor"] {
            assert_eq!(
                update.pointer(&format!("/properties/{field}/type")),
                Some(&serde_json::json!(["string", "null"])),
                "{update}"
            );
        }
    }

    #[cfg(feature = "json-schema")]
    #[test]
    fn schema_lets_only_the_summary_the_owner_and_the_zone_of_a_board_update_be_null() {
        let schema = serde_json::to_value(schemars::schema_for!(Operations)).unwrap();

        let update = schema
            .pointer("/$defs")
            .and_then(serde_json::Value::as_object)
            .unwrap()
            .values()
            .filter_map(|definition| definition.pointer("/oneOf")?.as_array())
            .flatten()
            .find(|variant| {
                variant.pointer("/properties/op/const") == Some(&serde_json::json!("update_board"))
            })
            .unwrap();
        assert_eq!(
            update.pointer("/required"),
            Some(&serde_json::json!(["op", "board"])),
            "{update}"
        );
        assert_eq!(
            update.pointer("/properties/title/type"),
            Some(&serde_json::json!("string")),
            "{update}"
        );
        assert_eq!(
            update.pointer("/properties/summary/type"),
            Some(&serde_json::json!(["string", "null"])),
            "{update}"
        );
        for field in ["owner", "zone"] {
            let schema = update
                .pointer(&format!("/properties/{field}"))
                .unwrap()
                .to_string();
            assert!(schema.contains("null"), "{update}");
        }
    }

    #[cfg(feature = "json-schema")]
    #[test]
    fn schema_names_exactly_the_modes_and_kinds_a_zone_takes() {
        let schema = serde_json::to_value(schemars::schema_for!(Operations)).unwrap();
        let definitions = schema.pointer("/$defs").unwrap();
        let names = |definition: &str| -> Vec<String> {
            let text = definitions
                .pointer(&format!("/{definition}"))
                .unwrap()
                .to_string();
            [
                "free",
                "mirrored",
                "synced",
                "locked",
                "current",
                "decision",
                "idea",
                "knowledge",
                "question",
                "target",
                "historic",
            ]
            .into_iter()
            .filter(|name| text.contains(&format!("\"{name}\"")))
            .map(str::to_owned)
            .collect()
        };

        let zone = definitions.pointer("/Zone").unwrap();
        assert_eq!(
            zone.pointer("/required"),
            Some(&serde_json::json!(["mode", "statement"])),
            "{zone}"
        );
        assert_eq!(
            zone.pointer("/additionalProperties"),
            Some(&serde_json::Value::Bool(false)),
            "{zone}"
        );
        assert_eq!(names("Mode"), ["free", "mirrored"]);
        assert_eq!(
            names("StatementKind"),
            [
                "current",
                "decision",
                "idea",
                "knowledge",
                "question",
                "target"
            ]
        );
    }

    #[cfg(feature = "json-schema")]
    #[test]
    fn schema_makes_the_pin_of_a_move_an_optional_boolean() {
        let schema = serde_json::to_value(schemars::schema_for!(Operations)).unwrap();

        let definitions = schema
            .pointer("/$defs")
            .and_then(serde_json::Value::as_object)
            .unwrap();
        let movement = definitions
            .values()
            .filter_map(|definition| definition.pointer("/oneOf")?.as_array())
            .flatten()
            .find(|variant| {
                variant.pointer("/properties/op/const") == Some(&serde_json::json!("move_element"))
            })
            .unwrap();
        assert_eq!(
            movement.pointer("/properties/pinned/type"),
            Some(&serde_json::json!("boolean")),
            "{movement}"
        );
        let required = movement
            .get("required")
            .and_then(serde_json::Value::as_array)
            .unwrap();
        assert!(
            !required.contains(&serde_json::json!("pinned")),
            "{movement}"
        );
        assert_eq!(
            movement.get("additionalProperties"),
            Some(&serde_json::json!(false)),
            "{movement}"
        );
    }

    #[cfg(feature = "json-schema")]
    #[test]
    fn schema_bounds_the_coordinates_of_a_move_as_the_core_does() {
        let schema = serde_json::to_value(schemars::schema_for!(Operations)).unwrap();

        let definitions = schema
            .pointer("/$defs")
            .and_then(serde_json::Value::as_object)
            .unwrap();
        let movement = definitions
            .values()
            .filter_map(|definition| definition.pointer("/oneOf")?.as_array())
            .flatten()
            .find(|variant| {
                variant.pointer("/properties/op/const") == Some(&serde_json::json!("move_element"))
            })
            .unwrap();
        for coordinate in ["x", "y"] {
            let property = movement
                .pointer(&format!("/properties/{coordinate}"))
                .unwrap();
            assert_eq!(
                property.get("minimum"),
                Some(&serde_json::json!(-MAX_COORDINATE)),
                "{property}"
            );
            assert_eq!(
                property.get("maximum"),
                Some(&serde_json::json!(MAX_COORDINATE)),
                "{property}"
            );
        }
    }
}
