//! The detail levels as the AI reads them: all boards as an overview, a
//! board as a compact text outline, and one element in detail.
//!
//! Titles, types, names, summaries, anchors, relation labels, and
//! descriptions come from files that a clone or a pull request can bring
//! in. Every value but the description is written on one line, and every
//! character that could break a line or hide or reorder text is escaped;
//! the description keeps its lines and tabs.

use std::collections::BTreeMap;

use abyme_model::{
    Board, BoardId, Element, ElementId, Rect, RelationKind, StatementKind, Zone,
    hides_or_breaks_text,
};

use crate::apply::Snapshot;
use crate::board_tree::BoardTree;
use crate::review::{Flag, Priority, Review};
use crate::statement::kind_of;

/// The widest a column is padded to, in characters. A longer value runs past
/// its column, so one long name cannot pad every other line.
const MAX_COLUMN_WIDTH: usize = 40;

/// What the outline shows for a placement whose element file is missing,
/// and for an owner whose element file is missing.
const MISSING_MARKER: &str = "⚠ missing";

/// What the outline shows for an owner, with a file, that no board places.
const NO_HOME_MARKER: &str = "⚠ no home";

/// What the outline shows for a board that lies inside itself.
const CYCLE_MARKER: &str = "⚠ cycle";

/// What an element line shows before the IDs of the boards the element owns.
const INNER_BOARDS_MARKER: &str = "▸";

/// What a statement kind column shows for an element without one, so that
/// no empty cell lets the name beside it pass for a kind.
const NO_KIND: &str = "none";

/// What the zone column of the overview shows for a board that is no zone.
const NO_ZONE: &str = "no zone";

/// When element lines show the statement kind column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindColumn {
    /// When one of the listed elements has a statement kind.
    WhenAnyHasOne,
    /// Always: on the outline of a zone, lines without the column would read
    /// as if they took the zone's kind.
    Always,
}

/// How many changed files a review flag names at most.
const SHOWN_CHANGED_FILES: usize = 10;

/// How many elements [`status_report`] lists at most.
const STATUS_LINES: usize = 50;

/// How many boards [`overview`] lists at most.
const OVERVIEW_LINES: usize = 50;

/// How many characters of a board's title [`overview`] shows at most.
/// Cutting titles and summaries keeps one long text from making the whole
/// overview too large to answer.
const OVERVIEW_TITLE_CHARS: usize = 80;

/// How many characters of a board's summary [`overview`] shows at most.
const OVERVIEW_SUMMARY_CHARS: usize = 160;

/// Whether an outline shows where each element lies on its board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Geometry {
    /// The lines show no position and size.
    Hidden,
    /// Each line shows the element's position and size right after its ID,
    /// as `@x,y WIDTHxHEIGHT`.
    Shown,
}

/// No board has the ID the outline was asked for.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no board has the ID {0}")]
pub struct UnknownBoard(pub BoardId);

/// No element file has the ID the detail was asked for.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no element has the ID {0}")]
pub struct UnknownElement(pub ElementId);

