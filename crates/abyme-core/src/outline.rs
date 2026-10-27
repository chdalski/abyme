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
use crate::review::{Flag, Priority, Review};

/// The widest a column is padded to, in characters. A longer value runs past
/// its column, so one long name cannot pad every other line.
const MAX_COLUMN_WIDTH: usize = 40;

/// What the outline shows for a placement whose element file is missing.
const MISSING_MARKER: &str = "⚠ missing";

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
/// The outline has a header line with the board's ID and title, a blank
/// line, and one line per placed element, ordered by ID, with the element's
/// ID, type, name, and summary, and, given a `review`, `⚠ task` or
/// `⚠ hint` for the highest priority of the element's flags. A placement
/// whose element is missing shows its ID and `⚠ missing`. With
/// [`Geometry::Shown`], each line also shows the placement's position and
/// size from the board file, right after the ID.
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
    let rows: Vec<Vec<String>> = found
        .placements
        .iter()
        .map(|(id, rect)| {
            let mut columns = row(id, snapshot.elements.get(id), review);
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
        "{}\n\n{}{relation_lines}",
        header(found),
        lines(&rows)
    ))
}

/// Renders the elements with the given IDs as the outline's element lines.
///
/// The lines come in the given order, without a header: one line per ID,
/// each ending in a line feed, with the element's ID, type, name, and
/// summary, and the marker of its review flags as in [`outline`]. An ID
/// whose element is missing shows `⚠ missing`. An ID given
/// twice is rendered twice; limiting and deduplicating is the caller's
/// business.
///
/// All element text is escaped onto its line already, so callers must not
/// escape it again.
pub fn element_list<'a>(
    snapshot: &Snapshot,
    ids: impl IntoIterator<Item = &'a ElementId>,
    review: Option<&Review>,
) -> String {
    let rows: Vec<Vec<String>> = ids
        .into_iter()
        .map(|id| row(id, snapshot.elements.get(id), review))
        .collect();
    lines(&rows)
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
///   flag, `Summary:` if it has one, and `Boards:` with every board that
///   places it, by ID, or `none`;
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
    let boards = if boards.is_empty() {
        "none".to_owned()
    } else {
        boards.join(", ")
    };
    Ok(format!(
        "Element: {id} · {}\nType: {}\nAnchor: {anchor}\nVerified: {verified}\n{}\
         {summary}Boards: {boards}\n\n\
         Outgoing relations:\n{}\nIncoming relations:\n{}\nDescription:\n{}",
        one_line(element.name()),
        one_line(element.type_ref().as_str()),
        review
            .map(|review| review_lines(review, id))
            .unwrap_or_default(),
        relation_lines(&outgoing),
        relation_lines(&incoming),
        multi_line(element.body()),
    ))
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

fn header(board: &Board) -> String {
    format!("Board: {} · {}", board.id, one_line(&board.title))
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

/// The columns of one placement's line; the last one is never padded.
fn row(id: &ElementId, element: Option<&Element>, review: Option<&Review>) -> Vec<String> {
    let Some(element) = element else {
        return vec![id.to_string(), MISSING_MARKER.to_owned()];
    };
    let mut columns = vec![
        id.to_string(),
        one_line(element.type_ref().as_str()),
        one_line(element.name()),
    ];
    let summary = element.summary().map(one_line);
    match review.and_then(|review| review.priority(id)) {
        // An empty summary keeps the marker in the marker column.
        Some(priority) => {
            columns.extend([summary.unwrap_or_default(), marker(priority).to_owned()]);
        }
        None => columns.extend(summary),
    }
    columns
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
