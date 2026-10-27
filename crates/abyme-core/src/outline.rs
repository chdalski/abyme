//! The detail levels as the AI reads them: a board as a compact text
//! outline, and one element in detail.
//!
//! Titles, types, names, summaries, anchors, relation labels, and
//! descriptions come from files that a clone or a pull request can bring
//! in. Every value but the description is written on one line, and every
//! character that could break a line or hide or reorder text is escaped;
//! the description keeps its lines and tabs.

use abyme_model::{Board, BoardId, Element, ElementId, Rect, RelationKind, hides_or_breaks_text};

use crate::apply::Snapshot;
use crate::board_tree::BoardTree;
use crate::review::{Flag, Priority, Review};

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

/// How many changed files a review flag names at most.
const SHOWN_CHANGED_FILES: usize = 10;

/// How many elements [`status_report`] lists at most.
const STATUS_LINES: usize = 50;

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

/// Renders `board` as an outline.
///
/// The outline has a header line with the board's ID and title; for a board
/// with an owner, a line `Inside of:` with the owner and, if a board places
/// the owner, `Above:` with the owner's home (see [`owner_line`] for the
/// markers); for a board with a summary, a line `Summary:` with it. A blank
/// line follows, and one line per placed element, ordered by ID, as
/// [`element_list`] renders it. A placement whose element is missing shows
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
    let rows: Vec<Vec<String>> = element_rows(snapshot, &tree, found.placements.keys(), review)
        .into_iter()
        .zip(found.placements.values())
        .map(|(mut columns, rect)| {
            if geometry == Geometry::Shown {
                columns.insert(1, position(rect));
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
/// each ending in a line feed, with the element's ID, type, name, and
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
    lines(&element_rows(snapshot, &tree, ids, review))
}

/// The rows of the elements with the given IDs, in the given order, as
/// [`element_list`] describes them.
fn element_rows<'a>(
    snapshot: &Snapshot,
    tree: &BoardTree,
    ids: impl IntoIterator<Item = &'a ElementId>,
    review: Option<&Review>,
) -> Vec<Vec<String>> {
    let elements: Vec<(&ElementId, Option<&Element>, String)> = ids
        .into_iter()
        .map(|id| (id, snapshot.elements.get(id), inner_boards(tree, id)))
        .collect();
    let shows_inner_boards = elements
        .iter()
        .any(|(_, element, inner)| element.is_some() && !inner.is_empty());
    elements
        .into_iter()
        .map(|(id, element, inner)| row(id, element, shows_inner_boards.then_some(inner), review))
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
/// - `Element:` with its ID and name, `Type:`, `Anchor:` with its anchor or
///   `none`, `Verified:` with the stamp's commit and `by ai` or
///   `by person`, or `none`, given a `review`, `Review:` with the highest
///   priority of the element's flags or `none` and one indented line per
///   flag, `Summary:` if it has one, `Boards:` with every board that places
///   it, by ID, or `none`, and `Inner board:` with every board it owns, by
///   ID, or `none`;
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
    Ok(format!(
        "Element: {id} · {}\nType: {}\nAnchor: {anchor}\nVerified: {verified}\n{}\
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
/// title, then the owner line if it has an owner, then the summary if it has
/// one that is not empty.
fn header(snapshot: &Snapshot, tree: &BoardTree, board: &Board) -> String {
    let owner = tree
        .owner(&board.id)
        .map(|owner| owner_line(snapshot, tree, &board.id, owner))
        .unwrap_or_default();
    let summary = board
        .summary
        .as_deref()
        .filter(|summary| !summary.is_empty())
        .map(|summary| format!("Summary: {}\n", one_line(summary)))
        .unwrap_or_default();
    format!(
        "Board: {} · {}\n{owner}{summary}",
        board.id,
        one_line(&board.title)
    )
}

/// The line naming `owner`, whose inside `board` is: `Inside of:` and the
/// owner, marked `⚠ missing` without an element file; then, if a board
/// places the owner, `Above:` and the owner's home, marked `⚠ cycle` when
/// `board` lies inside itself, and otherwise `⚠ no home` for an owner with a
/// file. The markers add up, so no state hides another.
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
    format!("Inside of: {owner}{missing}{above}\n")
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

/// The columns of one element's line: ID, type, name, summary, the inner
/// boards when `inner_boards` is given, and the review marker. Empty columns
/// at the end are left out; an empty one before a filled one stays, so the
/// columns keep their places. The last column is never padded.
fn row(
    id: &ElementId,
    element: Option<&Element>,
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
    while optional.last().is_some_and(String::is_empty) {
        optional.pop();
    }
    [
        id.to_string(),
        one_line(element.type_ref().as_str()),
        one_line(element.name()),
    ]
    .into_iter()
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