/// Renders all boards as an overview.
///
/// One line per board, depth first as [`BoardTree::depth_first`] orders
/// them, each level indented two spaces further: the board's ID and title;
/// the zone as `<mode> · <kind>`, or `no zone`, in a column shown only when
/// one of the boards listed is a zone; the summary; title and summary each
/// cut after [`OVERVIEW_TITLE_CHARS`] and
/// [`OVERVIEW_SUMMARY_CHARS`] characters and then ending in `…`; how many
/// elements with a file have their home on it; given a `review`, how many
/// of those have a task and how many only a hint, as `⚠ 2 tasks, 1 hint`;
/// and for a top-level board with an owner, the line
/// [`outline`] names the owner with, markers included. Empty columns at the
/// end of a line are left out. At most [`OVERVIEW_LINES`] boards are listed,
/// then how many more there are. Without boards the overview is
/// `No boards.`
#[must_use]
pub fn overview(snapshot: &Snapshot, review: Option<&Review>) -> String {
    let tree = BoardTree::of(&snapshot.boards);
    let order = tree.depth_first(|owner| snapshot.elements.contains_key(owner));
    if order.is_empty() {
        return "No boards.".to_owned();
    }
    let own = own_elements(snapshot, &tree, review);
    // Only the boards shown are rendered, so no line costs a walk or a width
    // beyond them.
    let shown: Vec<(usize, &Board)> = order
        .iter()
        .take(OVERVIEW_LINES)
        .filter_map(|&(depth, id)| Some((depth, snapshot.boards.get(id)?)))
        .collect();
    let shows_zones = shown.iter().any(|(_, board)| board.zone.is_some());
    let rows: Vec<Vec<String>> = shown
        .into_iter()
        .map(|(depth, board)| {
            let id = &board.id;
            let counts = own.get(id).copied().unwrap_or_default();
            let zone =
                shows_zones.then(|| board.zone.map_or_else(|| NO_ZONE.to_owned(), zone_text));
            let mut columns: Vec<String> = [
                Some(format!("{}{id}", "  ".repeat(depth))),
                Some(one_line(&cut(&board.title, OVERVIEW_TITLE_CHARS))),
                zone,
                Some(
                    board
                        .summary
                        .as_deref()
                        .map(|summary| one_line(&cut(summary, OVERVIEW_SUMMARY_CHARS)))
                        .unwrap_or_default(),
                ),
                Some(counted(counts.elements, "element")),
                Some(
                    review
                        .map(|_| flag_counts(counts.tasks, counts.hints))
                        .unwrap_or_default(),
                ),
                Some(
                    tree.owner(id)
                        .filter(|_| depth == 0)
                        .map(|owner| owner_line(snapshot, &tree, id, owner))
                        .unwrap_or_default(),
                ),
            ]
            .into_iter()
            .flatten()
            .collect();
            drop_empty_tail(&mut columns);
            columns
        })
        .collect();
    let more = order.len().saturating_sub(OVERVIEW_LINES);
    let rest = match more {
        0 => String::new(),
        1 => "… and 1 more board\n".to_owned(),
        _ => format!("… and {more} more boards\n"),
    };
    format!("{}{rest}", lines(&rows))
}

/// `text` up to `limit` characters, and `…` after it if it is longer. It is
/// cut before it is escaped, so no escape is ever cut in two.
fn cut(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((end, _)) => format!("{}…", text.get(..end).unwrap_or(text)),
        None => text.to_owned(),
    }
}

/// What [`overview`] counts of the elements whose home is one board.
#[derive(Debug, Default, Clone, Copy)]
struct OwnElements {
    elements: usize,
    tasks: usize,
    hints: usize,
}

/// The elements with a file counted by their home, and given a `review`,
/// by the highest priority of their flags.
fn own_elements<'a>(
    snapshot: &Snapshot,
    tree: &'a BoardTree,
    review: Option<&Review>,
) -> BTreeMap<&'a BoardId, OwnElements> {
    snapshot
        .elements
        .keys()
        .filter_map(|id| tree.home(id).map(|home| (home, id)))
        .fold(BTreeMap::new(), |mut counts, (home, id)| {
            let own: &mut OwnElements = counts.entry(home).or_default();
            own.elements += 1;
            match review.and_then(|review| review.priority(id)) {
                Some(Priority::Task) => own.tasks += 1,
                Some(Priority::Hint) => own.hints += 1,
                None => {}
            }
            counts
        })
}

/// `⚠` and the tasks and hints that are not zero, such as `⚠ 2 tasks,
/// 1 hint`, or nothing.
fn flag_counts(tasks: usize, hints: usize) -> String {
    let counts: Vec<String> = [(tasks, "task"), (hints, "hint")]
        .into_iter()
        .filter(|&(count, _)| count > 0)
        .map(|(count, noun)| counted(count, noun))
        .collect();
    if counts.is_empty() {
        String::new()
    } else {
        format!("⚠ {}", counts.join(", "))
    }
}

