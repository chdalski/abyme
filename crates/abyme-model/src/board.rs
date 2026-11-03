//! The board file: which elements lie where on a board.
//!
//! A board file is JSON with one placement per line, ordered by element ID,
//! so that moving an element changes exactly one line of the diff.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::num::NonZeroU32;

use serde::Deserialize;
use serde_json::Value;

use crate::id::{BoardId, ElementId};
use crate::object_only::ObjectOnly;
use crate::untrusted::shown;
use crate::zone::Zone;

pub use placement::Placement;

/// The placement type, in a module of its own so that its derives may read
/// the fields that the rest of the workspace reads only through [`Board`].
mod placement {
    #![expect(
        clippy::disallowed_fields,
        reason = "the derives compare, copy, and print the fields as stored"
    )]

    use super::{ElementId, Rect};

    /// An element's place on a board.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Placement {
        /// The position and size the board file stores: a board position,
        /// or, while the attachment holds, the offset from the box it hangs
        /// on. Read where the element lies through
        /// [`Board::rect_of`](super::Board::rect_of) or
        /// [`Board::placed`](super::Board::placed), and change it through
        /// [`Board::set_rect`](super::Board::set_rect).
        pub rect: Rect,
        /// Whether a person placed it by hand: a move must then say what it
        /// wants with the pin, and a layout leaves it where it is.
        pub pinned: bool,
        /// The element the placement hangs on, as the file names it. Whether
        /// the attachment holds is for
        /// [`Board::attachment`](super::Board::attachment) to say.
        pub attached_to: Option<ElementId>,
    }
}

/// The version of the board file format this crate reads and writes.
pub const BOARD_FORMAT: u32 = 1;

/// The largest distance of a position from the origin, in each direction.
///
/// A move accepts no position beyond it and an attached element reaches none:
/// a position near the `i32` limit would leave later placements on that board
/// no room below it.
pub const MAX_COORDINATE: i32 = 1_000_000;

/// How many characters of a parser message an error shows.
const DETAIL_LIMIT: usize = 200;

/// A board: its title and summary, the element whose inside it is, its
/// zone, and where each element on it lies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    /// The board's ID, which is also its file name.
    pub id: BoardId,
    /// The title shown for the board.
    pub title: String,
    /// The element whose inside the board is, if it is an inner board.
    pub owner: Option<ElementId>,
    /// A short summary of what the board shows, for every output.
    pub summary: Option<String>,
    /// The board's mode and the statement kind of its content, if it is a
    /// zone.
    pub zone: Option<Zone>,
    /// The placement of each element on the board. A map, so an element
    /// lies on a board at most once and the order is always by element ID.
    pub placements: BTreeMap<ElementId, Placement>,
}

/// A position and a size on a board.
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

impl Rect {
    /// The right edge; `i32` plus `u32` cannot overflow `i64`.
    #[must_use]
    pub fn right(&self) -> i64 {
        i64::from(self.x) + i64::from(self.width.get())
    }

    /// The bottom edge; `i32` plus `u32` cannot overflow `i64`.
    #[must_use]
    pub fn bottom(&self) -> i64 {
        i64::from(self.y) + i64::from(self.height.get())
    }

    /// Whether the two rectangles share any area; touching edges share none.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        i64::from(self.x) < other.right()
            && i64::from(other.x) < self.right()
            && i64::from(self.y) < other.bottom()
            && i64::from(other.y) < self.bottom()
    }
}

