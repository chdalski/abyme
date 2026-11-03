//! Finding free space on a board for a new element.
//!
//! A new box goes to the first place in a fixed grid near the origin where
//! it fits; when there is none, below everything else on the board. The taken
//! cells are computed once per board, so placing many elements on a board
//! with many placements stays O(placements × cells).

use std::num::NonZeroU32;

use abyme_model::{Board, Rect};

use super::Size;

/// The space between two cells, and between the lowest element and one placed
/// below it.
const GAP: u32 = 40;
/// The distance between the corners of two grid cells: a box of the default
/// size and the gap.
const CELL: Size = Size {
    width: Size::DEFAULT.width.saturating_add(GAP),
    height: Size::DEFAULT.height.saturating_add(GAP),
};
/// The columns of the grid.
const COLUMNS: usize = 4;
/// The rows of the grid; together with the columns, the cells checked
/// before falling back to the space below the board's content.
const ROWS: usize = 64;

/// Where new elements can go on one board.
pub struct FreeSpace {
    cells: Vec<Cell>,
    lowest_bottom: Option<i64>,
}

/// What is taken of one grid cell.
#[derive(Clone, Copy)]
struct Cell {
    /// Something overlaps a box of the default size in this cell.
    box_taken: bool,
    /// Something overlaps the cell or the gaps to its neighbors, which a box
    /// across several cells lies over too.
    area_taken: bool,
}

impl Cell {
    /// Cell number `cell`, as far as `covered` says that a rectangle overlaps
    /// something. Every cell of the grid fits in `i32`.
    fn taken_by(cell: usize, covered: impl Fn(&Rect) -> bool) -> Self {
        let taken = |size| cell_rect(cell, size).is_none_or(|rect| covered(&rect));
        Self {
            box_taken: taken(Size::DEFAULT),
            area_taken: taken(CELL),
        }
    }
}

impl FreeSpace {
    /// The free space on `board` as it is now.
    pub fn of(board: &Board) -> Self {
        let cells = (0..COLUMNS * ROWS)
            .map(|cell| {
                Cell::taken_by(cell, |rect| {
                    board.placed().any(|(_, _, placed)| placed.overlaps(rect))
                })
            })
            .collect();
        let lowest_bottom = board.placed().map(|(_, _, placed)| placed.bottom()).max();
        Self {
            cells,
            lowest_bottom,
        }
    }

    /// The rectangle for the next new box of `size`, which then counts as
    /// occupied; `None` if no position fits in `i32`.
    pub fn place(&mut self, size: Size) -> Option<Rect> {
        let rect = match self.first_fit(size) {
            Some(cell) => cell_rect(cell, size)?,
            None => below(self.lowest_bottom, size)?,
        };
        self.occupy(&rect);
        Some(rect)
    }

    /// Counts `rect`, where the request moved an element, as occupied, so
    /// that no later placement of the request overlaps it. The cells the
    /// element left stay counted as occupied.
    pub fn occupy(&mut self, rect: &Rect) {
        for (number, cell) in self.cells.iter_mut().enumerate() {
            let taken = Cell::taken_by(number, |other| rect.overlaps(other));
            cell.box_taken |= taken.box_taken;
            cell.area_taken |= taken.area_taken;
        }
        self.raise_bottom(rect);
    }