/// `count` and `noun`, plural unless the count is one.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Leaves out the empty columns at the end of `columns`.
fn drop_empty_tail(columns: &mut Vec<String>) {
    while columns.last().is_some_and(String::is_empty) {
        columns.pop();
    }
}

/// Renders `board` as an outline.
///
/// The outline has a header line with the board's ID and title; for a board
/// with an owner, a line `Inside of:` with the owner and, if a board places
/// the owner, `Above:` with the owner's home (see [`owner_line`] for the
/// markers); for a zone, a line `Zone: <mode> · <kind>`; for a board with
/// a summary, a line `Summary:` with it. A blank line follows, and one line
/// per placed element, ordered by ID, as [`element_list`] renders it; on a
/// zone, the lines always show the statement kind column, each with the
/// element's own kind. A placement whose element is missing shows
/// its ID and `⚠ missing`. With [`Geometry::Shown`], each line also shows
/// the placement's position and size from the board file, right after the
/// ID.
///
/// If relations connect elements on the board, a blank line and one line
/// per relation follow: `<from> → <to>`, the kind, and the label, if any.
/// A relation is shown when its source's file exists and both ends lie on
/// the board; the lines come by source, then in the order of the source's
/// relations.
///
/// # Errors
///
/// Returns [`UnknownBoard`] when the snapshot has no such board.
pub fn outline(
    snapshot: &Snapshot,
    board: &BoardId,
    geometry: Geometry,
    review: Option<&Review>,
) -> Result<String, UnknownBoard> {
    let found = snapshot
        .boards
        .get(board)
        .ok_or_else(|| UnknownBoard(board.clone()))?;
    let tree = BoardTree::of(&snapshot.boards);
    let kind_column = if found.zone.is_some() {
        KindColumn::Always
    } else {
        KindColumn::WhenAnyHasOne
    };
    let rows: Vec<Vec<String>> = element_rows(
        snapshot,
        &tree,
        found.placements.keys(),
        review,
        kind_column,
    )
    .into_iter()
    .zip(found.placements.values())
    .map(|(mut columns, placement)| {
        if geometry == Geometry::Shown {
            columns.insert(1, position(&placement.rect));
        }
        columns
    })
    .collect();
    let relations = relation_rows(snapshot, found);
    // The relation lines are aligned on their own, so that they never change
    // the columns of the element lines.
    let relation_lines = if relations.is_empty() {
        String::new()
    } else {
        format!("\n{}", lines(&relations))
    };
    Ok(format!(
        "{}\n{}{relation_lines}",
        header(snapshot, &tree, found),
        lines(&rows)
    ))
}

/// Renders the elements with the given IDs as the outline's element lines.
///
/// The lines come in the given order, without a header: one line per ID,
/// each ending in a line feed, with the element's ID and type; its
/// statement kind, the kind of its home board's zone or `none`, in a column
/// that a list shows only when one of its elements has a kind; its name and
/// summary; `▸` and the IDs of the boards the element owns, in a column that
/// a list shows only when one of its elements owns a board; and, given a
/// `review`, `⚠ task` or `⚠ hint` for the highest priority of the element's
/// flags. Empty columns at the end of a line are left out. An ID whose
/// element is missing shows `⚠ missing`. An ID given twice is rendered
/// twice; limiting and deduplicating is the caller's business.
///
/// All element text is escaped onto its line already, so callers must not
/// escape it again.
pub fn element_list<'a>(
    snapshot: &Snapshot,
    ids: impl IntoIterator<Item = &'a ElementId>,
    review: Option<&Review>,
) -> String {
    let tree = BoardTree::of(&snapshot.boards);
    lines(&element_rows(
        snapshot,
        &tree,
        ids,
        review,
        KindColumn::WhenAnyHasOne,
    ))
}