/// Why a position cannot be stored on a board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PositionError {
    /// The board does not place the element.
    #[error("the board does not place the element")]
    NotPlaced,
    /// The offset from the box the element hangs on does not fit in `i32`.
    #[error("the offset from the box the element hangs on does not fit")]
    OffsetTooLarge,
    /// The attachment would not hold: the box hangs on another element, or
    /// the element would lie beyond [`MAX_COORDINATE`] from it.
    #[error("the attachment would not hold")]
    DoesNotHold,
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
    #[serde(default)]
    owner: Option<ElementId>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    zone: Option<Zone>,
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
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    attached_to: Option<ElementId>,
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
            owner: file.owner,
            summary: file.summary,
            zone: file.zone,
        })
    }

    /// Where `element` lies on this board, if the board places it. Every
    /// reader of a position asks the board, not the placement's stored rect:
    /// while the attachment holds, the stored position is an offset from the
    /// box.
    #[must_use]
    pub fn rect_of(&self, element: &ElementId) -> Option<Rect> {
        self.placements
            .get(element)
            .map(|placement| self.resolved(element, placement).0)
    }

    /// Every placement with where its element lies on this board: each
    /// exactly once, in the order of `placements`, which is by element ID.
    /// A layout relies on that order to write its rects back.
    pub fn placed(&self) -> impl Iterator<Item = (&ElementId, &Placement, Rect)> {
        self.placements
            .iter()
            .map(|(element, placement)| (element, placement, self.resolved(element, placement).0))
    }

    /// The element `element` hangs on, if its attachment holds: the file names
    /// an element that this board places, that is not `element` itself and
    /// hangs on nothing, and the position it gives lies within
    /// [`MAX_COORDINATE`] in both directions.
    #[must_use]
    pub fn attachment(&self, element: &ElementId) -> Option<&ElementId> {
        self.placements
            .get(element)
            .and_then(|placement| self.resolved(element, placement).1)
    }

    /// The element the file says `element` hangs on, whether the attachment
    /// holds or not.
    #[must_use]
    #[expect(
        clippy::disallowed_fields,
        reason = "the one reader of the field as written, for an output that tells whether it holds"
    )]
    pub fn named_attachment(&self, element: &ElementId) -> Option<&ElementId> {
        self.placements
            .get(element)
            .and_then(|placement| placement.attached_to.as_ref())
    }

    /// The box `element` names, if the board places it, it is another element,
    /// and it hangs on nothing. The attachment may still not hold, when the
    /// position it gives lies beyond [`MAX_COORDINATE`]: where the element
    /// lies, [`Board::rect_of`] says, and whether it hangs on the box,
    /// [`Board::attachment`].
    #[must_use]
    pub fn named_box(&self, element: &ElementId) -> Option<&ElementId> {
        let placement = self.placements.get(element)?;
        self.host(element, placement).map(|(named, _)| named)
    }

    /// The elements whose attachment to `element` holds, by ID.
    #[must_use]
    pub fn hanging_on(&self, element: &ElementId) -> Vec<ElementId> {
        self.placements
            .iter()
            .filter(|(id, placement)| self.resolved(id, placement).1 == Some(element))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Puts `element` at `rect`, a board position with its size. While the
    /// element hangs on a box that this board places, that is another element
    /// and hangs on nothing, the position is stored as the offset from that
    /// box, so it moves along with the box.
    ///
    /// # Errors
    ///
    /// Returns [`PositionError::NotPlaced`] when the board does not place the
    /// element, and [`PositionError::OffsetTooLarge`] when the offset does not
    /// fit in `i32`; then nothing changes.
    #[expect(
        clippy::disallowed_fields,
        reason = "the one writer of a position, which stores it relative to the box"
    )]
    pub fn set_rect(&mut self, element: &ElementId, rect: Rect) -> Result<(), PositionError> {
        let placement = self
            .placements
            .get(element)
            .ok_or(PositionError::NotPlaced)?;
        let stored = match self.host(element, placement) {
            Some((_, host)) => Rect {
                x: offset(rect.x, host.x)?,
                y: offset(rect.y, host.y)?,
                ..rect
            },
            None => rect,
        };
        if let Some(placement) = self.placements.get_mut(element) {
            placement.rect = stored;
        }
        Ok(())
    }

    /// Hangs `element` on `host` where it lies now, so it moves along with
    /// `host` from then on; an attachment to another box ends.
    ///
    /// # Errors
    ///
    /// Returns [`PositionError::NotPlaced`] when the board does not place the
    /// element, [`PositionError::OffsetTooLarge`] when its offset from `host`
    /// does not fit in `i32`, and [`PositionError::DoesNotHold`] when the
    /// attachment would not hold where the element lies, or something hangs
    /// on the element; then nothing changes.
    #[expect(
        clippy::disallowed_fields,
        reason = "attaching sets the field, and restores the placement when it fails"
    )]
    pub fn attach(&mut self, element: &ElementId, host: &ElementId) -> Result<(), PositionError> {
        let rect = self.rect_of(element).ok_or(PositionError::NotPlaced)?;
        // What hangs on the element would stop holding and move.
        if !self.hanging_on(element).is_empty() {
            return Err(PositionError::DoesNotHold);
        }
        let saved = self
            .placements
            .get(element)
            .cloned()
            .ok_or(PositionError::NotPlaced)?;
        if let Some(placement) = self.placements.get_mut(element) {
            placement.attached_to = Some(host.clone());
        }
        let outcome = self.set_rect(element, rect).and_then(|()| {
            let holds =
                self.attachment(element) == Some(host) && self.rect_of(element) == Some(rect);
            if holds {
                Ok(())
            } else {
                Err(PositionError::DoesNotHold)
            }
        });
        if outcome.is_err()
            && let Some(placement) = self.placements.get_mut(element)
        {
            *placement = saved;
        }
        outcome
    }

    /// Ends the attachment the file names for `element`, which keeps where it
    /// lies, now stored as a board position. Returns whether the file named
    /// one; if not, nothing changes.
    pub fn detach(&mut self, element: &ElementId) -> bool {
        self.detach_where(|id, _| id == element)
    }

    /// Ends the attachment of `element` if the file names `host` for it, as
    /// [`Board::detach`] does. Returns whether it did.
    pub fn detach_named(&mut self, element: &ElementId, host: &ElementId) -> bool {
        self.detach_where(|id, named| id == element && named == host)
    }

    /// Ends every attachment to `element` on this board: each element that
    /// names it keeps where it lies, now stored as a board position.
    pub fn detach_from(&mut self, element: &ElementId) {
        self.detach_where(|_, named| named == element);
    }

    /// Whether the file of any placement on this board names `element` as
    /// the box it hangs on, whether the attachment holds or not: a change of
    /// `element` can make such an attachment start to hold.
    #[must_use]
    #[expect(
        clippy::disallowed_fields,
        reason = "whether anything names a given element, which reveals no box"
    )]
    pub fn is_named(&self, element: &ElementId) -> bool {
        self.placements
            .values()
            .any(|placement| placement.attached_to.as_ref() == Some(element))
    }

    /// Whether the file says that `element` hangs on `host`, whether the
    /// attachment holds or not.
    #[must_use]
    #[expect(
        clippy::disallowed_fields,
        reason = "a comparison with a given box, which reveals no other box"
    )]
    pub fn names(&self, element: &ElementId, host: &ElementId) -> bool {
        self.placements
            .get(element)
            .is_some_and(|placement| placement.attached_to.as_ref() == Some(host))
    }

    /// Ends each attachment for which `matches` holds, given the element and
    /// the box its file names; each keeps where it lies. Returns whether any
    /// ended.
    #[expect(
        clippy::disallowed_fields,
        reason = "detaching rewrites the stored position and the field together"
    )]
    fn detach_where(&mut self, matches: impl Fn(&ElementId, &ElementId) -> bool) -> bool {
        let detached: Vec<(ElementId, Rect)> = self
            .placements
            .iter()
            .filter(|(id, placement)| {
                placement
                    .attached_to
                    .as_ref()
                    .is_some_and(|named| matches(id, named))
            })
            .map(|(id, placement)| (id.clone(), self.resolved(id, placement).0))
            .collect();
        for (id, rect) in &detached {
            if let Some(placement) = self.placements.get_mut(id) {
                placement.rect = *rect;
                placement.attached_to = None;
            }
        }
        !detached.is_empty()
    }

    /// The box `placement` of `element` names, with the box's stored rect,
    /// when the board places it, it is another element, and it hangs on
    /// nothing; its stored rect is then its board position.
    #[expect(
        clippy::disallowed_fields,
        reason = "the rule of an attachment reads the fields as written"
    )]
    fn host(&self, element: &ElementId, placement: &Placement) -> Option<(&ElementId, Rect)> {
        let named = placement
            .attached_to
            .as_ref()
            .filter(|named| *named != element)?;
        let (named, host) = self.placements.get_key_value(named)?;
        host.attached_to.is_none().then_some((named, host.rect))
    }

    /// Where `placement` of `element` lies, and the box it hangs on if its
    /// attachment holds. A position computed from a box must lie within
    /// [`MAX_COORDINATE`]; otherwise the stored rect counts as a board
    /// position.
    #[expect(
        clippy::disallowed_fields,
        reason = "resolving a position reads the stored rect"
    )]
    fn resolved(&self, element: &ElementId, placement: &Placement) -> (Rect, Option<&ElementId>) {
        let stored = placement.rect;
        let held = self.host(element, placement).and_then(|(named, host)| {
            let x = within_range(i64::from(host.x) + i64::from(stored.x))?;
            let y = within_range(i64::from(host.y) + i64::from(stored.y))?;
            Some((Rect { x, y, ..stored }, named))
        });
        held.map_or((stored, None), |(rect, named)| (rect, Some(named)))
    }

    /// Writes the board file, one placement per line, ordered by element ID.
    /// The owner, the summary, and the zone each get a line of their own,
    /// and only when the board has them.
    #[must_use]
    pub fn to_json(&self) -> String {
        let lines: Vec<String> = self
            .placements
            .iter()
            .map(|(element, placement)| placement_line(element, placement))
            .collect();
        let placements = if lines.is_empty() {
            "[]".to_owned()
        } else {
            format!("[\n{}\n  ]", lines.join(",\n"))
        };
        let owner = optional_line(
            "owner",
            self.owner.as_ref().map(|owner| json_string(owner.as_str())),
        );
        let summary = optional_line("summary", self.summary.as_deref().map(json_string));
        let zone = optional_line(
            "zone",
            self.zone.map(|zone| {
                format!(
                    "{{\"mode\": {}, \"statement\": {}}}",
                    json_string(zone.mode.as_str()),
                    json_string(zone.statement.as_str())
                )
            }),
        );
        format!(
            "{{\n  \"format\": {BOARD_FORMAT},\n  \"id\": {},\n  \"title\": {},\n{owner}{summary}{zone}  \"placements\": {placements}\n}}\n",
            json_string(self.id.as_str()),
            json_string(&self.title),
        )
    }
}