    /// The first cell whose corner a box of `size` can lie at. A box within one
    /// cell needs only that place free; a larger one lies over the gaps of
    /// every cell it spans, and all of it must stay inside the grid.
    fn first_fit(&self, size: Size) -> Option<usize> {
        let columns = span(size.width, CELL.width);
        let rows = span(size.height, CELL.height);
        if columns == 1 && rows == 1 {
            return self.cells.iter().position(|cell| !cell.box_taken);
        }
        (0..self.cells.len()).find(|cell| {
            columns <= COLUMNS - cell % COLUMNS
                && rows <= ROWS - cell / COLUMNS
                && (0..rows)
                    .flat_map(|row| (0..columns).map(move |column| row * COLUMNS + column))
                    .all(|offset| {
                        self.cells
                            .get(cell + offset)
                            .is_some_and(|spanned| !spanned.area_taken)
                    })
        })
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

/// How many cells a box of `length` spans, when `pitch` is the distance
/// between two cells and a gap follows the box.
fn span(length: NonZeroU32, pitch: NonZeroU32) -> usize {
    let cells = (u64::from(length.get()) + u64::from(GAP)).div_ceil(u64::from(pitch.get()));
    usize::try_from(cells).unwrap_or(usize::MAX)
}

/// A rectangle of `size` at the corner of grid cell `cell`, counted row by
/// row, if it fits in `i32`, which every corner of the grid does.
fn cell_rect(cell: usize, size: Size) -> Option<Rect> {
    let column = i64::try_from(cell % COLUMNS).ok()?;
    let row = i64::try_from(cell / COLUMNS).ok()?;
    let x = i32::try_from(column * i64::from(CELL.width.get())).ok()?;
    let y = i32::try_from(row * i64::from(CELL.height.get())).ok()?;
    Some(size.at(x, y))
}

/// A rectangle of `size` below everything, or at the top of an empty board, if
/// it fits in `i32`.
fn below(lowest_bottom: Option<i64>, size: Size) -> Option<Rect> {
    let y = lowest_bottom.map_or(0, |lowest| lowest + i64::from(GAP));
    let rect = size.at(0, i32::try_from(y).ok()?);
    (rect.bottom() <= i64::from(i32::MAX)).then_some(rect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use abyme_model::{BoardId, ElementId, Placement};
    use proptest::prelude::*;

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
                .map(|(id, rect)| {
                    (
                        id.parse::<ElementId>().unwrap(),
                        Placement {
                            rect: *rect,
                            pinned: false,
                            attached_to: None,
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        }
    }

    #[test]
    fn places_the_first_element_at_the_origin() {
        let mut space = FreeSpace::of(&board(&[]));

        assert_eq!(space.place(Size::DEFAULT), Some(rect(0, 0, 180, 90)));
    }

    #[test]
    fn places_one_element_after_another_without_overlap() {
        let mut space = FreeSpace::of(&board(&[]));

        let first = space.place(Size::DEFAULT).unwrap();
        let second = space.place(Size::DEFAULT).unwrap();

        assert_eq!(second, rect(220, 0, 180, 90));
        assert!(!first.overlaps(&second));
    }

    #[test]
    fn skips_cells_that_existing_placements_cover() {
        let mut space = FreeSpace::of(&board(&[("a", rect(100, 50, 200, 20))]));

        let placed = space.place(Size::DEFAULT).unwrap();

        assert_eq!(placed, rect(440, 0, 180, 90));
    }

    #[test]
    fn places_below_everything_when_the_grid_is_full() {
        let mut space = FreeSpace::of(&board(&[("wall", rect(-10, -10, 1_000, 9_000))]));

        let placed = space.place(Size::DEFAULT).unwrap();

        assert_eq!(placed, rect(0, 9_030, 180, 90));
    }

    #[test]
    fn reports_a_full_board_instead_of_overflowing() {
        let mut space = FreeSpace::of(&board(&[
            ("huge", rect(i32::MIN, i32::MIN, u32::MAX, u32::MAX)),
            ("low", rect(0, i32::MAX - 10, 1, 1)),
        ]));

        assert_eq!(space.place(Size::DEFAULT), None);
    }

    #[test]
    fn reports_a_full_board_when_only_the_box_below_would_overflow() {
        let mut space = FreeSpace::of(&board(&[
            ("wall", rect(-10, -10, 1_000, 9_000)),
            ("low", rect(0, i32::MAX - 100, 1, 1)),
        ]));

        assert_eq!(space.place(Size::DEFAULT), None);
    }

    fn sized(width: u32, height: u32) -> Size {
        Size {
            width: NonZeroU32::new(width).unwrap(),
            height: NonZeroU32::new(height).unwrap(),
        }
    }

    #[test]
    fn a_tall_box_takes_a_cell_whose_column_has_enough_free_rows() {
        let mut space = FreeSpace::of(&board(&[("below", rect(0, 130, 180, 90))]));

        assert_eq!(space.place(sized(180, 200)), Some(rect(220, 0, 180, 200)));
    }

    #[test]
    fn a_box_of_one_row_still_fits_above_a_taken_row() {
        let mut space = FreeSpace::of(&board(&[("below", rect(0, 130, 180, 90))]));

        assert_eq!(space.place(sized(180, 90)), Some(rect(0, 0, 180, 90)));
    }

    #[test]
    fn a_tall_box_does_not_lie_over_a_placement_in_the_gap_between_two_rows() {
        let in_the_gap = rect(0, 95, 10, 5);
        let mut space = FreeSpace::of(&board(&[("gap", in_the_gap)]));

        let placed = space.place(sized(180, 200)).unwrap();

        assert_eq!(placed, rect(220, 0, 180, 200));
        assert_eq!(space.place(Size::DEFAULT), Some(rect(0, 0, 180, 90)));
    }

    #[test]
    fn a_box_after_a_tall_one_goes_around_it() {
        let mut space = FreeSpace::of(&board(&[]));

        let tall = space.place(sized(180, 300)).unwrap();
        let next = space.place(Size::DEFAULT).unwrap();

        assert_eq!(tall, rect(0, 0, 180, 300));
        assert_eq!(next, rect(220, 0, 180, 90));
    }

    #[test]
    fn a_tall_box_does_not_reach_past_the_last_row_of_the_grid() {
        // Only the last row is free, which a box of two rows cannot fit in.
        let mut space = FreeSpace::of(&board(&[("wall", rect(-10, -10, 1_000, 8_200))]));

        assert_eq!(space.place(sized(180, 200)), Some(rect(0, 8_230, 180, 200)));
        assert_eq!(space.place(Size::DEFAULT), Some(rect(220, 8_190, 180, 90)));
    }

    #[test]
    fn a_box_too_large_for_the_grid_lies_at_the_top_of_an_empty_board() {
        let mut space = FreeSpace::of(&board(&[]));

        assert_eq!(
            space.place(sized(180, 20_000)),
            Some(rect(0, 0, 180, 20_000))
        );
    }

    proptest! {
        #[test]
        fn no_box_of_any_size_overlaps_anything(
            placed in prop::collection::vec(
                (-500..3_000_i32, -500..3_000_i32, 1..600_u32, 1..600_u32),
                0..12,
            ),
            sizes in prop::collection::vec((1..700_u32, 1..700_u32), 1..12),
        ) {
            let placements: Vec<(String, Rect)> = placed
                .iter()
                .enumerate()
                .map(|(index, (x, y, width, height))| {
                    (format!("e{index}"), rect(*x, *y, *width, *height))
                })
                .collect();
            let borrowed: Vec<(&str, Rect)> = placements
                .iter()
                .map(|(id, rect)| (id.as_str(), *rect))
                .collect();
            let mut space = FreeSpace::of(&board(&borrowed));
            let mut taken: Vec<Rect> = placements.iter().map(|(_, rect)| *rect).collect();

            for (width, height) in sizes {
                let new = space.place(sized(width, height)).unwrap();

                prop_assert_eq!(
                    (new.width.get(), new.height.get()),
                    (width, height)
                );
                prop_assert!(taken.iter().all(|old| !old.overlaps(&new)));
                taken.push(new);
            }
        }
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
        let mut space = FreeSpace::of(&board(&borrowed));

        let placed: Vec<Option<Rect>> = (0..100).map(|_| space.place(Size::DEFAULT)).collect();

        let all: Vec<Rect> = placed.iter().flatten().copied().collect();
        assert_eq!(all.len(), 100);
        for (index, new) in all.iter().enumerate() {
            assert!(placements.iter().all(|(_, old)| !old.overlaps(new)));
            assert!(all.iter().skip(index + 1).all(|other| !other.overlaps(new)));
        }
    }
}