/// The rows of the elements with the given IDs, in the given order, as
/// [`element_list`] describes them.
fn element_rows<'a>(
    snapshot: &Snapshot,
    tree: &BoardTree,
    ids: impl IntoIterator<Item = &'a ElementId>,
    review: Option<&Review>,
    kind_column: KindColumn,
) -> Vec<Vec<String>> {
    let elements: Vec<(&ElementId, Option<&Element>, Option<StatementKind>, String)> = ids
        .into_iter()
        .map(|id| {
            let element = snapshot.elements.get(id);
            let kind = element
                .and_then(|_| kind_of(snapshot, tree, id))
                .map(|(kind, _)| kind);
            (id, element, kind, inner_boards(tree, id))
        })
        .collect();
    let shows_kinds =
        kind_column == KindColumn::Always || elements.iter().any(|(_, _, kind, _)| kind.is_some());
    let shows_inner_boards = elements
        .iter()
        .any(|(_, element, _, inner)| element.is_some() && !inner.is_empty());
    elements
        .into_iter()
        .map(|(id, element, kind, inner)| {
            let kind = shows_kinds.then(|| kind.map_or(NO_KIND, StatementKind::as_str).to_owned());
            row(
                id,
                element,
                kind,
                shows_inner_boards.then_some(inner),
                review,
            )
        })
        .collect()
}

/// `▸` and the IDs of the boards `element` owns, or nothing if it owns none.
fn inner_boards(tree: &BoardTree, element: &ElementId) -> String {
    let owned: Vec<&str> = tree.owned_by(element).map(BoardId::as_str).collect();
    if owned.is_empty() {
        String::new()
    } else {
        format!("{INNER_BOARDS_MARKER} {}", owned.join(", "))
    }
}

/// Renders the open review flags of the project.
///
/// A line counts the elements by their highest priority; their lines
/// follow as in [`element_list`], those with a task first, each group by
/// ID, at most [`STATUS_LINES`] of them and then how many more there are.
/// Without flags the report is `No open review flags.` `review` must have
/// been computed from `snapshot`.
#[must_use]
pub fn status_report(snapshot: &Snapshot, review: &Review) -> String {
    let (tasks, hints): (Vec<&ElementId>, Vec<&ElementId>) = review
        .flagged()
        .partition(|id| review.priority(id) == Some(Priority::Task));
    if tasks.is_empty() && hints.is_empty() {
        return "No open review flags.".to_owned();
    }
    let more = (tasks.len() + hints.len()).saturating_sub(STATUS_LINES);
    let rest = if more > 0 {
        format!("… and {more} more\n")
    } else {
        String::new()
    };
    let shown = tasks.iter().chain(&hints).take(STATUS_LINES).copied();
    format!(
        "Elements to review: {} with a task, {} with only a hint.\n{}{rest}",
        tasks.len(),
        hints.len(),
        element_list(snapshot, shown, Some(review))
    )
}