/// `value` as a position, if it lies within [`MAX_COORDINATE`].
fn within_range(value: i64) -> Option<i32> {
    i32::try_from(value)
        .ok()
        .filter(|value| (-MAX_COORDINATE..=MAX_COORDINATE).contains(value))
}

/// The offset of `position` from `origin`, if it fits in `i32`.
fn offset(position: i32, origin: i32) -> Result<i32, PositionError> {
    i32::try_from(i64::from(position) - i64::from(origin))
        .map_err(|_| PositionError::OffsetTooLarge)
}

/// The line `  "key": value,` for a field the board has, or nothing.
fn optional_line(key: &str, value: Option<String>) -> String {
    value.map_or(String::new(), |value| format!("  \"{key}\": {value},\n"))
}

/// Collects the placement lines into a map and rejects an element placed
/// twice; the map keeps the check at O(n log n) for files of any size.
fn collect_placements(
    board: &BoardId,
    lines: Vec<ObjectOnly<PlacementLine>>,
) -> Result<BTreeMap<ElementId, Placement>, BoardError> {
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
                slot.insert(Placement {
                    rect,
                    pinned: line.pinned,
                    attached_to: line.attached_to,
                });
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

/// One placement as a line of the file, with the stored rect as it is.
#[expect(
    clippy::disallowed_fields,
    reason = "the file holds the fields as stored"
)]
fn placement_line(element: &ElementId, placement: &Placement) -> String {
    let Placement {
        rect,
        pinned,
        attached_to,
    } = placement;
    let pin = if *pinned { ", \"pinned\": true" } else { "" };
    let attached = attached_to.as_ref().map_or(String::new(), |named| {
        format!(", \"attached_to\": {}", json_string(named.as_str()))
    });
    format!(
        "    {{\"element\": {}, \"x\": {}, \"y\": {}, \"width\": {}, \"height\": {}{pin}{attached}}}",
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
    #![expect(
        clippy::disallowed_fields,
        reason = "the tests compare the fields as stored with what the board reads"
    )]

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
            owner: None,
            summary: None,
            zone: None,
            placements: BTreeMap::from([
                (
                    element_id("checkout-service"),
                    Placement {
                        rect: rect(80, 200),
                        pinned: false,
                        attached_to: None,
                    },
                ),
                (
                    element_id("payment-service"),
                    Placement {
                        rect: rect(400, 200),
                        pinned: false,
                        attached_to: None,
                    },
                ),
            ]),
        }
    }

    /// A valid board file `main` with the placements given as JSON lines.
    fn main_file(placements: &str) -> String {
        format!(r#"{{"format": 1, "id": "main", "title": "t", "placements": [{placements}]}}"#)
    }

    /// A board whose file lists three placements out of ID order, one pinned.
    fn unordered_board() -> Board {
        Board::from_json(
            &board_id("main"),
            &main_file(
                r#"{"element": "c", "x": -7, "y": 11, "width": 3, "height": 5, "pinned": true},
                   {"element": "a", "x": 40, "y": -2, "width": 250, "height": 60},
                   {"element": "b", "x": 0, "y": 900, "width": 1, "height": 1}"#,
            ),
        )
        .unwrap()
    }

    #[test]
    fn rect_of_a_placed_element_is_its_stored_rect() {
        let board = unordered_board();

        for (element, placement) in &board.placements {
            assert_eq!(board.rect_of(element), Some(placement.rect), "{element}");
        }
        assert_eq!(
            board.rect_of(&element_id("c")),
            Some(Rect {
                x: -7,
                y: 11,
                width: NonZeroU32::new(3).unwrap(),
                height: NonZeroU32::new(5).unwrap(),
            })
        );
    }

    #[test]
    fn rect_of_an_element_the_board_does_not_place_is_none() {
        let mut board = unordered_board();

        assert_eq!(board.rect_of(&element_id("d")), None);
        board.placements.clear();
        assert_eq!(board.rect_of(&element_id("a")), None);
    }

    #[test]
    fn placed_yields_every_placement_once_in_id_order_with_its_stored_rect() {
        let board = unordered_board();

        let placed: Vec<(&ElementId, &Placement, Rect)> = board.placed().collect();

        assert_eq!(placed.len(), board.placements.len());
        assert_eq!(
            placed.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
            board.placements.keys().collect::<Vec<_>>()
        );
        for (id, placement, rect) in placed {
            assert_eq!(placement, &board.placements[id]);
            assert_eq!(rect, placement.rect);
        }
    }

    #[test]
    fn placed_on_an_empty_board_yields_nothing() {
        let board = Board::from_json(&board_id("main"), &main_file("")).unwrap();

        assert_eq!(board.placed().count(), 0);
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

    /// `sample_board` with `payment-service` pinned.
    fn board_with_a_pin() -> Board {
        let mut board = sample_board();
        if let Some(placement) = board.placements.get_mut(&element_id("payment-service")) {
            placement.pinned = true;
        }
        board
    }

    #[test]
    fn writes_a_pin_at_the_end_of_its_placement_line() {
        let expected = r#"{
  "format": 1,
  "id": "container-level",
  "title": "Container Level",
  "placements": [
    {"element": "checkout-service", "x": 80, "y": 200, "width": 180, "height": 90},
    {"element": "payment-service", "x": 400, "y": 200, "width": 180, "height": 90, "pinned": true}
  ]
}
"#;

        assert_eq!(board_with_a_pin().to_json(), expected);
    }

    #[test]
    fn reads_back_a_pin() {
        let board = board_with_a_pin();

        assert_eq!(Board::from_json(&board.id, &board.to_json()), Ok(board));
    }

    #[test]
    fn reads_pinned_false_as_not_pinned_and_writes_nothing_for_it() {
        let text = main_file(
            r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": false}"#,
        );

        let board = Board::from_json(&board_id("main"), &text).unwrap();

        assert_eq!(
            board
                .placements
                .get(&element_id("a"))
                .map(|placed| placed.pinned),
            Some(false)
        );
        assert!(!board.to_json().contains("pinned"), "{}", board.to_json());
    }

    #[test]
    fn pinning_one_placement_changes_exactly_one_line() {
        let before = sample_board().to_json();
        let after = board_with_a_pin().to_json();

        let changed = before
            .lines()
            .zip(after.lines())
            .filter(|(old, new)| old != new)
            .count();

        assert_eq!(before.lines().count(), after.lines().count());
        assert_eq!(changed, 1);
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
        after.placements.insert(
            element_id("payment-service"),
            Placement {
                rect: rect(420, 240),
                pinned: false,
                attached_to: None,
            },
        );

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
            owner: None,
            summary: None,
            zone: None,
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

    /// The sample board as the inner board of `shop-system`, with a
    /// summary.
    fn inner_board() -> Board {
        Board {
            owner: Some(element_id("shop-system")),
            summary: Some("The containers of the shop".to_owned()),
            ..sample_board()
        }
    }

    #[test]
    fn writes_the_owner_and_the_summary_on_lines_of_their_own() {
        let expected = r#"{
  "format": 1,
  "id": "container-level",
  "title": "Container Level",
  "owner": "shop-system",
  "summary": "The containers of the shop",
  "placements": [
    {"element": "checkout-service", "x": 80, "y": 200, "width": 180, "height": 90},
    {"element": "payment-service", "x": 400, "y": 200, "width": 180, "height": 90}
  ]
}
"#;

        assert_eq!(inner_board().to_json(), expected);
        assert_eq!(
            Board::from_json(&board_id("container-level"), expected).unwrap(),
            inner_board()
        );
    }

    #[test]
    fn writes_a_board_without_owner_and_summary_as_before() {
        let text = main_file(r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1}"#);
        let written = Board::from_json(&board_id("main"), &text)
            .unwrap()
            .to_json();

        let board = Board::from_json(&board_id("main"), &written).unwrap();

        assert_eq!(board.owner, None);
        assert_eq!(board.summary, None);
        assert_eq!(board.to_json(), written);
        assert!(!written.contains("owner"));
        assert!(!written.contains("summary"));
        assert!(!written.contains("zone"));
    }

    #[test]
    fn keeps_a_summary_with_special_characters_on_its_line() {
        let board = Board {
            summary: Some("line\nbreak \"quoted\" back\\slash \u{2028} \u{1b}[31m ]}".to_owned()),
            ..inner_board()
        };

        let json = board.to_json();

        assert_eq!(
            json.lines().count(),
            inner_board().to_json().lines().count()
        );
        assert_eq!(Board::from_json(&board.id, &json).unwrap(), board);
    }

    #[test]
    fn reads_a_null_owner_and_summary_as_none() {
        let text = r#"{"format": 1, "id": "main", "title": "t", "owner": null, "summary": null, "placements": []}"#;

        let board = Board::from_json(&board_id("main"), text).unwrap();

        assert_eq!((board.owner, board.summary), (None, None));
    }

    #[rstest]
    #[case::path(r#""../x""#)]
    #[case::capitals(r#""Shop""#)]
    #[case::too_long(r#""aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa""#)]
    #[case::number("1")]
    #[case::object(r#"{"id": "shop"}"#)]
    fn rejects_an_owner_that_is_no_element_id(#[case] owner: &str) {
        let text = format!(
            r#"{{"format": 1, "id": "main", "title": "t", "owner": {owner}, "placements": []}}"#
        );

        assert!(matches!(
            Board::from_json(&board_id("main"), &text),
            Err(BoardError::Invalid { .. })
        ));
    }

    #[rstest]
    #[case::summary_as_number(r#""summary": 1"#)]
    #[case::summary_as_array(r#""summary": ["a"]"#)]
    #[case::owner_twice(r#""owner": "a", "owner": "b""#)]
    #[case::summary_twice(r#""summary": "a", "summary": "b""#)]
    fn rejects_owner_and_summary_fields_it_does_not_expect(#[case] fields: &str) {
        let text =
            format!(r#"{{"format": 1, "id": "main", "title": "t", {fields}, "placements": []}}"#);

        assert!(matches!(
            Board::from_json(&board_id("main"), &text),
            Err(BoardError::Invalid { .. })
        ));
    }

    const MIRRORED_CURRENT: Zone = Zone {
        mode: crate::Mode::Mirrored,
        statement: crate::StatementKind::Current,
    };

    #[test]
    fn writes_the_zone_on_a_line_of_its_own_after_the_summary() {
        let expected = r#"{
  "format": 1,
  "id": "container-level",
  "title": "Container Level",
  "owner": "shop-system",
  "summary": "The containers of the shop",
  "zone": {"mode": "mirrored", "statement": "current"},
  "placements": [
    {"element": "checkout-service", "x": 80, "y": 200, "width": 180, "height": 90},
    {"element": "payment-service", "x": 400, "y": 200, "width": 180, "height": 90}
  ]
}
"#;
        let board = Board {
            zone: Some(MIRRORED_CURRENT),
            ..inner_board()
        };

        assert_eq!(board.to_json(), expected);
        assert_eq!(
            Board::from_json(&board_id("container-level"), expected).unwrap(),
            board
        );
    }

    #[test]
    fn writes_a_zone_without_owner_and_summary_after_the_title() {
        let board = Board {
            id: board_id("main"),
            title: "t".to_owned(),
            owner: None,
            summary: None,
            zone: Some(MIRRORED_CURRENT),
            placements: BTreeMap::new(),
        };

        assert_eq!(
            board.to_json(),
            "{\n  \"format\": 1,\n  \"id\": \"main\",\n  \"title\": \"t\",\n  \
             \"zone\": {\"mode\": \"mirrored\", \"statement\": \"current\"},\n  \
             \"placements\": []\n}\n"
        );
    }

    #[test]
    fn reads_a_null_zone_as_none_and_writes_no_zone_line() {
        let text = r#"{"format": 1, "id": "main", "title": "t", "zone": null, "placements": []}"#;

        let board = Board::from_json(&board_id("main"), text).unwrap();

        assert_eq!(board.zone, None);
        assert!(!board.to_json().contains("zone"));
    }

    #[rstest]
    #[case::synced(r#"{"mode": "synced", "statement": "current"}"#)]
    #[case::locked(r#"{"mode": "locked", "statement": "current"}"#)]
    #[case::historic(r#"{"mode": "free", "statement": "historic"}"#)]
    #[case::capitals(r#"{"mode": "free", "statement": "Current"}"#)]
    #[case::trailing_space(r#"{"mode": "free", "statement": "current "}"#)]
    #[case::empty_name(r#"{"mode": "free", "statement": ""}"#)]
    #[case::preset_name(r#""current""#)]
    #[case::as_an_array(r#"["free", "current"]"#)]
    #[case::as_a_boolean("true")]
    #[case::as_a_number("1")]
    #[case::mode_as_an_object(r#"{"mode": {"free": null}, "statement": "current"}"#)]
    #[case::kind_as_an_object(r#"{"mode": "free", "statement": {"idea": null}}"#)]
    #[case::null_mode(r#"{"mode": null, "statement": "current"}"#)]
    #[case::no_mode(r#"{"statement": "current"}"#)]
    #[case::no_kind(r#"{"mode": "free"}"#)]
    #[case::empty("{}")]
    #[case::unknown_field(r#"{"mode": "free", "statement": "current", "preset": "current"}"#)]
    #[case::mode_twice(r#"{"mode": "free", "mode": "mirrored", "statement": "idea"}"#)]
    fn rejects_any_other_zone_in_a_board_file(#[case] zone: &str) {
        let text = format!(
            r#"{{"format": 1, "id": "main", "title": "t", "zone": {zone}, "placements": []}}"#
        );

        let error = Board::from_json(&board_id("main"), &text).unwrap_err();

        assert!(matches!(error, BoardError::Invalid { .. }), "{error}");
        assert!(error.to_string().starts_with("board main: "), "{error}");
    }

    #[test]
    fn rejects_a_zone_given_twice() {
        let zone = r#"{"mode": "free", "statement": "idea"}"#;
        let text = format!(
            r#"{{"format": 1, "id": "main", "title": "t", "zone": {zone}, "zone": {zone}, "placements": []}}"#
        );

        assert!(matches!(
            Board::from_json(&board_id("main"), &text),
            Err(BoardError::Invalid { .. })
        ));
    }

    #[test]
    fn shows_no_raw_control_character_of_an_unknown_kind() {
        let text = r#"{"format": 1, "id": "main", "title": "t",
            "zone": {"mode": "free", "statement": "x\n\u001b[31m"}, "placements": []}"#;

        let error = Board::from_json(&board_id("main"), text)
            .unwrap_err()
            .to_string();

        assert!(!error.chars().any(char::is_control), "{error:?}");
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
        r#"{"format": 1, "id": "main", "title": "t", "placements": [], "unexpected": {}}"#
    )]
    #[case::unknown_placement_field(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pin": true}"#
    ))]
    #[case::pinned_null(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": null}"#
    ))]
    #[case::pinned_zero(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": 0}"#
    ))]
    #[case::pinned_one(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": 1}"#
    ))]
    #[case::pinned_string(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": "true"}"#
    ))]
    #[case::pinned_object(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": {}}"#
    ))]
    #[case::pinned_array(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": []}"#
    ))]
    #[case::pinned_twice(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": true, "pinned": false}"#
    ))]
    #[case::pinned_capitalized(&main_file(
        r#"{"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "Pinned": true}"#
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

    #[rstest]
    #[case::same_place(rect(0, 0), true)]
    #[case::one_unit_inside_the_right_edge(rect(179, 0), true)]
    #[case::one_unit_inside_the_bottom_edge(rect(0, 89), true)]
    #[case::touching_the_right_edge(rect(180, 0), false)]
    #[case::touching_the_bottom_edge(rect(0, 90), false)]
    #[case::touching_the_left_edge(rect(-180, 0), false)]
    #[case::touching_the_top_edge(rect(0, -90), false)]
    fn rectangles_overlap_only_when_they_share_area(#[case] other: Rect, #[case] expected: bool) {
        let origin = rect(0, 0);

        assert_eq!(origin.overlaps(&other), expected);
        assert_eq!(other.overlaps(&origin), expected);
    }

    #[test]
    fn edges_of_the_extreme_rectangle_do_not_overflow() {
        let extreme = Rect {
            x: i32::MAX,
            y: i32::MAX,
            width: NonZeroU32::MAX,
            height: NonZeroU32::MAX,
        };

        let expected = i64::from(i32::MAX) + i64::from(u32::MAX);
        assert_eq!(extreme.right(), expected);
        assert_eq!(extreme.bottom(), expected);
    }

    /// The line of `element` at `x` and `y`, 180 by 90, with `rest` after the
    /// height.
    fn line(element: &str, x: i32, y: i32, rest: &str) -> String {
        format!(
            r#"{{"element": "{element}", "x": {x}, "y": {y}, "width": 180, "height": 90{rest}}}"#
        )
    }

    /// A board `main` with these placement lines.
    fn board_of(lines: &[String]) -> Board {
        Board::from_json(&board_id("main"), &main_file(&lines.join(", "))).unwrap()
    }

    /// A board with the box `b` at 100, 200 and the note `n` named on it with
    /// the offset `dx`, `dy`.
    fn note_on_box(dx: i32, dy: i32) -> Board {
        board_of(&[
            line("b", 100, 200, ""),
            line("n", dx, dy, r#", "attached_to": "b""#),
        ])
    }

    fn at(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width: NonZeroU32::new(width).unwrap(),
            height: NonZeroU32::new(height).unwrap(),
        }
    }

    #[test]
    fn writes_an_attachment_last_on_its_line_and_reads_it_back() {
        let text = main_file(&line("n", 5, 6, r#", "pinned": true, "attached_to": "b""#));
        let board = Board::from_json(&board_id("main"), &text).unwrap();

        let json = board.to_json();

        assert!(
            json.contains(
                r#"    {"element": "n", "x": 5, "y": 6, "width": 180, "height": 90, "pinned": true, "attached_to": "b"}"#
            ),
            "{json}"
        );
        assert_eq!(Board::from_json(&board.id, &json), Ok(board));
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "the test checks the field as written"
    )]
    #[test]
    fn reads_a_null_attachment_as_none_and_writes_nothing_for_it() {
        let board = board_of(&[line("n", 0, 0, r#", "attached_to": null"#)]);

        assert_eq!(board.named_attachment(&element_id("n")), None);
        assert!(
            !board.to_json().contains("attached_to"),
            "{}",
            board.to_json()
        );
    }

    #[rstest]
    #[case::path(r#""attached_to": "../x""#)]
    #[case::capitals(r#""attached_to": "Shop""#)]
    #[case::empty(r#""attached_to": """#)]
    #[case::too_long(&format!(r#""attached_to": "{}""#, "a".repeat(65)))]
    #[case::number(r#""attached_to": 1"#)]
    #[case::object(r#""attached_to": {"id": "b"}"#)]
    #[case::array(r#""attached_to": ["b"]"#)]
    #[case::boolean(r#""attached_to": true"#)]
    #[case::twice(r#""attached_to": "b", "attached_to": "c""#)]
    #[case::camel_case(r#""attachedTo": "b""#)]
    #[case::hyphen(r#""attached-to": "b""#)]
    #[case::capitalized(r#""Attached_to": "b""#)]
    #[case::line_break(r#""attached_to": "b\nc""#)]
    #[case::escape(r#""attached_to": "b\u001b[31m""#)]
    fn rejects_an_attachment_that_is_no_element_id(#[case] field: &str) {
        let text = main_file(&line("n", 0, 0, &format!(", {field}")));

        let error = Board::from_json(&board_id("main"), &text).unwrap_err();

        assert!(matches!(error, BoardError::Invalid { .. }), "{error:?}");
        let message = error.to_string();
        assert!(message.starts_with("board main: "), "{message}");
        assert!(!message.chars().any(char::is_control), "{message:?}");
    }

    #[test]
    fn attaching_or_moving_a_note_changes_exactly_one_line_and_moving_its_box_only_the_box() {
        let loose = board_of(&[line("b", 100, 200, ""), line("n", 5, 6, "")]);
        let attached = note_on_box(5, 6);
        let mut moved_note = attached.clone();
        moved_note
            .set_rect(&element_id("n"), at(300, 300, 180, 90))
            .unwrap();
        let mut moved_box = attached.clone();
        moved_box
            .set_rect(&element_id("b"), at(0, 0, 180, 90))
            .unwrap();

        for (before, after) in [
            (&loose, &attached),
            (&attached, &moved_note),
            (&attached, &moved_box),
        ] {
            let (before, after) = (before.to_json(), after.to_json());
            let changed = before
                .lines()
                .zip(after.lines())
                .filter(|(old, new)| old != new)
                .count();
            assert_eq!(changed, 1, "{before}\n{after}");
        }
        let box_line = moved_box.to_json();
        assert!(
            box_line.contains(r#""element": "b", "x": 0, "y": 0"#),
            "{box_line}"
        );
    }

    #[test]
    fn a_holding_attachment_puts_the_note_at_the_box_plus_its_offset_with_its_own_size() {
        let board = board_of(&[
            line("b", 100, 200, ""),
            r#"{"element": "n", "x": -10, "y": 20, "width": 7, "height": 3, "attached_to": "b"}"#
                .to_owned(),
        ]);

        assert_eq!(board.rect_of(&element_id("n")), Some(at(90, 220, 7, 3)));
        assert_eq!(board.attachment(&element_id("n")), Some(&element_id("b")));
        assert_eq!(board.rect_of(&element_id("b")), Some(at(100, 200, 180, 90)));
        assert_eq!(board.attachment(&element_id("b")), None);
    }

    #[rstest]
    #[case::x_at_the_upper_limit(MAX_COORDINATE - 100, 0, true)]
    #[case::x_beyond_the_upper_limit(MAX_COORDINATE - 99, 0, false)]
    #[case::x_at_the_lower_limit(-MAX_COORDINATE - 100, 0, true)]
    #[case::x_beyond_the_lower_limit(-MAX_COORDINATE - 101, 0, false)]
    #[case::y_at_the_upper_limit(0, MAX_COORDINATE - 200, true)]
    #[case::y_beyond_the_upper_limit(0, MAX_COORDINATE - 199, false)]
    #[case::y_at_the_lower_limit(0, -MAX_COORDINATE - 200, true)]
    #[case::y_beyond_the_lower_limit(0, -MAX_COORDINATE - 201, false)]
    fn an_attachment_holds_only_within_the_range_of_a_move(
        #[case] dx: i32,
        #[case] dy: i32,
        #[case] holds: bool,
    ) {
        let board = note_on_box(dx, dy);
        let note = element_id("n");

        let expected = if holds {
            at(100 + dx, 200 + dy, 180, 90)
        } else {
            at(dx, dy, 180, 90)
        };

        assert_eq!(board.rect_of(&note), Some(expected));
        assert_eq!(board.attachment(&note).is_some(), holds);
        #[expect(
            clippy::disallowed_methods,
            reason = "the test checks the field as written"
        )]
        let named = board.named_attachment(&note);
        assert_eq!(named, Some(&element_id("b")));
    }

    #[rstest]
    #[case::box_at_min_offset_at_min(i32::MIN, i32::MIN)]
    #[case::box_at_min_offset_at_max(i32::MIN, i32::MAX)]
    #[case::box_at_max_offset_at_max(i32::MAX, i32::MAX)]
    #[case::box_at_max_offset_at_min(i32::MAX, i32::MIN)]
    fn extreme_positions_overflow_nothing(#[case] position: i32, #[case] offset: i32) {
        let board = board_of(&[
            line("b", position, position, ""),
            line("n", offset, offset, r#", "attached_to": "b""#),
        ]);
        let note = element_id("n");

        let rect = board.rect_of(&note).unwrap();
        let sum = i64::from(position) + i64::from(offset);
        if (-1_000_000..=1_000_000).contains(&sum) {
            assert_eq!(board.attachment(&note), Some(&element_id("b")));
        } else {
            assert_eq!(rect, at(offset, offset, 180, 90));
            assert_eq!(board.attachment(&note), None);
        }
    }

    #[rstest]
    #[case::itself(vec![line("n", 5, 6, r#", "attached_to": "n""#)])]
    #[case::an_element_the_board_does_not_place(vec![line("n", 5, 6, r#", "attached_to": "b""#)])]
    #[case::an_attached_box(vec![
        line("a", 1000, 1000, ""),
        line("b", 100, 200, r#", "attached_to": "a""#),
        line("n", 5, 6, r#", "attached_to": "b""#),
    ])]
    #[case::a_box_attached_to_it(vec![
        line("b", 100, 200, r#", "attached_to": "n""#),
        line("n", 5, 6, r#", "attached_to": "b""#),
    ])]
    fn an_attachment_to_no_free_box_on_the_board_reads_as_none(#[case] lines: Vec<String>) {
        let board = board_of(&lines);
        let note = element_id("n");

        assert_eq!(board.rect_of(&note), Some(at(5, 6, 180, 90)));
        assert_eq!(board.attachment(&note), None);
        #[expect(
            clippy::disallowed_methods,
            reason = "the test checks the field as written"
        )]
        let named = board.named_attachment(&note);
        assert!(named.is_some());
        for (id, _, rect) in board.placed() {
            assert_eq!(board.rect_of(id), Some(rect), "{id}");
        }
    }

    #[test]
    fn placed_yields_board_positions_once_each_in_id_order() {
        let board = board_of(&[
            line("c", 1, 2, r#", "attached_to": "b""#),
            line("b", 100, 200, ""),
            line("a", 3, 4, r#", "attached_to": "b""#),
        ]);

        let placed: Vec<(String, Rect)> = board
            .placed()
            .map(|(id, _, rect)| (id.to_string(), rect))
            .collect();

        assert_eq!(
            placed,
            [
                ("a".to_owned(), at(103, 204, 180, 90)),
                ("b".to_owned(), at(100, 200, 180, 90)),
                ("c".to_owned(), at(101, 202, 180, 90)),
            ]
        );
        assert_eq!(
            board.hanging_on(&element_id("b")),
            [element_id("a"), element_id("c")]
        );
        assert_eq!(board.hanging_on(&element_id("a")), Vec::<ElementId>::new());
    }

    #[test]
    fn set_rect_stores_a_board_position_relative_to_the_box_the_note_hangs_on() {
        let mut board = note_on_box(5, 6);
        let note = element_id("n");

        board.set_rect(&note, at(-20, 300, 50, 60)).unwrap();

        assert_eq!(board.rect_of(&note), Some(at(-20, 300, 50, 60)));
        assert_eq!(board.placements[&note].rect, at(-120, 100, 50, 60));
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "the test checks the field as written"
    )]
    #[test]
    fn set_rect_stores_a_board_position_as_it_is_without_a_free_box() {
        let mut board = board_of(&[line("n", 5, 6, r#", "attached_to": "gone""#)]);
        let note = element_id("n");

        board.set_rect(&note, at(-20, 300, 50, 60)).unwrap();

        assert_eq!(board.placements[&note].rect, at(-20, 300, 50, 60));
        assert_eq!(board.named_attachment(&note), Some(&element_id("gone")));
    }

    #[test]
    fn set_rect_refuses_an_element_the_board_does_not_place_and_an_offset_beyond_i32() {
        let mut far = board_of(&[
            line("b", i32::MIN, 0, ""),
            line("n", 0, 0, r#", "attached_to": "b""#),
        ]);
        let before = far.clone();

        assert_eq!(
            far.set_rect(&element_id("x"), at(0, 0, 1, 1)),
            Err(PositionError::NotPlaced)
        );
        assert_eq!(
            far.set_rect(&element_id("n"), at(MAX_COORDINATE, 0, 1, 1)),
            Err(PositionError::OffsetTooLarge)
        );
        assert_eq!(far, before);
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "the test checks the field as written"
    )]
    #[test]
    fn detaching_keeps_where_each_note_lies_and_drops_the_field() {
        let mut board = board_of(&[
            line("b", 100, 200, ""),
            line("m", 7, 8, r#", "attached_to": "b""#),
            line("n", 5, 6, r#", "attached_to": "b""#),
            line("o", 1, 1, r#", "attached_to": "x""#),
        ]);
        let before: Vec<(ElementId, Rect)> = board
            .placed()
            .map(|(id, _, rect)| (id.clone(), rect))
            .collect();

        board.detach_from(&element_id("b"));

        let after: Vec<(ElementId, Rect)> = board
            .placed()
            .map(|(id, _, rect)| (id.clone(), rect))
            .collect();
        assert_eq!(after, before);
        assert_eq!(board.named_attachment(&element_id("m")), None);
        assert_eq!(board.named_attachment(&element_id("n")), None);
        assert_eq!(
            board.named_attachment(&element_id("o")),
            Some(&element_id("x"))
        );
        assert_eq!(
            board.placements[&element_id("n")].rect,
            at(105, 206, 180, 90)
        );
    }

    #[test]
    fn attaching_keeps_where_the_note_lies_and_stores_its_offset() {
        let mut board = board_of(&[line("b", 100, 200, ""), line("n", 30, 40, "")]);
        let (note, host) = (element_id("n"), element_id("b"));

        board.attach(&note, &host).unwrap();

        assert_eq!(board.rect_of(&note), Some(at(30, 40, 180, 90)));
        assert_eq!(board.attachment(&note), Some(&host));
        assert_eq!(board.placements[&note].rect, at(-70, -160, 180, 90));
    }

    #[test]
    fn attaching_to_another_box_moves_the_attachment_and_keeps_the_position() {
        let mut board = note_on_box(5, 6);
        board
            .placements
            .insert(element_id("c"), placed(-50, 0, false));
        let note = element_id("n");

        board.attach(&note, &element_id("c")).unwrap();

        assert_eq!(board.rect_of(&note), Some(at(105, 206, 180, 90)));
        assert_eq!(board.attachment(&note), Some(&element_id("c")));
    }

    fn placed(x: i32, y: i32, pinned: bool) -> Placement {
        Placement {
            rect: at(x, y, 180, 90),
            pinned,
            attached_to: None,
        }
    }

    #[rstest]
    #[case::a_box_that_hangs_on_another(vec![
        line("a", 0, 0, ""),
        line("b", 100, 200, r#", "attached_to": "a""#),
        line("n", 5, 6, ""),
    ], "b", PositionError::DoesNotHold)]
    #[case::a_box_the_board_does_not_place(vec![line("n", 5, 6, "")], "b", PositionError::DoesNotHold)]
    #[case::itself(vec![line("n", 5, 6, "")], "n", PositionError::DoesNotHold)]
    #[case::a_note_beyond_the_range(vec![
        line("b", 0, 0, ""),
        line("n", 2_000_000, 6, ""),
    ], "b", PositionError::DoesNotHold)]
    #[case::an_offset_beyond_i32(vec![
        line("b", i32::MIN, 0, ""),
        line("n", i32::MAX, 0, ""),
    ], "b", PositionError::OffsetTooLarge)]
    fn a_failed_attachment_changes_nothing(
        #[case] lines: Vec<String>,
        #[case] host: &str,
        #[case] expected: PositionError,
    ) {
        let mut board = board_of(&lines);
        let before = board.clone();

        assert_eq!(
            board.attach(&element_id("n"), &element_id(host)),
            Err(expected)
        );
        assert_eq!(board, before);
        assert_eq!(
            board.attach(&element_id("x"), &element_id(host)),
            Err(PositionError::NotPlaced)
        );
        assert_eq!(board, before);
    }

    #[test]
    fn detaching_keeps_the_position_and_reports_whether_the_file_named_a_box() {
        let mut board = note_on_box(5, 6);
        board
            .placements
            .insert(element_id("loose"), placed(1, 2, false));
        let note = element_id("n");

        assert!(board.detach(&note));
        assert_eq!(board.rect_of(&note), Some(at(105, 206, 180, 90)));
        assert!(!board.names(&note, &element_id("b")));
        let before = board.clone();
        assert!(!board.detach(&note));
        assert!(!board.detach(&element_id("loose")));
        assert!(!board.detach(&element_id("nowhere")));
        assert_eq!(board, before);
    }

    #[test]
    fn detaching_from_a_named_box_ends_only_that_attachment_also_when_it_does_not_hold() {
        let mut board = board_of(&[
            line("b", 100, 200, ""),
            line("m", 7, 8, r#", "attached_to": "b""#),
            line("n", 5, 6, r#", "attached_to": "gone""#),
        ]);
        let before = board.clone();

        assert!(!board.detach_named(&element_id("n"), &element_id("b")));
        assert_eq!(board, before);
        assert!(board.names(&element_id("n"), &element_id("gone")));
        assert!(board.detach_named(&element_id("n"), &element_id("gone")));
        assert_eq!(board.rect_of(&element_id("n")), Some(at(5, 6, 180, 90)));
        assert!(!board.names(&element_id("n"), &element_id("gone")));
        assert_eq!(board.attachment(&element_id("m")), Some(&element_id("b")));
    }

    #[test]
    fn attaching_an_element_something_hangs_on_changes_no_byte() {
        let mut board = board_of(&[
            line("b", 100, 200, ""),
            line("c", 0, 0, ""),
            line("x", 1, 2, r#", "attached_to": "c""#),
        ]);
        let before = board.to_json();

        assert_eq!(
            board.attach(&element_id("c"), &element_id("b")),
            Err(PositionError::DoesNotHold)
        );
        assert_eq!(board.to_json(), before);
    }

    #[rstest]
    #[case::holding(vec![line("n", 0, 0, ""), line("x", 1, 2, r#", "attached_to": "n""#)])]
    #[case::out_of_range(vec![
        line("n", 0, 0, ""),
        line("x", 2_000_000, 2, r#", "attached_to": "n""#),
    ])]
    #[case::in_a_chain(vec![
        line("b", 0, 0, ""),
        line("n", 0, 0, r#", "attached_to": "b""#),
        line("x", 1, 2, r#", "attached_to": "n""#),
    ])]
    #[case::unplaced(vec![line("x", 1, 2, r#", "attached_to": "n""#)])]
    fn an_element_is_named_by_a_file_whether_the_attachment_holds_or_not(
        #[case] lines: Vec<String>,
    ) {
        let board = board_of(&lines);

        assert!(board.is_named(&element_id("n")));
        assert!(!board.is_named(&element_id("x")));
    }
}
