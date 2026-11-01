//! Arranging a board as a layered diagram from its elements and relations.
//!
//! Pure, and dependent only on IDs, sizes, relations, and pins, so a repeated
//! layout changes no byte. Its input comes from files and requests, so every
//! phase is iterative and at most linear in what [`LayoutWork`] has paid for.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use abyme_model::{Board, Element, ElementId, Rect, RelationKind};

use super::Size;
use super::box_size::box_size;
use crate::operation::MAX_COORDINATE;

#[cfg(test)]
mod tests;

/// What all the layouts of one request may cost together: a box, rebuilding the
/// free space for a box after the layout, a relation of an element on the board,
/// [`TEXT_BYTES_PER_WORK`] bytes of text to size, or a pin to check for one row
/// each count as one.
const MAX_LAYOUT_WORK: u64 = 4_000_000;
/// How many bytes of text cost one unit of work.
const TEXT_BYTES_PER_WORK: usize = 64;
/// The space between two boxes of a row, two rows, and two parts.
const GAP: i64 = 40;
/// How often the order within the ranks is improved, alternating downward and
/// upward.
const SWEEPS: usize = 4;
/// The columns of the grid of boxes without relations.
const GRID_COLUMNS: usize = 4;

/// Why a board cannot be laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// The request's layouts would cost more than they may.
    TooLarge,
    /// A box would lie outside the range a move accepts.
    OutOfRange,
}

/// What the layouts of one request may still cost.
#[derive(Clone, Copy)]
pub struct LayoutWork {
    left: u64,
}

impl LayoutWork {
    /// The work one request may spend.
    pub const fn for_a_request() -> Self {
        Self {
            left: MAX_LAYOUT_WORK,
        }
    }

    /// Pays for work before it is done.
    fn spend(&mut self, amount: u64) -> Result<(), Problem> {
        self.left = self.left.checked_sub(amount).ok_or(Problem::TooLarge)?;
        Ok(())
    }
}

/// A board as a graph, each box numbered by its place in the board's
/// placements, which is the order of the element IDs.
struct Graph {
    /// The size each free box gets; a pinned box keeps its own, so its entry
    /// goes unread.
    sizes: Vec<Size>,
    /// Whether each box is pinned.
    pinned: Vec<bool>,
    /// For each box, the targets of its relations that rank it above them.
    above: Vec<Vec<usize>>,
    /// For each box, every box a relation links it to, in either direction.
    linked: Vec<Vec<usize>>,
}

impl Graph {
    /// The boxes linked to `node`.
    fn linked_to(&self, node: usize) -> &[usize] {
        self.linked.get(node).map_or(&[], Vec::as_slice)
    }

    /// Whether `node` is a box a layout may place.
    fn is_free(&self, node: usize) -> bool {
        self.pinned.get(node).is_some_and(|pinned| !pinned)
    }

    /// The size of `node`.
    fn size(&self, node: usize) -> Size {
        self.sizes.get(node).copied().unwrap_or(Size::DEFAULT)
    }
}

