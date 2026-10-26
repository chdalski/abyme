//! The operations a request carries, as data.
//!
//! An AI sends them over MCP and the canvas sends them over IPC, so they are
//! untrusted input: every operation is read only from a JSON object, unknown
//! fields are rejected, and references and keys pass the same checks as
//! everything else.

use std::fmt;

use abyme_model::{BoardId, ElementId, ObjectOnly, RelationKind, shown};
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
    },
    /// Creates a board. Its ID is derived from the title.
    CreateBoard {
        /// The key by which later operations of the request refer to it.
        key: LocalKey,
        /// The title shown for the board.
        title: String,
    },
    /// Gives an element its home on a board.
    PlaceElement {
        /// The element to place.
        element: Ref<ElementId>,
        /// The board to place it on.
        board: Ref<BoardId>,
    },
    /// Moves an element on a board where it lies: sets the position of its
    /// placement there and keeps its size.
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
    },
    /// Adds a relation from one element to another.
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
    /// Removes a relation from the element it goes out from.
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
            })
        );
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
    #[case::unknown_field(r#"[{"op": "create_board", "key": "k", "title": "t", "zone": {}}]"#)]
    #[case::duplicate_field(r#"[{"op": "create_board", "key": "k", "key": "j", "title": "t"}]"#)]
    #[case::duplicate_op(
        r#"[{"op": "create_board", "op": "create_board", "key": "k", "title": "t"}]"#
    )]
    #[case::missing_op(r#"[{"key": "k", "title": "t"}]"#)]
    #[case::op_as_number(r#"[{"op": 1, "key": "k", "title": "t"}]"#)]
    #[case::unknown_op(r#"[{"op": "delete_board", "key": "k"}]"#)]
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
        assert_eq!(variants.len(), 6 + 2 + 2, "{schema}");
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