/// Renders the element `id` in detail, in this order:
///
/// - `Element:` with its ID and name, `Type:`, `Statement kind:` with the
///   kind of its home board's zone and `from board` that board, or `none`,
///   `Anchor:` with its anchor or `none`, `Verified:` with the stamp's
///   commit and `by ai` or `by person`, or `none`, given a `review`,
///   `Review:` with the highest priority of the element's flags or `none`
///   and one indented line per flag, `Summary:` if it has one, `Boards:`
///   with every board that places it, by ID, or `none`, and `Inner board:`
///   with every board it owns, by ID, or `none`;
/// - after a blank line, `Outgoing relations:` with one line per relation
///   as in the outline, a target without a file marked `⚠ missing`, or
///   `none`;
/// - after a blank line, `Incoming relations:` with the relations of other
///   elements to it, by source, or `none`;
/// - after a blank line, `Description:` and the description.
///
/// Every value but the description is escaped onto one line. The
/// description comes last and keeps its line feeds, tabs, and backslashes;
/// a carriage return before a line feed is left out, and every other
/// character the outline escapes is escaped.
///
/// # Errors
///
/// Returns [`UnknownElement`] when the snapshot has no element file with
/// this ID, also when a board places the ID.
pub fn element_detail(
    snapshot: &Snapshot,
    id: &ElementId,
    review: Option<&Review>,
) -> Result<String, UnknownElement> {
    let element = snapshot
        .elements
        .get(id)
        .ok_or_else(|| UnknownElement(id.clone()))?;
    let anchor = element
        .anchor()
        .map_or_else(|| "none".to_owned(), |anchor| one_line(anchor.as_str()));
    let verified = element.verified().map_or_else(
        || "none".to_owned(),
        |stamp| format!("{} by {}", stamp.commit.as_str(), stamp.by.as_str()),
    );
    let summary = element
        .summary()
        .map(|summary| format!("Summary: {}\n", one_line(summary)))
        .unwrap_or_default();
    let boards: Vec<&str> = snapshot
        .boards
        .values()
        .filter(|board| board.placements.contains_key(id))
        .map(|board| board.id.as_str())
        .collect();
    let outgoing: Vec<Vec<String>> = element
        .relations()
        .iter()
        .map(|((kind, to), label)| {
            let target = if snapshot.elements.contains_key(to) {
                to.to_string()
            } else {
                format!("{to} {MISSING_MARKER}")
            };
            relation_row(id, &target, *kind, label.as_deref())
        })
        .collect();
    let incoming: Vec<Vec<String>> = snapshot
        .elements
        .values()
        .flat_map(|source| {
            source
                .relations()
                .iter()
                .filter(|((_, to), _)| to == id)
                .map(|((kind, _), label)| {
                    relation_row(source.id(), id.as_str(), *kind, label.as_deref())
                })
        })
        .collect();
    let tree = BoardTree::of(&snapshot.boards);
    let inner_boards: Vec<&str> = tree.owned_by(id).map(BoardId::as_str).collect();
    let statement = kind_of(snapshot, &tree, id).map_or_else(
        || NO_KIND.to_owned(),
        |(kind, home)| format!("{}, from board {home}", kind.as_str()),
    );
    Ok(format!(
        "Element: {id} · {}\nType: {}\nStatement kind: {statement}\nAnchor: {anchor}\n\
         Verified: {verified}\n{}\
         {summary}Boards: {}\nInner board: {}\n\n\
         Outgoing relations:\n{}\nIncoming relations:\n{}\nDescription:\n{}",
        one_line(element.name()),
        one_line(element.type_ref().as_str()),
        review
            .map(|review| review_lines(review, id))
            .unwrap_or_default(),
        ids_or_none(&boards),
        ids_or_none(&inner_boards),
        relation_lines(&outgoing),
        relation_lines(&incoming),
        multi_line(element.body()),
    ))
}

/// The IDs joined by commas, or `none`.
fn ids_or_none(ids: &[&str]) -> String {
    if ids.is_empty() {
        "none".to_owned()
    } else {
        ids.join(", ")
    }
}

/// The `Review:` line with the highest priority of the element's flags, or
/// `none`, and one indented line per flag.
fn review_lines(review: &Review, id: &ElementId) -> String {
    let priority = match review.priority(id) {
        Some(Priority::Task) => "task",
        Some(Priority::Hint) => "hint",
        None => "none",
    };
    let reasons: Vec<String> = review
        .flags(id)
        .iter()
        .map(|flag| format!("  {}\n", reason(flag)))
        .collect();
    format!("Review: {priority}\n{}", reasons.concat())
}

/// What a flag says, on one line; file names are escaped.
fn reason(flag: &Flag) -> String {
    match flag {
        Flag::NeverVerified => "never verified".to_owned(),
        Flag::UnknownStamp => {
            "the stamp's commit is unknown to the repository or no ancestor of HEAD".to_owned()
        }
        Flag::Changed(paths) => {
            let shown: Vec<String> = paths
                .iter()
                .take(SHOWN_CHANGED_FILES)
                .map(|path| one_line(&String::from_utf8_lossy(path)))
                .collect();
            let more = paths.len().saturating_sub(SHOWN_CHANGED_FILES);
            let rest = if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            };
            format!("changed since the stamp: {}{rest}", shown.join(", "))
        }
        Flag::AnchorMissing => "no file that Git knows lies under the anchor".to_owned(),
        Flag::Upstream { kind, target, .. } => {
            format!(
                "{kind} {target}: what its anchor stands for may have changed since this element's stamp"
            )
        }
    }
}