/// The rectangle of each box of `board` in the order of its placements, `None`
/// for a pinned box, which keeps its own. `element` looks up an element as the
/// request stands.
///
/// # Errors
///
/// Returns [`Problem::TooLarge`] when `work` does not cover the layout, and
/// [`Problem::OutOfRange`] when a box would lie outside the range of a move.
pub fn layered<'a>(
    board: &Board,
    element: impl Fn(&ElementId) -> Option<&'a Element>,
    work: &mut LayoutWork,
) -> Result<Vec<Option<Rect>>, Problem> {
    // The caller's rebuild of the free space checks every box against each cell.
    work.spend(u64::try_from(board.placements.len()).unwrap_or(u64::MAX))?;
    let graph = read_graph(board, element, work)?;
    let ranks = ranks(&graph.above);
    let (parts, loose) = parts(&graph);
    let mut part_rows: Vec<Vec<Vec<usize>>> =
        parts.iter().map(|part| rows_of(part, &ranks)).collect();
    let grid: Vec<&[usize]> = loose.chunks(GRID_COLUMNS).collect();
    check_row_count(&graph, &part_rows)?;
    let row_count = part_rows.iter().map(Vec::len).sum::<usize>() + grid.len();
    let pins = pins_by_left_edge(board);
    work.spend(u64::try_from(row_count.saturating_mul(pins.len())).unwrap_or(u64::MAX))?;

    let mut position = vec![0; graph.sizes.len()];
    for rows in &mut part_rows {
        improve_order(rows, &graph, &ranks, &mut position);
    }

    let mut placer = Placer {
        graph: &graph,
        pins: &pins,
        placed: vec![None; graph.sizes.len()],
    };
    let mut left = 0;
    let mut lowest = None;
    for rows in &part_rows {
        let extent = placer.place_part(rows, left)?;
        if let Some((right, bottom)) = extent {
            left = right + GAP;
            lowest = Some(lowest.map_or(bottom, |lowest: i64| lowest.max(bottom)));
        }
    }
    placer.place_grid(&grid, lowest.map_or(0, |lowest| lowest + GAP))?;
    Ok(placer.placed)
}

/// Reads the sizes and relations of the boxes on `board`, paying for them first.
fn read_graph<'a>(
    board: &Board,
    element: impl Fn(&ElementId) -> Option<&'a Element>,
    work: &mut LayoutWork,
) -> Result<Graph, Problem> {
    let ids: Vec<&ElementId> = board.placements.keys().collect();
    let mut graph = Graph {
        sizes: Vec::with_capacity(ids.len()),
        pinned: board.placements.values().map(|p| p.pinned).collect(),
        above: vec![Vec::new(); ids.len()],
        linked: vec![Vec::new(); ids.len()],
    };
    for (node, (id, placement)) in board.placements.iter().enumerate() {
        let found = element(id);
        let size = if let Some(shown) = found.filter(|_| !placement.pinned) {
            work.spend(text_work(shown))?;
            box_size(shown.type_ref().as_str(), shown.name(), shown.summary())
        } else {
            work.spend(1)?;
            Size::DEFAULT
        };
        graph.sizes.push(size);
        if let Some(found) = found {
            read_relations(found, node, &ids, &mut graph, work)?;
        }
    }
    for targets in graph.above.iter_mut().chain(&mut graph.linked) {
        targets.sort_unstable();
        targets.dedup();
    }
    Ok(graph)
}

/// What sizing the box of `element` costs.
fn text_work(element: &Element) -> u64 {
    let bytes = element.type_ref().as_str().len()
        + element.name().len()
        + element.summary().map_or(0, str::len);
    1 + u64::try_from(bytes / TEXT_BYTES_PER_WORK).unwrap_or(u64::MAX)
}

/// Enters the relations of `element`, box number `node`, paying for each, also
/// for one to an element not on the board, which counts for nothing else.
fn read_relations(
    element: &Element,
    node: usize,
    ids: &[&ElementId],
    graph: &mut Graph,
    work: &mut LayoutWork,
) -> Result<(), Problem> {
    for (kind, to) in element.relations().keys() {
        work.spend(1)?;
        let Ok(target) = ids.binary_search(&to) else {
            continue;
        };
        if target == node {
            continue;
        }
        match kind {
            RelationKind::Concerns
            | RelationKind::DependsOn
            | RelationKind::Describes
            | RelationKind::Uses => {
                if let Some(targets) = graph.above.get_mut(node) {
                    targets.push(target);
                }
            }
            RelationKind::SeeAlso => {}
        }
        if let Some(links) = graph.linked.get_mut(node) {
            links.push(target);
        }
        if let Some(links) = graph.linked.get_mut(target) {
            links.push(node);
        }
    }
    Ok(())
}

