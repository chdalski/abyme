//! The board file: which elements lie where on a board.
//!
//! A board file is JSON with one placement per line, ordered by element ID,
//! so that moving an element changes exactly one line of the diff.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroU32;

use serde::de::value::MapAccessDeserializer;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::id::{BoardId, ElementId};
use crate::untrusted::shown;

/// The version of the board file format this crate reads and writes.
pub const BOARD_FORMAT: u32 = 1;

/// How many characters of a parser message an error shows.
const DETAIL_LIMIT: usize = 200;

/// A board: its title and where each element on it lies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    /// The board's ID, which is also its file name.
    pub id: BoardId,
    /// The title shown for the board.
    pub title: String,
    /// The rectangle of each element on the board. A map, so an element
    /// lies on a board at most once and the order is always by element ID.
    pub placements: BTreeMap<ElementId, Rect>,
}

/// Where an element lies on a board and how large it is.
///
/// Positions fit in `i32` and sizes in `u32`, so a position plus a size
/// cannot overflow `i64`, and every value survives a JavaScript number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// The left edge.
    pub x: i32,
    /// The top edge.
    pub y: i32,
    /// The width, never zero.
    pub width: NonZeroU32,
    /// The height, never zero.
    pub height: NonZeroU32,
}

/// A board file that cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BoardError {
    /// The text is not a board file: invalid JSON, a missing or unknown
    /// field, or a value of the wrong kind.
    #[error("board {board}: {detail}")]
    Invalid {
        /// The board the file name names.
        board: BoardId,
        /// What the parser found, escaped and shortened.
        detail: String,
    },
    /// The file has a format version this crate does not read.
    #[error("board {board}: format {found} is not supported, only {BOARD_FORMAT}")]
    UnsupportedFormat {
        /// The board the file name names.
        board: BoardId,
        /// The format version in the file.
        found: u32,
    },
    /// The file's `id` differs from its file name.
    #[error("board {board}: the file calls itself {found}")]
    IdMismatch {
        /// The board the file name names.
        board: BoardId,
        /// The ID inside the file.
        found: BoardId,
    },
    /// The file places one element twice.
    #[error("board {board}: element {element} is placed twice")]
    DuplicatePlacement {
        /// The board the file name names.
        board: BoardId,
        /// The element placed twice.
        element: ElementId,
    },
}

/// The board file as it lies on disk, before its rules are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoardFile {
    format: u32,
    id: BoardId,
    title: String,
    placements: Vec<ObjectOnly<PlacementLine>>,
}

/// One line of the `placements` list.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementLine {
    element: ElementId,
    x: i32,
    y: i32,
    width: NonZeroU32,
    height: NonZeroU32,
}

/// A `T` read only from a JSON object.
///
/// A derived struct also reads the array form (`[1, "main", …]`), so the
/// same file would be a board here and something else to every other JSON
/// reader. This wrapper keeps the derive's checks for unknown and duplicate
/// fields and rejects every other shape.
struct ObjectOnly<T>(T);

/// The visitor behind [`ObjectOnly`]; it accepts nothing but a map.
struct ObjectOnlyVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for ObjectOnly<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ObjectOnlyVisitor(PhantomData))
    }
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectOnlyVisitor<T> {
    type Value = ObjectOnly<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        T::deserialize(MapAccessDeserializer::new(map)).map(ObjectOnly)
    }
}

impl Board {
    /// Reads a board file. `expected` is the ID its file name gives, and
    /// the file's own `id` must match it. Placements may come in any
    /// order; the board keeps them ordered by element ID.
    ///
    /// # Errors
    ///
    /// Returns a [`BoardError`] naming `expected` when the text is not a
    /// board file of [`BOARD_FORMAT`], names another board, or places an
    /// element twice.
    pub fn from_json(expected: &BoardId, text: &str) -> Result<Self, BoardError> {
        let ObjectOnly(file) =
            serde_json::from_str::<ObjectOnly<BoardFile>>(text).map_err(|error| {
                BoardError::Invalid {
                    board: expected.clone(),
                    detail: shown(&error.to_string(), DETAIL_LIMIT),
                }
            })?;
        if file.format != BOARD_FORMAT {
            return Err(BoardError::UnsupportedFormat {
                board: expected.clone(),
                found: file.format,
            });
        }
        if file.id != *expected {
            return Err(BoardError::IdMismatch {
                board: expected.clone(),
                found: file.id,
            });
        }
        Ok(Self {
            placements: collect_placements(expected, file.placements)?,
            id: file.id,
            title: file.title,
        })
    }

    /// Writes the board file, one placement per line, ordered by element ID.
    #[must_use]
    pub fn to_json(&self) -> String {
        let lines: Vec<String> = self
            .placements
            .iter()
            .map(|(element, rect)| placement_line(element, rect))
            .collect();
        let placements = if lines.is_empty() {
            "[]".to_owned()
        } else {
            format!("[\n{}\n  ]", lines.join(",\n"))
        };
        format!(
            "{{\n  \"format\": {BOARD_FORMAT},\n  \"id\": {},\n  \"title\": {},\n  \"placements\": {placements}\n}}\n",
            json_string(self.id.as_str()),
            json_string(&self.title),
        )
    }
}