/// The relation rows as aligned lines, or the line `none`.
fn relation_lines(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        "none\n".to_owned()
    } else {
        lines(rows)
    }
}

/// The rows as aligned lines, each ending in a line feed.
fn lines(rows: &[Vec<String>]) -> String {
    let widths = column_widths(rows);
    let mut text = String::new();
    for row in rows {
        text.push_str(&aligned(row, &widths));
        text.push('\n');
    }
    text
}

/// A placement's position and size, built from the board file's numbers
/// only.
fn position(rect: &Rect) -> String {
    format!("@{},{} {}x{}", rect.x, rect.y, rect.width, rect.height)
}

/// The header lines of `board`, each ending in a line feed: the ID and
/// title, then the owner line if it has an owner, the zone if it is one, and
/// the summary if it has one that is not empty.
fn header(snapshot: &Snapshot, tree: &BoardTree, board: &Board) -> String {
    let owner = tree
        .owner(&board.id)
        .map(|owner| format!("{}\n", owner_line(snapshot, tree, &board.id, owner)))
        .unwrap_or_default();
    let zone = board
        .zone
        .map(|zone| format!("Zone: {}\n", zone_text(zone)))
        .unwrap_or_default();
    let summary = board
        .summary
        .as_deref()
        .filter(|summary| !summary.is_empty())
        .map(|summary| format!("Summary: {}\n", one_line(summary)))
        .unwrap_or_default();
    format!(
        "Board: {} · {}\n{owner}{zone}{summary}",
        board.id,
        one_line(&board.title)
    )
}

/// A zone as `<mode> · <kind>`.
fn zone_text(zone: Zone) -> String {
    format!("{} · {}", zone.mode.as_str(), zone.statement.as_str())
}

/// The line naming `owner`, whose inside `board` is: `Inside of:` and the
/// owner, marked `⚠ missing` without an element file; then, if a board
/// places the owner, `Above:` and the owner's home, marked `⚠ cycle` when
/// `board` lies inside itself, and otherwise `⚠ no home` for an owner with a
/// file. The markers add up, so no state hides another. The line ends
/// without a line feed.
fn owner_line(snapshot: &Snapshot, tree: &BoardTree, board: &BoardId, owner: &ElementId) -> String {
    let has_file = snapshot.elements.contains_key(owner);
    let missing = if has_file {
        String::new()
    } else {
        format!(" {MISSING_MARKER}")
    };
    let above = match tree.home(owner) {
        Some(home) if tree.lies_inside_itself(board) => format!(" · Above: {home} {CYCLE_MARKER}"),
        Some(home) => format!(" · Above: {home}"),
        None if has_file => format!(" {NO_HOME_MARKER}"),
        None => String::new(),
    };
    format!("Inside of: {owner}{missing}{above}")
}

/// The rows of the relations between elements on `board`: by source, then
/// in the order of the source's relations.
fn relation_rows(snapshot: &Snapshot, board: &Board) -> Vec<Vec<String>> {
    board
        .placements
        .keys()
        .filter_map(|id| snapshot.elements.get(id))
        .flat_map(|source| {
            source
                .relations()
                .iter()
                .filter(|((_, to), _)| board.placements.contains_key(to))
                .map(|((kind, to), label)| {
                    relation_row(source.id(), to.as_str(), *kind, label.as_deref())
                })
        })
        .collect()
}

/// The columns of one relation's line. `to` is the target's ID, followed by
/// a marker the caller adds. The label is the last column, so it is never
/// padded; an empty label is left out like a missing one.
fn relation_row(
    from: &ElementId,
    to: &str,
    kind: RelationKind,
    label: Option<&str>,
) -> Vec<String> {
    let mut columns = vec![format!("{from} → {to}"), kind.as_str().to_owned()];
    columns.extend(label.filter(|label| !label.is_empty()).map(one_line));
    columns
}