/// The rank of each box, 0 at the top: a source lies above its targets. A walk
/// over the boxes in ID order leaves out each relation that would close a
/// cycle, which is one whose target it finishes after its source.
fn ranks(above: &[Vec<usize>]) -> Vec<usize> {
    let finished = finishing_order(above);
    let mut finished_at = vec![0; above.len()];
    for (at, node) in finished.iter().enumerate() {
        if let Some(slot) = finished_at.get_mut(*node) {
            *slot = at;
        }
    }
    let mut ranks = vec![0; above.len()];
    for source in finished.iter().rev() {
        let rank = ranks.get(*source).copied().unwrap_or(0);
        let source_at = finished_at.get(*source).copied().unwrap_or(0);
        for target in above.get(*source).map_or(&[][..], Vec::as_slice) {
            if finished_at.get(*target).copied().unwrap_or(0) < source_at
                && let Some(slot) = ranks.get_mut(*target)
            {
                *slot = (*slot).max(rank + 1);
            }
        }
    }
    ranks
}

/// The boxes in the order a depth-first walk in ID order finishes them, with
/// its own stack, so that a long chain cannot overflow the thread's.
fn finishing_order(above: &[Vec<usize>]) -> Vec<usize> {
    let mut visited = vec![false; above.len()];
    let mut finished = Vec::with_capacity(above.len());
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for start in 0..above.len() {
        if visited.get(start).copied().unwrap_or(true) {
            continue;
        }
        if let Some(slot) = visited.get_mut(start) {
            *slot = true;
        }
        stack.push((start, 0));
        while let Some((node, next)) = stack.pop() {
            let target = above.get(node).and_then(|targets| targets.get(next));
            match target {
                Some(target) => {
                    stack.push((node, next + 1));
                    if let Some(slot) = visited.get_mut(*target)
                        && !*slot
                    {
                        *slot = true;
                        stack.push((*target, 0));
                    }
                }
                None => finished.push(node),
            }
        }
    }
    finished
}

/// The connected parts of more than one box, ordered by their smallest ID, and
/// the free boxes without relations, in ID order.
fn parts(graph: &Graph) -> (Vec<Vec<usize>>, Vec<usize>) {
    let mut seen = vec![false; graph.sizes.len()];
    let mut parts = Vec::new();
    let mut loose = Vec::new();
    for start in 0..graph.sizes.len() {
        if graph.linked_to(start).is_empty() {
            if graph.is_free(start) {
                loose.push(start);
            }
            continue;
        }
        if seen.get(start).copied().unwrap_or(true) {
            continue;
        }
        if let Some(slot) = seen.get_mut(start) {
            *slot = true;
        }
        let mut part = vec![start];
        let mut pending = vec![start];
        while let Some(node) = pending.pop() {
            for next in graph.linked_to(node) {
                if let Some(slot) = seen.get_mut(*next)
                    && !*slot
                {
                    *slot = true;
                    part.push(*next);
                    pending.push(*next);
                }
            }
        }
        part.sort_unstable();
        parts.push(part);
    }
    (parts, loose)
}

/// The boxes of `part` by rank, each row in ID order.
fn rows_of(part: &[usize], ranks: &[usize]) -> Vec<Vec<usize>> {
    let mut rows: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for node in part {
        rows.entry(ranks.get(*node).copied().unwrap_or(0))
            .or_default()
            .push(*node);
    }
    rows.into_values().collect()
}

/// Refuses a part whose rows of free boxes cannot fit the range, before the
/// sweeps run on it.
fn check_row_count(graph: &Graph, part_rows: &[Vec<Vec<usize>>]) -> Result<(), Problem> {
    let shortest_row = i64::from(Size::DEFAULT.height.get()) + GAP;
    let fits = part_rows.iter().all(|rows| {
        let free = rows
            .iter()
            .filter(|row| row.iter().any(|node| graph.is_free(*node)))
            .count();
        i64::try_from(free).is_ok_and(|free| free <= i64::from(MAX_COORDINATE) / shortest_row + 1)
    });
    if fits {
        Ok(())
    } else {
        Err(Problem::OutOfRange)
    }
}

