//! Finding free space on a board for a new element.
//!
//! New elements go into the first free cell of a fixed grid near the
//! origin; when the grid is full, below everything else on the board. The
//! occupied cells are computed once per board, so placing many elements on
//! a board with many placements stays O(placements × cells).

use std::num::NonZeroU32;

use abyme_model::{Board, Rect};

/// The width of an element the skeleton places: 180.
const WIDTH: NonZeroU32 = NonZeroU32::MIN.saturating_add(179);
/// The height of an element the skeleton places: 90.
const HEIGHT: NonZeroU32 = NonZeroU32::MIN.saturating_add(89);
/// The horizontal distance between the left edges of two grid cells.
const CELL_WIDTH: i64 = 220;
/// The vertical distance between the top edges of two grid cells.
const CELL_HEIGHT: i64 = 130;
/// The columns of the grid.
const COLUMNS: usize = 4;
/// The rows of the grid; together with the columns, the cells checked
/// before falling back to the space below the board's content.
const ROWS: usize = 64;
/// The space between the lowest element and one placed below it.
const GAP: i64 = 40;

/// Where new elements can go on one board.
pub struct Layout {
    occupied: Vec<bool>,
    lowest_bottom: Option<i64>,
}

impl Layout {
    /// The free space on `board` as it is now.
    pub fn of(board: &Board) -> Self {
        let occupied = (0..COLUMNS * ROWS)
            .map(|cell| {
                cell_rect(cell).is_none_or(|candidate| {
                    board
                        .placements
                        .values()
                        .any(|rect| rect.overlaps(&candidate))
                })
            })
            .collect();
        let lowest_bottom = board.placements.values().map(Rect::bottom).max();
        Self {
            occupied,
            lowest_bottom,
        }
    }

    /// The rectangle for the next new element, which this layout then counts
    /// as occupied; `None` if no position fits in `i32`.
    pub fn place(&mut self) -> Option<Rect> {
        let free_cell = self
            .occupied
            .iter_mut()
            .enumerate()
            .find(|(_, occupied)| !**occupied);
        let rect = match free_cell {
            Some((cell, occupied)) => {
                *occupied = true;
                cell_rect(cell)?
            }
            None => below(self.lowest_bottom?)?,
        };
        self.raise_bottom(&rect);
        Some(rect)
    }

    /// Counts `rect`, where the request moved an element, as occupied, so
    /// that no later placement of the request overlaps it. The cells the
    /// element left stay counted as occupied.
    pub fn occupy(&mut self, rect: &Rect) {
        for (cell, occupied) in self.occupied.iter_mut().enumerate() {
            if cell_rect(cell).is_some_and(|candidate| rect.overlaps(&candidate)) {
                *occupied = true;
            }
        }
        self.raise_bottom(rect);
    }

    /// Takes `rect` into account for the space below the board's content.
    fn raise_bottom(&mut self, rect: &Rect) {
        let new_bottom = rect.bottom();
        self.lowest_bottom = Some(
            self.lowest_bottom
                .map_or(new_bottom, |lowest| lowest.max(new_bottom)),
        );
    }
}

/// The rectangle of grid cell `cell`, counted row by row, if it fits in
/// `i32`, which every cell of the grid does.
fn cell_rect(cell: usize) -> Option<Rect> {
    let column = i64::try_from(cell % COLUMNS).ok()?;
    let row = i64::try_from(cell / COLUMNS).ok()?;
    let x = i32::try_from(column * CELL_WIDTH).ok()?;
    let y = i32::try_from(row * CELL_HEIGHT).ok()?;
    Some(sized_rect(x, y))
}

/// A rectangle of the skeleton's size below everything, if it fits in `i32`.
fn below(lowest_bottom: i64) -> Option<Rect> {
    let y = i32::try_from(lowest_bottom + GAP).ok()?;
    let rect = sized_rect(0, y);
    (rect.bottom() <= i64::from(i32::MAX)).then_some(rect)
}

/// A rectangle of the skeleton's size at `x`, `y`.
const fn sized_rect(x: i32, y: i32) -> Rect {
    Rect {
        x,
        y,
        width: WIDTH,
        height: HEIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use abyme_model::{BoardId, ElementId};

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width: NonZeroU32::new(width).unwrap(),
            height: NonZeroU32::new(height).unwrap(),
        }
    }

    fn board(placements: &[(&str, Rect)]) -> Board {
        Board {
            id: "main".parse::<BoardId>().unwrap(),
            title: String::new(),
            owner: None,
            summary: None,
            zone: None,
            placements: placements
                .iter()
                .map(|(id, rect)| (id.parse::<ElementId>().unwrap(), *rect))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    #[test]
    fn places_the_first_element_at_the_origin() {
        let mut layout = Layout::of(&board(&[]));

        assert_eq!(layout.place(), Some(rect(0, 0, 180, 90)));
    }

    #[test]
    fn places_one_element_after_another_without_overlap() {
        let mut layout = Layout::of(&board(&[]));

        let first = layout.place().unwrap();
        let second = layout.place().unwrap();

        assert_eq!(second, rect(220, 0, 180, 90));
        assert!(!first.overlaps(&second));
    }

    #[test]
    fn skips_cells_that_existing_placements_cover() {
        let mut layout = Layout::of(&board(&[("a", rect(100, 50, 200, 20))]));

        let placed = layout.place().unwrap();

        assert_eq!(placed, rect(440, 0, 180, 90));
    }

    #[test]
    fn places_below_everything_when_the_grid_is_full() {
        let mut layout = Layout::of(&board(&[("wall", rect(-10, -10, 1_000, 9_000))]));

        let placed = layout.place().unwrap();

        assert_eq!(placed, rect(0, 9_030, 180, 90));
    }

    #[test]
    fn reports_a_full_board_instead_of_overflowing() {
        let mut layout = Layout::of(&board(&[
            ("huge", rect(i32::MIN, i32::MIN, u32::MAX, u32::MAX)),
            ("low", rect(0, i32::MAX - 10, 1, 1)),
        ]));

        assert_eq!(layout.place(), None);
    }

    #[test]
    fn reports_a_full_board_when_only_the_box_below_would_overflow() {
        let mut layout = Layout::of(&board(&[
            ("wall", rect(-10, -10, 1_000, 9_000)),
            ("low", rect(0, i32::MAX - 100, 1, 1)),
        ]));

        assert_eq!(layout.place(), None);
    }

    #[test]
    fn handles_many_placements_and_extreme_rectangles() {
        let placements: Vec<(String, Rect)> = (0..10_000_i32)
            .map(|index| (format!("e{index}"), rect(index * 7, index * 3, u32::MAX, 5)))
            .collect();
        let borrowed: Vec<(&str, Rect)> = placements
            .iter()
            .map(|(id, rect)| (id.as_str(), *rect))
            .collect();
        let mut layout = Layout::of(&board(&borrowed));

        let placed: Vec<Option<Rect>> = (0..100).map(|_| layout.place()).collect();

        let all: Vec<Rect> = placed.iter().flatten().copied().collect();
        assert_eq!(all.len(), 100);
        for (index, new) in all.iter().enumerate() {
            assert!(placements.iter().all(|(_, old)| !old.overlaps(new)));
            assert!(all.iter().skip(index + 1).all(|other| !other.overlaps(new)));
        }
    }
}