/// The columns of one element's line: ID, type, the statement kind when
/// `kind` is given, name, summary, the inner boards when `inner_boards` is
/// given, and the review marker. Empty columns at the end are left out; an
/// empty one before a filled one stays, so the columns keep their places.
/// The last column is never padded.
fn row(
    id: &ElementId,
    element: Option<&Element>,
    kind: Option<String>,
    inner_boards: Option<String>,
    review: Option<&Review>,
) -> Vec<String> {
    let Some(element) = element else {
        return vec![id.to_string(), MISSING_MARKER.to_owned()];
    };
    let marker = review
        .and_then(|review| review.priority(id))
        .map(marker)
        .unwrap_or_default();
    let mut optional: Vec<String> = [
        Some(element.summary().map(one_line).unwrap_or_default()),
        inner_boards,
        Some(marker.to_owned()),
    ]
    .into_iter()
    .flatten()
    .collect();
    drop_empty_tail(&mut optional);
    [
        Some(id.to_string()),
        Some(one_line(element.type_ref().as_str())),
        kind,
        Some(one_line(element.name())),
    ]
    .into_iter()
    .flatten()
    .chain(optional)
    .collect()
}

/// What an element line shows for the highest priority of its flags.
const fn marker(priority: Priority) -> &'static str {
    match priority {
        Priority::Task => "⚠ task",
        Priority::Hint => "⚠ hint",
    }
}

/// The width of each column but the last of any row: its widest value, at
/// most [`MAX_COLUMN_WIDTH`].
fn column_widths(rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = Vec::new();
    for row in rows {
        let padded = row.len().saturating_sub(1);
        for (index, value) in row.iter().take(padded).enumerate() {
            let width = value.chars().count().min(MAX_COLUMN_WIDTH);
            match widths.get_mut(index) {
                Some(widest) => *widest = (*widest).max(width),
                None => widths.push(width),
            }
        }
    }
    widths
}

/// Joins a row's columns with two spaces, padding each column but the last
/// to its width.
fn aligned(row: &[String], widths: &[usize]) -> String {
    let last = row.len().saturating_sub(1);
    row.iter()
        .enumerate()
        .map(|(index, value)| match widths.get(index) {
            Some(&width) if index < last => format!("{value:<width$}"),
            _ => value.clone(),
        })
        .collect::<Vec<_>>()
        .join("  ")
}

/// Writes `text` on one line. A backslash becomes `\\`; line feed, carriage
/// return, and tab become `\n`, `\r`, and `\t`; every other control
/// character (Unicode Cc) and every character in
/// [`FORMAT_AND_SEPARATORS`](abyme_model::FORMAT_AND_SEPARATORS) becomes
/// `\u{…}`. Everything else stays as it is.
fn one_line(text: &str) -> String {
    let mut line = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => line.push_str("\\\\"),
            '\n' => line.push_str("\\n"),
            '\r' => line.push_str("\\r"),
            '\t' => line.push_str("\\t"),
            _ if hides_or_breaks_text(character) => line.extend(character.escape_unicode()),
            _ => line.push(character),
        }
    }
    line
}

/// Writes a description: line feeds, tabs, and backslashes stay as they
/// are, a carriage return before a line feed is left out, and every other
/// character [`one_line`] escapes is escaped the same way, so the text can
/// neither hide nor reorder anything. A backslash stays as it is, so that
/// a description the AI reads and writes back does not grow.
fn multi_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\n' | '\t' => out.push(character),
            '\r' if characters.peek() == Some(&'\n') => {}
            '\r' => out.push_str("\\r"),
            _ if hides_or_breaks_text(character) => out.extend(character.escape_unicode()),
            _ => out.push(character),
        }
    }
    out
}

#[cfg(test)]
mod tests;