/// Improves the order within the rows by barycenters: each box moves toward the
/// average position of the boxes linked to it in the rows already swept, ties by
/// ID. A `see_also` counts here and nowhere else.
fn improve_order(rows: &mut [Vec<usize>], graph: &Graph, ranks: &[usize], position: &mut [usize]) {
    for row in rows.iter() {
        record_positions(row, position);
    }
    for sweep in 0..SWEEPS {
        let downward = sweep % 2 == 0;
        let row_count = rows.len();
        for index in 0..row_count {
            let at = if downward {
                index
            } else {
                row_count - 1 - index
            };
            if let Some(row) = rows.get_mut(at) {
                sort_row(row, graph, ranks, position, downward);
                record_positions(row, position);
            }
        }
    }
}

/// Notes where each box of `row` stands in it.
fn record_positions(row: &[usize], position: &mut [usize]) {
    for (at, node) in row.iter().enumerate() {
        if let Some(slot) = position.get_mut(*node) {
            *slot = at;
        }
    }
}

/// Sorts `row` by the average position of the boxes linked to each from the
/// direction the sweep comes from, or from their own row. A box with no such
/// link keeps its place as its average.
fn sort_row(row: &mut [usize], graph: &Graph, ranks: &[usize], position: &[usize], downward: bool) {
    let at = |node: usize| position.get(node).copied().unwrap_or(0);
    let rank = |node: usize| ranks.get(node).copied().unwrap_or(0);
    let mut keyed: Vec<(usize, u128, u128)> = row
        .iter()
        .map(|node| {
            let (sum, count) = graph
                .linked_to(*node)
                .iter()
                .filter(|other| {
                    if downward {
                        rank(**other) <= rank(*node)
                    } else {
                        rank(**other) >= rank(*node)
                    }
                })
                .fold((0, 0), |(sum, count), other| {
                    (sum + widened(at(*other)), count + 1)
                });
            if count == 0 {
                (*node, widened(at(*node)), 1)
            } else {
                (*node, sum, count)
            }
        })
        .collect();
    keyed.sort_unstable_by(|left, right| by_average(*left, *right));
    for (slot, (node, _, _)) in row.iter_mut().zip(keyed) {
        *slot = node;
    }
}

/// `value` in the type that holds the cross products of [`by_average`].
fn widened(value: usize) -> u128 {
    u128::try_from(value).unwrap_or(u128::MAX >> 64)
}

/// Compares two averages by integers only, ties by ID. Sums and counts are
/// bounded by the number of boxes and links, so the products fit `u128`.
fn by_average(left: (usize, u128, u128), right: (usize, u128, u128)) -> Ordering {
    (left.1 * right.2)
        .cmp(&(right.1 * left.2))
        .then(left.0.cmp(&right.0))
}

/// A pinned box's edges, in the arithmetic of the layout.
#[derive(Clone, Copy)]
struct Pin {
    left: i64,
    right: i64,
    top: i64,
    bottom: i64,
}

/// The pinned boxes of `board`, sorted once by their left edge, so that every
/// row can filter them and stay sorted.
fn pins_by_left_edge(board: &Board) -> Vec<Pin> {
    let mut pins: Vec<Pin> = board
        .placements
        .values()
        .filter(|placement| placement.pinned)
        .map(|placement| Pin {
            left: i64::from(placement.rect.x),
            right: placement.rect.right(),
            top: i64::from(placement.rect.y),
            bottom: placement.rect.bottom(),
        })
        .collect();
    pins.sort_unstable_by_key(|pin| pin.left);
    pins
}

/// Places the free boxes of a layout row by row. Rows lie in separate bands and
/// parts side by side, so a row checks only the pins that reach into its band.
struct Placer<'a> {
    graph: &'a Graph,
    pins: &'a [Pin],
    placed: Vec<Option<Rect>>,
}