/// Collects the placement lines into a map and rejects an element placed
/// twice; the map keeps the check at O(n log n) for files of any size.
fn collect_placements(
    board: &BoardId,
    lines: Vec<ObjectOnly<PlacementLine>>,
) -> Result<BTreeMap<ElementId, Rect>, BoardError> {
    let mut placements = BTreeMap::new();
    for ObjectOnly(line) in lines {
        let rect = Rect {
            x: line.x,
            y: line.y,
            width: line.width,
            height: line.height,
        };
        match placements.entry(line.element) {
            Entry::Vacant(slot) => {
                slot.insert(rect);
            }
            Entry::Occupied(slot) => {
                return Err(BoardError::DuplicatePlacement {
                    board: board.clone(),
                    element: slot.key().clone(),
                });
            }
        }
    }
    Ok(placements)
}

/// One placement as a line of the file.
fn placement_line(element: &ElementId, rect: &Rect) -> String {
    format!(
        "    {{\"element\": {}, \"x\": {}, \"y\": {}, \"width\": {}, \"height\": {}}}",
        json_string(element.as_str()),
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    )
}

/// `text` as a JSON string literal. Every string goes through `serde_json`'s
/// escaping, so no value can end a line or a string early.
fn json_string(text: &str) -> String {
    Value::from(text).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn board_id(value: &str) -> BoardId {
        value.parse().unwrap()
    }

    fn element_id(value: &str) -> ElementId {
        value.parse().unwrap()
    }

    fn rect(x: i32, y: i32) -> Rect {
        Rect {
            x,
            y,
            width: NonZeroU32::new(180).unwrap(),
            height: NonZeroU32::new(90).unwrap(),
        }
    }

    fn sample_board() -> Board {
        Board {
            id: board_id("container-level"),
            title: "Container Level".to_owned(),
            placements: BTreeMap::from([
                (element_id("checkout-service"), rect(80, 200)),
                (element_id("payment-service"), rect(400, 200)),
            ]),
        }
    }

    /// A valid board file `main` with the placements given as JSON lines.
    fn main_file(placements: &str) -> String {
        format!(r#"{{"format": 1, "id": "main", "title": "t", "placements": [{placements}]}}"#)
    }

    #[test]
    fn writes_one_placement_per_line_in_the_documented_layout() {
        let expected = r#"{
  "format": 1,
  "id": "container-level",
  "title": "Container Level",
  "placements": [
    {"element": "checkout-service", "x": 80, "y": 200, "width": 180, "height": 90},
    {"element": "payment-service", "x": 400, "y": 200, "width": 180, "height": 90}
  ]
}
"#;

        assert_eq!(sample_board().to_json(), expected);
    }

    #[test]
    fn reads_back_what_it_writes() {
        let board = sample_board();

        let read = Board::from_json(&board.id, &board.to_json()).unwrap();

        assert_eq!(read, board);
    }

    #[test]
    fn moving_one_placement_changes_exactly_one_line() {
        let before = sample_board();
        let mut after = before.clone();
        after
            .placements
            .insert(element_id("payment-service"), rect(420, 240));

        let before_json = before.to_json();
        let after_json = after.to_json();
        let changed = before_json
            .lines()
            .zip(after_json.lines())
            .filter(|(old, new)| old != new)
            .count();

        assert_eq!(before_json.lines().count(), after_json.lines().count());
        assert_eq!(changed, 1);
    }

    #[test]
    fn writes_an_empty_board_with_an_empty_list() {
        let board = Board {
            id: board_id("empty"),
            title: String::new(),
            placements: BTreeMap::new(),
        };

        let json = board.to_json();

        assert!(json.contains("\"placements\": []"));
        assert_eq!(Board::from_json(&board.id, &json).unwrap(), board);
    }

    #[test]
    fn reads_unsorted_placements_and_writes_them_sorted() {
        let text = main_file(
            r#"{"element": "b", "x": 0, "y": 0, "width": 1, "height": 1},
               {"element": "a", "x": 5, "y": 5, "width": 1, "height": 1}"#,
        );

        let json = Board::from_json(&board_id("main"), &text)
            .unwrap()
            .to_json();

        let a = json.find("\"a\"").unwrap();
        let b = json.find("\"b\"").unwrap();
        assert!(a < b);
    }

    #[test]
    fn keeps_a_title_with_special_characters() {
        let board = Board {
            title: "line\nbreak \"quoted\" back\\slash \u{2028} \u{1f600} ]}".to_owned(),
            ..sample_board()
        };

        let json = board.to_json();

        assert_eq!(
            json.lines().count(),
            sample_board().to_json().lines().count()
        );
        assert_eq!(Board::from_json(&board.id, &json).unwrap(), board);
    }

    #[rstest]
    #[case::invalid_json("{")]
    #[case::duplicate_id_key(
        r#"{"format": 1, "id": "main", "id": "other", "title": "t", "placements": []}"#
    )]
    #[case::duplicate_key_in_placement(&main_file(
        r#"{"element": "a", "element": "b", "x": 0, "y": 0, "width": 1, "height": 1}"#
    ))]
    #[case::unknown_board_field(
        r#"{"format": 1, "id": "main", "title": "t", "placements": [], "zone": {}}"#
    )]
    #[case::unknown_placement_field(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": true}"#
    ))]
    #[case::two_documents(&format!("{}{}", main_file(""), main_file("")))]
    #[case::top_level_array("[]")]
    #[case::board_as_array(r#"[1, "main", "t", []]"#)]
    #[case::placement_as_array(&main_file(r#"["a", 0, 0, 1, 1]"#))]
    #[case::top_level_string(r#""main""#)]
    #[case::top_level_null("null")]
    #[case::missing_title(r#"{"format": 1, "id": "main", "placements": []}"#)]
    #[case::format_as_string(r#"{"format": "1", "id": "main", "title": "t", "placements": []}"#)]
    #[case::format_as_float(r#"{"format": 1.0, "id": "main", "title": "t", "placements": []}"#)]
    #[case::format_null(r#"{"format": null, "id": "main", "title": "t", "placements": []}"#)]
    #[case::format_negative(r#"{"format": -1, "id": "main", "title": "t", "placements": []}"#)]
    #[case::x_as_float(&main_file(r#"{"element": "a", "x": 1.5, "y": 0, "width": 1, "height": 1}"#))]
    #[case::x_as_exponent(&main_file(r#"{"element": "a", "x": 1e3, "y": 0, "width": 1, "height": 1}"#))]
    #[case::x_beyond_i32(&main_file(
        r#"{"element": "a", "x": 2147483648, "y": 0, "width": 1, "height": 1}"#
    ))]
    #[case::width_zero(&main_file(r#"{"element": "a", "x": 0, "y": 0, "width": 0, "height": 1}"#))]
    #[case::width_negative(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": -1, "height": 1}"#
    ))]
    #[case::element_path_traversal(&main_file(
        r#"{"element": "../evil", "x": 0, "y": 0, "width": 1, "height": 1}"#
    ))]
    #[case::invalid_board_id(r#"{"format": 1, "id": "Board", "title": "t", "placements": []}"#)]
    #[case::lone_surrogate_in_title(
        r#"{"format": 1, "id": "main", "title": "\ud800", "placements": []}"#
    )]
    #[case::leading_byte_order_mark(&format!("\u{feff}{}", main_file("")))]
    #[case::deep_nesting(&"[".repeat(10_000))]
    fn rejects_files_that_are_not_board_files(#[case] text: &str) {
        let error = Board::from_json(&board_id("main"), text).unwrap_err();

        assert!(matches!(error, BoardError::Invalid { .. }), "{error:?}");
        assert!(error.to_string().starts_with("board main: "), "{error}");
    }

    #[rstest]
    #[case::zero(0)]
    #[case::two(2)]
    fn rejects_unknown_format_versions(#[case] format: u32) {
        let text =
            format!(r#"{{"format": {format}, "id": "main", "title": "t", "placements": []}}"#);

        let error = Board::from_json(&board_id("main"), &text).unwrap_err();

        assert_eq!(
            error,
            BoardError::UnsupportedFormat {
                board: board_id("main"),
                found: format,
            }
        );
    }

    #[test]
    fn rejects_a_file_that_names_another_board() {
        let text = r#"{"format": 1, "id": "other", "title": "t", "placements": []}"#;

        let error = Board::from_json(&board_id("main"), text).unwrap_err();

        assert_eq!(
            error,
            BoardError::IdMismatch {
                board: board_id("main"),
                found: board_id("other"),
            }
        );
    }

    #[test]
    fn rejects_an_element_placed_twice() {
        let text = main_file(
            r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1},
               {"element": "a", "x": 9, "y": 9, "width": 1, "height": 1}"#,
        );

        let error = Board::from_json(&board_id("main"), &text).unwrap_err();

        assert_eq!(
            error,
            BoardError::DuplicatePlacement {
                board: board_id("main"),
                element: element_id("a"),
            }
        );
    }

    #[rstest]
    #[case::board_as_array(r#"[1, "main", "t", []]"#)]
    #[case::placement_as_array(&main_file(r#"["a", 0, 0, 1, 1]"#))]
    fn rejects_the_array_form_because_it_is_not_an_object(#[case] text: &str) {
        let error = Board::from_json(&board_id("main"), text).unwrap_err();

        assert!(
            error.to_string().contains("expected a JSON object"),
            "{error}"
        );
    }

    #[test]
    fn error_escapes_an_unknown_field_name() {
        let text = r#"{"format": 1, "id": "main", "title": "t", "placements": [], "a\nb": 1}"#;

        let error = Board::from_json(&board_id("main"), text).unwrap_err();

        assert!(!error.to_string().contains('\n'), "{error}");
    }
}