impl Placer<'_> {
    /// Places a part with its left edge at `left`, each row centered in the
    /// widest. Returns its right and bottom edge, if it has a free box.
    fn place_part(
        &mut self,
        rows: &[Vec<usize>],
        left: i64,
    ) -> Result<Option<(i64, i64)>, Problem> {
        let width = rows
            .iter()
            .map(|row| self.row_width(row))
            .max()
            .unwrap_or(0);
        let mut top = 0;
        let mut extent = None;
        for row in rows {
            let start = left + (width - self.row_width(row)) / 2;
            if let Some((right, bottom)) = self.place_row(row, start, top)? {
                top = bottom + GAP;
                extent = Some(extent.map_or((right, bottom), |(far, low): (i64, i64)| {
                    (far.max(right), low.max(bottom))
                }));
            }
        }
        Ok(extent)
    }

    /// Places the boxes without relations in rows from `top` down.
    fn place_grid(&mut self, grid: &[&[usize]], top: i64) -> Result<(), Problem> {
        let mut top = top;
        for row in grid {
            if let Some((_, bottom)) = self.place_row(row, 0, top)? {
                top = bottom + GAP;
            }
        }
        Ok(())
    }

    /// The width of the free boxes of `row` with the gaps between them.
    fn row_width(&self, row: &[usize]) -> i64 {
        let free = row.iter().filter(|node| self.graph.is_free(**node));
        let (widths, count) = free.fold((0, 0), |(widths, count), node| {
            (
                widths + i64::from(self.graph.size(*node).width.get()),
                count + 1,
            )
        });
        widths + GAP * (count - 1_i64).max(0)
    }

    /// Places the free boxes of `row` from `start` to the right at `top`, each
    /// past the pins it would overlap. The row runs only to the right, so one
    /// pass over the merged pins of its band is enough. Returns its right and
    /// bottom edge, if it has a free box.
    fn place_row(
        &mut self,
        row: &[usize],
        start: i64,
        top: i64,
    ) -> Result<Option<(i64, i64)>, Problem> {
        let graph = self.graph;
        let free: Vec<usize> = row
            .iter()
            .copied()
            .filter(|node| graph.is_free(*node))
            .collect();
        let Some(height) = free
            .iter()
            .map(|node| i64::from(graph.size(*node).height.get()))
            .max()
        else {
            return Ok(None);
        };
        let bottom = top + height;
        let merged = merged_pins(self.pins, top, bottom);
        let mut intervals = merged.iter().copied().peekable();
        let mut next = start;
        let mut right = start;
        for node in free {
            let size = graph.size(node);
            let width = i64::from(size.width.get());
            let mut left = next;
            while let Some((from, to)) = intervals.peek().copied() {
                if from >= left + width {
                    break;
                }
                if to > left {
                    left = to + GAP;
                }
                intervals.next();
            }
            let rect = rect_in_range(size, left, top)?;
            if let Some(slot) = self.placed.get_mut(node) {
                *slot = Some(rect);
            }
            right = left + width;
            next = right + GAP;
        }
        Ok(Some((right, bottom)))
    }
}

/// The edges of the pins that reach into the band from `top` to `bottom`, merged
/// where they overlap, from left to right.
fn merged_pins(pins: &[Pin], top: i64, bottom: i64) -> Vec<(i64, i64)> {
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for pin in pins
        .iter()
        .filter(|pin| pin.top < bottom && pin.bottom > top)
    {
        match merged.last_mut() {
            Some((_, to)) if pin.left <= *to => *to = (*to).max(pin.right),
            Some(_) | None => merged.push((pin.left, pin.right)),
        }
    }
    merged
}

/// A rectangle of `size` at `left`, `top`, if all of it lies in the range of a
/// move.
fn rect_in_range(size: Size, left: i64, top: i64) -> Result<Rect, Problem> {
    let limit = i64::from(MAX_COORDINATE);
    let x = i32::try_from(left).map_err(|_| Problem::OutOfRange)?;
    let y = i32::try_from(top).map_err(|_| Problem::OutOfRange)?;
    let rect = size.at(x, y);
    let fits = left >= -limit && top >= -limit && rect.right() <= limit && rect.bottom() <= limit;
    fits.then_some(rect).ok_or(Problem::OutOfRange)
}
